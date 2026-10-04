//! 主板与 BIOS 详情卡（F-02 · 规格 §五「主板」「BIOS」行集，片2-b，2026-10-03）。
//!
//! **复用结论（DP-1）**：四条 WQL 与"BaseBoard 空则回退 ComputerSystem"的兜底链取自
//! `Source/NexBox/src-tauri/src/hardware.rs:1592-1651`（②档改造移植）。
//!
//! 🔴 与母本的偏离：
//! 1. 母本把机箱类型（`Win32_SystemEnclosure.ChassisTypes`）渲染成 `"未知"` 字符串塞进
//!    `MotherboardInfo.chassis_type`；规格 §五 的主板行集只有**型号、制造商**两项，
//!    本域因此**不出机箱行**（多出来的信息项就是无人认领的展示面），但保留该查询的缺失留痕；
//! 2. 母本连主板 `SerialNumber` 一起采（`:1642`）——规格 §五 主板/BIOS 行集**不含序列号**，
//!    R-22 的豁免只覆盖物理盘与内存条，🔴 主板序列号不采不展示不上报告。

use infra::wmi::Row;

use super::rows::{cim_date, kv};
use crate::overview::dto::{DetailCardDto, DetailKind};

const BOARD_WQL: &str = "SELECT Manufacturer, Product FROM Win32_BaseBoard";
const SYSTEM_WQL: &str = "SELECT Manufacturer, Model FROM Win32_ComputerSystem";
const BIOS_WQL: &str = "SELECT Manufacturer, SMBIOSBIOSVersion, ReleaseDate FROM Win32_BIOS";

/// 主板卡：`Win32_BaseBoard` 为空（品牌机常见）时回退 `Win32_ComputerSystem`。
///
/// 🔴 查询失败与"查到了但为空"是两种原因，降级卡的 `reason` 必须分辨（R-08：不许把 COM 故障
/// 说成"没有这块板子"）；回退链两级都要查，任一级取到就出卡。
#[must_use]
pub fn board_card() -> DetailCardDto {
    let board = match infra::wmi::query(BOARD_WQL) {
        Ok(rows) => Some(rows),
        Err(reason) => {
            // 第一级就失败：仍尝试第二级，但把失败原因留下来，两级都拿不到时用它的命名做降级说明
            let fallback = infra::wmi::query(SYSTEM_WQL);
            return match fallback {
                Ok(rows) => rows.first().map_or_else(
                    || board_degraded(&reason),
                    |row| card_of(row.text("Model"), row.text("Manufacturer")),
                ),
                Err(fallback_reason) => {
                    board_degraded(&format!("{reason}；回退查询同样失败: {fallback_reason}"))
                }
            };
        }
    };
    let board_rows = board.unwrap_or_default();
    if let Some(row) = board_rows.first() {
        return card_of(row.text("Product"), row.text("Manufacturer"));
    }
    let system_rows = infra::wmi::query(SYSTEM_WQL).unwrap_or_default();
    system_rows.first().map_or_else(
        || board_degraded("WMI 未返回主板信息（Win32_BaseBoard 与 Win32_ComputerSystem 均为空）"),
        |row| card_of(row.text("Model"), row.text("Manufacturer")),
    )
}

#[must_use]
fn board_degraded(reason: &str) -> DetailCardDto {
    DetailCardDto {
        kind: DetailKind::Board,
        index: 0,
        reason: Some(reason.to_string()),
        rows: Vec::new(),
    }
}

fn card_of(model: Option<String>, manufacturer: Option<String>) -> DetailCardDto {
    DetailCardDto {
        kind: DetailKind::Board,
        index: 0,
        reason: None,
        rows: vec![
            kv("detail.board.model", model),
            kv("detail.board.manufacturer", manufacturer),
        ],
    }
}

/// BIOS 卡（版本 / 制造商 / 日期）。
#[must_use]
pub fn bios_card() -> DetailCardDto {
    let rows = match infra::wmi::query(BIOS_WQL) {
        Ok(rows) => rows,
        Err(reason) => return bios_degraded(&reason),
    };
    let Some(row) = rows.first() else {
        return bios_degraded("WMI 未返回 BIOS 信息（Win32_BIOS 为空）");
    };
    DetailCardDto {
        kind: DetailKind::Bios,
        index: 0,
        reason: None,
        rows: vec![
            kv("detail.bios.version", row.text("SMBIOSBIOSVersion")),
            kv("detail.bios.manufacturer", row.text("Manufacturer")),
            kv("detail.bios.date", bios_date(row)),
        ],
    }
}

#[must_use]
fn bios_degraded(reason: &str) -> DetailCardDto {
    DetailCardDto {
        kind: DetailKind::Bios,
        index: 0,
        reason: Some(reason.to_string()),
        rows: Vec::new(),
    }
}

/// BIOS 日期：`ReleaseDate` 走 CIM 串解码，解不出即 `None`（R-08，🔴 不拿原始残串糊上去）。
fn bios_date(row: &Row) -> Option<String> {
    cim_date(row.text("ReleaseDate").as_deref())
}

#[cfg(test)]
mod tests {
    use super::*;

    use infra::wmi::Value;

    #[test]
    fn bios_date_decodes_cim_string_only_when_complete() {
        let dated = Row::from_pairs(&[(
            "ReleaseDate",
            Value::Text("20240315000000.000000+000".to_string()),
        )]);
        assert_eq!(bios_date(&dated).as_deref(), Some("2024-03-15"));
        let garbage = Row::from_pairs(&[("ReleaseDate", Value::Text("N/A".to_string()))]);
        assert_eq!(bios_date(&garbage), None);
        let absent = Row::from_pairs(&[]);
        assert_eq!(bios_date(&absent), None);
    }

    #[test]
    fn board_card_rows_carry_keys_only_never_placeholder_text() {
        let card = card_of(Some("B650M AORUS ELITE".to_string()), None);
        assert_eq!(card.kind, DetailKind::Board);
        assert_eq!(card.rows[0].key, "detail.board.model");
        assert_eq!(
            card.rows[0].value.as_deref(),
            Some("B650M AORUS ELITE"),
            "母板行集顺序必须是型号在前"
        );
        assert_eq!(
            card.rows[1].value, None,
            "取不到的制造商必须是 None，不是\"未知\""
        );
    }
}
