//! 详情卡的取值与解码（片2-b，2026-10-03）。
//!
//! 本文件只做两件事：**把 WMI 原始值折成可展示字符串**、**把数值编码折成类目名**。
//! 采集语句在各采集器文件里，装配顺序在 `hardware.rs` 门面的 `collect()` 里。
//!
//! 🔴 解码表的唯一口径来自规格 §五「详细信息」的信息项清单，母本行号逐条登记在各采集器头注。
//! 与母本的一处**有意偏离**：母本所有解码函数在未知档返回写死的 `"未知"` 字符串
//! （`NexBox/src-tauri/src/hardware.rs:1372/:1387/:1418/:1460`），本工程按 R-08 改判 `None`
//! ——前端拿到 `None` 才会渲染统一的"未检测"样式，拿到"未知"则等于把"没查到"和"查到值就是未知"
//! 混成同一个显示。

use crate::overview::dto::KvRowDto;

/// 一行键值（`value` 为 `None` 时前端出"未检测"，R-08）。
#[must_use]
pub fn kv(key: &str, value: Option<String>) -> KvRowDto {
    KvRowDto {
        key: key.to_string(),
        value,
    }
}

/// 一行键值（直接给字符串，空串当没取到）。
#[must_use]
pub fn kv_text(key: &str, value: Option<&str>) -> KvRowDto {
    kv(
        key,
        value
            .map(str::trim)
            .filter(|text| !text.is_empty())
            .map(ToString::to_string),
    )
}

/// WMI 占位串判定：空串、全 `0`、全 `F`（大小写均见实证）——母本 `hardware.rs:1670` 同判据。
///
/// 这类值出现在未烧录 SMBIOS 的板子上（`PartNumber = "0000000000000000"`），
/// 🔴 当真实值展示就是把垃圾交给用户。
#[must_use]
pub fn is_placeholder(text: &str) -> bool {
    let trimmed = text.trim().trim_end_matches('\0').trim();
    // 移除所有 null 字符后再判断
    let no_nulls: String = trimmed.chars().filter(|c| *c != '\0').collect();
    no_nulls.is_empty()
        || no_nulls.chars().all(|c| c == '0')
        || no_nulls.chars().all(|c| c.eq_ignore_ascii_case(&'f'))
}

/// 占位串安全的取值：命中占位形态即 `None`（R-08）。
#[must_use]
pub fn real_text(value: Option<&str>) -> Option<String> {
    let text = value?.trim().trim_end_matches('\0').trim();
    if text.is_empty() || is_placeholder(text) {
        return None;
    }
    Some(text.to_string())
}

/// 多设备去重汇总（内存类型/频率一类：条与条可能不同，🔴 不得只取第一条当全体）。
///
/// 顺序保持首次出现顺序；全部为空返回 `None`。
#[must_use]
pub fn join_distinct(values: &[Option<String>]) -> Option<String> {
    let mut seen: Vec<String> = Vec::new();
    for value in values.iter().flatten() {
        if !seen.iter().any(|existing| existing == value) {
            seen.push(value.clone());
        }
    }
    if seen.is_empty() {
        return None;
    }
    Some(seen.join(" / "))
}

/// `Win32_Processor.Architecture` 解码（母本 `hardware.rs:1372` 十档照搬，未知档改 `None`）。
#[must_use]
pub fn architecture_name(code: Option<u16>) -> Option<&'static str> {
    Some(match code? {
        0 => "x86",
        1 => "MIPS",
        2 => "Alpha",
        3 => "PowerPC",
        5 => "ARM",
        6 => "ia64",
        7 => "Alpha64",
        9 => "x64",
        12 => "ARM64",
        _ => return None,
    })
}

/// `Win32_PhysicalMemory.MemoryType` / `SMBIOSMemoryType` 解码（母本 `hardware.rs:1418`）。
#[must_use]
pub fn memory_type_name(code: Option<u16>) -> Option<&'static str> {
    Some(match code? {
        1 => "Other",
        2 => "DRAM",
        3 => "Synchronous DRAM",
        4 => "Cache DRAM",
        5 => "EDO",
        6 => "EDRAM",
        7 => "VRAM",
        8 => "SRAM",
        9 => "RAM",
        10 => "ROM",
        11 => "Flash",
        12 => "EEPROM",
        13 => "FEPROM",
        14 => "EPROM",
        15 => "CDRAM",
        16 => "3DRAM",
        17 => "SDRAM",
        18 => "SGRAM",
        19 => "RDRAM",
        20 => "DDR",
        21 => "DDR2",
        22 => "DDR2 FB-DIMM",
        24 => "DDR3",
        25 => "FBD2",
        26 => "DDR4",
        27 => "LPDDR",
        28 => "LPDDR2",
        29 => "LPDDR3",
        30 => "LPDDR4",
        31 => "Logical non-volatile",
        32 => "HBM",
        33 => "HBM2",
        34 => "DDR5",
        35 => "LPDDR5",
        36 => "HBM3",
        // 0（Unknown）与表外值都算没有证据（R-08）
        _ => return None,
    })
}

/// CIM 日期串 → `YYYY-MM-DD`（母本 `hardware.rs:942 format_driver_date` 同法：只取数字前 8 位）。
///
/// 实测形态两种：`Win32_BIOS.ReleaseDate` = `"20240101000000.000000+480"`，
/// `Win32_VideoController.DriverDate` 同族；不足 8 位数字即 `None`（🔴 不把残串当日期）。
#[must_use]
pub fn cim_date(value: Option<&str>) -> Option<String> {
    let raw = value?;
    let digits: String = raw.chars().filter(char::is_ascii_digit).take(8).collect();
    if digits.len() != 8 {
        return None;
    }
    Some(format!(
        "{}-{}-{}",
        &digits[0..4],
        &digits[4..6],
        &digits[6..8]
    ))
}

/// 链路速率：bps → 可读（`1000000000` → `1 Gbps`，`1_000_000` 整除则走 Mbps）。
///
/// `0` 与负数（未协商）一律 `None`——R-08：没协商上就是没有链路，不报 "0 Mbps"。
#[must_use]
pub fn link_speed_text(bits_per_second: Option<i64>) -> Option<String> {
    const MBPS: i64 = 1_000_000;
    const GBPS: i64 = 1_000_000_000;
    let bps = bits_per_second.filter(|value| *value > 0)?;
    if bps % GBPS == 0 {
        return Some(format!("{} Gbps", bps / GBPS));
    }
    Some(format!("{} Mbps", bps / MBPS))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn placeholder_tokens_are_rejected() {
        assert!(is_placeholder(""));
        assert!(is_placeholder("   "));
        assert!(is_placeholder("0000000000000000"));
        assert!(is_placeholder("FFFFFFFFFFFF"));
        assert!(is_placeholder("ffffff"));
        assert!(!is_placeholder("M321R8GA0EB2-CCP"));
        assert!(real_text(Some("\u{0}0000\u{0}")).is_none());
        assert_eq!(real_text(Some(" F9K4N1J2 ")).as_deref(), Some("F9K4N1J2"));
    }

    #[test]
    fn blank_kv_values_fold_to_missing() {
        assert_eq!(kv_text("detail.cpu.model", Some("  ")).value, None);
        assert_eq!(
            kv_text("detail.cpu.model", Some(" i7 ")).value.as_deref(),
            Some("i7")
        );
    }

    #[test]
    fn distinct_join_preserves_first_seen_order_and_skips_duplicates() {
        let rows = join_distinct(&[
            Some("DDR5".to_string()),
            None,
            Some("DDR5".to_string()),
            Some("DDR4".to_string()),
        ]);
        assert_eq!(rows.as_deref(), Some("DDR5 / DDR4"));
        assert_eq!(join_distinct(&[None, None]), None);
    }

    #[test]
    fn decode_tables_cover_the_observed_codes() {
        assert_eq!(architecture_name(Some(9)), Some("x64"));
        assert_eq!(architecture_name(Some(12)), Some("ARM64"));
        assert_eq!(
            architecture_name(Some(99)),
            None,
            "表外码不得当成某个已知架构"
        );
        assert_eq!(architecture_name(None), None);
        assert_eq!(memory_type_name(Some(34)), Some("DDR5"));
        assert_eq!(
            memory_type_name(Some(0)),
            None,
            "0 是 Unknown，不是 DDR 家族"
        );
        assert_eq!(
            memory_type_name(Some(0)),
            None,
            "0 是 Unknown，不是 DDR 家族"
        );
    }

    #[test]
    fn cim_date_takes_only_the_leading_eight_digits() {
        assert_eq!(
            cim_date(Some("20240101000000.000000+480")).as_deref(),
            Some("2024-01-01")
        );
        assert_eq!(
            cim_date(Some("202401")).as_deref(),
            None,
            "残串不得拼成日期"
        );
        assert_eq!(cim_date(None), None);
    }

    #[test]
    fn link_speed_distinguishes_absent_from_zero() {
        assert_eq!(
            link_speed_text(Some(1_000_000_000)).as_deref(),
            Some("1 Gbps")
        );
        assert_eq!(
            link_speed_text(Some(300_000_000)).as_deref(),
            Some("300 Mbps")
        );
        assert_eq!(link_speed_text(Some(0)), None);
        assert_eq!(link_speed_text(Some(-1)), None, "未协商不是 0 Mbps");
        assert_eq!(link_speed_text(None), None);
    }
}
