//! 设备被动发现（F-FT-07 / R-6）。
//!
//! ## R-6 把规则钉得很死，这里逐条对应
//!
//! | 规格原文 | 落点 |
//! |---|---|
//! | 固定节奏（默认 **3 秒**）宣告本机 | [`ANNOUNCE_INTERVAL`]（节奏由服务层用） |
//! | **10 秒**未再听到即视为离线 | [`OFFLINE_AFTER`] + [`PeerTable::sweep`] |
//! | 按**设备标识**去重（多网卡只一条） | 对端表以 `device_id` 为键；换地址只更新、不新增 |
//! | 过滤本机自己的宣告 | [`PeerEvent::SelfAnnouncement`]（不落表） |
//! | 设备名清理不可见字符 | [`clean_device_name`]（复用 `infra::safe::strip_invisible`） |
//! | 离线判定用**本机收到消息的时刻** | 只读 `last_seen_local_ms`；对端自报时刻仅作展示 |
//! | 设备标识**重启后稳定**（机器名 + 持久随机令牌） | [`SelfIdentity`] + 设置里的 `deviceToken` |
//! | 移除离线设备按**对象身份**而非地址 | 键是 `device_id`，`sweep` 返回被移除的对象本身 |
//!
//! ## 🔴 便利过滤 ≠ 安全边界（R-1）
//!
//! 任何在局域网里的人都能伪造一条宣告，所以本模块产出的一切（在线列表、地址、设备名）
//! **只能**用于"减少误选、减少噪音"。**任何跨机写入之前都必须存在有效配对凭据**——
//! 这条由配对层保证，本模块不提供也不声称提供任何安全性。
//! 故本模块**不做**任何"信任"判定，也没有"可信设备"这种字段。
//!
//! ## 为什么把对端表做成纯逻辑
//!
//! 发现最容易出的错全在**状态迁移**上（离线阈值、时钟偏差、换地址、同名不同机），
//! 而这些不需要真网络就能穷举。真 socket 只负责收发字节，另有环回用例兜住。
//!
//! 复用结论（DP-1）：③思路复用 ← 母本 `DeviceDiscoveryService.cs`（3 秒宣告 / 10 秒离线 /
//! 按 `DeviceId` 去重 / 过滤自身 / 设备名清洗）与 `DiscoveryProtocol` 的报文形状。
//! 🔴 **一处口径修正**：母本离线判定用对端自报的 `Timestamp`——那会受时钟偏差影响
//! （对端时钟快 1 小时就立刻被判离线）。规格 R-6 明写改用**本机接收时刻**，本模块按规格做。
//! 报文魔数/版本由本工程自定（两端都是 Aegis，不要求与旧版互通）。

use std::collections::HashMap;
use std::net::SocketAddr;
use std::time::Duration;

use serde::{Deserialize, Serialize};

/// 宣告报文的魔数。与 `protocol.rs` 的 `AGFT` 区分开，便于抓包时一眼分辨。
pub const DISCOVERY_MAGIC: [u8; 4] = *b"AGFD";

/// 宣告报文版本。🔴 不符即整包丢弃（不做降级协商，同 `protocol.rs` 口径）。
pub const DISCOVERY_VERSION: u8 = 1;

/// 定长前缀长度：`magic(4) + version(1)`。
pub const PREFIX_LEN: usize = 5;

/// 宣告报文上限。宣告只有几十字节，超过即判垃圾——**在解析之前**就丢，
/// 不让对端用一个巨大的数据报消耗我们的解析预算。
pub const MAX_DATAGRAM_BYTES: usize = 2048;

/// 宣告节奏（F-FT-07：默认 3 秒）。
pub const ANNOUNCE_INTERVAL: Duration = Duration::from_secs(3);

/// 离线阈值（F-FT-07：10 秒未再听到即离线）。
pub const OFFLINE_AFTER: Duration = Duration::from_secs(10);

/// 设备名缺失时的回退文案（F-FT-42 同口径："无名显示未知设备"）。
pub const UNKNOWN_DEVICE_NAME: &str = "未知设备";

/// 报文问题。
///
/// 🔴 全部**不携带任何对端可控内容**（与 `protocol::FrameError` 同纪律）：
/// 把攻击者的字节回显进日志或界面是自找麻烦。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DiscoveryError {
    /// 数据报超上限（未解析即拒）
    TooLarge(usize),
    /// 短于定长前缀
    TooShort(usize),
    /// 魔数不符
    BadMagic,
    /// 版本不符
    UnsupportedVersion(u8),
    /// JSON 解析失败
    Malformed,
    /// 字段语义不合法（空设备标识 / 端口越界）
    InvalidField(&'static str),
}

impl std::fmt::Display for DiscoveryError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::TooLarge(size) => write!(f, "宣告数据报超上限（{size} 字节）"),
            Self::TooShort(size) => write!(f, "宣告数据报过短（{size} 字节）"),
            Self::BadMagic => f.write_str("宣告魔数不符"),
            Self::UnsupportedVersion(version) => {
                write!(f, "宣告版本不支持（{version}）")
            }
            Self::Malformed => f.write_str("宣告内容无法解析"),
            Self::InvalidField(name) => write!(f, "宣告字段非法：{name}"),
        }
    }
}

impl std::error::Error for DiscoveryError {}

/// 一条设备宣告（F-FT-07 的报文载荷）。
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Announcement {
    /// 设备标识（机器名 + 持久随机令牌；**重启后稳定**，R-6）
    pub device_id: String,
    /// 设备显示名（已清理不可见字符）
    pub device_name: String,
    /// 对端传输服务端口（TCP）
    pub transfer_port: u16,
    /// 对端自报的发送时刻（Unix 毫秒）。
    /// 🔴 **仅用于展示/诊断**（例如提示"对端与本机时钟相差 N 分钟"）；
    /// **绝不**用于离线判定——那会变成"对端时钟快就立刻被判离线"（R-6 时钟自洽）。
    pub sent_at_ms: i64,
}

/// 清理设备显示名：去不可见字符 → 去首尾空白 → 空则回退 [`UNKNOWN_DEVICE_NAME`]。
///
/// 复用 `infra::safe::strip_invisible`（同一张不可见字符表，不在本模块重抄）。
#[must_use]
pub fn clean_device_name(raw: &str) -> String {
    let cleaned = infra::safe::strip_invisible(raw);
    let trimmed = cleaned.trim();
    if trimmed.is_empty() {
        UNKNOWN_DEVICE_NAME.to_owned()
    } else {
        trimmed.to_owned()
    }
}

/// 编码一条宣告（`magic|version|JSON`）。
///
/// # Errors
/// JSON 序列化失败时返回 [`DiscoveryError::Malformed`]（实践中不会发生，但不 unwrap）。
pub fn encode(announcement: &Announcement) -> Result<Vec<u8>, DiscoveryError> {
    let body = serde_json::to_vec(announcement).map_err(|_| DiscoveryError::Malformed)?;
    let mut out = Vec::with_capacity(PREFIX_LEN + body.len());
    out.extend_from_slice(&DISCOVERY_MAGIC);
    out.push(DISCOVERY_VERSION);
    out.extend_from_slice(&body);
    Ok(out)
}

/// 解析一条宣告。
///
/// 校验顺序是**由廉价到昂贵**：长度 → 魔数 → 版本 → JSON → 字段语义。
/// 故"误连到本端口的路人"付出的解析代价只有几字节比对。
///
/// 解析成功时**已清理设备名**（[`clean_device_name`]），调用方拿到的一定是可显示的字符串。
///
/// # Errors
/// 见 [`DiscoveryError`] 各变体。
pub fn decode(bytes: &[u8]) -> Result<Announcement, DiscoveryError> {
    if bytes.len() > MAX_DATAGRAM_BYTES {
        return Err(DiscoveryError::TooLarge(bytes.len()));
    }
    if bytes.len() < PREFIX_LEN {
        return Err(DiscoveryError::TooShort(bytes.len()));
    }
    if bytes[..4] != DISCOVERY_MAGIC {
        return Err(DiscoveryError::BadMagic);
    }
    if bytes[4] != DISCOVERY_VERSION {
        return Err(DiscoveryError::UnsupportedVersion(bytes[4]));
    }
    let raw: Announcement =
        serde_json::from_slice(&bytes[PREFIX_LEN..]).map_err(|_| DiscoveryError::Malformed)?;

    if raw.device_id.trim().is_empty() {
        return Err(DiscoveryError::InvalidField("deviceId"));
    }
    if raw.transfer_port < super::settings::MIN_PORT {
        return Err(DiscoveryError::InvalidField("transferPort"));
    }

    Ok(Announcement {
        device_id: raw.device_id.trim().to_owned(),
        device_name: clean_device_name(&raw.device_name),
        transfer_port: raw.transfer_port,
        sent_at_ms: raw.sent_at_ms,
    })
}

/// 本机设备标识（R-6：机器名 + **持久**随机令牌，重启后稳定）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SelfIdentity {
    machine_name: String,
    token: String,
}

impl SelfIdentity {
    /// 用已持久化的令牌构造；令牌为空视为"尚无"，由 [`Self::load_or_create`] 生成。
    #[must_use]
    pub fn new(machine_name: impl Into<String>, token: impl Into<String>) -> Self {
        Self {
            machine_name: machine_name.into(),
            token: token.into(),
        }
    }

    /// 从设置里读令牌；缺失则**生成一个新的**并回传待持久化。
    ///
    /// 返回 `(identity, Some(新令牌))` 时调用方**必须**把新令牌写回设置——
    /// 否则每次重启都会换一个标识，对端会看到"同一台机器反复出现又消失"。
    #[must_use]
    pub fn load_or_create(
        machine_name: &str,
        persisted_token: Option<&str>,
    ) -> (Self, Option<String>) {
        if let Some(token) = persisted_token.map(str::trim).filter(|t| !t.is_empty()) {
            (Self::new(machine_name, token), None)
        } else {
            let token = uuid::Uuid::new_v4().simple().to_string();
            (Self::new(machine_name, &token), Some(token))
        }
    }

    #[must_use]
    pub fn machine_name(&self) -> &str {
        &self.machine_name
    }

    #[must_use]
    pub fn token(&self) -> &str {
        &self.token
    }

    /// 稳定的设备标识：`机器名#令牌`。
    #[must_use]
    pub fn device_id(&self) -> String {
        format!("{}#{}", self.machine_name, self.token)
    }

    /// 构造本机宣告。
    #[must_use]
    pub fn announcement(&self, transfer_port: u16, now_ms: i64) -> Announcement {
        Announcement {
            device_id: self.device_id(),
            device_name: clean_device_name(&self.machine_name),
            transfer_port,
            sent_at_ms: now_ms,
        }
    }
}

/// 对端表里的一台设备。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Peer {
    pub device_id: String,
    pub device_name: String,
    /// 最后一次听到它的来源地址（发现通道的地址，不是传输端口）
    pub address: SocketAddr,
    pub transfer_port: u16,
    /// 🔴 本机收到最后一次宣告的时刻（离线判定**只**看它）
    pub last_seen_local_ms: i64,
    /// 对端自报时刻（仅展示）
    pub announced_at_ms: i64,
}

/// 一次 `observe` 的结论（供上层决定要不要发事件、要不要刷界面）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PeerEvent {
    /// 新设备出现
    Appeared { device_id: String },
    /// 已知设备又听到一次（地址未变）
    Refreshed { device_id: String },
    /// 同一设备换了地址（多网卡 / 换网段）；**不新增条目**
    AddressChanged {
        device_id: String,
        from: SocketAddr,
        to: SocketAddr,
    },
    /// 本机自己的宣告——按 R-6 必须过滤
    SelfAnnouncement,
}

/// 被动监听得到的设备快照。
#[derive(Debug, Clone)]
pub struct PeerTable {
    self_id: String,
    offline_after_ms: i64,
    peers: HashMap<String, Peer>,
}

impl PeerTable {
    /// `self_id` 取 [`SelfIdentity::device_id`]。
    #[must_use]
    pub fn new(self_id: impl Into<String>, offline_after: Duration) -> Self {
        Self {
            self_id: self_id.into(),
            offline_after_ms: i64::try_from(offline_after.as_millis()).unwrap_or(i64::MAX),
            peers: HashMap::new(),
        }
    }

    /// 收到一条宣告。
    ///
    /// `now_ms` 是**本机**当前时刻——离线判定与"最后可见"都以它为准。
    pub fn observe(
        &mut self,
        announcement: &Announcement,
        from: SocketAddr,
        now_ms: i64,
    ) -> PeerEvent {
        if announcement.device_id == self.self_id {
            return PeerEvent::SelfAnnouncement;
        }
        if let Some(peer) = self.peers.get_mut(&announcement.device_id) {
            let previous = peer.address;
            // `clone_from` 复用已有分配，避免每 3 秒一次的宣告都新分配一次设备名
            peer.device_name.clone_from(&announcement.device_name);
            peer.transfer_port = announcement.transfer_port;
            peer.last_seen_local_ms = now_ms;
            peer.announced_at_ms = announcement.sent_at_ms;
            if previous == from {
                PeerEvent::Refreshed {
                    device_id: announcement.device_id.clone(),
                }
            } else {
                peer.address = from;
                PeerEvent::AddressChanged {
                    device_id: announcement.device_id.clone(),
                    from: previous,
                    to: from,
                }
            }
        } else {
            self.peers.insert(
                announcement.device_id.clone(),
                Peer {
                    device_id: announcement.device_id.clone(),
                    device_name: announcement.device_name.clone(),
                    address: from,
                    transfer_port: announcement.transfer_port,
                    last_seen_local_ms: now_ms,
                    announced_at_ms: announcement.sent_at_ms,
                },
            );
            PeerEvent::Appeared {
                device_id: announcement.device_id.clone(),
            }
        }
    }

    /// 摘掉已离线者并**按对象身份**返回它们（供上层发"设备离线"事件）。
    ///
    /// 返回顺序按 `device_id` 排序 —— 遍历 `HashMap` 的顺序不稳定，测试与事件流都需要确定序。
    pub fn sweep(&mut self, now_ms: i64) -> Vec<Peer> {
        let threshold = self.offline_after_ms;
        let expired: Vec<String> = self
            .peers
            .iter()
            .filter(|(_, peer)| now_ms.saturating_sub(peer.last_seen_local_ms) >= threshold)
            .map(|(id, _)| id.clone())
            .collect();
        let mut removed: Vec<Peer> = expired
            .into_iter()
            .filter_map(|id| self.peers.remove(&id))
            .collect();
        removed.sort_by(|a, b| a.device_id.cmp(&b.device_id));
        removed
    }

    /// 当前在线快照（按 `device_id` 排序，便于界面与测试得到稳定顺序）。
    #[must_use]
    pub fn online(&self) -> Vec<Peer> {
        let mut peers: Vec<Peer> = self.peers.values().cloned().collect();
        peers.sort_by(|a, b| a.device_id.cmp(&b.device_id));
        peers
    }

    #[must_use]
    pub fn get(&self, device_id: &str) -> Option<&Peer> {
        self.peers.get(device_id)
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.peers.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.peers.is_empty()
    }
}

#[cfg(test)]
mod tests;
