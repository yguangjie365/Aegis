//! 内存详情卡（F-02 · 规格 §五「内存」行集：总容量、类型、频率、条数、序列号，片2-b）。
//!
//! **复用结论（DP-1）**：WQL 与两条回退链取自 `Source/NexBox/src-tauri/src/hardware.rs:1656-1705`
//! （②档改造移植）；🔴 **不移植 `crate::jedec`（2262 行的内存厂商 JEDEC 码表）**——规格 §五 的
//! 内存行集没有"制造商"这一项，那张表在本工程无消费点（登记见 `DEPENDENCIES.md §三`）。
//!
//! 🔴 与母本的偏离：
//! 1. 母本每根条子建一个 `MemoryInfo`，本工程按规格出**一张汇总卡**（总容量/类型/频率/条数/序列号），
//!    多根不同型号时类型与频率走 [`super::rows::join_distinct`] 逐值列出，🔴 不得只取第一条当全体；
//! 2. 序列号按 **R-22** 采集并展示（用户拍板③，2026-10-03），但🔴 值不得进任何日志正文——
//!    本文件的诊断只说"第 n 条取到/未取到"，🔴 也不拿型号（`PartNumber`）或容量冒充序列号。

use infra::wmi::Row;

use super::rows::{join_distinct, kv, memory_type_name, real_text};
use crate::overview::dto::{gib_text, DetailCardDto, DetailKind};

const WQL: &str = "SELECT Capacity, MemoryType, SMBIOSMemoryType, ConfiguredClockSpeed, Speed, \
                   SerialNumber FROM Win32_PhysicalMemory";

/// 内存汇总卡（单卡，`index` 恒 0）。
#[must_use]
pub fn card() -> DetailCardDto {
    match infra::wmi::query(WQL) {
        Ok(rows) if !rows.is_empty() => card_of(&rows),
        Ok(_) => degraded("WMI 未返回内存条信息（Win32_PhysicalMemory 空结果）"),
        Err(reason) => degraded(&reason),
    }
}

fn card_of(rows: &[Row]) -> DetailCardDto {
    let total: u64 = rows.iter().filter_map(|row| row.u64("Capacity")).sum();
    // 单条容量都取不到时不能报"总容量 0 GB"——那是把没证据说成测到了零
    let total_text = (total > 0).then(|| gib_text(total));

    let types: Vec<Option<String>> = rows.iter().map(memory_type_of).collect();
    let speeds: Vec<Option<String>> = rows.iter().map(speed_text_of).collect();
    let serials: Vec<Option<String>> = rows.iter().map(serial_of).collect();

    DetailCardDto {
        kind: DetailKind::Memory,
        index: 0,
        reason: None,
        rows: vec![
            kv("detail.memory.totalCapacity", total_text),
            kv("detail.memory.type", join_distinct(&types)),
            kv("detail.memory.frequency", join_distinct(&speeds)),
            kv("detail.memory.sticks", Some(rows.len().to_string())),
            kv("detail.memory.serial", join_distinct(&serials)),
        ],
    }
}

/// 类型：`MemoryType` 未知（0/缺失，DDR5 平台常见）时回退 `SMBIOSMemoryType`（母本同链 `:1676`）。
fn memory_type_of(row: &Row) -> Option<String> {
    memory_type_name(row.u16("MemoryType"))
        .or_else(|| memory_type_name(row.u16("SMBIOSMemoryType")))
        .map(ToString::to_string)
}

/// 频率：优先 `ConfiguredClockSpeed`（实际运行频率），为 0/缺失才回退 `Speed`（额定，母本 `:1690`）。
fn speed_text_of(row: &Row) -> Option<String> {
    let configured = row.u32("ConfiguredClockSpeed").filter(|value| *value > 0);
    let mhz = configured.or_else(|| row.u32("Speed").filter(|value| *value > 0))?;
    Some(format!("{mhz} MT/s"))
}

/// 序列号：占位串（全 0 / 全 F / 空，未烧录 SMBIOS 的条子）当没取到（R-08 + R-22）。
fn serial_of(row: &Row) -> Option<String> {
    real_text(row.text("SerialNumber").as_deref())
}

#[must_use]
fn degraded(reason: &str) -> DetailCardDto {
    DetailCardDto {
        kind: DetailKind::Memory,
        index: 0,
        reason: Some(reason.to_string()),
        rows: Vec::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use infra::wmi::Value;

    fn row(pairs: &[(&str, Value)]) -> Row {
        Row::from_pairs(pairs)
    }

    #[test]
    fn type_falls_back_to_smbios_when_memorytype_is_unknown() {
        let ddr5 = row(&[
            ("MemoryType", Value::Unsigned(0)),
            ("SMBIOSMemoryType", Value::Unsigned(34)),
        ]);
        assert_eq!(memory_type_of(&ddr5).as_deref(), Some("DDR5"));
        let empty = row(&[("MemoryType", Value::Unsigned(0))]);
        assert_eq!(memory_type_of(&empty), None, "两路都无证据时不得编造型号");
    }

    #[test]
    fn frequency_prefers_configured_then_rated() {
        let both = row(&[
            ("ConfiguredClockSpeed", Value::Unsigned(5600)),
            ("Speed", Value::Unsigned(6000)),
        ]);
        assert_eq!(speed_text_of(&both).as_deref(), Some("5600 MT/s"));
        let only_rated = row(&[
            ("ConfiguredClockSpeed", Value::Unsigned(0)),
            ("Speed", Value::Unsigned(4800)),
        ]);
        assert_eq!(speed_text_of(&only_rated).as_deref(), Some("4800 MT/s"));
        assert_eq!(speed_text_of(&row(&[])), None);
    }

    #[test]
    fn placeholder_serial_is_missing_not_the_model_number() {
        let stub = row(&[
            ("SerialNumber", Value::Text("00000000".to_string())),
            ("PartNumber", Value::Text("M321R8GA0EB2-CCP".to_string())),
        ]);
        assert_eq!(serial_of(&stub), None, "🔴 严禁用型号顶替序列号");
        let real = row(&[("SerialNumber", Value::Text(" 9A3B1 ".to_string()))]);
        assert_eq!(serial_of(&real).as_deref(), Some("9A3B1"));
    }

    #[test]
    fn mixed_sticks_list_every_distinct_value() {
        let rows = vec![
            row(&[
                ("Capacity", Value::Unsigned(17_179_869_184)),
                ("SMBIOSMemoryType", Value::Unsigned(34)),
                ("ConfiguredClockSpeed", Value::Unsigned(5600)),
            ]),
            row(&[
                ("Capacity", Value::Unsigned(17_179_869_184)),
                ("SMBIOSMemoryType", Value::Unsigned(34)),
                ("ConfiguredClockSpeed", Value::Unsigned(4800)),
            ]),
        ];
        let card = card_of(&rows);
        let find = |key: &str| {
            card.rows
                .iter()
                .find(|row| row.key == key)
                .and_then(|row| row.value.clone())
        };
        assert_eq!(find("detail.memory.type").as_deref(), Some("DDR5"));
        assert_eq!(
            find("detail.memory.frequency").as_deref(),
            Some("5600 MT/s / 4800 MT/s"),
            "两根不同频率时必须都列出来"
        );
        assert_eq!(find("detail.memory.sticks").as_deref(), Some("2"));
        assert_eq!(
            find("detail.memory.totalCapacity").as_deref(),
            Some("32.0 GB")
        );
    }

    #[test]
    fn zero_capacity_total_is_reported_as_missing() {
        let rows = vec![row(&[("Capacity", Value::Unsigned(0))])];
        let card = card_of(&rows);
        let total = card
            .rows
            .iter()
            .find(|row| row.key == "detail.memory.totalCapacity")
            .and_then(|row| row.value.clone());
        assert_eq!(total, None, "🔴 没证据不能报 0 GB");
    }
}
