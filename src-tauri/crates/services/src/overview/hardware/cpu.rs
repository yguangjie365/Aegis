//! 处理器详情卡（F-02 · 规格 §五「处理器」行集，片2-b，2026-10-03）。
//!
//! **复用结论（DP-1）**：WQL 与字段取自 `Source/NexBox/src-tauri/src/hardware.rs:1530`
//! （②档改造移植，登记见 `DEPENDENCIES.md §三`）。
//!
//! 🔴 与母本的两处**有意偏离**：
//! 1. `SELECT` 只取规格 §五 用得上的六列（母本一次取 20 列，其中 `ProcessorId`/`Family`/
//!    `Stepping`/`VoltageCaps` 等在本工程无展示位）——列越少，COM 回包越小，也避免"采到了却不展示"
//!    的额外指纹面（R-22 只豁免盘与内存条的序列号，未豁免处理器标识）；
//! 2. 母本把缺失值写成 `"未知CPU"` / `"未知"` 字符串，本域改判 `None`（R-08，理由见 [`super::rows`] 头注）；
//!    值内也不写中文文案（"核心/线程"是信息项名，归键的 i18n 文案），值只交 `8 / 16` 这类裸数据。

use infra::wmi::Row;

use super::rows::{architecture_name, kv};
use crate::overview::dto::{DetailCardDto, DetailKind};

/// 规格 §五 的"主频"取 `MaxClockSpeed`（额定基频，MHz）。
const WQL: &str = "SELECT Name, NumberOfCores, NumberOfLogicalProcessors, MaxClockSpeed, \
                   L3CacheSize, Architecture FROM Win32_Processor";

/// 处理器卡（单值卡，`index` 恒 0）。
///
/// 温度一行本批恒空——🔴 温度唯一来源是 sidecar（K-004），由片2-e 的快照装配注入。
#[must_use]
pub fn card() -> DetailCardDto {
    match infra::wmi::query(WQL) {
        Ok(rows) => rows.first().map_or_else(
            || degraded("WMI 未返回处理器信息（Win32_Processor 空结果）"),
            cpu_card_of,
        ),
        Err(reason) => degraded(&reason),
    }
}

fn cpu_card_of(row: &Row) -> DetailCardDto {
    DetailCardDto {
        kind: DetailKind::Cpu,
        index: 0,
        reason: None,
        rows: vec![
            kv("detail.cpu.model", row.text("Name")),
            kv("detail.cpu.temperature", None),
            kv(
                "detail.cpu.cores",
                row.u32("NumberOfCores").and_then(|cores| {
                    let threads = row.u32("NumberOfLogicalProcessors")?;
                    Some(format!("{cores} / {threads}"))
                }),
            ),
            kv(
                "detail.cpu.frequency",
                row.u32("MaxClockSpeed").and_then(clock_text),
            ),
            kv(
                "detail.cpu.cacheL3",
                row.u32("L3CacheSize").and_then(cache_text),
            ),
            kv(
                "detail.cpu.architecture",
                architecture_name(row.u16("Architecture")).map(ToString::to_string),
            ),
        ],
    }
}

/// MHz → GHz 文本（`3708` → `3.71 GHz`，整 GHz 去尾零）；`0` 当没有证据（R-08）。
#[must_use]
#[allow(clippy::cast_precision_loss)] // 频率量级远小于 2^53，换算无精度损失
fn clock_text(mhz: u32) -> Option<String> {
    if mhz == 0 {
        return None;
    }
    let text = format!("{:.2}", f64::from(mhz) / 1000.0);
    Some(format!(
        "{} GHz",
        text.trim_end_matches('0').trim_end_matches('.')
    ))
}

/// KB → MB 文本；`0` 当没有证据（部分平台 `L3CacheSize` 不上报）。
#[must_use]
fn cache_text(kib: u32) -> Option<String> {
    if kib == 0 {
        return None;
    }
    Some(format!("{} MB", kib / 1024))
}

#[must_use]
fn degraded(reason: &str) -> DetailCardDto {
    DetailCardDto {
        kind: DetailKind::Cpu,
        index: 0,
        reason: Some(reason.to_string()),
        rows: Vec::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clock_text_rounds_to_two_decimals_and_trims_zeros() {
        assert_eq!(clock_text(3708).as_deref(), Some("3.71 GHz"));
        assert_eq!(clock_text(3000).as_deref(), Some("3 GHz"));
        assert_eq!(
            clock_text(240).as_deref(),
            Some("0.24 GHz"),
            "240 MHz 是合法低功耗频率"
        );
        assert_eq!(clock_text(0), None, "0 MHz 才是缺失");
    }

    #[test]
    fn zero_cache_is_treated_as_missing() {
        assert_eq!(cache_text(0), None);
        assert_eq!(cache_text(32_768).as_deref(), Some("32 MB"));
    }

    #[test]
    fn degraded_card_keeps_the_reason() {
        let card = degraded("COM 初始化失败");
        assert!(card.rows.is_empty(), "整卡降级时不再摆空行");
        assert_eq!(card.reason.as_deref(), Some("COM 初始化失败"));
        assert_eq!(card.kind, DetailKind::Cpu);
    }
}
