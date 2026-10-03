//! 实时采样与卡片装配（M-01 片1：F-01 / F-08 / F-10 的采集侧，2026-10-03）。
//!
//! 复用结论（DP-1）：
//! - 占用/容量/身份项走 `sysinfo`（`NexBox` `hardware.rs` 同库同版本线，0.30.13 实测钉版）；
//! - 温度/传感器负载走本域 `sensor_bridge`（`NexBox` `sensor.rs` 的②改造移植）；
//! - 🔴 **不移植 `NexBox` `hardware.rs::long_os_version`**：母本用 `libloading` + 手写
//!   `RtlGetVersion` FFI 取内核版本，本工程禁手写 Win32 绑定（01 总览 §二），
//!   改采 `sysinfo` 的注册表实现（同读 `HKLM\...\CurrentVersion`，语义等价），
//!   已登记为对母本的有意偏离（`DEPENDENCIES.md §三`）；
//! - 存储卡口径沿用 `SystemToolkit` 2026-09-02 起的**卷（DriveInfo）单一来源**：
//!   已用率取多卷最大值，副标题按介质聚合（R-06）。
//!
//! 🔴 非阻塞纪律：本文件任何路径都不得等待 sidecar。传感器读取只经 `kick_sensor_read`
//! 投递到专用线程，节拍消费缓存——首屏不被最长 23 秒的通道接入拖住（N-03/N-04）。

use std::sync::Arc;

use sysinfo::{CpuRefreshKind, DiskKind, Disks, System};

use infra::registry;

use super::badges::{
    build_badge, gpu_card_worth_showing, pick_big_value, short_cpu_brand, short_gpu_name,
};
use super::dto::{
    clamp_percent, gib_text, CardKind, HeaderDto, LiveSampleDto, MonitorState, MonitorStatusDto,
    StatCardDto,
};
use super::sensor_bridge::{FaultKind, SensorBridge, SensorSnapshot};
use super::state::{OverviewInner, StorageSummary, SystemIdentity};

/// 内核驱动卸载项名（Q-12=B：只读探测，🔴 不安装、不分发，见 DEPENDENCIES.md §四）。
const KERNEL_DRIVER_UNINSTALL_KEY: &str = "PawnIO";

/// 盘符/介质分类（R-05 信号链在本批可得的部分，见 `media_of`）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Media {
    Ssd,
    Hdd,
    Unknown,
}

impl Media {
    #[must_use]
    pub fn label(self) -> &'static str {
        match self {
            Self::Ssd => "SSD",
            Self::Hdd => "HDD",
            Self::Unknown => "未知介质",
        }
    }
}

/// R-05 介质判定信号链在本批的**可得片段**：sysinfo 在 Windows 上经
/// `IOCTL_STORAGE_QUERY_PROPERTY / StorageDeviceSeekPenaltyProperty`（寻道惩罚）给出介质，
/// 拿不到时是 `Unknown`。母本链的另外两级（转速、型号关键词启发式）需要物理盘对象，
/// 随片2 的 `Win32_DiskDrive` 采集补齐；🔴 全部未知时不臆断（走文案兜底）。
#[must_use]
pub fn media_of(kind: DiskKind) -> Media {
    match kind {
        DiskKind::SSD => Media::Ssd,
        DiskKind::HDD => Media::Hdd,
        DiskKind::Unknown(_) => Media::Unknown,
    }
}

/// R-06 副标题：按介质聚合容量，固定 SSD→HDD 顺序；零容量类目不出现。
#[must_use]
pub fn media_text(groups: &[(Media, u64)]) -> String {
    let mut parts: Vec<String> = Vec::new();
    for media in [Media::Ssd, Media::Hdd] {
        let total = groups
            .iter()
            .filter(|(kind, _)| *kind == media)
            .map(|(_, bytes)| *bytes)
            .sum::<u64>();
        if total > 0 {
            parts.push(format!("{} {}", media.label(), gib_text(total)));
        }
    }
    parts.join(" + ")
}

/// 多卷已用率取最大（SystemToolkit 口径：统计卡与副文案同源；聚合分母会掩盖单卷爆满）。
#[must_use]
pub fn max_used_percent(volumes: &[(u64, u64)]) -> Option<f64> {
    volumes
        .iter()
        .filter_map(|(total, free)| used_percent_of(*total, *free))
        .reduce(f64::max)
}

/// 单卷已用率；总量为 0 时返回 `None`（🔴 不得当成 0%，R-08）。
#[must_use]
#[allow(clippy::cast_precision_loss)] // 容量超 2^53 B（9 PB）才会失真，本机位宽恒安全
pub fn used_percent_of(total: u64, free: u64) -> Option<f64> {
    if total == 0 {
        return None;
    }
    let used = total.saturating_sub(free) as f64;
    Some(clamp_percent(used / total as f64 * 100.0))
}

/// 挂载点 → 盘符（`C:\` 或 `\\?\D:\` → `C:`/`D:`）；形态不认识时原样返回，便于诊断。
#[must_use]
pub fn drive_letter(mount: &str) -> String {
    let normalized = mount.trim_start_matches(r"\\?\");
    let bytes = normalized.as_bytes();
    if bytes.len() >= 2 && bytes[1] == b':' && bytes[0].is_ascii_alphabetic() {
        normalized[..2].to_string()
    } else {
        normalized.to_string()
    }
}

/// 采集存储慢变量（R-12：只在首次/全量采集时确定，不入 2 秒节拍）。
///
/// 可移动卷（U 盘/光驱）不进汇总：插拔会让存储卡容量在节拍间跳变，与"慢变量"语义冲突；
/// 物理盘逐台列出属片2 详情卡（R-01）。
#[must_use]
pub fn collect_storage() -> StorageSummary {
    let mut groups: Vec<(Media, u64)> = Vec::new();
    let mut volumes: Vec<(u64, u64)> = Vec::new();
    let mut total_bytes = 0_u64;
    let mut free_bytes = 0_u64;
    let mut largest: Option<(u64, String)> = None;
    let mut volume_count = 0_usize;

    for disk in Disks::new_with_refreshed_list().list() {
        if disk.is_removable() {
            continue;
        }
        let total = disk.total_space();
        if total == 0 {
            continue;
        }
        let free = disk.available_space().min(total);
        volume_count += 1;
        total_bytes = total_bytes.saturating_add(total);
        free_bytes = free_bytes.saturating_add(free);
        volumes.push((total, free));
        groups.push((media_of(disk.kind()), total));
        let mount = drive_letter(&disk.mount_point().to_string_lossy());
        let replace = match &largest {
            Some((best, _)) => total > *best,
            None => true,
        };
        if replace {
            largest = Some((total, mount));
        }
    }

    if volume_count == 0 {
        return StorageSummary {
            media_text: String::new(),
            used_percent: 0.0,
            total_bytes: 0,
            free_bytes: 0,
            largest_volume: String::new(),
            volume_count: 0,
            unavailable_reason: Some("未获取到本机卷信息（存储枚举无返回）".to_string()),
        };
    }

    let (total_of_largest, largest_volume) = largest.unwrap_or((0, String::new()));
    let aggregated = media_text(&groups);
    // R-06 回退支：介质全部解析失败 → 最大卷形态（片2 拿到物理盘型号后替换）
    let media_text = if aggregated.is_empty() {
        format!("{} {}", largest_volume, gib_text(total_of_largest))
    } else {
        aggregated
    };

    StorageSummary {
        media_text,
        used_percent: max_used_percent(&volumes).unwrap_or(0.0),
        total_bytes,
        free_bytes,
        largest_volume,
        volume_count,
        unavailable_reason: None,
    }
}

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
        badge: build_badge(CardKind::Memory, memory_temperature, has_memory_temperature),
        degrade_reason: degrade.map(str::to_owned),
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
        badge: build_badge(CardKind::Storage, storage_temperature, has_storage_sensor),
        degrade_reason: storage.unavailable_reason.clone(),
    }
}

/// 桥状态 → 用户可见的通道状态 + 原因（N-06：每次降级都要说清为什么）。
fn map_bridge_state(bridge: &SensorBridge) -> (MonitorState, Option<String>) {
    let last_reason = bridge.last_fault().map(|fault| fault.reason.clone());
    // "组件根本没随包提供"比"冷却中/已停用"更能解释界面缺了什么（N-06）：
    // exe 路径查找每次只扫一遍，失败原因不随重试次数变化，故两处都优先报它。
    let exe_missing = bridge
        .last_fault()
        .is_some_and(|fault| fault.kind == FaultKind::ExeMissing);
    if bridge.is_disabled() {
        return (
            if exe_missing {
                MonitorState::ExeMissing
            } else {
                MonitorState::Disabled
            },
            Some(last_reason.unwrap_or_else(|| "传感器通道已停用".to_string())),
        );
    }
    if bridge.cooling_down() {
        return (
            if exe_missing {
                MonitorState::ExeMissing
            } else {
                MonitorState::CoolingDown
            },
            Some(last_reason.unwrap_or_else(|| "传感器通道冷却中".to_string())),
        );
    }
    if bridge.has_reads() {
        return (MonitorState::Ready, None);
    }
    if let Some(reason) = last_reason {
        // 未就绪且留过故障：区分"组件根本没随包提供"与"接不上"，两者文案不同
        return if exe_missing {
            (MonitorState::ExeMissing, Some(reason))
        } else {
            (MonitorState::CoolingDown, Some(reason))
        };
    }
    (
        MonitorState::Starting,
        Some("传感器通道首次接入中（硬件初始化上限 15 秒）".to_string()),
    )
}

#[must_use]
pub fn monitor_status(inner: &OverviewInner) -> MonitorStatusDto {
    let (state, reason, sidecar_pid) = {
        let bridge = inner.bridge();
        let (state, reason) = map_bridge_state(&bridge);
        (state, reason, bridge.sidecar_pid())
    };
    // 只读探测：装了就读到版本，没装就如实 false；本工程永不写注册表、永不发起安装。
    let driver = registry::detect_driver(KERNEL_DRIVER_UNINSTALL_KEY);
    MonitorStatusDto {
        state,
        reason,
        sidecar_pid,
        driver_installed: driver.installed,
        driver_version: driver.version,
    }
}

/// 把一次 sidecar 读取投递到专用线程（同一时刻至多一个在途，R-13）。
fn kick_sensor_read(inner: &Arc<OverviewInner>) {
    {
        let bridge = inner.bridge();
        if bridge.is_disabled() || bridge.cooling_down() {
            return;
        }
    }
    if !inner.try_begin_read() {
        return;
    }
    let shared = Arc::clone(inner);
    let spawned = std::thread::Builder::new()
        .name("overview-sensor".to_string())
        .spawn(move || {
            let outcome = {
                let mut bridge = shared.bridge();
                bridge.read_once()
            };
            match outcome {
                Ok(snapshot) => shared.store_sensors(snapshot),
                // 🔴 失败不清缓存：保留上一轮可用值（R-08 不给错数，也不突然空白）
                Err(fault) => eprintln!("[overview] 传感器读取失败：{}", fault.reason),
            }
            shared.end_read();
        });
    if let Err(error) = spawned {
        inner.end_read();
        eprintln!("[overview] 传感器读取线程未能启动: {error}");
    }
}

/// 首次调用时确定系统身份项（会话内不变，之后直接复用缓存）。
fn ensure_identity(inner: &OverviewInner) -> SystemIdentity {
    if let Some(existing) = inner.identity() {
        return existing;
    }
    let sys = inner.system();
    let identity = SystemIdentity {
        computer_name: System::host_name().unwrap_or_default(),
        os_name: System::long_os_version().unwrap_or_default(),
        cpu_brand: sys
            .cpus()
            .first()
            .map(|cpu| cpu.brand().to_string())
            .unwrap_or_default(),
        physical_cores: sys.physical_core_count(),
        logical_cores: sys.cpus().len(),
    };
    drop(sys);
    inner.store_identity(identity.clone());
    identity
}

/// 一次节拍采样（F-01/F-08/F-10 采集侧）。
#[must_use]
pub fn sample(inner: &Arc<OverviewInner>) -> LiveSampleDto {
    kick_sensor_read(inner);

    let (cpu_usage_percent, memory_usage_percent, memory_total_bytes) = {
        let mut sys = inner.system();
        sys.refresh_cpu_specifics(CpuRefreshKind::everything());
        // 首拍 PDH 计数器还没有第二个采样点，cpu_usage() 是 0 填充值（L-02）→ 判为无基线（R-08）
        let reads = inner.note_cpu_refresh();
        let cpu = if reads >= 2 {
            Some(f64::from(sys.global_cpu_info().cpu_usage()))
        } else {
            None
        };
        sys.refresh_memory();
        let total = sys.total_memory();
        // `used_percent_of` 收的是「空闲量」，内存侧 sysinfo 只给「已用量」→ 先取补，
        // 保证内存卡与存储卷走同一个已用率口径（R-07/R-08 单源）。
        let memory = used_percent_of(total, total.saturating_sub(sys.used_memory()));
        (cpu, memory, total)
    };

    let identity = ensure_identity(inner);
    // R-12：存储只在慢变量缺失时确定一次，不随节拍变化
    if inner.storage().is_none() {
        inner.store_storage(collect_storage());
    }

    let sensors = inner.sensors();
    let storage = inner.storage();
    let monitor = monitor_status(inner);
    let inputs = CardInputs {
        identity: &identity,
        sensors: sensors.as_ref(),
        storage: storage.as_ref(),
        cpu_usage_percent,
        memory_usage_percent,
        memory_total_bytes,
        monitor_reason: monitor.reason.as_deref(),
    };

    // 先算卡片再搬 `identity`：`inputs` 借用它，两榜与页头共用同一份身份缓存（R-21）
    let cards = build_cards(&inputs);

    LiveSampleDto {
        header: HeaderDto {
            computer_name: identity.computer_name,
            os_name: identity.os_name,
            uptime_secs: System::uptime(),
        },
        cards,
        monitor,
        in_flight: inner.read_in_flight(),
    }
}

#[cfg(test)]
mod tests {
    use super::super::sensor_bridge::GpuSensor;
    use super::*;
    use std::time::{Duration, Instant};

    const GIB: u64 = 1024 * 1024 * 1024;

    fn snapshot(gpus: Vec<GpuSensor>) -> SensorSnapshot {
        SensorSnapshot {
            cpu_load_percent: None,
            cpu_temperature_c: None,
            memory_load_percent: None,
            memory_temperature_c: None,
            storage_temperature_c: None,
            gpus,
            has_storage_sensor: false,
            has_memory_temperature: false,
            taken_at: Instant::now(),
        }
    }

    #[test]
    fn media_text_aggregates_by_media_class_in_fixed_order() {
        let groups = [
            (Media::Hdd, 931 * GIB),
            (Media::Ssd, 476 * GIB),
            (Media::Ssd, 24 * GIB),
            (Media::Unknown, 8 * GIB),
        ];
        assert_eq!(media_text(&groups), "SSD 500.0 GB + HDD 931.0 GB");
    }

    #[test]
    fn media_text_hides_unknown_and_leaves_fallback_to_caller() {
        assert_eq!(media_text(&[(Media::Unknown, 476 * GIB)]), "");
        assert_eq!(media_text(&[]), "");
    }

    #[test]
    fn used_percent_guarded_against_zero_total() {
        assert_eq!(used_percent_of(0, 0), None, "总量为 0 不得当成 0%");
        let value = used_percent_of(100, 22).expect("应有已用率");
        assert!((value - 78.0).abs() < 1e-9);
        assert_eq!(used_percent_of(100, 300), Some(0.0), "空闲卷不得为负");
    }

    #[test]
    fn max_used_percent_takes_the_worst_volume() {
        let value = max_used_percent(&[(100, 50), (100, 2), (100, 90)]).expect("应有值");
        assert!((value - 98.0).abs() < 1e-9);
        assert_eq!(max_used_percent(&[]), None);
    }

    #[test]
    fn drive_letter_normalises_mount_points() {
        assert_eq!(drive_letter("C:\\"), "C:");
        assert_eq!(drive_letter(r"\\?\D:\"), "D:");
        assert_eq!(drive_letter("/mnt/data"), "/mnt/data");
    }

    #[test]
    fn cpu_card_falls_back_to_sensor_load_without_baseline() {
        let identity = SystemIdentity {
            cpu_brand: "12th Gen Intel(R) Core(TM) i7-12700H".to_string(),
            physical_cores: Some(14),
            logical_cores: 20,
            ..SystemIdentity::default()
        };
        let mut sensors = snapshot(Vec::new());
        sensors.cpu_load_percent = Some(22.5);
        let inputs = CardInputs {
            identity: &identity,
            sensors: Some(&sensors),
            storage: None,
            cpu_usage_percent: None,
            memory_usage_percent: Some(61.0),
            memory_total_bytes: 32 * GIB,
            monitor_reason: None,
        };
        let cards = build_cards(&inputs);
        let cpu = cards
            .iter()
            .find(|card| card.kind == CardKind::Cpu)
            .expect("处理器卡必须存在");
        assert_eq!(cpu.big_value, Some(23), "无基线时应回退传感器负载 22.5→23");
        assert_eq!(
            cpu.value_source,
            Some(super::super::dto::ValueSource::SensorLoad)
        );
        assert_eq!(cpu.label, "Intel Core i7-12700H");
        assert_eq!(cpu.detail, "14 核心 · 20 线程");
        // CPU 有温度语义却没读到 → 未检测（不是隐藏、更不是 0）
        assert_eq!(cpu.badge.state, super::super::dto::BadgeState::Undetected);
    }

    #[test]
    fn gpu_cards_appear_only_when_they_can_say_something() {
        let identity = SystemIdentity::default();
        let without_sensors = CardInputs {
            identity: &identity,
            sensors: None,
            storage: None,
            cpu_usage_percent: Some(10.0),
            memory_usage_percent: Some(50.0),
            memory_total_bytes: GIB,
            monitor_reason: Some("未找到 AegisMonitor.exe"),
        };
        let cards = build_cards(&without_sensors);
        assert!(
            !cards.iter().any(|card| card.kind == CardKind::Gpu),
            "无传感器时不得凭空造显卡卡"
        );
        let cpu = cards
            .iter()
            .find(|card| card.kind == CardKind::Cpu)
            .expect("应有处理器卡");
        assert!(
            cpu.degrade_reason
                .as_deref()
                .unwrap_or_default()
                .contains("AegisMonitor"),
            "L-08：整卡降级须带一行原因"
        );

        let mut sensors = snapshot(vec![
            GpuSensor {
                device: "NVIDIA GeForce RTX 3060 Laptop GPU".to_string(),
                temperature_c: Some(56.0),
                load_percent: Some(1.0),
            },
            GpuSensor {
                device: "Silenced Adapter".to_string(),
                temperature_c: None,
                load_percent: None,
            },
        ]);
        sensors.cpu_temperature_c = Some(91.0);
        let with_sensors = CardInputs {
            identity: &identity,
            sensors: Some(&sensors),
            storage: None,
            cpu_usage_percent: None,
            memory_usage_percent: Some(50.0),
            memory_total_bytes: GIB,
            monitor_reason: None,
        };
        let cards = build_cards(&with_sensors);
        let gpus: Vec<_> = cards
            .iter()
            .filter(|card| card.kind == CardKind::Gpu)
            .collect();
        assert_eq!(gpus.len(), 1, "负载与温度皆无的那张不得出现（R-07）");
        assert_eq!(gpus[0].label, "RTX 3060");
        assert_eq!(gpus[0].big_value, Some(1));
        assert_eq!(
            gpus[0].badge.level,
            Some(super::super::dto::BadgeLevel::Normal)
        );

        // 91℃ 处理器且无使用率无负载 → 温度顶位 + 过热徽章
        let cpu = cards
            .iter()
            .find(|card| card.kind == CardKind::Cpu)
            .expect("应有处理器卡");
        assert_eq!(
            cpu.badge.level,
            Some(super::super::dto::BadgeLevel::Critical)
        );
        assert_eq!(cpu.big_value, Some(91));
        assert_eq!(cpu.bar_percent, None, "温度顶位没有百分比语义");
    }

    #[test]
    fn storage_card_uses_media_subtitle_and_hides_disk_badge_without_evidence() {
        let identity = SystemIdentity::default();
        let storage = StorageSummary {
            media_text: "SSD 476.9 GB".to_string(),
            used_percent: 78.4,
            total_bytes: 476 * GIB,
            free_bytes: 100 * GIB,
            largest_volume: "C:".to_string(),
            volume_count: 2,
            unavailable_reason: None,
        };
        let inputs = CardInputs {
            identity: &identity,
            sensors: None,
            storage: Some(&storage),
            cpu_usage_percent: Some(5.0),
            memory_usage_percent: Some(50.0),
            memory_total_bytes: GIB,
            monitor_reason: None,
        };
        let cards = build_cards(&inputs);
        let disk = cards
            .iter()
            .find(|card| card.kind == CardKind::Storage)
            .expect("应有存储卡");
        assert_eq!(disk.label, "SSD 476.9 GB");
        assert_eq!(disk.big_value, Some(78));
        assert_eq!(disk.detail, "C: 剩余 100.0 GB");
        // 无盘温证据 → 隐藏徽章（R-09），不是"未检测"
        assert_eq!(
            disk.badge.state,
            super::super::dto::BadgeState::NotApplicable
        );
    }

    #[test]
    fn storage_card_without_summary_still_exists_with_reason() {
        let identity = SystemIdentity::default();
        let inputs = CardInputs {
            identity: &identity,
            sensors: None,
            storage: None,
            cpu_usage_percent: Some(5.0),
            memory_usage_percent: Some(50.0),
            memory_total_bytes: GIB,
            monitor_reason: None,
        };
        let cards = build_cards(&inputs);
        let disk = cards
            .iter()
            .find(|card| card.kind == CardKind::Storage)
            .expect("L-08：卡不得消失");
        assert_eq!(disk.big_value, None, "无证据不得给数字（R-08）");
        assert!(disk.degrade_reason.is_some(), "空态须写明原因");
    }

    #[test]
    fn collect_storage_on_this_machine_finds_the_system_volume() {
        let summary = collect_storage();
        assert!(
            summary.volume_count > 0,
            "本机应至少枚举到一个固定卷：{:?}",
            summary.unavailable_reason
        );
        assert!(summary.total_bytes > 0);
        assert!((0.0..=100.0).contains(&summary.used_percent));
        assert!(
            !summary.media_text.is_empty(),
            "副标题不得为空：{summary:?}"
        );
        assert!(summary.unavailable_reason.is_none());
    }

    #[test]
    fn bridge_without_attempt_reports_starting() {
        let bridge = SensorBridge::new();
        let (state, reason) = map_bridge_state(&bridge);
        assert_eq!(state, MonitorState::Starting);
        assert!(reason.expect("首屏须给出原因").contains("15 秒"));
    }

    #[test]
    fn ready_bridge_reports_ready_without_reason() {
        let inner = OverviewInner::new();
        inner.bridge().mark_ready_for_tests();
        let status = monitor_status(&inner);
        assert_eq!(status.state, MonitorState::Ready);
        assert!(status.reason.is_none(), "就绪态不应挂任何故障文案");
    }

    #[test]
    fn missing_sidecar_is_reported_as_exe_missing() {
        let inner = OverviewInner::new();
        inner.bridge().force_exe_missing_for_tests();
        let status = monitor_status(&inner);
        assert_eq!(status.state, MonitorState::ExeMissing);
        assert!(
            status
                .reason
                .as_deref()
                .unwrap_or_default()
                .contains("AegisMonitor.exe"),
            "原因须点名组件，实得：{:?}",
            status.reason
        );
        assert_eq!(status.sidecar_pid, None, "未接管进程时不得伪造 PID");
    }

    /// 通道确定不可用的 inner：🔴 单测绝不真去 spawn 本机已构建的 `AegisMonitor.exe`——
    /// 一旦真起子进程，GPU 卡会在断言中途随传感器到货而点亮，卡数 3→4，测试变成和子进程抢时序。
    fn inner_without_channel() -> Arc<OverviewInner> {
        let inner = OverviewInner::new();
        inner.bridge().force_exe_missing_for_tests();
        inner
    }

    /// 组件缺失后桥会进入 30 秒冷却（重试用尽），但界面缺的不是"冷却"这件事。
    #[test]
    fn cooling_after_a_missing_component_still_names_the_component() {
        let inner = inner_without_channel();
        let fault = inner
            .bridge()
            .read_once()
            .expect_err("exe 不在必须快速失败，不得静默成功");
        assert_eq!(fault.kind, FaultKind::ExeMissing);
        assert!(
            inner.bridge().cooling_down(),
            "失败一轮后应进入冷却（30 秒后才再试）"
        );
        let status = monitor_status(&inner);
        assert_eq!(
            status.state,
            MonitorState::ExeMissing,
            "冷却态不得盖掉更稳定的原因（N-06）"
        );
        assert!(status
            .reason
            .expect("降级必须给原因")
            .contains("AegisMonitor.exe"));
    }

    #[test]
    fn sample_never_waits_for_the_sidecar() {
        let inner = inner_without_channel();
        let started = Instant::now();
        let first = sample(&inner);
        let elapsed = started.elapsed();
        assert!(
            elapsed < Duration::from_millis(1_500),
            "首拍不得等待通道接入，实得 {elapsed:?}"
        );
        assert!(!first.header.computer_name.is_empty(), "页头须有计算机名");
        assert!(
            first.cards.len() >= 3,
            "至少处理器/内存/存储三卡在场，实得 {}",
            first.cards.len()
        );
        assert!(first
            .cards
            .iter()
            .all(|card| card.big_value.map_or(true, |value| value <= 100)));
        assert_eq!(first.cards[0].kind, CardKind::Cpu, "卡序固定：处理器在最前");
        assert_eq!(
            first.monitor.state,
            MonitorState::ExeMissing,
            "通道不可用必须如实带上行给出（F-12/N-06），不得伪装成就绪"
        );
    }

    #[test]
    fn second_sample_reports_uptime_monotonic_and_cached_identity() {
        let inner = inner_without_channel();
        let first = sample(&inner);
        let second = sample(&inner);
        assert_eq!(
            first.header.os_name, second.header.os_name,
            "身份项应走缓存"
        );
        assert!(second.header.uptime_secs >= first.header.uptime_secs);
        assert_eq!(
            second.cards.len(),
            first.cards.len(),
            "通道状态未变时卡数不得随节拍跳动（L-08 稳定布局）"
        );
    }
}
