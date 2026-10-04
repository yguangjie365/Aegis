//! 文件互传的 Tauri 命令面（04 分册 §一：命令实体落 `services`，壳只做薄转发）。
//!
//! ## 这一层只做三件事
//!
//! 1. **持有服务实例**：`TransferService` 是进程级单例（一次运行期一个），
//!    由本模块的静态槽持有；不放在 app state 里是因为本 crate 不许引用 Tauri 宿主类型。
//! 2. **解包络、包结果**：入参 `contracts::CmdEnvelope`，出参一律 `serde` DTO（camelCase 给 TS）。
//! 3. **落日志**：每个命令一条，字段按 01 专题 §2.1 的八字段命名（见下）。
//!
//! ## 🔴 日志的现状与边界（如实登记）
//!
//! 八字段**JSONL 落盘**（W0-10 日志总线）**尚未落地**：本仓未装 `tracing_subscriber`，
//! 故本模块的日志目前**没有落点**。W0-07 也未把 `LogEntry` 放进 `contracts`。
//!
//! 本批的处理：**用 `tracing` 按八字段的名字输出结构化字段**（`module` / `action` /
//! `result` / `correlation_id` / `duration_ms` / `exception`），
//! 字段名与前端 `src/lib/logging/index.ts` 的手工投影逐字对齐，
//! `module` 取 `contracts::LOG_MODULE_TRANSFER`（**不散落魔法字符串**）。
//! ⇒ W0-10 装上 sink 后这些行即可直接落盘，无需回头改调用点。
//! 🔴 **本模块不自造 JSONL 写入器**——那是 W0-10 的职责，自造会与其冲突。
//!
//! ## 🔴 禁止静默失败
//!
//! 每个命令**无论成功失败都落一条日志**；失败路径一律返回带 `code` 的 DTO
//! （`code` 取 `reasons` 的 16 码冻结表，或本模块的启动期码——两者不混用，
//! 因为启动期失败不属于"一次传输"）。

// 🔴 Tauri 命令**必须按值接收参数**（`generate_handler!` 生成的反序列化要靠它），
// 故 `envelope: contracts::CmdEnvelope` 是宏契约要求的形状，不是可以改成引用的选择。
#![allow(clippy::needless_pass_by_value)]

use std::path::PathBuf;
use std::sync::{Mutex, MutexGuard, OnceLock};
use std::time::Instant;

use serde::{Deserialize, Serialize};

use super::conflict::ConflictPolicy;
use super::discovery::Peer;
use super::reasons;
use super::service::{SendFailure, StartFailure, TransferService};
use super::settings::{self, LoadOutcome, TransferSettings};
use super::tasks::Direction;
use super::transport::TransportError;

/// 启动期失败码（🔴 **不属** `reasons` 的 16 码冻结表——那张表说的是"一次传输"的失败）。
pub const CODE_START_INVALID_PORT: &str = "START_INVALID_PORT";
pub const CODE_START_RECEIVE_DIR_FAILED: &str = "START_RECEIVE_DIR_FAILED";
pub const CODE_START_TRANSFER_FAILED: &str = "START_TRANSFER_FAILED";
pub const CODE_START_DISCOVERY_FAILED: &str = "START_DISCOVERY_FAILED";
/// 命令层自身的用法错误（未启动就发送、载荷缺字段等）。
pub const CODE_NOT_RUNNING: &str = "SERVICE_NOT_RUNNING";
pub const CODE_ALREADY_RUNNING: &str = "SERVICE_ALREADY_RUNNING";
pub const CODE_BAD_PAYLOAD: &str = "BAD_PAYLOAD";
pub const CODE_SETTINGS_IO: &str = "SETTINGS_IO_FAILED";
/// 答复的待决确认已不存在（超时判拒 / 已被答复 / 服务已停）。
///
/// 🔴 独立成一个码（而不是复用 `SERVICE_NOT_RUNNING`）：界面必须能区分
/// "服务没跑"与"你点慢了，那条请求已经按拒绝处理了"——两者对用户的意思完全不同。
pub const CODE_CONFIRM_GONE: &str = "CONFIRM_GONE";
/// 有传输在跑，此刻不能停服务（不是失败，是「现在不能」）。
pub const CODE_SERVICE_BUSY: &str = "SERVICE_BUSY";

/// 进程级状态：运行中的服务 + 设置文件路径。
#[derive(Debug, Default)]
struct CommandState {
    // 用 Arc 而不是裸值：发送是长时间阻塞的，若在持有 lock_state 守卫期间传输，
    // 其它所有命令（状态/待决确认/台账）都会卡在同一把锁上——弹窗轮询根本执行不到。
    // 存 Arc 就能做到「取出 → 放锁 → 再传输」。
    service: Option<std::sync::Arc<TransferService>>,
    /// 设置文件落点（测试可换到临时目录，避免污染真实用户配置）
    settings_path: Option<PathBuf>,
}

fn state() -> &'static Mutex<CommandState> {
    static STATE: OnceLock<Mutex<CommandState>> = OnceLock::new();
    STATE.get_or_init(|| Mutex::new(CommandState::default()))
}

fn lock_state() -> MutexGuard<'static, CommandState> {
    // 命令层是进程级临界区：一个线程 panic 后仍应能继续服务（毒锁恢复），
    // 否则整个互传功能会因一次意外而永久不可用。
    state()
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

fn settings_path(state: &CommandState) -> PathBuf {
    state
        .settings_path
        .clone()
        .unwrap_or_else(settings::default_settings_path)
}

// ── DTO（camelCase 给 TS） ────────────────────────────────────────────────

/// 服务状态快照（界面据此显示"当前服务运行于 …"与配对码倒计时）。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TransferStatusDto {
    pub running: bool,
    pub transfer_port: u16,
    pub discovery_port: u16,
    pub receive_directory: String,
    /// 服务未启动时为 `None`（界面显示占位"——————"，F-FT-38）
    pub pairing_code: Option<String>,
    pub pairing_expires_in_ms: i64,
    pub online_peer_count: usize,
    pub live_connections: usize,
    /// 本机设备标识（R-6；未启动时为 `None`）
    pub device_id: Option<String>,
}

/// 在线设备一行（F-FT-08）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TransferPeerDto {
    pub device_id: String,
    pub device_name: String,
    pub ip: String,
    pub port: u16,
    /// 本机收到最后一次宣告的时刻（界面算"最近刷新"与相对时间）
    pub last_seen_ms: i64,
}

/// 失败载荷（🔴 只回**原因码 + 可读文案**，不回显对端输入）。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TransferErrorDto {
    pub code: String,
    pub message: String,
}

impl TransferErrorDto {
    fn new(code: &str, message: impl Into<String>) -> Self {
        Self {
            code: code.to_owned(),
            message: message.into(),
        }
    }
}

/// 设置加载结论（禁止静默失败：把"文件缺失/损坏/修了什么"如实带回界面）。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SettingsLoadedDto {
    pub settings: TransferSettings,
    pub file_missing: bool,
    pub corrupt: bool,
    pub repairs: Vec<String>,
}

/// 停止结论（R-28：停止失败不得宣称已停止）。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StopReportDto {
    pub all_stopped: bool,
    pub threads_joined: usize,
    pub message: String,
}

/// 发送结论。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SendResultDto {
    /// `Delivered` / `Skipped` / `HashMismatch`
    pub outcome: String,
    /// 非成功时带原因码（取 `reasons` 表）
    pub reason_code: Option<String>,
}

/// `transfer_send_file` 的载荷。
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SendFilePayload {
    pub ip: String,
    pub port: u16,
    pub path: String,
    #[serde(default)]
    pub pair_code: Option<String>,
}

// ── 八字段日志（见模块头对现状的说明） ────────────────────────────────────

/// 一条命令的日志（字段名与前端手工投影逐字对齐）。
fn log_command(
    action: &str,
    result: ResultLabel,
    correlation_id: &str,
    started: Instant,
    exception: Option<&str>,
) {
    let duration_ms = started.elapsed().as_millis();
    let module = contracts::LOG_MODULE_TRANSFER;
    // 成功一条 info、其余一条 warn（八字段同名；见模块头对 sink 现状的说明）
    if result == ResultLabel::Success {
        tracing::info!(
            module,
            action,
            result = result.as_str(),
            correlation_id,
            duration_ms,
            "transfer command"
        );
    } else {
        tracing::warn!(
            module,
            action,
            result = result.as_str(),
            correlation_id,
            duration_ms,
            exception = exception.unwrap_or(""),
            "transfer command"
        );
    }
}

/// 八字段里 `Result` 的取值（01 专题 §2.1：`Success / Failed / Cancelled / Rejected`）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ResultLabel {
    Success,
    Failed,
    Cancelled,
    Rejected,
}

impl ResultLabel {
    fn as_str(self) -> &'static str {
        match self {
            Self::Success => "Success",
            Self::Failed => "Failed",
            Self::Cancelled => "Cancelled",
            Self::Rejected => "Rejected",
        }
    }
}

// ── 命令实现（抽成可测的 `*_impl`，命令函数只是薄壳） ──────────────────────

/// 状态快照。
fn status_impl() -> TransferStatusDto {
    let state = lock_state();
    let now = super::sink::now_unix_ms();
    let persisted = settings::load(&settings_path(&state)).settings;
    match state.service.as_ref() {
        Some(service) => TransferStatusDto {
            running: true,
            transfer_port: service.local_addr().port(),
            discovery_port: service.discovery_port(),
            receive_directory: service.receive_root().display().to_string(),
            pairing_code: Some(service.current_pairing_code(now)),
            pairing_expires_in_ms: service.pairing_expires_in_ms(now),
            online_peer_count: service.online_peers().len(),
            live_connections: service.live_connections(),
            device_id: Some(service.device_id()),
        },
        None => TransferStatusDto {
            running: false,
            transfer_port: persisted.transfer_port,
            discovery_port: persisted.discovery_port,
            receive_directory: persisted
                .receive_directory
                .clone()
                .unwrap_or_else(|| settings::default_receive_directory().display().to_string()),
            pairing_code: None,
            pairing_expires_in_ms: 0,
            online_peer_count: 0,
            live_connections: 0,
            device_id: None,
        },
    }
}

/// 启动服务（用**已持久化的设置**；`machine_name` 取设置里的名字或环境变量）。
fn start_impl() -> Result<TransferStatusDto, TransferErrorDto> {
    let mut state = lock_state();
    if state.service.is_some() {
        return Err(TransferErrorDto::new(
            CODE_ALREADY_RUNNING,
            "传输与设备发现服务已在运行",
        ));
    }
    let path = settings_path(&state);
    let outcome = settings::load(&path);
    let machine_name = outcome
        .settings
        .device_name
        .clone()
        .unwrap_or_else(machine_name);
    // 先把"启动前"的设置留一份：`start` 会把它移走，而下面要拿它比对新令牌
    let before = outcome.settings.clone();

    // 🔴 历史与设置**同目录**：这样"测试换掉设置路径"这一步自动把历史也隔离了，
    //    不会再出现"跑一次测试就把用户的真实历史写脏"（2026-10-04 实观测到过）。
    let history_path =
        state
            .settings_path
            .as_ref()
            .map_or_else(super::history::default_history_path, |path| {
                path.parent()
                    .map_or_else(|| path.clone(), |dir| dir.join("history.json"))
            });

    let service =
        TransferService::start_with_history(outcome.settings, &machine_name, history_path)
            .map_err(|failure| {
                let (code, message) = match &failure {
                    StartFailure::InvalidPort { .. } => {
                        (CODE_START_INVALID_PORT, failure.to_string())
                    }
                    StartFailure::ReceiveDirectory { .. } => {
                        (CODE_START_RECEIVE_DIR_FAILED, failure.to_string())
                    }
                    StartFailure::Transfer { .. } => {
                        (CODE_START_TRANSFER_FAILED, failure.to_string())
                    }
                    StartFailure::Discovery { .. } => {
                        (CODE_START_DISCOVERY_FAILED, failure.to_string())
                    }
                };
                TransferErrorDto::new(code, message)
            })?;

    // 🔴 R-6：新生成的设备令牌必须写回设置，否则每次重启都会换标识
    if before.device_token.as_deref() != Some(service.device_token()) {
        let mut updated = before.clone();
        updated.device_token = Some(service.device_token().to_owned());
        // 写回失败不该让"服务已起来"这件事回滚（服务本身没受影响）；
        // 但要如实报——故把失败记在状态里由下一次 `status` 带出？本批先尽力而为并留日志。
        if let Err(error) = settings::save(&path, &updated) {
            tracing::warn!(
                module = contracts::LOG_MODULE_TRANSFER,
                action = "transferServiceStart",
                result = "Failed",
                exception = error.as_str(),
                "设备令牌写回设置失败（下次重启标识会变）"
            );
        }
    }

    state.service = Some(std::sync::Arc::new(service));
    drop(state);
    Ok(status_impl())
}

/// 停止服务。
fn stop_impl() -> Result<StopReportDto, TransferErrorDto> {
    let mut state = lock_state();
    let Some(service) = state.service.take() else {
        return Err(TransferErrorDto::new(CODE_NOT_RUNNING, "服务未在运行"));
    };
    // 🔴 传输进行中时，发送路径克隆过一份 Arc ⇒ 此刻**不能**解包。
    //    这不是错误，是「现在不能停」：把服务放回去，如实回一个码。
    let owned = match std::sync::Arc::try_unwrap(service) {
        Ok(owned) => owned,
        Err(still_shared) => {
            state.service = Some(still_shared);
            return Err(TransferErrorDto::new(
                CODE_SERVICE_BUSY,
                "有传输正在进行，请先取消或等它结束再停止服务。",
            ));
        }
    };
    drop(state);
    let report = owned.stop();
    Ok(StopReportDto {
        all_stopped: report.all_stopped,
        threads_joined: report.threads_joined,
        message: report.describe(),
    })
}

/// 加载设置。
fn settings_load_impl() -> SettingsLoadedDto {
    let state = lock_state();
    let path = settings_path(&state);
    drop(state);
    let LoadOutcome {
        settings,
        file_missing,
        corrupt,
        repairs,
    } = settings::load(&path);
    SettingsLoadedDto {
        settings,
        file_missing,
        corrupt,
        repairs,
    }
}

/// 保存设置（🔴 原子写；运行中的服务不会热更新端口——那需要重启服务，与 F-FT-06 的两态提示一致）。
fn settings_save_impl(settings: &TransferSettings) -> Result<(), TransferErrorDto> {
    let state = lock_state();
    let path = settings_path(&state);
    drop(state);
    settings::save(&path, settings)
        .map_err(|error| TransferErrorDto::new(CODE_SETTINGS_IO, format!("写入设置失败：{error}")))
}

/// 在线设备列表。
fn peers_impl() -> Vec<TransferPeerDto> {
    let state = lock_state();
    let Some(service) = state.service.as_ref() else {
        return Vec::new();
    };
    let mut peers: Vec<Peer> = service.online_peers();
    peers.sort_by(|a, b| a.device_id.cmp(&b.device_id));
    peers
        .into_iter()
        .map(|peer| TransferPeerDto {
            device_id: peer.device_id,
            device_name: peer.device_name,
            ip: peer.address.ip().to_string(),
            port: peer.transfer_port,
            last_seen_ms: peer.last_seen_local_ms,
        })
        .collect()
}

/// 发送一个文件到指定设备。
fn send_impl(payload: &SendFilePayload) -> Result<SendResultDto, TransferErrorDto> {
    {
        let state = lock_state();
        if state.service.is_none() {
            // 发送侧不要求本机服务在跑（对端在跑即可），但界面语境是"服务已启动"；
            // 这里**不拦**，只把事实记进日志——拦了反而让"只发不收"的用法不可用。
        }
    }
    let ip: std::net::IpAddr = payload.ip.parse().map_err(|_| {
        TransferErrorDto::new(CODE_BAD_PAYLOAD, format!("地址不合法：{}", payload.ip))
    })?;
    let peer = std::net::SocketAddr::new(ip, payload.port);
    let path = PathBuf::from(&payload.path);
    let cancel = tokio_util::sync::CancellationToken::new();
    let task_id = format!("t-{}", super::sink::now_unix_ms());

    // 🔴 服务在跑 → 走**会登记台账**的入口；没跑 → 退回纯传输。
    //    后者是「只发不收」的合法用法（对端在跑即可），但此时**没有台账可记**，如实降级。
    //    锁在这里就释放，别把 `lock_state()` 的守卫带进传输的阻塞期。
    // 🟠 注意：这个块把 lock_state() 的守卫 held 在**整段传输**期间（发送会阻塞很久）。
    //    后果是传输期间其它命令（状态 / 待决确认 / 台账）都卡在同一把锁上——
    //    同进程实测时 12 次待决采样全部落在裁决之后，接收确认弹窗因此永远出不来。
    //    修它需要让 TransferService::stop() 不再按值消费 self（好让状态里能存 Arc），
    //    属独立改动，已登记。
    // 🔴 先把 Arc<TransferService> 克隆出来、立刻放锁，再做长时间传输。
    //    第一版把 lock_state 的守卫 held 在整段传输里，于是传输期间其它命令
    //    全卡在同一把锁上——接收前询问的弹窗永远轮询不到待决。
    //    这不是推测：同进程实测时 12 次待决采样全部落在裁决之后。
    let service = {
        let state = lock_state();
        state.service.clone()
    };
    let result = match service {
        Some(service) => service.send_file_tracked(
            peer,
            &path,
            super::chunk::DEFAULT_CHUNK_SIZE,
            payload.pair_code.as_deref(),
            &cancel,
        ),
        None => super::service::send_to(
            peer,
            &path,
            super::chunk::DEFAULT_CHUNK_SIZE,
            payload.pair_code.as_deref(),
            &task_id,
            &cancel,
            |_| {},
        ),
    };

    match result {
        Ok(outcome) => Ok(SendResultDto {
            outcome: format!("{outcome:?}"),
            reason_code: None,
        }),
        Err(failure) => {
            let code = match &failure {
                SendFailure::NotAFile => CODE_BAD_PAYLOAD,
                SendFailure::Source(_) | SendFailure::Transport(TransportError::Source(_)) => {
                    reasons::INVALID_FILE_SIZE
                }
                // 对端明确说的话（拒收/取消）优先原样回传——R-16：以对端回传的原因码为准
                SendFailure::Transport(
                    TransportError::Rejected { reason_code }
                    | TransportError::Cancelled { reason_code },
                ) => reason_code,
                SendFailure::Transport(TransportError::Aborted) => reasons::USER_CANCEL,
                SendFailure::Transport(TransportError::RemoteFailed { reason_code }) => {
                    reason_code.as_deref().unwrap_or(reasons::PEER_DISCONNECTED)
                }
                // 其余都归"这一趟没成，对端或链路出了问题"
                SendFailure::Connect { .. }
                | SendFailure::Transport(
                    TransportError::TimedOut
                    | TransportError::PeerClosed
                    | TransportError::Io(_)
                    | TransportError::Frame(_)
                    | TransportError::Sink(_)
                    | TransportError::Unexpected(_),
                ) => reasons::PEER_DISCONNECTED,
            };
            Err(TransferErrorDto::new(code, failure.to_string()))
        }
    }
}

/// 机器名（设置里没写设备名时的回退）。
fn machine_name() -> String {
    std::env::var("COMPUTERNAME")
        .or_else(|_| std::env::var("HOSTNAME"))
        .unwrap_or_else(|_| "Aegis".to_owned())
}

// ── 命令（薄壳：解包络 → 调 impl → 落日志） ──────────────────────────────

/// 服务状态（界面轮询；无副作用）。
#[must_use]
#[tauri::command]
pub fn transfer_service_status(envelope: contracts::CmdEnvelope) -> TransferStatusDto {
    let started = Instant::now();
    let status = status_impl();
    log_command(
        "transferServiceStatus",
        ResultLabel::Success,
        &envelope.correlation_id,
        started,
        None,
    );
    status
}

/// 启动传输与设备发现服务（F-FT-02 的三步次序在 `service::start` 里）。
///
/// # Errors
/// 端口非法 / 传输段失败 / 发现段失败各有专属 `code` 与措辞（R-28）。
#[tauri::command]
pub fn transfer_service_start(
    envelope: contracts::CmdEnvelope,
) -> Result<TransferStatusDto, TransferErrorDto> {
    let started = Instant::now();
    let result = start_impl();
    let (label, exception) = match &result {
        Ok(_) => (ResultLabel::Success, None),
        Err(error) => (ResultLabel::Failed, Some(error.message.as_str())),
    };
    log_command(
        "transferServiceStart",
        label,
        &envelope.correlation_id,
        started,
        exception,
    );
    result
}

/// 停止服务。
///
/// 🔴 返回值里的 `allStopped` **必须**被界面用起来（R-28：停止失败不得宣称已停止）。
///
/// # Errors
/// 服务本来就没在跑时返回 [`CODE_NOT_RUNNING`]。
#[tauri::command]
pub fn transfer_service_stop(
    envelope: contracts::CmdEnvelope,
) -> Result<StopReportDto, TransferErrorDto> {
    let started = Instant::now();
    let result = stop_impl();
    let (label, exception) = match &result {
        Ok(report) if report.all_stopped => (ResultLabel::Success, None),
        Ok(report) => (ResultLabel::Failed, Some(report.message.as_str())),
        Err(error) => (ResultLabel::Failed, Some(error.message.as_str())),
    };
    log_command(
        "transferServiceStop",
        label,
        &envelope.correlation_id,
        started,
        exception,
    );
    result
}

/// 读取设置（含"文件缺失/损坏/修了什么"，供界面如实提示）。
#[must_use]
#[tauri::command]
pub fn transfer_settings_load(envelope: contracts::CmdEnvelope) -> SettingsLoadedDto {
    let started = Instant::now();
    let loaded = settings_load_impl();
    let label = if loaded.corrupt {
        ResultLabel::Failed
    } else {
        ResultLabel::Success
    };
    log_command(
        "transferSettingsLoad",
        label,
        &envelope.correlation_id,
        started,
        if loaded.corrupt {
            Some("设置文件损坏，已降级为默认值")
        } else {
            None
        },
    );
    loaded
}

/// 命令载荷里**至少要出现一个**这些键，才认为是"一份设置"。
///
/// 🔴 为什么需要这道校验：`TransferSettings` 带 `#[serde(default)]`（为**磁盘文件**的向前兼容，
/// 缺字段取默认值），于是**任意 JSON 对象**都能反序列化成功——包括 `{"nope":1}`。
/// 那意味着一个形状不对的载荷会**静默把用户设置清成默认值**（本批被自己的用例抓出）。
/// 故：磁盘文件保持宽容，**命令边界从严**。
const SETTINGS_KEYS: [&str; 10] = [
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
];

/// 该 JSON 是否"看起来像一份设置"（至少含一个已知键）。
fn looks_like_settings(value: &serde_json::Value) -> bool {
    value
        .as_object()
        .is_some_and(|object| SETTINGS_KEYS.iter().any(|key| object.contains_key(*key)))
}

/// 保存设置（原子写）。
///
/// # Errors
/// 载荷缺字段/形状不对返回 [`CODE_BAD_PAYLOAD`]；写盘失败返回 [`CODE_SETTINGS_IO`]。
#[tauri::command]
pub fn transfer_settings_save(envelope: contracts::CmdEnvelope) -> Result<(), TransferErrorDto> {
    let started = Instant::now();
    let parsed: Result<TransferSettings, _> = envelope
        .payload
        .clone()
        .ok_or("缺少 payload")
        .and_then(|value| {
            if !looks_like_settings(&value) {
                return Err("payload 里没有任何已知的设置字段（拒绝把设置清成默认）");
            }
            serde_json::from_value(value).map_err(|_| "payload 形状不对")
        });
    let result = match parsed {
        Ok(settings) => settings_save_impl(&settings),
        Err(message) => Err(TransferErrorDto::new(CODE_BAD_PAYLOAD, message)),
    };
    let (label, exception) = match &result {
        Ok(()) => (ResultLabel::Success, None),
        Err(error) => (ResultLabel::Failed, Some(error.message.as_str())),
    };
    log_command(
        "transferSettingsSave",
        label,
        &envelope.correlation_id,
        started,
        exception,
    );
    result
}

/// 在线设备列表（被动监听快照）。
///
/// `Vec` 不是 `#[must_use]` 类型，故这里显式加；🔴 `#[must_use]` 必须在
/// `#[tauri::command]` **之前**（宏会原样重放函数与其外层属性）。
#[must_use]
#[tauri::command]
pub fn transfer_peers_list(envelope: contracts::CmdEnvelope) -> Vec<TransferPeerDto> {
    let started = Instant::now();
    let peers = peers_impl();
    log_command(
        "transferPeersList",
        ResultLabel::Success,
        &envelope.correlation_id,
        started,
        None,
    );
    peers
}

/// 发送一个文件到指定设备（F-FT-11/12 的落点）。
///
/// # Errors
/// 载荷不合法 / 连不上 / 对端拒收（含 `PAIRING_INVALID`）时返回带原因码的 DTO。
#[tauri::command]
pub async fn transfer_send_file(
    envelope: contracts::CmdEnvelope,
) -> Result<SendResultDto, TransferErrorDto> {
    // 🔴 必须 async + spawn_blocking：发送会**阻塞**很久（握手最长 HANDSHAKE_WAIT=60s
    //    再加上整个文件的传输）。同步命令占住 Tauri 的命令线程 ⇒ 传输期间
    //    其它命令（状态 / 待决确认 / 台账）全部排队 ⇒ 界面看起来死了，
    //    「接收前询问」的弹窗也永远弹不出来。
    //
    //    这不是推测：**跨进程实测**已证（2026-10-04）——同一个应用自己发送时，
    //    12 次待决采样全部落在裁决之后；换成独立进程发送，弹窗立刻正常出现、
    //    接收成功、SHA-256 逐字节一致。
    let moved = envelope.clone();
    tauri::async_runtime::spawn_blocking(move || send_command_blocking(moved))
        .await
        .unwrap_or_else(|error| {
            Err(TransferErrorDto::new(
                "SEND_TASK_FAILED",
                format!("发送任务异常终止：{error}"),
            ))
        })
}

/// 发送命令的**阻塞本体**（由 `transfer_send_file` 放到阻塞线程池执行）。
fn send_command_blocking(
    envelope: contracts::CmdEnvelope,
) -> Result<SendResultDto, TransferErrorDto> {
    let started = Instant::now();
    let parsed: Result<SendFilePayload, _> = envelope
        .payload
        .clone()
        .ok_or("缺少 payload")
        .and_then(|value| serde_json::from_value(value).map_err(|_| "payload 形状不对"));
    let result = match parsed {
        Ok(payload) => send_impl(&payload),
        Err(message) => Err(TransferErrorDto::new(CODE_BAD_PAYLOAD, message)),
    };
    let (label, exception) = match &result {
        Ok(_) => (ResultLabel::Success, None),
        Err(error) if error.code == reasons::USER_CANCEL => (ResultLabel::Cancelled, None),
        Err(error) if error.code == reasons::PAIRING_INVALID => {
            (ResultLabel::Rejected, Some(error.message.as_str()))
        }
        Err(error) => (ResultLabel::Failed, Some(error.message.as_str())),
    };
    log_command(
        "transferSendFile",
        label,
        &envelope.correlation_id,
        started,
        exception,
    );
    result
}

/// 一条传输任务的对外快照（F-FT-30 的五态）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TransferTaskDto {
    pub id: String,
    /// `send` | `receive`
    pub direction: String,
    pub peer_label: String,
    pub file_name: String,
    pub file_size: u64,
    pub transferred: u64,
    /// 0..=100（由后端算，避免前后端各算一遍口径漂移）
    pub percent: u32,
    /// `active` | `completed` | `skipped` | `failed` | `cancelled`
    pub state: String,
    /// 终态原因码（取 reasons 的冻结表；进行中为 None）
    pub reason_code: Option<String>,
    pub started_at_ms: i64,
    pub finished_at_ms: Option<i64>,
}
/// 待用户裁决的接收确认（F-FT-14）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PendingConfirmDto {
    /// 裁决时回传此 id
    pub id: String,
    /// 对端标识（`ip:port`）
    pub peer_label: String,
    pub file_name: String,
    pub file_size: u64,
    /// 保存目录（空串表示未定，界面显示"—"）
    pub save_directory: String,
    /// 磁盘剩余；`None` = **无法判定**（界面显示"剩余空间无法判定"，不得显示 0）
    pub available_bytes: Option<u64>,
    /// 是否已存在同名文件。🔴 为假时界面**不得**显示冲突区（F-FT-14 原文"仅当存在同名文件时"）
    pub name_conflict: bool,
    /// 距超时判拒还剩多少毫秒（R-8：超时按拒绝，故界面必须给出这个倒计时）
    pub expires_in_ms: i64,
}

/// 列出待决确认。服务未运行时返回空表（不是错误——"没有待决"与"服务没跑"都是空）。
#[must_use]
#[tauri::command]
pub fn transfer_pending_confirms(envelope: contracts::CmdEnvelope) -> Vec<PendingConfirmDto> {
    let started = Instant::now();
    let list = pending_confirms_impl();
    log_command(
        "transferPendingConfirms",
        ResultLabel::Success,
        &envelope.correlation_id,
        started,
        None,
    );
    list
}

/// 答复一条待决确认（`accept=false` 即拒绝）。
///
/// # Errors
/// 服务未运行、或该条待决**已不存在**（超时判拒/已被答复）时返回带原因码的 DTO。
/// 🔴 后者不是"成功"：界面必须如实说"该请求已超时"，而不是显示"已接收"。
#[tauri::command]
pub fn transfer_confirm_receive(
    envelope: contracts::CmdEnvelope,
    id: String,
    accept: bool,
    // 本次传输的冲突策略覆盖（rename / ask / skip / overwrite）；None = 沿用设置。
    conflict_policy: Option<String>,
) -> Result<(), TransferErrorDto> {
    let started = Instant::now();
    // 🔴 只调一次：`confirm_impl` 是**有副作用**的（它消费掉那条待决），
    // 调两次会让第二次必然返回 CONFIRM_GONE——首版就是这么写错的
    let outcome = confirm_impl(&id, accept, conflict_policy.as_deref());
    let (label, exception) = match &outcome {
        Ok(()) => (ResultLabel::Success, None),
        Err(code) => (ResultLabel::Failed, Some(*code)),
    };
    log_command(
        "transferConfirmReceive",
        label,
        &envelope.correlation_id,
        started,
        exception,
    );
    outcome.map_err(|code| TransferErrorDto {
        code: code.to_owned(),
        message: if code == CODE_CONFIRM_GONE {
            "该接收请求已超时或被处理，无法再答复（已按拒绝处理）。".to_owned()
        } else {
            "传输服务未在运行。".to_owned()
        },
    })
}

fn pending_confirms_impl() -> Vec<PendingConfirmDto> {
    let state = lock_state();
    let Some(service) = state.service.as_ref() else {
        return Vec::new();
    };
    service
        .pending_confirms_with_ttl()
        .into_iter()
        .map(|(pending, expires_in_ms)| PendingConfirmDto {
            id: pending.id,
            peer_label: pending.peer_label,
            file_name: pending.file_name,
            file_size: pending.file_size,
            save_directory: pending.save_directory,
            available_bytes: pending.available_bytes,
            name_conflict: pending.name_conflict,
            expires_in_ms,
        })
        .collect()
}

fn confirm_impl(id: &str, accept: bool, policy: Option<&str>) -> Result<(), &'static str> {
    let state = lock_state();
    let Some(service) = state.service.as_ref() else {
        return Err(CODE_NOT_RUNNING);
    };
    // 策略串非法时**不静默忽略**：它是界面来的输入，出错必须让用户看见
    let parsed = match policy {
        None => None,
        Some(raw) => match parse_conflict_policy(raw) {
            Some(value) => Some(value),
            None => return Err(CODE_BAD_PAYLOAD),
        },
    };
    if service.resolve_confirm_with_policy(id, accept, parsed) {
        Ok(())
    } else {
        Err(CODE_CONFIRM_GONE)
    }
}

/// 传输任务台账快照（F-FT-25/30）。
///
/// 🔴 只回**只读快照**：任务由收发线程登记，命令层不得改写（R-16 终态不可回退
/// 由 `TaskRegistry` 独占保证，多一条写入路径就多一个破坏它的机会）。
#[must_use]
#[tauri::command]
pub fn transfer_tasks_list(envelope: contracts::CmdEnvelope) -> Vec<TransferTaskDto> {
    let started = Instant::now();
    let tasks = tasks_impl();
    log_command(
        "transferTasksList",
        ResultLabel::Success,
        &envelope.correlation_id,
        started,
        None,
    );
    tasks
}

fn tasks_impl() -> Vec<TransferTaskDto> {
    let state = lock_state();
    let Some(service) = state.service.as_ref() else {
        return Vec::new();
    };
    service
        .tasks_snapshot()
        .into_iter()
        .map(|task| {
            // 🔴 先算 percent：它在字段被移出后再调就会借用已移动的值
            let percent = task.percent();
            TransferTaskDto {
                id: task.id,
                direction: match task.direction {
                    Direction::Send => "send".to_owned(),
                    Direction::Receive => "receive".to_owned(),
                },
                peer_label: task.peer_label,
                file_name: task.file_name,
                file_size: task.file_size,
                transferred: task.transferred,
                percent,
                state: task.state.as_str().to_owned(),
                reason_code: task.reason_code,
                started_at_ms: task.started_at_ms,
                finished_at_ms: task.finished_at_ms,
            }
        })
        .collect()
}

/// 把界面传来的冲突策略串解析成枚举。
///
/// 🔴 不认识的串返回 `None`（调用方据此回 `BAD_PAYLOAD`），**不静默落回默认值**——
/// 静默落回会让用户以为"按我选的策略办了"，而实际按了别的。
fn parse_conflict_policy(raw: &str) -> Option<ConflictPolicy> {
    match raw {
        "rename" => Some(ConflictPolicy::Rename),
        "ask" => Some(ConflictPolicy::Ask),
        "skip" => Some(ConflictPolicy::Skip),
        "overwrite" => Some(ConflictPolicy::Overwrite),
        _ => None,
    }
}
/// 取消命令的返回（只回一个事实，不回文案——文案归界面）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TransferCancelDto {
    /// 是否真的发出了取消。`false` = 该任务已收尾或不存在
    pub cancelled: bool,
}

/// 取消一个进行中的传输任务（F-FT-26）。
///
/// 🔴 `cancelled: false` 表示**该任务已收尾或不存在**——这不是失败，
/// 是"没有什么可取消的"。界面应据此**刷新列表**，而不是弹失败提示
/// （那会让人以为出了错；而真实情况往往是任务刚跑完）。
#[must_use]
#[tauri::command]
pub fn transfer_cancel_task(envelope: contracts::CmdEnvelope, id: String) -> TransferCancelDto {
    let started = Instant::now();
    let outcome = {
        // 锁在 `{}` 里就释放：`cancel()` 会唤醒阻塞中的传输线程，
        // 把命令锁带进那个时刻没有好处
        let state = lock_state();
        state
            .service
            .as_ref()
            .map(|service| service.cancel_task(&id))
    };
    let (label, exception, cancelled) = match outcome {
        Some(true) => (ResultLabel::Success, None, true),
        // 服务没跑 / 任务已收尾：都不是错误，如实回报"没取消到"
        Some(false) | None => (ResultLabel::Rejected, Some("no-such-active-task"), false),
    };
    log_command(
        "transferCancelTask",
        label,
        &envelope.correlation_id,
        started,
        exception,
    );
    TransferCancelDto { cancelled }
}

/// 一条传输历史的对外快照（F-FT-54）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TransferHistoryEntryDto {
    pub id: String,
    /// `send` | `receive`
    pub direction: String,
    pub peer_label: String,
    pub file_name: String,
    pub file_size: u64,
    /// `completed` | `skipped` | `failed` | `cancelled`
    pub state: String,
    pub reason_code: Option<String>,
    /// 原因说明（🔴 未知码原样回显，不硬翻——F-FT-31 末句）
    pub reason_text: String,
    pub finished_at_ms: i64,
    /// 本地时区的可读时间（界面与 CSV 共用，避免两处各格式化一遍）
    pub finished_at_text: String,
}

/// 列出传输历史（F-FT-54）。服务未运行时返回**空表**（不是错误，与设备列表同口径）。
#[must_use]
#[tauri::command]
pub fn transfer_history_list(envelope: contracts::CmdEnvelope) -> Vec<TransferHistoryEntryDto> {
    let started = Instant::now();
    let list = history_impl();
    log_command(
        "transferHistoryList",
        ResultLabel::Success,
        &envelope.correlation_id,
        started,
        None,
    );
    list
}

/// 导出历史为 CSV 到 `path`（F-FT-58）。
///
/// 🔴 写盘放在**后端**（[`infra::fs::atomic_write`]）：这样不必为了导出再引入前端 fs 插件，
/// 也顺带拿到原子写——半截 CSV 比没有 CSV 更糟。
///
/// # Errors
/// 服务未运行、或路径不可写时返回带原因码的 DTO。
#[tauri::command]
pub fn transfer_history_export(
    envelope: contracts::CmdEnvelope,
    path: String,
) -> Result<(), TransferErrorDto> {
    let started = Instant::now();
    let csv = {
        let state = lock_state();
        match state.service.as_ref() {
            Some(service) => service.history_csv(),
            None => {
                return Err(TransferErrorDto::new(
                    CODE_NOT_RUNNING,
                    "传输服务未在运行。",
                ));
            }
        }
    };
    let target = std::path::PathBuf::from(&path);
    let result = infra::fs::atomic_write(&target, csv.as_bytes())
        .map_err(|error| TransferErrorDto::new(CODE_SETTINGS_IO, format!("导出失败：{error}")));
    let (label, exception) = match &result {
        Ok(()) => (ResultLabel::Success, None),
        Err(error) => (ResultLabel::Failed, Some(error.code.as_str())),
    };
    log_command(
        "transferHistoryExport",
        label,
        &envelope.correlation_id,
        started,
        exception,
    );
    result
}

fn history_impl() -> Vec<TransferHistoryEntryDto> {
    let state = lock_state();
    let Some(service) = state.service.as_ref() else {
        return Vec::new();
    };
    service
        .history_snapshot()
        .into_iter()
        .map(|entry| {
            // 🔴 先算 reason_text：字段被移出后再调就会借用已移动的值（任务 DTO 同款教训）
            let reason_text = entry.reason_text();
            TransferHistoryEntryDto {
                id: entry.id,
                direction: match entry.direction {
                    Direction::Send => "send".to_owned(),
                    Direction::Receive => "receive".to_owned(),
                },
                peer_label: entry.peer_label,
                file_name: entry.file_name,
                file_size: entry.file_size,
                state: entry.state.as_str().to_owned(),
                reason_code: entry.reason_code,
                reason_text,
                finished_at_ms: entry.finished_at_ms,
                finished_at_text: super::history::format_time(entry.finished_at_ms),
            }
        })
        .collect()
}

/// 文本空内容（命令级码，**不属于** [`super::reasons`] 那 16 个传输原因码）。
pub const CODE_TEXT_EMPTY: &str = "TEXT_EMPTY";

/// 发送文本的载荷（F-FT-33）。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SendTextPayload {
    /// 目标设备地址（`ip:port`）——🔴 由用户在界面上明确选定，不许猜（R-7）
    pub peer: String,
    pub text: String,
}

/// 文本发送的对外结论（F-FT-34）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TextOutcomeDto {
    /// 🔴 电脑通道上它的含义是**对方剪贴板写入成功**，不是"发出去了"。
    /// 界面文案必须照此措辞，不得写成"已发送"。
    pub delivered: bool,
    pub reason_code: Option<String>,
    /// 原因说明（🔴 未知码原样回显，F-FT-31 末句）
    pub reason_text: String,
}

/// 发送一条文本到指定设备（F-FT-33 的落点）。
///
/// # Errors
/// 载荷不合法 / 文本不合法（空、超 256 KB）/ 未起服务 / 连不上 / 对端拒收。
#[tauri::command]
pub async fn transfer_send_text(
    envelope: contracts::CmdEnvelope,
) -> Result<TextOutcomeDto, TransferErrorDto> {
    // 🔴 同 `transfer_send_file`：必须 async + spawn_blocking。
    //    发送要等接收端把「接收前询问」问完（最长 HANDSHAKE_WAIT=60s），
    //    同步命令会占住 Tauri 命令线程 ⇒ 弹窗相关的轮询全部排队。
    let moved = envelope.clone();
    tauri::async_runtime::spawn_blocking(move || send_text_command_blocking(moved))
        .await
        .unwrap_or_else(|error| {
            Err(TransferErrorDto::new(
                CODE_BAD_PAYLOAD,
                format!("发送任务异常终止：{error}"),
            ))
        })
}

/// 文本发送命令的**阻塞本体**（由 `transfer_send_text` 放到阻塞线程池执行）。
fn send_text_command_blocking(
    envelope: contracts::CmdEnvelope,
) -> Result<TextOutcomeDto, TransferErrorDto> {
    let started = Instant::now();
    let parsed: Result<SendTextPayload, _> = envelope
        .payload
        .clone()
        .ok_or("缺少 payload")
        .and_then(|value| serde_json::from_value(value).map_err(|_| "payload 形状不对"));
    let result = match parsed {
        Ok(payload) => send_text_impl(&payload),
        Err(message) => Err(TransferErrorDto::new(CODE_BAD_PAYLOAD, message)),
    };
    let (label, exception) = match &result {
        Ok(_) => (ResultLabel::Success, None),
        Err(error) if error.code == reasons::USER_CANCEL => (ResultLabel::Cancelled, None),
        Err(error) => (ResultLabel::Failed, Some(error.message.as_str())),
    };
    log_command(
        "transferSendText",
        label,
        &envelope.correlation_id,
        started,
        exception,
    );
    result
}

fn send_text_impl(payload: &SendTextPayload) -> Result<TextOutcomeDto, TransferErrorDto> {
    // ── F-FT-32：**发送前**判定，拒绝而非截断 ──
    // 🔴 这里绝不"取前 256 KB 发出去"：那会出现"发出去了但内容少了半段"，
    //    而发送方与接收方都无从察觉。
    let validation = super::text::validate(&payload.text);
    if !validation.is_valid() {
        let message =
            super::text::error_text(validation).unwrap_or_else(|| "文本不合法".to_owned());
        let code = match validation.kind {
            super::text::TextValidationKind::Empty => CODE_TEXT_EMPTY,
            _ => reasons::TEXT_TOO_LONG,
        };
        return Err(TransferErrorDto::new(code, message));
    }

    let peer: std::net::SocketAddr = payload.peer.parse().map_err(|_| {
        TransferErrorDto::new(
            CODE_BAD_PAYLOAD,
            format!("目标地址不合法：{}", payload.peer),
        )
    })?;

    // 🔴 取出 Arc 后**立刻放锁**：发送会阻塞数十秒，绝不能把命令状态锁 held 在传输期间
    //    （第 36 轮的实测结论：那会让「接收前询问」的弹窗永远弹不出来）。
    let service = { lock_state().service.clone() };
    let Some(service) = service else {
        return Err(TransferErrorDto::new(
            CODE_NOT_RUNNING,
            "传输服务未在运行。",
        ));
    };

    let cancel = tokio_util::sync::CancellationToken::new();
    match service.send_text_tracked(peer, &payload.text, &cancel) {
        // 🔴 交付 = 对方剪贴板写入成功（F-FT-34）
        Ok(super::transport::TextOutcome::Delivered) => Ok(TextOutcomeDto {
            delivered: true,
            reason_code: None,
            reason_text: String::new(),
        }),
        // 业务性拒收不是"命令失败"：如实告诉界面"没交付 + 为什么"
        Ok(super::transport::TextOutcome::Rejected { reason_code }) => Ok(TextOutcomeDto {
            reason_text: reasons::display_text(&reason_code),
            reason_code: Some(reason_code),
            delivered: false,
        }),
        Err(failure) => {
            let code = match &failure {
                super::service::SendFailure::Transport(TransportError::Aborted) => {
                    reasons::USER_CANCEL
                }
                super::service::SendFailure::Transport(TransportError::RemoteFailed {
                    reason_code,
                }) => reason_code.as_deref().unwrap_or(reasons::PEER_DISCONNECTED),
                _ => reasons::PEER_DISCONNECTED,
            };
            Err(TransferErrorDto::new(code, failure.to_string()))
        }
    }
}

#[cfg(test)]
mod tests;
