//! 概览展示口径（纯函数，M-01 片1，2026-10-03）。
//!
//! 本文件是"读数 → 用户可见值"这条链的**单源实现**：R-07 大数字回退链、R-09 温度徽章分级、
//! R-21 型号精简展示都在这里判完，`live.rs` 只负责取数、前端只负责渲染。
//! 之所以坚持纯函数：这三条规则是本模块最容易在真机上被"看起来不对"改错的地方，
//! 只有不依赖采样环境才能被测试钉住（见文件末测试集）。

use super::dto::{
    clamp_percent, to_big_value, BadgeLevel, BadgeState, CardKind, TempBadgeDto, ValueSource,
};

/// R-09 温度阈值（警告 / 临界），按卡类别区分。
///
/// 规格原文：处理器 70/90、显卡 75/90、内存 70/85、存储 70/85。
#[derive(Debug, Clone, Copy)]
pub struct TempThreshold {
    pub warning_c: f64,
    pub critical_c: f64,
}

#[must_use]
pub const fn thresholds(kind: CardKind) -> TempThreshold {
    match kind {
        CardKind::Cpu => TempThreshold {
            warning_c: 70.0,
            critical_c: 90.0,
        },
        CardKind::Gpu => TempThreshold {
            warning_c: 75.0,
            critical_c: 90.0,
        },
        CardKind::Memory | CardKind::Storage => TempThreshold {
            warning_c: 70.0,
            critical_c: 85.0,
        },
    }
}

#[must_use]
pub fn badge_level(temperature_c: f64, kind: CardKind) -> BadgeLevel {
    let limit = thresholds(kind);
    if temperature_c >= limit.critical_c {
        BadgeLevel::Critical
    } else if temperature_c >= limit.warning_c {
        BadgeLevel::Warning
    } else {
        BadgeLevel::Normal
    }
}

/// 温度徽章（R-09 + R-08）。
///
/// `has_temperature_source` = 该设备族在本次传感器快照里是否有温度来源：
/// CPU/显卡天然有温度语义，无读数时如实给 `Undetected`（前端出"未检测"）；
/// 内存条与盘温在本次环境无传感器证据时给 `NotApplicable`，🔴 隐藏徽章而非显示占位文字。
#[must_use]
pub fn build_badge(
    kind: CardKind,
    temperature_c: Option<f64>,
    has_temperature_source: bool,
) -> TempBadgeDto {
    let (state, level) = match temperature_c {
        Some(value) => (BadgeState::Measured, Some(badge_level(value, kind))),
        None if has_temperature_source => (BadgeState::Undetected, None),
        None => (BadgeState::NotApplicable, None),
    };
    TempBadgeDto {
        state,
        temperature_c,
        level,
    }
}

/// R-07 大数字回退链：系统使用率 → 传感器负载 → 温度顶位。
///
/// 返回 `(大数字, 来源, 进度条)`；三级全空时为 `(None, None, None)`（R-08 出破折号，绝不填 0）。
/// `temperature_can_stand_in`：是否允许温度顶位（显卡卡由调用方按"有负载或有温度"判定后传 true）。
#[must_use]
pub fn pick_big_value(
    usage_percent: Option<f64>,
    sensor_load_percent: Option<f64>,
    temperature_c: Option<f64>,
    temperature_can_stand_in: bool,
) -> (Option<u8>, Option<ValueSource>, Option<f64>) {
    if let Some(value) = usage_percent {
        let clamped = clamp_percent(value);
        return (
            Some(to_big_value(clamped)),
            Some(ValueSource::Usage),
            Some(clamped),
        );
    }
    if let Some(value) = sensor_load_percent {
        let clamped = clamp_percent(value);
        return (
            Some(to_big_value(clamped)),
            Some(ValueSource::SensorLoad),
            Some(clamped),
        );
    }
    if let (true, Some(value)) = (temperature_can_stand_in, temperature_c) {
        // 温度顶位只为"卡片不空白"，它没有百分比语义 → 不给进度条（避免把 88℃ 画成 88%）。
        return (
            Some(to_big_value(value)),
            Some(ValueSource::Temperature),
            None,
        );
    }
    (None, None, None)
}

/// R-07 显卡卡创建条件：仅在有负载或有温度时创建，二者皆无则整卡缺席。
#[must_use]
pub fn gpu_card_worth_showing(load_percent: Option<f64>, temperature_c: Option<f64>) -> bool {
    load_percent.is_some() || temperature_c.is_some()
}

/// 逐词判定是否为序数代际前缀（`11th` / `12th` / `13th` …）。
fn is_ordinal_generation(word: &str) -> bool {
    let lower = word.to_ascii_lowercase();
    let Some(head) = lower
        .strip_suffix("th")
        .or_else(|| lower.strip_suffix("st"))
    else {
        return false;
    };
    !head.is_empty() && head.chars().all(|c| c.is_ascii_digit())
}

/// 剔除序数代际词及其紧随的 "Gen"，保留其余词序。
fn remove_generation_tokens(brand: &str) -> String {
    let mut kept: Vec<&str> = Vec::new();
    let mut previous_was_ordinal = false;
    for word in brand.split_whitespace() {
        if is_ordinal_generation(word) {
            previous_was_ordinal = true;
            continue;
        }
        if previous_was_ordinal && word.eq_ignore_ascii_case("gen") {
            previous_was_ordinal = false;
            continue;
        }
        previous_was_ordinal = false;
        kept.push(word);
    }
    kept.join(" ")
}

/// R-21 处理器型号精简：去代际前缀（"11th Gen"/"12th Gen"）、去 `(TM)`/`(R)` 标记、压缩空白。
///
/// 原始完整型号仍在片2 的详情卡展示，这里只服务统计卡副标题。
#[must_use]
pub fn short_cpu_brand(brand: &str) -> String {
    let cleaned = remove_generation_tokens(brand)
        .replace("(R)", "")
        .replace("(TM)", "");
    let collapsed = cleaned.split_whitespace().collect::<Vec<_>>().join(" ");
    // 精简后为空（异常输入）时回退原文，宁给长串不给空串
    if collapsed.is_empty() {
        brand.trim().to_string()
    } else {
        collapsed
    }
}

/// R-21 显卡型号精简：去厂商前缀与便携后缀（"Laptop GPU"），保留产品名。
///
/// 例：`NVIDIA GeForce RTX 3060 Laptop GPU` -> `RTX 3060`；`AMD Radeon Graphics` -> `Radeon Graphics`。
/// 厂商词只在**开头**剥、且剥完仍要剩得下产品名；便携后缀只在**结尾**剥。
#[must_use]
pub fn short_gpu_name(device: &str) -> String {
    const VENDORS: [&str; 3] = ["NVIDIA", "AMD", "Intel"];
    const SERIES: [&str; 1] = ["GeForce"];

    let mut words: Vec<&str> = device
        .split_whitespace()
        .map(|word| word.trim_matches(','))
        .filter(|word| !word.is_empty())
        .collect();

    // 至少留两个词才允许剥前缀，防止 "Intel UHD Graphics" 被剥成 "Graphics"
    if words.len() > 2 {
        if let Some(first) = words.first() {
            if VENDORS
                .iter()
                .any(|vendor| first.eq_ignore_ascii_case(vendor))
            {
                words.remove(0);
            }
        }
    }
    if words.len() > 2 {
        if let Some(first) = words.first() {
            if SERIES
                .iter()
                .any(|series| first.eq_ignore_ascii_case(series))
            {
                words.remove(0);
            }
        }
    }
    while words.len() > 2 {
        let matched = match words.last() {
            Some(last) => last.eq_ignore_ascii_case("Laptop") || last.eq_ignore_ascii_case("GPU"),
            None => false,
        };
        if !matched {
            break;
        }
        words.pop();
    }

    let cleaned = words.join(" ").replace("(R)", "").replace("(TM)", "");
    let collapsed = cleaned.split_whitespace().collect::<Vec<_>>().join(" ");
    if collapsed.is_empty() {
        device.trim().to_string()
    } else {
        collapsed
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn badge_levels_follow_per_kind_thresholds() {
        assert_eq!(badge_level(69.0, CardKind::Cpu), BadgeLevel::Normal);
        assert_eq!(badge_level(70.0, CardKind::Cpu), BadgeLevel::Warning);
        assert_eq!(badge_level(90.0, CardKind::Cpu), BadgeLevel::Critical);
        // 显卡警告线比处理器高 5 度（R-09 原文）
        assert_eq!(badge_level(74.9, CardKind::Gpu), BadgeLevel::Normal);
        assert_eq!(badge_level(75.0, CardKind::Gpu), BadgeLevel::Warning);
        // 内存/存储临界线 85
        assert_eq!(badge_level(86.0, CardKind::Memory), BadgeLevel::Critical);
        assert_eq!(badge_level(84.9, CardKind::Storage), BadgeLevel::Warning);
    }

    #[test]
    fn badge_states_distinguish_undetected_from_not_applicable() {
        let measured = build_badge(CardKind::Cpu, Some(68.0), true);
        assert_eq!(measured.state, BadgeState::Measured);
        assert_eq!(measured.level, Some(BadgeLevel::Normal));
        assert_eq!(measured.temperature_c, Some(68.0));

        let undetected = build_badge(CardKind::Cpu, None, true);
        assert_eq!(undetected.state, BadgeState::Undetected);
        assert_eq!(undetected.level, None);

        // 无温度来源（本机内存条/盘温不可读）→ 隐藏徽章，而不是给占位文字
        let hidden = build_badge(CardKind::Memory, None, false);
        assert_eq!(hidden.state, BadgeState::NotApplicable);
        assert_eq!(hidden.temperature_c, None);
    }

    #[test]
    fn big_value_falls_back_in_spec_order() {
        let (value, source, bar) = pick_big_value(Some(43.4), Some(90.0), Some(88.0), true);
        assert_eq!((value, source), (Some(43), Some(ValueSource::Usage)));
        assert_eq!(bar, Some(43.4));

        let (value, source, bar) = pick_big_value(None, Some(27.6), Some(88.0), true);
        assert_eq!((value, source), (Some(28), Some(ValueSource::SensorLoad)));
        assert_eq!(bar, Some(27.6));

        // 温度顶位：不给进度条
        let (value, source, bar) = pick_big_value(None, None, Some(88.4), true);
        assert_eq!((value, source), (Some(88), Some(ValueSource::Temperature)));
        assert_eq!(bar, None);

        // 三级全空 → 不给数字（R-08）
        let (value, source, bar) = pick_big_value(None, None, Some(60.0), false);
        assert_eq!((value, source, bar), (None, None, None));
    }

    #[test]
    fn out_of_range_usage_is_clamped_not_rejected() {
        let (value, source, bar) = pick_big_value(Some(137.0), None, None, false);
        assert_eq!((value, source), (Some(100), Some(ValueSource::Usage)));
        assert_eq!(bar, Some(100.0));
        let (value, _, _) = pick_big_value(Some(-5.0), None, None, false);
        assert_eq!(value, Some(0));
    }

    #[test]
    fn gpu_card_only_exists_when_it_can_say_something() {
        assert!(gpu_card_worth_showing(Some(1.0), None));
        assert!(gpu_card_worth_showing(None, Some(56.0)));
        assert!(!gpu_card_worth_showing(None, None));
    }

    #[test]
    fn cpu_brand_drops_generation_prefix_and_trademark_marks() {
        assert_eq!(
            short_cpu_brand("12th Gen Intel(R) Core(TM) i7-12700H"),
            "Intel Core i7-12700H"
        );
        assert_eq!(
            short_cpu_brand("11th Gen Intel(R) Core(TM) i5-1135G7 @ 2.40GHz"),
            "Intel Core i5-1135G7 @ 2.40GHz"
        );
        // 无代际前缀的型号不得被改坏
        assert_eq!(short_cpu_brand("AMD Ryzen 5 5600G"), "AMD Ryzen 5 5600G");
        // 异常输入回退原文，不给空串
        assert_eq!(short_cpu_brand("   "), "");
        assert_eq!(short_cpu_brand("Intel(R)"), "Intel");
    }

    #[test]
    fn gpu_name_drops_vendor_and_laptop_suffix_but_keeps_product() {
        assert_eq!(
            short_gpu_name("NVIDIA GeForce RTX 3060 Laptop GPU"),
            "RTX 3060"
        );
        assert_eq!(short_gpu_name("NVIDIA GeForce RTX 4090"), "RTX 4090");
        // Radeon 不是被剥的系列词；"Graphics" 不是后缀噪声 → 产品名保持可读
        assert_eq!(short_gpu_name("AMD Radeon Graphics"), "Radeon Graphics");
        assert_eq!(
            short_gpu_name("Intel(R) UHD Graphics"),
            "Intel UHD Graphics"
        );
        // 词数不足 3 时绝不剥前缀
        assert_eq!(short_gpu_name("Intel GPU"), "Intel GPU");
        assert_eq!(
            short_gpu_name("Microsoft Basic Display Adapter"),
            "Microsoft Basic Display Adapter"
        );
    }
}
