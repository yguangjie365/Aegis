//! 四卡装配（F-01 / R-07 回退链）：`live` 的子模块。

use crate::overview::badges::{
    build_badge, gpu_card_worth_showing, pick_big_value, short_cpu_brand, short_gpu_name,
};
use crate::overview::dto::{gib_text, CardKind, SegLevel, StatCardDto};
use crate::overview::sensor_bridge::SensorSnapshot;
use crate::overview::state::{StorageSummary, SystemIdentity};

/// 卡片装配的输入集（打成一个结构体，避免七个并列参数导致口径漂移）。
#[derive(Debug, Clone)]
pub struct CardInputs<'a> {
    pub identity: &'a SystemIdentity,
    pub sensors: Option<&'a SensorSnapshot>,
    pub storage: Option<&'a StorageSummary>,
    pub cpu_usage_percent: Option<f64>,
    pub memory_usage_percent: Option<f64>,
    pub memory_total_bytes: u64,
    /// 传感器通道未就绪时的一行降级原因（L-08：卡不消失、卡高不变）
    pub monitor_reason: Option<&'a str>,
}

/// 装配实时概览卡（F-01 四卡 = 处理器 / 显卡 / 内存 / 存储；🔴 没有"网络卡"，见线框更正）。
#[must_use]
pub fn build_cards(inputs: &CardInputs<'_>) -> Vec<StatCardDto> {
    let degrade = inputs
        .monitor_reason
        .map(|reason| format!("传感器通道未就绪：温度等增强项不显示（{reason}）"));

    // 处理器卡（R-07 三级回退：使用率 → 传感器负载 → 温度顶位）
    let cpu_temperature = inputs.sensors.and_then(|sensors| sensors.cpu_temperature_c);
    let cpu_load = inputs.sensors.and_then(|sensors| sensors.cpu_load_percent);
    let (big, source, bar) =
        pick_big_value(inputs.cpu_usage_percent, cpu_load, cpu_temperature, true);
    let cores_text = match inputs.identity.physical_cores {
        Some(physical) if inputs.identity.logical_cores > 0 => {
            format!("{physical} 核心 · {} 线程", inputs.identity.logical_cores)
        }
        _ => String::new(),
    };
    let mut cards = vec![StatCardDto {
        kind: CardKind::Cpu,
        index: 0,
        label: short_cpu_brand(&inputs.identity.cpu_brand),
        detail: cores_text,
        big_value: big,
        value_source: source,
        bar_percent: bar,
        seg_level: SegLevel::from_percent(bar),
        badge: build_badge(CardKind::Cpu, cpu_temperature, true),
        degrade_reason: degrade.clone(),
    }];

    // 显卡卡（n 张，R-01 全列；R-07：负载与温度皆无的那张不出现）
    if let Some(sensors) = inputs.sensors {
        let mut index = 0_usize;
        for gpu in &sensors.gpus {
            if !gpu_card_worth_showing(gpu.load_percent, gpu.temperature_c) {
                continue;
            }
            let (big, source, bar) =
                pick_big_value(None, gpu.load_percent, gpu.temperature_c, true);
            cards.push(StatCardDto {
                kind: CardKind::Gpu,
                index,
                label: short_gpu_name(&gpu.device),
                detail: gpu.device.clone(),
                big_value: big,
                value_source: source,
                bar_percent: bar,
                seg_level: SegLevel::from_percent(bar),
                badge: build_badge(CardKind::Gpu, gpu.temperature_c, true),
                degrade_reason: None,
            });
            index += 1;
        }
    }

    cards.push(memory_card(inputs, degrade.as_deref()));
    cards.push(storage_card(inputs));

    cards
}

/// 内存卡（副标题 = 总容量；温度无来源时按 R-09 隐藏徽章）。
///
/// 与处理器卡同构，拆出来只为让 `build_cards` 一屏读完四张卡的装配顺序。
///
/// 🔴 2026-10-07：DIMM 读数不可得时给出**具体**降级原因（L-08），而非让用户
///   只看到一个笼统的「未检测」。判据见 `sensor_bridge/availability.rs`：
///   `PermissionDenied` 仅在 sidecar 诊断有明确权限字样时才成立。
#[must_use]
fn memory_card(inputs: &CardInputs<'_>, degrade: Option<&str>) -> StatCardDto {
    let memory_temperature = inputs
        .sensors
        .and_then(|sensors| sensors.memory_temperature_c);
    let has_memory_temperature = inputs
        .sensors
        .is_some_and(|sensors| sensors.has_memory_temperature);
    let memory_load = inputs
        .sensors
        .and_then(|sensors| sensors.memory_load_percent);

    // 🔴 2026-10-07：此处曾叠加一层「读数超龄 → Stale」的判定（阈值 6 秒），
    //   已**整体删除** —— 该阈值按「3 倍 2 秒节拍」拍出，但传感器实际更新周期由
    //   **sidecar 读耗时**支配（`FIRST_READ_BUDGET = 15s + 8s = 23s`），
    //   真机必然误报，界面长出「读数已过期」的橙色提示而数据其实正常
    //   （用户 2026-10-07 裁决：「不要显示，容易造成误解，几秒后就恢复」）。
    //   通道重试另有 `COOLDOWN` / 连败禁用两种状态表达，不需再叠时间维度。
    let memory_degrade = inputs
        .sensors
        .map_or(degrade.map(str::to_owned), |sensors| {
            if sensors.memory_availability.is_ok() {
                // 读得到温度时不因可得性而降级（通道级原因仍照原样透出）。
                return degrade.map(str::to_owned);
            }
            // 🔴 通道级原因优先：sidecar 都没起来时，传感器级状态没有意义。
            Some(degrade.map_or_else(
                || sensors.memory_availability.reason().to_owned(),
                str::to_owned,
            ))
        });

    let (big, source, bar) = pick_big_value(
        inputs.memory_usage_percent,
        memory_load,
        memory_temperature,
        false,
    );
    StatCardDto {
        kind: CardKind::Memory,
        index: 0,
        label: gib_text(inputs.memory_total_bytes),
        detail: String::new(),
        big_value: big,
        value_source: source,
        bar_percent: bar,
        seg_level: SegLevel::from_percent(bar),
        badge: build_badge(CardKind::Memory, memory_temperature, has_memory_temperature),
        degrade_reason: memory_degrade,
    }
}

/// 存储卡（R-12 慢变量：值来自缓存的存储汇总，不随 2 秒节拍变化）。
#[must_use]
fn storage_card(inputs: &CardInputs<'_>) -> StatCardDto {
    let storage_temperature = inputs
        .sensors
        .and_then(|sensors| sensors.storage_temperature_c);
    let has_storage_sensor = inputs
        .sensors
        .is_some_and(|sensors| sensors.has_storage_sensor);
    let Some(storage) = inputs.storage else {
        return StatCardDto {
            kind: CardKind::Storage,
            index: 0,
            label: String::new(),
            detail: String::new(),
            big_value: None,
            value_source: None,
            bar_percent: None,
            seg_level: SegLevel::from_percent(None),
            badge: build_badge(CardKind::Storage, None, false),
            degrade_reason: Some("未获取到存储汇总（等待首次采集）".to_string()),
        };
    };
    let detail = if storage.largest_volume.is_empty() {
        format!("剩余 {}", gib_text(storage.free_bytes))
    } else {
        format!(
            "{} 剩余 {}",
            storage.largest_volume,
            gib_text(storage.free_bytes)
        )
    };
    let (big, source, bar) = pick_big_value(Some(storage.used_percent), None, None, false);
    StatCardDto {
        kind: CardKind::Storage,
        index: 0,
        label: storage.media_text.clone(),
        detail,
        big_value: big,
        value_source: source,
        bar_percent: bar,
        seg_level: SegLevel::from_percent(bar),
        badge: build_badge(CardKind::Storage, storage_temperature, has_storage_sensor),
        degrade_reason: storage.unavailable_reason.clone(),
    }
}
