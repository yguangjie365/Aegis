//! 存储详情卡（F-02 · 规格 §五「存储」行集：每盘一行 型号/介质（容量）、序列号，片2-b）。
//!
//! **复用结论（DP-1）**：WQL 与介质判据取自 `Source/NexBox/src-tauri/src/hardware.rs:1718-1744`
//! （`Win32_DiskDrive` 采集）与 `:2660-2666`（健康面板的三级介质链，②档改造移植）。
//! 介质枚举 [`Media`] 复用片1 的 `crate::overview::live`——🔴 同一概念不在两处各长一遍（R-05 单源）。
//!
//! 🔴 与母本的偏离：
//! 1. **`Size` 的分区容量兜底不移植**（母本 `:1727 resolve_disk_size_gb`：`Size` 为 0 时改用分区求和）。
//!    规格 §五 要的是物理盘容量，分区和是"已分配量"，两者语义不同 → 取不到就是 `None`（R-08）；
//! 2. 母本把 `BytesPerSector`/`Partitions`/`FirmwareRevision`/`Status` 一起返回给前端，
//!    规格 §五 的存储行集没有这些项，本域不采（少一处指纹面）；
//! 3. 序列号按 **R-22**（用户拍板③）采集、展示并写进导出报告；🔴 值不得进任何日志正文，
//!    取不到按 R-08 显"未检测"，🔴 不得用型号顶替。
//! 4. 规格的"介质（容量）"是一个信息项带括号补语，本域拆成 `介质` 与 `容量` 两行键值
//!    （键值卡的键必须是信息项名，把容量塞进介质的值会让键名失去意义）——🟡 待片2-f UI 验收确认。

use infra::wmi::Row;

use super::rows::kv;
use crate::overview::dto::{gib_text, DetailCardDto, DetailKind};
use crate::overview::live::Media;

const WQL: &str = "SELECT Index, Model, Size, InterfaceType, MediaType, SerialNumber \
                   FROM Win32_DiskDrive";

/// 物理盘详情卡（每盘一张，R-01 全列；按 `Index` 升序保证节拍间稳定）。
#[must_use]
pub fn cards() -> Vec<DetailCardDto> {
    let rows = match infra::wmi::query(WQL) {
        Ok(rows) => rows,
        Err(reason) => return vec![degraded(&reason)],
    };
    let disks: Vec<&Row> = rows
        .iter()
        // 型号与容量都没有的行不是盘（枚举残留），🔴 不建卡（R-07：无意义的卡不创建）
        .filter(|row| row.text("Model").is_some() || row.u64("Size").is_some_and(|n| n > 0))
        .collect();
    if disks.is_empty() {
        return vec![degraded("WMI 未返回物理盘信息（Win32_DiskDrive 空结果）")];
    }

    let mut indexed: Vec<(u32, &Row)> = disks
        .into_iter()
        .map(|row| (row.u32("Index").unwrap_or(u32::MAX), row))
        .collect();
    indexed.sort_by_key(|(index, _)| *index);

    indexed
        .iter()
        .enumerate()
        .map(|(card_index, (_, row))| card_of(card_index, row))
        .collect()
}

fn card_of(index: usize, row: &Row) -> DetailCardDto {
    DetailCardDto {
        kind: DetailKind::Storage,
        index,
        reason: None,
        rows: vec![
            kv("detail.storage.model", row.text("Model")),
            kv("detail.storage.media", media_text(row)),
            kv("detail.storage.capacity", capacity_text(row)),
            kv("detail.storage.serial", serial_text(row)),
        ],
    }
}

/// 介质文案；`Unknown` 时返回 `None`（🔴 不臆断，前端的"未知介质"由键的 i18n 文案兜底）。
fn media_text(row: &Row) -> Option<String> {
    let media = media_of_row(row);
    (!matches!(media, Media::Unknown)).then(|| media.label().to_string())
}

/// 盘 → [`Media`]（R-05 信号链的静态部分；片1 的 `media_of(DiskKind)` 是卷侧的同族判定）。
///
/// 母本 `:2661` 的 `NVMe` 判据有三路（PNP 路径 / 接口类型 / 型号），本域不收 PNP 列，
/// 而 `InterfaceType == "SAS"` 是 SCSI 协议子集、单看它会把 SAS HDD 误判成 `NVMe`，
/// 🔴 因此只保留型号这一路可靠判据。
#[must_use]
pub fn media_of_row(row: &Row) -> Media {
    let model = row.text("Model").unwrap_or_default().to_lowercase();
    let media_type = row.text("MediaType").unwrap_or_default().to_lowercase();
    if model.contains("nvme") || media_type.contains("ssd") || media_type.contains("solid state") {
        return Media::Ssd;
    }
    if media_type.contains("hdd") || media_type.contains("fixed hard disk media") {
        return Media::Hdd;
    }
    Media::Unknown
}

/// 盘容量（`Size` 字节）；`0`/缺失一律 `None`（头注偏离 1）。
fn capacity_text(row: &Row) -> Option<String> {
    let bytes = row.u64("Size").filter(|value| *value > 0)?;
    Some(gib_text(bytes))
}

/// 序列号：占位串（全 0 / 全 F / 空）当没取到（R-08 + R-22）。
fn serial_text(row: &Row) -> Option<String> {
    super::rows::real_text(row.text("SerialNumber").as_deref())
}

#[must_use]
fn degraded(reason: &str) -> DetailCardDto {
    DetailCardDto {
        kind: DetailKind::Storage,
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
    fn nvme_is_ssd_even_when_mediatype_is_generic() {
        let nvme = row(&[
            ("Model", Value::Text("Samsung 980 PRO NVMe 1TB".to_string())),
            ("InterfaceType", Value::Text("SCSI".to_string())),
            (
                "MediaType",
                Value::Text("Fixed hard disk media".to_string()),
            ),
        ]);
        assert_eq!(
            media_of_row(&nvme),
            Media::Ssd,
            "型号含 NVMe 即 SSD（母本同判据）"
        );
    }

    #[test]
    fn media_type_keywords_decide_when_model_says_nothing() {
        let ssd = row(&[
            ("Model", Value::Text("Generic Disk".to_string())),
            ("MediaType", Value::Text("SSD".to_string())),
        ]);
        assert_eq!(media_of_row(&ssd), Media::Ssd);
        let hdd = row(&[(
            "MediaType",
            Value::Text("Fixed hard disk media".to_string()),
        )]);
        assert_eq!(media_of_row(&hdd), Media::Hdd);
        assert_eq!(
            media_of_row(&row(&[])),
            Media::Unknown,
            "🔴 全部未知时不臆断介质（走文案兜底）"
        );
    }

    #[test]
    fn unknown_media_row_stays_empty() {
        let disk = row(&[
            ("Index", Value::Unsigned(1)),
            ("Model", Value::Text("Mystery Volume".to_string())),
            ("Size", Value::Unsigned(500 * 1024_u64.pow(3))),
        ]);
        let card = card_of(0, &disk);
        let value_of = |key: &str| {
            card.rows
                .iter()
                .find(|item| item.key == key)
                .and_then(|item| item.value.clone())
        };
        assert_eq!(
            value_of("detail.storage.media"),
            None,
            "未知介质不出值，交前端出未检测"
        );
        assert_eq!(
            value_of("detail.storage.capacity").as_deref(),
            Some("500.0 GB")
        );
    }

    #[test]
    fn placeholder_serial_is_missing_not_the_model() {
        let disk = row(&[
            ("Model", Value::Text("Samsung SSD 870 EVO".to_string())),
            ("SerialNumber", Value::Text("000000000000".to_string())),
        ]);
        assert_eq!(serial_text(&disk), None, "🔴 严禁用型号顶替序列号");
        let real = row(&[("SerialNumber", Value::Text(" S6KN1NF0T12345K ".to_string()))]);
        assert_eq!(serial_text(&real).as_deref(), Some("S6KN1NF0T12345K"));
    }

    #[test]
    fn zero_size_is_not_reported_as_zero_capacity() {
        let disk = row(&[
            ("Model", Value::Text("Disk".to_string())),
            ("Size", Value::Unsigned(0)),
        ]);
        assert_eq!(capacity_text(&disk), None);
        assert_eq!(capacity_text(&row(&[])), None);
    }
}
