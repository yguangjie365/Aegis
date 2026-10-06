use super::cards::{build_cards, CardInputs};
use super::sampling::sample;
use super::status::{map_bridge_state, monitor_status};
use super::storage::{
    collect_storage, drive_letter, max_used_percent, media_text, used_percent_of, Media,
};
use crate::overview::dto::{CardKind, MonitorState};
use crate::overview::sensor_bridge::{FaultKind, GpuSensor, SensorBridge, SensorSnapshot};
use crate::overview::state::{OverviewInner, StorageSummary, SystemIdentity};
use std::sync::Arc;
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
        memory_availability: crate::overview::sensor_bridge::MetricAvailability::Unsupported,
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

/// 🔴 分段条三级阈值必须与母本 `StatCardVm.SegLevel` 逐值一致
/// （母本裁定原文："占用也要分级，7% 不应显红" → `<60` 绿 / `<85` 橙 / 其余红）。
/// 边界值逐个钉住：59.9/60/84.9/85 四个点都试一遍，防止有人把 `>=` 写成 `>`。
#[test]
fn seg_level_thresholds_match_mother_template() {
    use crate::overview::dto::SegLevel;
    let n = SegLevel::from_percent;
    assert_eq!(
        n(None),
        SegLevel::Normal,
        "无数据按Normal（轨道为空，颜色无从体现）"
    );
    assert_eq!(n(Some(0.0)), SegLevel::Normal);
    assert_eq!(n(Some(7.0)), SegLevel::Normal, "母本明示 7% 不应显红");
    assert_eq!(n(Some(59.9)), SegLevel::Normal);
    assert_eq!(n(Some(60.0)), SegLevel::Warning, "60是橙的下界（闭区间）");
    assert_eq!(n(Some(84.9)), SegLevel::Warning);
    assert_eq!(n(Some(85.0)), SegLevel::Critical, "85 是红的下界（闭区间）");
    assert_eq!(n(Some(100.0)), SegLevel::Critical);
}

/// 分级必须由后端判完并随 DTO 送出，前端只按值取色（避免两处判级漂移）。
#[test]
fn cards_carry_seg_level_from_their_own_bar_percent() {
    let inner = inner_without_channel();
    let out = sample(&inner);
    assert!(!out.cards.is_empty(), "至少应有 CPU 卡");
    for c in &out.cards {
        // 无百分比的降级卡必须是 Normal；其余必须与自身 bar_percent 自洽
        match c.bar_percent {
            None => assert_eq!(
                c.seg_level,
                crate::overview::dto::SegLevel::Normal,
                "{:?} 卡无百分比却带非 Normal 分级",
                c.kind
            ),
            Some(p) => assert_eq!(
                c.seg_level,
                crate::overview::dto::SegLevel::from_percent(Some(p)),
                "{:?} 卡分级与自身 barPercent 不自洽",
                c.kind
            ),
        }
    }
}
