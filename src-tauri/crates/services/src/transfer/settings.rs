//! 电脑通道的设置与持久化（F-FT-59；05 册 R-27 的设置纪律）。
//!
//! ## 规格要求（F-FT-59 原文要点）
//!
//! * 持久化项：接收目录、仅接受已发现设备、接收前询问、冲突策略、暂停自动取消上限、
//!   传输端口、发现端口、已知设备清单
//! * **原子写入**（不留半套配置）
//! * 设置变更后合法即**自动保存、无需"保存"按钮**
//! * **加载期间的变更不回写**（避免刚读就被覆盖）
//! * 配置文件里**非法端口按默认值回落**
//!
//! ## 三处设计取舍
//!
//! 1. **"加载期间的变更不回写"由结构满足，不需要标志位**：本模块的 `load` 是**同步**的，
//!    返回一个值之后才可能有人改它——不存在"读到一半被改"的窗口。
//!    母本需要标志位是因为 WPF 的可观察属性绑定在赋值时就触发保存；
//!    Rust 侧那个隐患在类型层面就不成立。**如实登记，不假装实现了一个标志位。**
//! 2. **非法端口逐个回落，而不是整份配置作废**：`99999` 这类超 `u16` 的值若走 serde 反序列化
//!    会让**整份文件**失败、连带丢掉用户其它设置。故先解析成 `Value`，端口**逐字段**取值并校验，
//!    其余字段再走 serde。这样"一个端口写错"只损失那个端口。
//! 3. **不持 Tauri 类型、不自己决定路径**：`load`/`save` 收 `&Path`，落点由命令层决定
//!    （`default_settings_path()` 只是给命令层一个符合本仓约定的默认值）。
//!    ⇒ 本模块可以脱离 Tauri 测试。
//!
//! 复用结论（DP-1）：③思路复用 ← 母本 `TransferSettings.cs` 的字段集与默认值、
//! 以及 `FileTransferDesktopViewModel` 的 `ModuleConfig` JSON 落盘方式。
//! 🔴 **一处口径修正**：母本 `RequireReceiveConfirmation` 无初始化器（C# `bool` = `false`），
//! 而界面层写 `true`——本工程取**契约层常量**（[`contracts::TRANSFER_REQUIRE_RECEIVE_CONFIRMATION_DEFAULT`]），
//! 即 Q-15 用户裁决的"默认开启"。

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use super::conflict::ConflictPolicy;

/// 传输服务默认端口（TCP）。取值来自母本实测（`TransferSettings.TransferPort = 18889`）。
pub const DEFAULT_TRANSFER_PORT: u16 = 18889;

/// 设备发现默认端口（UDP）。母本实测 `DiscoveryPort = 18888`。
pub const DEFAULT_DISCOVERY_PORT: u16 = 18888;

/// 端口合法下界（母本 `PortValidator.MinPort` 同值）。1024 以下属特权端口。
pub const MIN_PORT: u16 = 1024;

/// 暂停自动取消上限的默认值（分钟；**0 表示不限**，F-FT-27）。
pub const DEFAULT_PAUSE_TIMEOUT_MINUTES: u32 = 30;

/// 设置文件名（放在调用方给定的目录下）。
///
/// 🟠 `default_settings_path()` 里**就地写了字面量 `"settings.json"`** 而不是引用本常量——
/// 因为 `transfer::guard` 要求路径拼接的参数是**字面量**（常量参数会被判违规，见该守卫的判据说明）。
/// 两者的同值关系由 `default_paths_follow_the_house_convention` 用例钉住，不会漂移。
pub const SETTINGS_FILE_NAME: &str = "settings.json";

/// 一个已知设备（F-FT-10：按 **IP + 端口** 去重）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct KnownPeer {
    pub name: String,
    pub ip: String,
    pub port: u16,
}

/// 电脑通道设置（F-FT-59 的持久化项全集）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct TransferSettings {
    /// 接收目录；`None` = 未设置（启动服务时按 [`default_receive_directory`] 创建，F-FT-06）
    pub receive_directory: Option<String>,
    /// 仅接受已发现设备（F-FT-09，**默认开**；但按 R-1 它只是便利过滤、不是信任边界）
    pub require_known_peer: bool,
    /// 接收前询问（F-FT-18；🔴 默认取契约层常量 = **开**，见模块头）
    pub require_receive_confirmation: bool,
    /// 电脑通道"需要配对码"开关（F-FT-13；**默认开**，R-1 的信任根）。
    ///
    /// 🔴 **不落盘**（`serde(skip)`）：F-FT-59 的持久化清单**没有列它**，
    /// 而服务重启后回到安全默认（要求配对）比"记住用户上次关掉了它"更稳妥。
    /// 用户在一个运行期内关掉配对是 R-1 允许的（"需用户显式改动并承担后果"），
    /// 但那不该变成**持久**的静默降级。若将来确需持久化默认关闭，须先立一条决策。
    #[serde(skip)]
    pub require_pairing: bool,
    /// 同名冲突策略（F-FT-23，**默认自动改名**）
    pub conflict_policy: ConflictPolicy,
    /// 暂停自动取消上限（分钟，0 = 不限；F-FT-27）
    pub pause_timeout_minutes: u32,
    pub transfer_port: u16,
    pub discovery_port: u16,
    /// 已知设备清单（F-FT-10）
    pub known_peers: Vec<KnownPeer>,
    /// 本机设备名；`None` = 用机器名
    pub device_name: Option<String>,
    /// 本机设备标识的**持久随机令牌**（R-6：标识 = 机器名 + 令牌，**重启后必须稳定**）。
    ///
    /// 🔴 `None` 表示尚未生成 —— 由 `discovery::SelfIdentity::load_or_create` 生成后
    /// **必须写回本字段**；不写回则每次重启都换一个标识，
    /// 对端会看到"同一台机器反复出现又消失"。
    pub device_token: Option<String>,
}

impl Default for TransferSettings {
    fn default() -> Self {
        Self {
            receive_directory: None,
            require_known_peer: true,
            // 🔴 单一来源：契约层常量（Q-15 用户裁决 2026-10-04 = 默认开启）
            require_receive_confirmation: contracts::TRANSFER_REQUIRE_RECEIVE_CONFIRMATION_DEFAULT,
            // R-1：配对默认要求（信任只来自配对码）
            require_pairing: true,
            conflict_policy: ConflictPolicy::default(),
            pause_timeout_minutes: DEFAULT_PAUSE_TIMEOUT_MINUTES,
            transfer_port: DEFAULT_TRANSFER_PORT,
            discovery_port: DEFAULT_DISCOVERY_PORT,
            known_peers: Vec::new(),
            device_name: None,
            device_token: None,
        }
    }
}

/// 一次加载的**如实结论**（禁止静默失败：调用方据此落日志，见 02 册）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LoadOutcome {
    pub settings: TransferSettings,
    /// 文件不存在（首次运行，属正常；**不算**损坏）
    pub file_missing: bool,
    /// 文件存在但无法解析 → 已整体降级为默认值
    pub corrupt: bool,
    /// 逐项修复记录（如"传输端口越界，已回落 18889"），供日志如实呈现
    pub repairs: Vec<String>,
}

/// 端口问题（F-FT-06 的"端口设置非法"来源）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PortProblem {
    /// 低于特权端口下界
    TooLow { port: u16 },
    /// 与同组另一个端口重复
    Duplicated { port: u16 },
}

impl std::fmt::Display for PortProblem {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::TooLow { port } => {
                write!(f, "端口 {port} 低于 {MIN_PORT}（特权端口段）")
            }
            Self::Duplicated { port } => write!(f, "端口 {port} 在同组内重复"),
        }
    }
}

/// 校验端口（F-FT-06：端口必须合法，否则**不启动任何监听**）。
///
/// 返回空即合法。组内互斥关系：传输与发现端口不得相同。
#[must_use]
pub fn validate_ports(settings: &TransferSettings) -> Vec<PortProblem> {
    let mut problems = Vec::new();
    for port in [settings.transfer_port, settings.discovery_port] {
        if port < MIN_PORT {
            problems.push(PortProblem::TooLow { port });
        }
    }
    if settings.transfer_port == settings.discovery_port {
        problems.push(PortProblem::Duplicated {
            port: settings.transfer_port,
        });
    }
    problems
}

/// 接收目录的默认值：`<下载>\Received`。
///
/// 🟠 **规格未给该默认值**（C-14 缺项），取母本 `StartAsync` 的实测口径
/// （`下载\Received`；母本注释记载 2026-09-02 由"桌面\Received"改来）。
#[must_use]
pub fn default_receive_directory() -> PathBuf {
    let base = dirs::download_dir().unwrap_or_else(|| {
        std::env::var("USERPROFILE").map_or_else(|_| PathBuf::from("."), PathBuf::from)
    });
    base.join("Received")
}

/// 默认设置文件路径：`%LOCALAPPDATA%\aegis\transfer\settings.json`。
///
/// 与本仓既有约定一致（`overview/snapshot.rs` 用同一根）。
#[must_use]
pub fn default_settings_path() -> PathBuf {
    let root = std::env::var("LOCALAPPDATA").map_or_else(|_| PathBuf::from("."), PathBuf::from);
    root.join("aegis").join("transfer").join("settings.json")
}

/// 加载设置。
///
/// * 文件不存在 → 默认值（`file_missing = true`，**不算损坏**）
/// * 文件损坏/无法解析 → 默认值（`corrupt = true`）
/// * 端口越界 → **逐字段**回落默认并记入 `repairs`（不影响其它字段）
///
/// 🔴 本函数**同步**返回：不存在"加载期间的变更"（见模块头取舍 1）。
#[must_use]
pub fn load(path: &Path) -> LoadOutcome {
    let mut repairs = Vec::new();
    let text = match std::fs::read_to_string(path) {
        Ok(text) => text,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return LoadOutcome {
                settings: TransferSettings::default(),
                file_missing: true,
                corrupt: false,
                repairs,
            }
        }
        Err(error) => {
            return LoadOutcome {
                settings: TransferSettings::default(),
                file_missing: false,
                corrupt: true,
                repairs: vec![format!("读取设置文件失败：{error}")],
            }
        }
    };

    let value: serde_json::Value = match serde_json::from_str(&text) {
        Ok(value) => value,
        Err(error) => {
            return LoadOutcome {
                settings: TransferSettings::default(),
                file_missing: false,
                corrupt: true,
                repairs: vec![format!("设置文件不是合法 JSON：{error}")],
            }
        }
    };

    // 🔴 **先把端口字段就地修好，再交给 serde**：越界的端口（如 99999）会让 `u16` 字段的
    // **整份**反序列化失败，那样就永远走不到"逐字段回落"、用户的其它设置会被连带丢光。
    // 这一步是本函数存在的理由（首轮实测：先反序列化再修，越界端口直接判整份 corrupt）。
    let mut value = value;
    if let Some(object) = value.as_object_mut() {
        for (key, label, fallback) in [
            ("transferPort", "传输端口", DEFAULT_TRANSFER_PORT),
            ("discoveryPort", "发现端口", DEFAULT_DISCOVERY_PORT),
        ] {
            let Some(raw) = object.get(key) else { continue };
            let usable = raw
                .as_i64()
                .and_then(|v| u16::try_from(v).ok())
                .is_some_and(|port| port >= MIN_PORT);
            if !usable {
                object.insert(key.to_owned(), serde_json::Value::from(fallback));
                // 中文名给人看、JSON 键名给排查用（日志里两者都要有）
                repairs.push(format!("{label}（{key}）非法，已回落默认 {fallback}"));
            }
        }
    }

    let mut settings: TransferSettings = match serde_json::from_value(value) {
        Ok(settings) => settings,
        Err(error) => {
            return LoadOutcome {
                settings: TransferSettings::default(),
                file_missing: false,
                corrupt: true,
                repairs: vec![format!("设置文件字段不合法：{error}")],
            }
        }
    };

    repairs.extend(normalize(&mut settings));

    LoadOutcome {
        settings,
        file_missing: false,
        corrupt: false,
        repairs,
    }
}

/// 就地归一化：去重已知设备、丢弃不可用条目；返回修复记录。
///
/// 已知设备按 **IP + 端口** 去重（F-FT-10 原文："按 IP + 端口去重（重复加入不产生第二条）"）。
pub fn normalize(settings: &mut TransferSettings) -> Vec<String> {
    let mut repairs = Vec::new();

    let before = settings.known_peers.len();
    let mut seen = std::collections::HashSet::new();
    settings.known_peers.retain(|peer| {
        let usable = !peer.ip.trim().is_empty() && peer.port >= MIN_PORT;
        usable && seen.insert((peer.ip.trim().to_owned(), peer.port))
    });
    let removed = before - settings.known_peers.len();
    if removed > 0 {
        repairs.push(format!("已知设备清单清理了 {removed} 条重复或不可用条目"));
    }

    if settings.pause_timeout_minutes > 0 && settings.pause_timeout_minutes < 1 {
        // 目前取值范围是 0 或 >=1，这条为将来的上下界预留；当前恒不触发
        settings.pause_timeout_minutes = DEFAULT_PAUSE_TIMEOUT_MINUTES;
        repairs.push("暂停上限非法，已回落默认".to_owned());
    }

    repairs
}

/// 原子保存（F-FT-59："原子写入，不留半套配置"）。
///
/// 🔴 走 [`infra::fs::atomic_write`]（同目录 `.staging-` + fsync + rename）——
/// 规格 02 册 §5.2 把"settings 与全部配置文件"明列进必须原子写的清单。
///
/// # Errors
/// 序列化失败或落盘失败时返回错误描述（调用方须落日志，不得静默）。
pub fn save(path: &Path, settings: &TransferSettings) -> Result<(), String> {
    let text = serde_json::to_string_pretty(settings).map_err(|error| error.to_string())?;
    infra::fs::atomic_write(path, text.as_bytes()).map_err(|error| error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_dir(tag: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("aegis-settings-{}-{}", std::process::id(), tag));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("建临时目录");
        dir
    }

    /// 🔴 出厂默认必须逐条对上规格（这几条任何一条改了都是口径变更）。
    #[test]
    fn factory_defaults_match_the_spec() {
        let s = TransferSettings::default();
        assert!(s.require_known_peer, "F-FT-09：仅接受已发现设备默认开");
        assert!(
            s.require_receive_confirmation,
            "Q-15 用户裁决（2026-10-04）：接收前询问默认**开**"
        );
        assert_eq!(
            s.conflict_policy,
            ConflictPolicy::Rename,
            "F-FT-23：默认自动改名（house policy 绝不覆盖）"
        );
        assert_eq!(s.pause_timeout_minutes, 30, "F-FT-27：默认 30 分钟");
        assert_eq!(s.transfer_port, 18889);
        assert_eq!(s.discovery_port, 18888);
        assert_eq!(s.known_peers, [], "默认无已知设备");
        assert_eq!(s.receive_directory, None);
    }

    /// 🔴 默认值与契约层常量必须同源（Q-15 的"全链路统一"靠这一条守住）。
    #[test]
    fn confirmation_default_comes_from_the_contract_layer() {
        assert_eq!(
            TransferSettings::default().require_receive_confirmation,
            contracts::TRANSFER_REQUIRE_RECEIVE_CONFIRMATION_DEFAULT
        );
    }

    #[test]
    fn save_then_load_round_trips_every_field() {
        let dir = temp_dir("roundtrip");
        let path = dir.join(SETTINGS_FILE_NAME);
        let mut original = TransferSettings {
            receive_directory: Some(r"D:\收件".to_owned()),
            require_known_peer: false,
            require_receive_confirmation: false,
            // 🔴 刻意取**默认值**：本字段 `serde(skip)` 不落盘，若这里写 false
            //    读回来会是默认的 true，往返断言必然失败——那正是"不持久化"的表现，
            //    见下面单独的一条用例。
            require_pairing: true,
            conflict_policy: ConflictPolicy::Overwrite,
            pause_timeout_minutes: 0,
            transfer_port: 19000,
            discovery_port: 19001,
            known_peers: vec![KnownPeer {
                name: "书房台式机".to_owned(),
                ip: "192.168.1.24".to_owned(),
                port: 18889,
            }],
            device_name: Some("我的机器".to_owned()),
            // R-6：设备标识的持久令牌也要能往返（丢了它 → 重启后标识会变）
            device_token: Some("0123456789abcdef0123456789abcdef".to_owned()),
        };
        save(&path, &original).unwrap();

        let outcome = load(&path);
        assert!(!outcome.corrupt && !outcome.file_missing);
        assert_eq!(outcome.settings, original, "逐字段往返必须一致");

        // 🔴 配对开关**不参与往返**：它 `serde(skip)`，读回来一律是默认（要求配对）。
        //    这条断言把"不持久化"这件事变成可执行的事实，而不是文档里的一句话。
        let mut weakened = original.clone();
        weakened.require_pairing = false;
        save(&path, &weakened).unwrap();
        assert!(
            load(&path).settings.require_pairing,
            "关掉配对不该被持久化（重启应回到安全默认）"
        );
        assert!(outcome.repairs.is_empty(), "合法配置不该产生修复记录");
        let _ = std::fs::remove_dir_all(&dir);
        let _ = &mut original;
    }

    /// 🔴 非法端口**逐字段**回落，不得因一个端口写错就丢掉其余设置。
    #[test]
    fn out_of_range_port_falls_back_without_losing_other_settings() {
        let dir = temp_dir("badport");
        let path = dir.join(SETTINGS_FILE_NAME);
        // 手写一份：传输端口 99999（超 u16），其余字段合法且非默认
        std::fs::write(
            &path,
            r#"{"transferPort":99999,"discoveryPort":19001,"requireKnownPeer":false,
                "conflictPolicy":"skip","pauseTimeoutMinutes":5,"deviceName":"保留我"}"#,
        )
        .unwrap();

        let outcome = load(&path);
        assert!(!outcome.corrupt, "一个端口越界不得判整份损坏");
        assert_eq!(
            outcome.settings.transfer_port, DEFAULT_TRANSFER_PORT,
            "越界端口回落默认"
        );
        assert_eq!(outcome.settings.discovery_port, 19001, "其它端口不受影响");
        assert!(!outcome.settings.require_known_peer, "🔴 其余设置必须保留");
        assert_eq!(outcome.settings.conflict_policy, ConflictPolicy::Skip);
        assert_eq!(outcome.settings.pause_timeout_minutes, 5);
        assert_eq!(outcome.settings.device_name.as_deref(), Some("保留我"));
        assert_eq!(outcome.repairs.len(), 1, "应如实记一条修复");
        assert!(outcome.repairs[0].contains("传输端口"));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn privileged_port_below_1024_falls_back_too() {
        let dir = temp_dir("lowport");
        let path = dir.join(SETTINGS_FILE_NAME);
        std::fs::write(&path, r#"{"transferPort":80}"#).unwrap();
        let outcome = load(&path);
        assert_eq!(outcome.settings.transfer_port, DEFAULT_TRANSFER_PORT);
        assert_eq!(outcome.repairs.len(), 1);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn missing_file_yields_defaults_and_is_not_called_corrupt() {
        let dir = temp_dir("missing");
        let outcome = load(&dir.join("nope.json"));
        assert!(outcome.file_missing);
        assert!(!outcome.corrupt, "首次运行没有文件是正常的，不是损坏");
        assert_eq!(outcome.settings, TransferSettings::default());
        assert_eq!(
            outcome.repairs,
            Vec::<String>::new(),
            "合法配置不该产生修复记录"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// F-FT-58 的同族纪律（设置侧）：损坏 → 降级为默认，**不让功能不可用**，且如实标记。
    #[test]
    fn corrupt_file_degrades_to_defaults_and_says_so() {
        let dir = temp_dir("corrupt");
        let path = dir.join(SETTINGS_FILE_NAME);
        std::fs::write(&path, b"{ this is not json").unwrap();
        let outcome = load(&path);
        assert!(outcome.corrupt, "必须如实标记损坏");
        assert!(!outcome.file_missing);
        assert_eq!(outcome.settings, TransferSettings::default());
        assert!(
            !outcome.repairs.is_empty(),
            "修复记录不得为空（禁止静默失败）"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn known_peers_are_deduped_by_ip_and_port() {
        let dir = temp_dir("dedupe");
        let path = dir.join(SETTINGS_FILE_NAME);
        let peer = |name: &str| KnownPeer {
            name: name.to_owned(),
            ip: "192.168.1.24".to_owned(),
            port: 18889,
        };
        let settings = TransferSettings {
            known_peers: vec![
                peer("第一台"),
                peer("重复的同一台"),
                KnownPeer {
                    name: "另一台".to_owned(),
                    ip: "192.168.1.24".to_owned(),
                    port: 18890, // 同 IP 不同端口 → 保留
                },
                KnownPeer {
                    name: "不可用".to_owned(),
                    ip: "   ".to_owned(), // 空 IP → 丢弃
                    port: 18889,
                },
            ],
            ..TransferSettings::default()
        };
        save(&path, &settings).unwrap();

        let outcome = load(&path);
        assert_eq!(
            outcome.settings.known_peers.len(),
            2,
            "同 IP+端口去重、空 IP 丢弃"
        );
        assert_eq!(
            outcome.settings.known_peers[0].name, "第一台",
            "保留先出现的"
        );
        assert_ne!(
            outcome.repairs,
            Vec::<String>::new(),
            "修复记录不得为空（禁止静默失败）"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn port_validation_catches_low_and_duplicate() {
        let mut s = TransferSettings::default();
        assert!(validate_ports(&s).is_empty(), "默认端口应合法");

        s.transfer_port = 80;
        assert_eq!(validate_ports(&s), vec![PortProblem::TooLow { port: 80 }]);

        s.transfer_port = 18888;
        s.discovery_port = 18888;
        assert_eq!(
            validate_ports(&s),
            vec![PortProblem::Duplicated { port: 18888 }]
        );
        // 两种问题可以并存：两个端口都低 + 且互相重复 → TooLow×2 + Duplicated×1
        s.transfer_port = 80;
        s.discovery_port = 80;
        let both = validate_ports(&s);
        assert_eq!(both.len(), 3, "两个端口都低且重复，应报三项：{both:?}");
        assert!(both.contains(&PortProblem::TooLow { port: 80 }));
        assert!(both.contains(&PortProblem::Duplicated { port: 80 }));
    }

    /// 🔴 序列化形状是**磁盘契约**：字段名按 camelCase、策略按小写名。
    /// 一旦漂移，用户已有配置文件会静默失效。
    #[test]
    fn json_shape_is_a_frozen_disk_contract() {
        let value = serde_json::to_value(TransferSettings::default()).unwrap();
        let obj = value.as_object().unwrap();
        for key in [
            "receiveDirectory",
            "requireKnownPeer",
            "requireReceiveConfirmation",
            "conflictPolicy",
            "pauseTimeoutMinutes",
            "transferPort",
            "discoveryPort",
            "knownPeers",
            "deviceName",
            "deviceToken",
        ] {
            assert!(obj.contains_key(key), "缺字段 {key}（磁盘契约）");
        }
        assert_eq!(
            obj.get("conflictPolicy").and_then(|v| v.as_str()),
            Some("rename"),
            "策略必须序列化为小写名，不得是裸数字（重排枚举会静默改变已有配置含义）"
        );
        for policy in [
            ConflictPolicy::Rename,
            ConflictPolicy::Ask,
            ConflictPolicy::Skip,
            ConflictPolicy::Overwrite,
        ] {
            let json = serde_json::to_string(&policy).unwrap();
            assert!(
                json.starts_with('"'),
                "策略 {policy:?} 必须序列化成字符串名，实得 {json}"
            );
        }
    }

    /// 保存必须是原子的：留下 `.staging-` 即说明路径走错了。
    #[test]
    fn save_leaves_no_staging_file() {
        let dir = temp_dir("atomic");
        let path = dir.join(SETTINGS_FILE_NAME);
        save(&path, &TransferSettings::default()).unwrap();
        let names: Vec<String> = std::fs::read_dir(&dir)
            .unwrap()
            .flatten()
            .map(|e| e.file_name().to_string_lossy().into_owned())
            .collect();
        assert_eq!(names, vec![SETTINGS_FILE_NAME.to_owned()]);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn default_paths_follow_the_house_convention() {
        let settings = default_settings_path();
        assert!(
            settings.ends_with(Path::new("aegis").join("transfer").join(SETTINGS_FILE_NAME)),
            "设置文件应落在 %LOCALAPPDATA%\\aegis\\transfer\\ 下，实得 {}",
            settings.display()
        );
        let receive = default_receive_directory();
        assert!(
            receive.ends_with("Received"),
            "接收目录默认应落在 <下载>\\Received，实得 {}",
            receive.display()
        );
    }
}
