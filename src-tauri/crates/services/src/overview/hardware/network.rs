//! 网卡详情卡（F-02 · 规格 §五「网卡（×n）」行集：型号/连接名称/适配器类型/链路速度，片2-b）。
//!
//! **复用结论（DP-1）**：WQL 取自 `Source/NexBox/src-tauri/src/hardware.rs:1803`（②档改造移植）。
//! 🔴 **过滤判定不在本文件**：网卡可见性只认 [`crate::network::visibility`]（R-02/R-20，
//! 与网络管理模块共用同一实现），本域只做"把 WMI 行喂给判定函数"和取值渲染。
//!
//! 🔴 与母本的四处偏离：
//! 1. 母本自带 20 条网卡名关键词，本域**不移植**——垃圾特征一律走 `visibility` 的 35 条实证黑名单
//!    （两处各写一份必然漂移，这是 R-20 点名要防的事）；
//! 2. 母本的 PNP 前缀过滤链含 `usb\` / `hid\` / `swd\` 三条，本域**只保留 `swd\`**：
//!    `usb\` 会误杀真实 USB 有线网卡、`hid\` 与本域无关（用户裁定 2026-10-03，蓝牙 PAN 等
//!    软件接口已由黑名单条目拦下）；
//! 3. 🔴 **不采 `MACAddress`**：规格 §五 的网卡行集没有 MAC 这一项，`MACAddress`/`GUID`/
//!    `ServiceName`/`Index`/`MaxSpeed` 列一并砍掉（零遥测纪律下"采了不展示"是白增指纹面）；
//! 4. 母本无"已连接"闸门（只查 `NetEnabled`），本域按 R-02 走 `is_user_facing_adapter`，
//!    因此**已启用但未连接**的网卡不出现在详情区，链路速度也就不会拿 0 冒充。
//!
//! 🟡 **只读接口标志本批取不到**：`Win32_NetworkAdapter` 没有 `ReceiveOnly` 列（该属性属
//! `MSFT_NetAdapter` 面），`is_user_facing_adapter` 的 `receive_only` 形参只能传 `false`。
//! 语义影响：只读（抓包型）抽象接口若不在黑名单里会漏进来。待 M-02 落地时与网络模块一并收口。

use infra::wmi::Row;

use super::rows::{kv, link_speed_text};
use crate::network::visibility::{
    display_order, is_user_facing_adapter, kind_from_type_name, AdapterKind,
};
use crate::overview::dto::{DetailCardDto, DetailKind};

const WQL: &str = "SELECT Name, AdapterType, Speed, NetConnectionID, NetConnectionStatus, \
                   PhysicalAdapter, NetEnabled, PNPDeviceID FROM Win32_NetworkAdapter";

/// 网卡卡（每张已连接的用户可见物理网卡一张，R-01 全列；Wi-Fi 排在以太之前，R-02 末句）。
#[must_use]
pub fn cards() -> Vec<DetailCardDto> {
    let rows = match infra::wmi::query(WQL) {
        Ok(rows) => rows,
        Err(reason) => return vec![degraded(&reason)],
    };
    let mut visible: Vec<(&Row, AdapterKind)> = Vec::new();
    let mut seen_descriptions: Vec<String> = Vec::new();
    for row in &rows {
        let Some(kind) = visible_kind(row) else {
            continue;
        };
        let description = row.text("Name").unwrap_or_default();
        // R-02"同一物理卡的多接口按设备描述去重"：同描述只留第一条
        if seen_descriptions.iter().any(|seen| seen == &description) {
            continue;
        }
        seen_descriptions.push(description);
        visible.push((row, kind));
    }
    if visible.is_empty() {
        return vec![degraded("无已连接的用户可见物理网卡（R-02 口径）")];
    }
    // 先按类型权重（Wi-Fi 优先），同权重按型号名稳定排序
    visible.sort_by(|(left, left_kind), (right, right_kind)| {
        display_order(*left_kind)
            .cmp(&display_order(*right_kind))
            .then_with(|| left.text("Name").cmp(&right.text("Name")))
    });

    visible
        .iter()
        .enumerate()
        .map(|(index, (row, _))| card_of(index, row))
        .collect()
}

/// 判定入口：返回 `Some(kind)` 表示该卡应当展示（🔴 判定本体在 `visibility`，此处只喂参数）。
fn visible_kind(row: &Row) -> Option<AdapterKind> {
    let pnp = row.text("PNPDeviceID").unwrap_or_default();
    if pnp.to_lowercase().starts_with(r"swd\") {
        return None;
    }
    let physical = row.flag("PhysicalAdapter").unwrap_or(false);
    let enabled = row.flag("NetEnabled").unwrap_or(false);
    if !physical || !enabled {
        return None;
    }
    let description = row.text("Name").unwrap_or_default();
    let adapter_type = row.text("AdapterType").unwrap_or_default();
    let kind = kind_from_type_name(&adapter_type);
    // `NetConnectionStatus` 2 = 已连接（WMI 枚举值，母本与网络模块同用该语义）
    let is_up = row.u16("NetConnectionStatus") == Some(2);
    let connection_name = row.text("NetConnectionID").unwrap_or_default();
    let passes = is_user_facing_adapter(kind, is_up, false, &description, &connection_name);
    passes.then_some(kind)
}

fn card_of(index: usize, row: &Row) -> DetailCardDto {
    let adapter_type = row.text("AdapterType");
    DetailCardDto {
        kind: DetailKind::Network,
        index,
        reason: None,
        rows: vec![
            kv("detail.network.model", row.text("Name")),
            kv("detail.network.connectionName", row.text("NetConnectionID")),
            kv("detail.network.adapterType", adapter_type),
            kv(
                "detail.network.linkSpeed",
                link_speed_text(row.i64("Speed")),
            ),
        ],
    }
}

#[must_use]
fn degraded(reason: &str) -> DetailCardDto {
    DetailCardDto {
        kind: DetailKind::Network,
        index: 0,
        reason: Some(reason.to_string()),
        rows: Vec::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use infra::wmi::Value;

    fn real_nic(pairs: &[(&str, Value)]) -> Row {
        let mut base: Vec<(&str, Value)> = vec![
            (
                "Name",
                Value::Text("Intel(R) Ethernet Controller".to_string()),
            ),
            ("AdapterType", Value::Text("Ethernet 802.3".to_string())),
            ("NetConnectionID", Value::Text("以太网".to_string())),
            ("NetConnectionStatus", Value::Unsigned(2)),
            ("PhysicalAdapter", Value::Flag(true)),
            ("NetEnabled", Value::Flag(true)),
            (
                "PNPDeviceID",
                Value::Text("PCI\\VEN_8086&DEV_15FF".to_string()),
            ),
        ];
        base.extend_from_slice(pairs);
        Row::from_pairs(&base)
    }

    #[test]
    fn connected_physical_ethernet_passes_the_shared_gate() {
        assert_eq!(
            visible_kind(&real_nic(&[])),
            Some(AdapterKind::Ethernet),
            "判定必须走 visibility 单源，不能在概览域另写一份"
        );
    }

    #[test]
    fn disabled_or_disconnected_adapters_are_skipped() {
        let disabled = real_nic(&[("NetEnabled", Value::Flag(false))]);
        assert_eq!(visible_kind(&disabled), None);
        let disconnected = real_nic(&[("NetConnectionStatus", Value::Unsigned(7))]);
        assert_eq!(
            visible_kind(&disconnected),
            None,
            "R-02：只展示**已连接**的网卡（7 = 介质断开）"
        );
    }

    #[test]
    fn software_device_prefix_is_filtered() {
        let swd = real_nic(&[(
            "PNPDeviceID",
            Value::Text("SWD\\REMOTENP\\00000001".to_string()),
        )]);
        assert_eq!(visible_kind(&swd), None, "母本 `:1817` 的 `swd\\` 条目照搬");
        // 🔴 与母本的偏离 2：USB 网卡必须保留（母本会把 `usb\` 开头的真实网卡杀掉）
        let usb_nic = real_nic(&[(
            "Name",
            Value::Text("Realtek USB GbE Family Controller".to_string()),
        )]);
        assert_eq!(
            visible_kind(&usb_nic),
            Some(AdapterKind::Ethernet),
            "只保留 `swd\\`，不再按 `usb\\` 前缀误杀"
        );
    }

    #[test]
    fn blacklisted_tunnel_and_loopback_are_junk_not_visible() {
        let hyper_v = real_nic(&[(
            "Name",
            Value::Text("Hyper-V Virtual Ethernet Adapter".to_string()),
        )]);
        assert_eq!(visible_kind(&hyper_v), None, "35 条实证黑名单负责拦虚拟化");
        let loopback = real_nic(&[
            (
                "AdapterType",
                Value::Text("Generic Access Network Adapter".to_string()),
            ),
            ("Name", Value::Text("WAN Miniport (IP)".to_string())),
        ]);
        assert_eq!(
            visible_kind(&loopback),
            None,
            "折叠成 Other 即按类型闸门拦下"
        );
    }

    #[test]
    fn missing_speed_is_not_rendered_as_zero() {
        let card = card_of(0, &real_nic(&[]));
        let value_of = |key: &str| {
            card.rows
                .iter()
                .find(|item| item.key == key)
                .and_then(|item| item.value.clone())
        };
        assert_eq!(value_of("detail.network.linkSpeed"), None);
        let gigabit = card_of(0, &real_nic(&[("Speed", Value::Unsigned(1_000_000_000))]));
        assert_eq!(
            gigabit
                .rows
                .iter()
                .find(|item| item.key == "detail.network.linkSpeed")
                .and_then(|item| item.value.clone())
                .as_deref(),
            Some("1 Gbps")
        );
    }
}
