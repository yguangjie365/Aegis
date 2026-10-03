//! 网卡可见性判定（R-02 / R-20 全工程唯一实现，2026-10-03）。
//!
//! 复用结论（DP-1）：②改造移植自 `Source/SystemToolkit/src/SystemToolkit.Core/Utilities/NetAdapterFilter.cs`
//! （GPL-3.0 同源，登记见 `DEPENDENCIES.md §三`）。黑名单条目**逐条照搬**并保留其实机实证出处——
//! 这些特征是实机踩出来的（2026-09-02 Killer WLAN + Hyper-V + 蓝牙机型；2026-09-10 火绒 NDIS 过滤驱动），
//! 🔴 删改任一条都会让对应垃圾条目重新混进界面，新增条目须带机型/日期注释。
//!
//! 为什么单列一模块（R-20）：概览的网卡枚举（F-02 网卡卡）与网络管理模块的适配器下拉
//! 必须给出一致的可见性结论；判定散落两处必然漂移。本模块只含**纯函数**，
//! 适配器枚举与 WMI 取数在 `services::overview`（片2）里做，判定一律回这里。
//!
//! 术语：`description`=设备描述（驱动/设备名），`name`=连接名（如 "WLAN" / "本地连接"）。
//! 母本实证：垃圾特征有的出现在描述里、有的出现在连接名里，故两者都要查。

/// 已知垃圾特征（全部按**字面子串**、大小写不敏感匹配；条目一律写成小写）。
///
/// 🔴 不要"整理"这个列表：分组注释即实证来源，删一条等于把该类接口放回界面。
/// 🔴 列表与母本**逐条一致**（35 条），未增未减：ISATAP / Teredo / 6to4 之类未列的隧道接口
/// 由 [`kind_from_type_name`] 折叠成 `Other`、经类型闸门拦下，无需凭猜测加条。
const JUNK_PATTERNS: &[&str] = &[
    // 虚拟化 / 隧道 / 回环
    "virtual",
    "vethernet",
    "tap-",
    "tunnel",
    "loopback",
    "vmware",
    "virtualbox",
    "hyper-v",
    "pseudo",
    "vpn",
    // 绑在物理卡上的过滤驱动子接口（与物理卡同类型、同速率、状态 Up，最容易被误当网卡）
    "qos packet scheduler",
    "packet scheduler",
    "wifi filter",
    "native wifi",
    "wfp",
    "windows filter",
    "npcap",
    "wintun",
    "lightweight filter",
    // 第三方安全软件注入的 NDIS 过滤驱动接口（母本 2026-09-10 实机：火绒「WLAN-Huorong NDIS Filter Driver-0000」；
    // 描述与真网卡不同，故"描述+MAC 去重"拦不住，必须名称层拦）
    "ndis filter",
    "filter driver",
    "ndis light weight",
    // 外设 / 调试 / 热点
    "bluetooth",
    "btlan",
    "wan miniport",
    "kernel debug",
    "内核调试器",
    "wi-fi direct",
    "hosted network",
    // 字面含星号：Windows 给 Wi-Fi Direct 虚拟适配器取名「本地连接* 1」，故这里匹配的是字面「本地连接*」
    "本地连接*",
    "microsoft wi-fi",
    "openvpn",
    "wireguard",
    "zerotier",
    "tailscale",
];

/// 接口物理类型（R-02 只承认这两种为"用户可见物理网卡"）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AdapterKind {
    Ethernet,
    Wireless80211,
    /// 回环、PPP、隧道、Pseudo 等（母本 `NetworkInterfaceType` 的其余取值折叠到此）
    Other,
}

/// 把系统返回的类型名折叠成三类（取值来自 WMI `MSFT_NicConfiguration.InstanceName` /
/// `Win32_NetworkAdapter.MediaType` 的文本形态；认不出的一律 `Other`，🔴 不得默认当物理卡）。
#[must_use]
pub fn kind_from_type_name(type_name: &str) -> AdapterKind {
    let lowered = type_name.to_ascii_lowercase();
    if lowered.contains("wireless") || lowered.contains("wlan") || lowered.contains("802.11") {
        AdapterKind::Wireless80211
    } else if lowered.contains("ethernet")
        || lowered.contains("ieee802.3")
        || lowered.contains("local area connection")
    {
        AdapterKind::Ethernet
    } else {
        AdapterKind::Other
    }
}

/// 名称黑名单命中即为垃圾条目（描述与连接名任一命中）。
#[must_use]
pub fn is_junk_adapter(description: &str, name: &str) -> bool {
    let description = description.to_ascii_lowercase();
    let name = name.to_ascii_lowercase();
    JUNK_PATTERNS.iter().any(|pattern| {
        description.contains(pattern) || (!name.is_empty() && name.contains(pattern))
    })
}

/// 连接名是否带 Windows 自动编号后缀（"WLAN 2" / "无线 3"）：末尾"空格 + 纯数字"。
///
/// 同名连接对象重复出现时由系统自动追加，是驱动残留实例最稳定的特征（母本实机：WLAN 2/4/5）。
#[must_use]
pub fn has_auto_number_suffix(name: &str) -> bool {
    match name.rsplit_once(' ') {
        Some((_prefix, suffix)) => {
            !suffix.is_empty() && suffix.bytes().all(|byte| byte.is_ascii_digit())
        }
        None => false,
    }
}

/// 是否 APIPA 自动私有地址（169.254/16）——母本实证：残留幽灵接口只持 APIPA 或根本无地址。
#[must_use]
pub fn is_apipa(ipv4: [u8; 4]) -> bool {
    ipv4[0] == 169 && ipv4[1] == 254
}

/// 断开接口的幽灵实例判定（R-02 的"已连接"之外的补集，母本 2026-09-07 实证）。
///
/// 判据顺序（与母本一致，改动须同步两侧）：
/// 1. MAC 字节数为 0 → 软件抽象层，必是幽灵；
/// 2. 已连接 → 必真实；
/// 3. 断开但速率已知（>0）→ 真实（实证：物理 LAN 断开仍报 1 Gbps）；
/// 4. 断开且持非 APIPA 的 IPv4 → 真实（配过静态/真实地址）；
/// 5. 断开 + 无速率 + 无真实 IPv4 + 连接名**无**自动编号后缀 → 🔴 按真实物理卡保留
///    （手动禁用的真卡正是这副模样，误杀代价远高于漏网幽灵——用户将再也无法在界面里启用它）；
/// 6. 其余（带自动编号后缀）→ 幽灵。
///
/// `speed_bits_per_second` 用 `i64`：系统对未协商接口报 `-1`（未知），🔴 不得当成 0 以外的语义。
#[must_use]
pub fn is_ghost_adapter(
    is_up: bool,
    speed_bits_per_second: i64,
    ipv4_addresses: &[[u8; 4]],
    connection_name: &str,
    mac_address_bytes: usize,
) -> bool {
    if mac_address_bytes == 0 {
        return true;
    }
    if is_up {
        return false;
    }
    if speed_bits_per_second > 0 {
        return false;
    }
    if ipv4_addresses.iter().any(|address| !is_apipa(*address)) {
        return false;
    }
    // 走到这里只剩"干净名 = 被禁用的真卡"与"带编号后缀 = 驱动残留实例"两条路
    has_auto_number_suffix(connection_name)
}

/// 概览展示口径（R-02）：只保留**已连接**的以太网 / Wi-Fi 物理网卡。
#[must_use]
pub fn is_user_facing_adapter(
    kind: AdapterKind,
    is_up: bool,
    receive_only: bool,
    description: &str,
    name: &str,
) -> bool {
    if !is_up || receive_only {
        return false;
    }
    if !matches!(kind, AdapterKind::Ethernet | AdapterKind::Wireless80211) {
        return false;
    }
    !is_junk_adapter(description, name)
}

/// 展示排序权重（R-02 末句："Wi-Fi 卡排在以太网卡之前"）。
#[must_use]
pub fn display_order(kind: AdapterKind) -> u8 {
    match kind {
        AdapterKind::Wireless80211 => 0,
        AdapterKind::Ethernet => 1,
        AdapterKind::Other => 2,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn filter_driver_children_of_the_real_card_are_junk() {
        // 母本实证：与物理卡同类型、同速率、状态 Up，最容易漏进界面
        assert!(is_junk_adapter(
            "Killer(r) Wi-Fi 6 AX1690i 160MHz Wireless Network Adapter - QoS Packet Scheduler#0",
            "WLAN"
        ));
        assert!(is_junk_adapter(
            "WFP Native MAC Layer LightWeight Filter#0",
            ""
        ));
        assert!(is_junk_adapter(
            "WLAN-Huorong NDIS Filter Driver-0000",
            "本地连接* 12"
        ));
        assert!(is_junk_adapter("", "vEthernet (WSL)"));
        assert!(is_junk_adapter(
            "Bluetooth Device (Personal Area Network)",
            "蓝牙网络连接"
        ));
    }

    #[test]
    fn real_cards_are_not_junk() {
        assert!(!is_junk_adapter(
            "Intel(r) Ethernet Connection I219-V",
            "以太网"
        ));
        assert!(!is_junk_adapter(
            "Realtek 8822CE Wireless LAN 802.11ac PCI-E NIC",
            "WLAN"
        ));
        assert!(!is_junk_adapter("Killer(r) Wi-Fi 6 AX1690i", "WLAN"));
    }

    #[test]
    fn empty_name_does_not_match_everything() {
        // 空串 `contains("")` 恒真：连接名为空时不得被当成命中
        assert!(!is_junk_adapter("Intel(r) Ethernet I219-V", ""));
        assert!(!is_junk_adapter("", ""));
    }

    #[test]
    fn auto_number_suffix_recognition() {
        assert!(has_auto_number_suffix("WLAN 2"));
        assert!(has_auto_number_suffix("无线 13"));
        assert!(!has_auto_number_suffix("WLAN"));
        assert!(!has_auto_number_suffix("WLAN 2x"));
        assert!(!has_auto_number_suffix("WLAN "));
        // 母本形态：名称本身带空格前缀 + 编号
        assert!(has_auto_number_suffix("本地连接* 3"));
    }

    #[test]
    fn kind_folding_keeps_unrecognized_types_as_other() {
        assert_eq!(
            kind_from_type_name("wireless80211"),
            AdapterKind::Wireless80211
        );
        assert_eq!(kind_from_type_name("WLAN"), AdapterKind::Wireless80211);
        assert_eq!(kind_from_type_name("Ethernet802.3"), AdapterKind::Ethernet);
        assert_eq!(kind_from_type_name("Tunnel"), AdapterKind::Other);
        assert_eq!(kind_from_type_name(""), AdapterKind::Other);
    }

    #[test]
    fn ghost_rules_follow_the_mother_implementation() {
        // 1) 无 MAC = 软件抽象层
        assert!(is_ghost_adapter(
            true,
            1_000_000_000,
            &[[192, 168, 1, 20]],
            "以太网",
            0
        ));
        // 2) 已连接必真实
        assert!(!is_ghost_adapter(true, -1, &[], "WLAN", 6));
        // 3) 断开但速率已知（物理 LAN 断开仍报 1Gbps）
        assert!(!is_ghost_adapter(false, 1_000_000_000, &[], "以太网", 6));
        // 4) 断开、无速率，但持真实 IPv4
        assert!(!is_ghost_adapter(
            false,
            -1,
            &[[192, 168, 1, 20]],
            "以太网",
            6
        ));
        // 5) 逃生舱：干净连接名 + 断开 + 无速率 + 只有 APIPA → 按禁用的真卡保留
        assert!(!is_ghost_adapter(false, -1, &[[169, 254, 1, 1]], "WLAN", 6));
        // 6) 带自动编号后缀的残留实例 → 幽灵
        assert!(is_ghost_adapter(
            false,
            -1,
            &[[169, 254, 1, 1]],
            "WLAN 4",
            6
        ));
        assert!(is_ghost_adapter(false, -1, &[], "WLAN 5", 6));
    }

    #[test]
    fn apipa_detection() {
        assert!(is_apipa([169, 254, 7, 7]));
        assert!(!is_apipa([169, 253, 7, 7]));
        assert!(!is_apipa([192, 168, 0, 1]));
    }

    #[test]
    fn user_facing_requires_up_physical_and_clean_name() {
        assert!(is_user_facing_adapter(
            AdapterKind::Wireless80211,
            true,
            false,
            "Realtek 8822CE Wireless LAN",
            "WLAN"
        ));
        assert!(
            !is_user_facing_adapter(
                AdapterKind::Ethernet,
                false,
                false,
                "Intel I219-V",
                "以太网"
            ),
            "未连接不可见（R-02）"
        );
        assert!(
            !is_user_facing_adapter(AdapterKind::Ethernet, true, true, "Intel I219-V", "以太网"),
            "只收接口不可见"
        );
        assert!(
            !is_user_facing_adapter(
                AdapterKind::Other,
                true,
                false,
                "Loopback Pseudo",
                "Loopback"
            ),
            "非物理类型不可见"
        );
        assert!(!is_user_facing_adapter(
            AdapterKind::Ethernet,
            true,
            false,
            "Hyper-V Ethernet Adapter",
            "vEthernet"
        ));
    }

    #[test]
    fn wireless_sorts_before_ethernet() {
        assert!(display_order(AdapterKind::Wireless80211) < display_order(AdapterKind::Ethernet));
        assert!(display_order(AdapterKind::Ethernet) < display_order(AdapterKind::Other));
    }
}
