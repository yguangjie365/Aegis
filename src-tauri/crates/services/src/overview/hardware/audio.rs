//! 声卡详情卡（F-02 · 规格 §五「声卡（×n）」行集：型号、制造商；过滤口径 R-03，片2-b）。
//!
//! **复用结论（DP-1）**：WQL 取自 `Source/NexBox/src-tauri/src/hardware.rs:1758`（②档改造移植）；
//! 过滤基线取自 `Source/SystemToolkit` `OverviewService.cs:362 IsUserFacingSoundDevice`
//! （③思路复用——该四条正是 R-03 文案的出处：virtual / mirroring / remote audio / nahimic），
//! 再叠母本实机增补的虚拟音效框架关键词。
//!
//! 🔴 与母本的偏离（都朝"少杀真实硬件"的方向）：
//! 1. 母本按 PNP 前缀 `usb\` / `hid\` 整段拦掉——**USB DAC、HID 合规耳机都是真实音频硬件**，
//!    R-03 的"仅保留真实音频硬件"不允许按总线类型一刀切，本域🔴 不移植（软件设备只按 `swd\` 拦）；
//! 2. 母本关键词表里的 `Bluetooth` / `Hands-Free` / `DTS` / `Dolby` / `S/PDIF` 五条🔴 不采用：
//!    蓝牙音箱、带 Dolby/DTS 支持的 Realtek 硬件、光纤（S/PDIF）输出都是真实设备，
//!    按名称一刀切会把用户唯一的声卡杀没（🔴 面板整体为空比多列几条虚拟条目更糟，R-10）；
//! 3. `Status != "OK"` 的设备不展示（母本 `:1771` 照搬）——禁用/故障设备无展示价值；
//! 4. 母本每卡带 `Status`/`DeviceID` 字段，规格 §五 只有型号与制造商两项，本域不采。

use infra::wmi::Row;

use super::rows::kv;
use crate::overview::dto::{DetailCardDto, DetailKind};

const WQL: &str = "SELECT Name, Manufacturer, Status, PNPDeviceID FROM Win32_SoundDevice";

/// R-03 虚拟音频设备特征：`SystemToolkit` 四条基线 + 母本实机增补的框架名。
///
/// 🔴 只按名称拦"虚拟/远程/框架"，不按总线拦（见头注偏离 1/2）。
const VIRTUAL_AUDIO_KEYWORDS: [&str; 12] = [
    "virtual",
    "mirroring",
    "remote audio",
    "nahimic",
    "vb-audio",
    "voicemeeter",
    "cable",
    "sonic studio",
    "nvidia virtual",
    "steam streaming",
    "oculus",
    "wave link",
];

/// 声卡卡（每张真实音频设备一张，R-01 全列；一张都不剩时出一张降级卡说明为什么空）。
#[must_use]
pub fn cards() -> Vec<DetailCardDto> {
    let rows = match infra::wmi::query(WQL) {
        Ok(rows) => rows,
        Err(reason) => return vec![degraded(&reason)],
    };
    let visible: Vec<&Row> = rows
        .iter()
        .filter(|row| is_visible_sound_device(row))
        .collect();
    if visible.is_empty() {
        return vec![degraded(
            "WMI 未返回可展示的真实音频设备（Win32_SoundDevice 无匹配条目，R-03 口径）",
        )];
    }
    let mut sorted: Vec<&Row> = visible;
    sorted.sort_by_key(|row| row.text("Name"));
    sorted
        .iter()
        .enumerate()
        .map(|(index, &row)| card_of(index, row))
        .collect()
}

/// R-03 判定：状态可用 + 非软件枚举设备 + 名称不含虚拟音频特征。
fn is_visible_sound_device(row: &Row) -> bool {
    let status = row.text("Status").unwrap_or_default();
    if !status.eq_ignore_ascii_case("OK") {
        return false;
    }
    let pnp = row.text("PNPDeviceID").unwrap_or_default();
    if pnp.to_lowercase().starts_with(r"swd\") {
        return false;
    }
    let Some(name) = row.text("Name") else {
        return false;
    };
    let lowered = name.to_lowercase();
    !VIRTUAL_AUDIO_KEYWORDS
        .iter()
        .any(|keyword| lowered.contains(keyword))
}

fn card_of(index: usize, row: &Row) -> DetailCardDto {
    DetailCardDto {
        kind: DetailKind::Audio,
        index,
        reason: None,
        rows: vec![
            kv("detail.audio.model", row.text("Name")),
            kv("detail.audio.manufacturer", row.text("Manufacturer")),
        ],
    }
}

#[must_use]
fn degraded(reason: &str) -> DetailCardDto {
    DetailCardDto {
        kind: DetailKind::Audio,
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

    fn device(name: &str, extra: &str) -> Row {
        row(&[
            ("Name", Value::Text(name.to_string())),
            ("Manufacturer", Value::Text(extra.to_string())),
            ("Status", Value::Text("OK".to_string())),
            ("PNPDeviceID", Value::Text("HDAUDIO\\FUNC_01".to_string())),
        ])
    }

    #[test]
    fn real_devices_survive_the_filter() {
        assert!(is_visible_sound_device(&device(
            "Realtek(R) Audio",
            "Realtek Semiconductor Corp."
        )));
        // 🔴 偏离 1：USB DAC 是真实硬件，不得按总线前缀杀掉
        let usb_dac = row(&[
            ("Name", Value::Text("USB Audio DAC".to_string())),
            ("Status", Value::Text("OK".to_string())),
            (
                "PNPDeviceID",
                Value::Text("USB\\VID_04D9&PID_0001".to_string()),
            ),
        ]);
        assert!(is_visible_sound_device(&usb_dac));
        // 🔴 偏离 2：带 Dolby 支持的硬件、蓝牙音箱不因名称被一刀切
        let dolby = device("Realtek Audio with Dolby Access", "Realtek");
        assert!(is_visible_sound_device(&dolby));
    }

    #[test]
    fn virtual_and_software_devices_are_filtered() {
        assert!(
            !is_visible_sound_device(&device("VB-Audio Virtual Cable", "VB Audio")),
            "R-03：virtual 命中即虚拟设备"
        );
        assert!(!is_visible_sound_device(&device(
            "Steam Streaming Microphone",
            "Valve"
        )));
        assert!(!is_visible_sound_device(&device("Nahimic v3", "A-Volute")));
        assert!(!is_visible_sound_device(&device(
            "Microsoft Remote Audio",
            "MS"
        )));
        let swd = row(&[
            ("Name", Value::Text("Some Aux Device".to_string())),
            ("Status", Value::Text("OK".to_string())),
            ("PNPDeviceID", Value::Text("SWD\\AUDIO\\0001".to_string())),
        ]);
        assert!(!is_visible_sound_device(&swd), "`swd\\` 软件枚举设备不展示");
    }

    #[test]
    fn non_ok_status_is_not_displayed() {
        let disabled = row(&[
            ("Name", Value::Text("Realtek(R) Audio".to_string())),
            ("Status", Value::Text("Degraded".to_string())),
        ]);
        assert!(!is_visible_sound_device(&disabled));
        let no_status = row(&[("Name", Value::Text("Realtek(R) Audio".to_string()))]);
        assert!(
            !is_visible_sound_device(&no_status),
            "状态缺失按不可用处理，🔴 不当 OK 猜"
        );
    }

    #[test]
    fn card_rows_carry_only_the_two_specified_items() {
        let card = card_of(
            0,
            &device("Realtek(R) Audio", "Realtek Semiconductor Corp."),
        );
        assert_eq!(card.rows.len(), 2, "规格 §五 的声卡行集只有型号与制造商");
        assert_eq!(card.rows[0].key, "detail.audio.model");
        assert_eq!(card.rows[1].key, "detail.audio.manufacturer");
        assert_eq!(
            card.rows[1].value.as_deref(),
            Some("Realtek Semiconductor Corp.")
        );
    }
}
