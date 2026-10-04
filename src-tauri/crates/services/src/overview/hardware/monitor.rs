//! 显示器详情卡收集器（M-01 片2-c，2026-10-03）。
//!
//! **复用结论（DP-1）**：
//! - 分辨率/刷新率 ← `infra::display::active_display_modes()`（GDI `EnumDisplaySettingsW`）
//! - 型号/制造商 ← `infra::display::read_edid_from_registry()`（EDID blob 解码）
//!
//! 🔴 规格 §五「显示器（×n）」要求：型号、制造商、分辨率（仅第一台）。本实现按此产出单卡（index=0），
//! 多屏时只取首台活动桌面的主显示模式；EDID 可能缺型号/制造商，此时该行不出现（R-08 诚实降级）。

use crate::overview::dto::{DetailCardDto, DetailKind, KvRowDto};

use super::rows::{kv, kv_text};

/// 采集显示器详情卡（单卡，index=0）。
#[must_use]
pub fn card() -> DetailCardDto {
    let modes = infra::display::active_display_modes();
    let edids = infra::display::read_edid_from_registry();

    let mut rows: Vec<KvRowDto> = Vec::new();
    let mut reason_parts = Vec::new();

    // 分辨率行（来自 GDI，只取第一台活动桌面的当前模式）
    if let Ok(modes_list) = &modes {
        if let Some(first_mode) = modes_list.first() {
            let res_text = format!("{} × {}", first_mode.width, first_mode.height);
            rows.push(kv("detail.display.resolution", Some(res_text)));
            if first_mode.refresh_hz > 0 {
                rows.push(kv(
                    "detail.display.refreshRate",
                    Some(format!("{} Hz", first_mode.refresh_hz)),
                ));
            }
        } else {
            reason_parts.push("GDI 未枚举到活动桌面".to_string());
        }
    } else {
        reason_parts.push(format!("GDI 读取失败: {:?}", modes.as_ref().err()));
    }

    // EDID 信息（型号/制造商，只取第一个有效 blob）
    if let Ok(edid_list) = &edids {
        if let Some(first_edid) = edid_list.first() {
            if let Some(model) = &first_edid.model {
                rows.push(kv_text("detail.display.model", Some(model)));
            }
            if let Some(manufacturer) = &first_edid.manufacturer {
                rows.push(kv_text("detail.display.manufacturer", Some(manufacturer)));
            }
        } else {
            reason_parts.push("注册表未找到 EDID blob".to_string());
        }
    } else {
        reason_parts.push(format!("EDID 读取失败: {:?}", edids.as_ref().err()));
    }

    let reason = if reason_parts.is_empty() {
        None
    } else {
        Some(reason_parts.join("; "))
    };

    DetailCardDto {
        kind: DetailKind::Display,
        index: 0,
        rows,
        reason,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::overview::dto::DetailKind;

    #[test]
    fn card_always_returns_display_kind() {
        let card = card();
        assert_eq!(card.kind, DetailKind::Display);
        assert_eq!(card.index, 0);
    }

    #[test]
    fn resolution_row_present_when_gdi_succeeds() {
        let card = card();
        let has_resolution = card.rows.iter().any(|row| {
            row.key == "resolution" && row.value.as_deref().map_or(false, |v| !v.is_empty())
        });
        // GDI 成功时有分辨率行；失败时 reason 必有值
        if card.reason.is_none() {
            assert!(has_resolution, "无失败原因时必须产出分辨率行");
        }
    }

    #[test]
    fn model_and_manufacturer_only_when_edid_available() {
        let card = card();
        let has_model = card.rows.iter().any(|row| row.key == "model");
        let has_manufacturer = card.rows.iter().any(|row| row.key == "manufacturer");
        // EDID 缺失时这两行不应出现（R-08 诚实降级）
        if has_model || has_manufacturer {
            assert!(
                card.reason.is_none() || !card.reason.as_ref().unwrap().contains("EDID"),
                "EDID 失败时不得出现型号/制造商行"
            );
        }
    }

    #[test]
    fn reason_explains_failure_when_both_sources_fail() {
        // 在 CI / 远程会话中 GDI 与注册表都可能不可用，此时 reason 必须非空
        let card = card();
        if card.rows.is_empty() {
            assert!(
                card.reason.is_some(),
                "空卡必须带 reason 说明（面板不得静默消失）"
            );
        }
    }
}
