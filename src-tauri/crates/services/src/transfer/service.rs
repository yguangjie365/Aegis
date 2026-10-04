//! 传输与设备发现服务的**生命周期**（F-FT-02 / R-28）。
//!
//! ## F-FT-02 的启动次序（本模块的骨架）
//!
//! 原文：*"启动按序执行：先做端口合法性校验 → 拉起传输服务 → 拉起设备发现服务"*。
//! 三步的顺序**不能换**：端口非法就不该起任何监听（否则先占住 TCP 再报端口错，
//! 用户看到一个"起了又挂"的怪状态）；发现服务起不来时必须**回滚**已经起来的传输服务。
//!
//! ## R-28 的两条硬要求
//!
//! 1. **阶段化归因 + 回滚**：两段各有专属失败措辞，任一段失败要回滚已起来的部分。
//!    本模块用 [`StartFailure`] 的变体承载"卡在哪一段"以及**回滚是否成功**。
//! 2. **停止失败不得宣称已停止**：*"停止失败不得宣称已停止，而应提示
//!    『（服务可能仍在运行，可重试停止）』"*。故 [`TransferService::stop`] 返回
//!    [`StopReport::all_stopped`]，**不**用 `()` 假装干净收场。
//!
//! ## 并发：两道互不相同的闸（都在本模块里是活的）
//!
//! | 闸 | 值 | 作用 |
//! |---|---|---|
//! | **连接硬上限** | [`CONNECTION_HARD_CAP`] | 抗"狂开连接"：超出即回一条 `Error` 再关（让对端拿到可读原因，而不是干等） |
//! | **接收并发** | `admission::MAX_RECEIVE_CONCURRENCY` | R-13 第 3 关：第 9 个传输会话经握手回执拿到 `CONCURRENCY_LIMIT` |
//!
//! 🔴 两者若只留一个都会有缺口：只有硬上限 → 第 9 个会话被静默丢弃、对端不知为何；
//! 只有并发闸 → 不设防的连接洪水会开出无限线程。
//!
//! ## 为什么线程数可控
//!
//! 每连接一个线程（见 `transport.rs` 模块头对"阻塞 vs async"的论证），
//! 线程数上界 = 连接硬上限（32），且每个连接有 30 秒读超时兜底 ⇒ 不会无限增长。
//!
//! 复用结论（DP-1）：③思路复用 ← 母本 `FileTransferService.StartAsync/StopAsync`
//! 的三步启动与失败措辞、以及"回滚已起来的部分"的处置次序（GPL-3.0 同向）。
//! 🔴 **两处本工程新增**：① 连接硬上限（母本靠 `WatsonTcp` 的会话管理，无此层）；
//! ② `stop` 返回"是否真的停住了"，把 R-28 的措辞要求变成**类型上的事实**而不是调用方的自觉。

use std::collections::HashMap;
use std::io;
use std::net::{IpAddr, SocketAddr, TcpListener, TcpStream, UdpSocket};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicI64, AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use std::time::Duration;

use tokio_util::sync::CancellationToken;

use super::admission::{self, Admission, AdmissionRequest, DiskSpace, MAX_RECEIVE_CONCURRENCY};
use super::confirm::{self, ConfirmGate};
use super::conflict::ConflictPolicy;
use super::discovery::{
    self, Announcement, Peer, PeerTable, SelfIdentity, ANNOUNCE_INTERVAL, OFFLINE_AFTER,
};
use super::pairing::PairingGuard;
use super::protocol::TransferMessage;
use super::reasons;
use super::settings::{self, PortProblem, TransferSettings};
use super::sink::now_unix_ms;
use super::source;
use super::tasks::{Direction, TaskRegistry, TaskState};
use super::transport::{self, HandshakeFacts, ReceiveOutcome, SendOutcome, TransportError};

/// 同时在册的连接硬上限（抗连接洪水；见模块头"两道闸"）。
pub const CONNECTION_HARD_CAP: usize = 32;

/// 发现套接字的单次读超时（决定"检查取消令牌"的粒度，也决定宣告节奏的精度）。
pub const DISCOVERY_POLL: Duration = Duration::from_millis(500);

/// 连对端的超时（发送侧）。
pub const CONNECT_TIMEOUT: Duration = Duration::from_secs(5);

/// 服务启动失败。
#[derive(Debug)]
pub enum StartFailure {
    /// 第 1 步就失败：端口非法（F-FT-02 原文"先做端口合法性校验"）——**未起任何监听**
    InvalidPort { problems: Vec<PortProblem> },
    /// 第 2 步失败：**接收目录建不出来**（路径不可写 / 网络路径未就绪）。
    ///
    /// 🔴 独立成一种失败（而不是并进 `Transfer`）：否则界面会显示"TCP 端口可能被占用"，
    /// 而真实原因是目录不可写——**归因错了比不归因更糟**（用户会去查端口）。
    ReceiveDirectory { path: PathBuf, cause: io::Error },
    /// 第 3 步失败：传输服务起不来（TCP 端口被占用等）
    Transfer { port: u16, cause: io::Error },
    /// 第 4 步失败：发现服务起不来。此时传输服务**已回滚**
    Discovery {
        port: u16,
        cause: io::Error,
        /// 回滚是否成功（R-28 要求回滚；回滚本身失败更要如实说）
        rollback_ok: bool,
    },
}

impl std::fmt::Display for StartFailure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidPort { problems } => {
                let list: Vec<String> = problems.iter().map(ToString::to_string).collect();
                write!(f, "端口设置非法，未启动任何监听：{}", list.join("；"))
            }
            Self::ReceiveDirectory { path, cause } => write!(
                f,
                "接收目录不可用：{}（{cause}）——请改到可写目录后重试",
                path.display()
            ),
            Self::Transfer { port, cause } => {
                write!(f, "传输服务启动失败（TCP {port} 可能被占用）：{cause}")
            }
            Self::Discovery {
                port,
                cause,
                rollback_ok,
            } => {
                write!(f, "设备发现服务启动失败（UDP {port} 可能被占用）：{cause}")?;
                if *rollback_ok {
                    f.write_str("；已回滚传输服务")
                } else {
                    f.write_str("；🔴 回滚传输服务失败，请停止服务后重试")
                }
            }
        }
    }
}

impl std::error::Error for StartFailure {}

/// 停止结论。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StopReport {
    /// 成功 join 的线程数
    pub threads_joined: usize,
    /// **是否全部停住**。为假时调用方必须提示"（服务可能仍在运行，可重试停止）"（R-28）
    pub all_stopped: bool,
}

impl StopReport {
    /// 供界面直接使用的措辞（R-28 的原文就在这条里）。
    #[must_use]
    pub fn describe(&self) -> String {
        if self.all_stopped {
            "✅ 传输与设备发现服务已停止".to_owned()
        } else {
            "⚠️ 传输与设备发现服务停止不完整（服务可能仍在运行，可重试停止）".to_owned()
        }
    }
}

/// 服务运行期共享的状态（被各线程持有）。
#[derive(Debug)]
struct ServiceContext {
    settings: TransferSettings,
    identity: SelfIdentity,
    receive_root: PathBuf,
    pairing: Mutex<PairingGuard>,
    /// 接收确认闸门（F-FT-18 / R-8）
    confirm: ConfirmGate,
    peers: Mutex<PeerTable>,
    /// 运行期任务台账（F-FT-25/29/30；🔴 R-16 终态不可回退由 `TaskRegistry` 保证）
    tasks: Mutex<TaskRegistry>,
    /// 传输历史（F-FT-54~58：上限 200、快照不可变、原因码原样存）。
    history: Mutex<super::history::History>,
    /// 历史落盘位置（启动时定下，避免运行期依赖当前目录）。
    history_path: PathBuf,
    /// 任务 id → 取消令牌（F-FT-26「取消单个任务」）。
    ///
    /// 🔴 没有这张表就没法"按 id 取消"：每次发送各自 `new` 一个令牌，
    /// 命令层手上只有任务 id，无从取回那个令牌。任务收尾时**必须**移除，
    /// 否则表会随会话无限增长。
    cancels: Mutex<HashMap<String, CancellationToken>>,
    /// 任务 id → 暂停闸门（F-FT-26/27）。
    ///
    /// 🔴 与 `cancels` 同一条纪律：任务收尾时**必须**移除，否则表会随会话无限增长。
    /// 存 `Arc` 而不是裸值：闸门要同时被**命令层**（按下暂停）与**传输闭包**（在块边界等待）
    /// 持有，两处必须拿到**同一个**闸门——这正是"暂停"能生效的前提。
    pauses: Mutex<HashMap<String, std::sync::Arc<super::pause::PauseGate>>>,
    /// 任务 id 计数（只保证同一会话内不重名，不追求跨进程唯一）
    task_seq: AtomicU64,
    last_refresh_ms: AtomicI64,
    active_receives: AtomicUsize,
}

impl ServiceContext {
    /// 生成一个新的任务 id。
    fn next_task_id(&self) -> String {
        let seq = self.task_seq.fetch_add(1, Ordering::SeqCst);
        format!("t{seq}")
    }

    fn lock_tasks(&self) -> std::sync::MutexGuard<'_, TaskRegistry> {
        // 同 `lock_peers`：毒锁恢复，一个线程 panic 不该让台账整体失效
        self.tasks
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    fn lock_cancels(&self) -> std::sync::MutexGuard<'_, HashMap<String, CancellationToken>> {
        self.cancels
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    /// 登记某个任务的取消令牌。
    fn register_cancel(&self, id: &str, token: CancellationToken) {
        self.lock_cancels().insert(id.to_owned(), token);
    }

    /// 任务收尾时移除令牌（**必须**调用，否则这张表会随会话无限增长）。
    fn forget_cancel(&self, id: &str) {
        self.lock_cancels().remove(id);
    }

    fn lock_pauses(
        &self,
    ) -> std::sync::MutexGuard<'_, HashMap<String, std::sync::Arc<super::pause::PauseGate>>> {
        self.pauses
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    /// 登记暂停闸门并**把同一个 `Arc` 返回**给调用方（传输闭包要用它等待）。
    fn register_pause(&self, id: &str) -> std::sync::Arc<super::pause::PauseGate> {
        let gate = std::sync::Arc::new(super::pause::PauseGate::new());
        self.lock_pauses()
            .insert(id.to_owned(), std::sync::Arc::clone(&gate));
        gate
    }

    /// 取某个任务的闸门（没有 ⇒ `None`，说明任务已收尾或不存在）。
    fn pause_gate(&self, id: &str) -> Option<std::sync::Arc<super::pause::PauseGate>> {
        self.lock_pauses().get(id).map(std::sync::Arc::clone)
    }

    /// 任务收尾时移除闸门（与 `forget_cancel` 同理，**必须**调用）。
    fn forget_pause(&self, id: &str) {
        self.lock_pauses().remove(id);
    }

    /// 本地暂停的上限（F-FT-27）：`0` = 不限 ⇒ `None`。
    fn pause_limit(&self) -> Option<std::time::Duration> {
        match self.settings.pause_timeout_minutes {
            0 => None,
            minutes => Some(std::time::Duration::from_secs(u64::from(minutes) * 60)),
        }
    }

    /// 历史表的守卫（毒锁恢复，同其它锁）。
    fn lock_history(&self) -> std::sync::MutexGuard<'_, super::history::History> {
        self.history
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    /// 把一条**终态**记进历史（F-FT-54）并落盘。
    ///
    /// 🔴 落盘失败**只报不抛**：历史写不进去，不该把一次已经成功的传输变成失败。
    /// 但也不能静默——这里用 `eprintln!` 而不是 `tracing::warn!`：
    /// 本仓**尚未安装 tracing subscriber**，tracing 调用目前全是空转
    /// （这正是 R-12「放行但记录」曾经不成立的原因）。
    fn record_history(&self, entry: super::history::HistoryEntry) {
        let mut history = self.lock_history();
        history.push(entry);
        if let Err(error) = history.save(&self.history_path) {
            eprintln!("[transfer] 历史落盘失败（不影响本次传输）：{error}");
        }
    }

    /// 取消某个任务。返回 `false` = 该任务已收尾或不存在。
    ///
    /// 🔴 `false` **不是"取消失败"**，是"没有什么可取消的"——界面应据此刷新列表，
    /// 而不是弹一个失败提示（那会让人以为出了错）。
    fn cancel_task(&self, id: &str) -> bool {
        // 🔴 只在表里取一下令牌就放锁：`cancel()` 会唤醒阻塞中的传输线程，
        //    在锁内做可能把"取锁的收尾路径"与它串起来。
        let token = self.lock_cancels().get(id).cloned();
        match token {
            Some(token) => {
                token.cancel();
                true
            }
            None => false,
        }
    }

    fn lock_peers(&self) -> std::sync::MutexGuard<'_, PeerTable> {
        // 共享缓存用"毒锁恢复"而非 unwrap：一个线程 panic 不该让发现功能整体瘫掉
        self.peers
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    fn lock_pairing(&self) -> std::sync::MutexGuard<'_, PairingGuard> {
        self.pairing
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    /// 该地址是否在"已发现在线"或"已知清单"里（F-FT-09 的白名单输入）。
    fn is_known_peer(&self, ip: IpAddr) -> bool {
        let online = self
            .lock_peers()
            .online()
            .iter()
            .any(|p| p.address.ip() == ip);
        if online {
            return true;
        }
        self.settings.known_peers.iter().any(|known| {
            known
                .ip
                .parse::<IpAddr>()
                .is_ok_and(|known_ip| known_ip == ip)
        })
    }

    /// 接收侧准入裁决（R-13 的闸门顺序**全部**在这里走完）。
    ///
    /// 顺序：白名单（纯） → 配对（有副作用：消费码/记失败） → 并发 → 分块 → 大小 → 磁盘。
    /// 🔴 配对放在白名单**之后**执行，是为了不让"未在白名单里的陌生地址"把失败计数打满
    /// （否则一个扫描者就能把某台正常设备的 5 次机会耗光）。
    fn admit(&self, peer: IpAddr, facts: &HandshakeFacts<'_>, disk: DiskSpace) -> Admission {
        let peer_discovered = self.is_known_peer(peer);
        if self.settings.require_known_peer && !peer_discovered {
            return Admission::Rejected {
                gate: admission::Gate::Whitelist,
                reason: reasons::PEER_NOT_DISCOVERED,
            };
        }

        let verdict = self
            .lock_pairing()
            .authorize(peer, facts.pair_code, now_unix_ms());
        if !verdict.is_allowed() {
            return Admission::Rejected {
                gate: admission::Gate::Pairing,
                reason: reasons::PAIRING_INVALID,
            };
        }

        let request = AdmissionRequest {
            peer_discovered,
            require_known_peer: self.settings.require_known_peer,
            // 走到这里配对该关已过（`NotRequired` 同样算过）
            paired: true,
            // 🔴 减 1：`active_receives` 含本次会话自己，闸门要的是"除我之外还有几个"
            active_receives: u32::try_from(
                self.active_receives
                    .load(Ordering::SeqCst)
                    .saturating_sub(1),
            )
            .unwrap_or(u32::MAX),
            declared_chunk_size: facts.chunk_size,
            file_size: facts.file_size,
            disk,
        };
        admission::evaluate(&request)
    }
}

/// 传输与设备发现服务。
#[derive(Debug)]
pub struct TransferService {
    local_addr: SocketAddr,
    discovery_port: u16,
    cancel: CancellationToken,
    threads: Vec<JoinHandle<()>>,
    ctx: Arc<ServiceContext>,
    live: Arc<AtomicUsize>,
}

impl TransferService {
    /// 按 F-FT-02 的次序启动：**校验端口 → 传输（TCP） → 发现（UDP）**，任一段失败即回滚。
    ///
    /// `machine_name` 取设置里的设备名；`device_token` 为 `None` 时**会生成一个**
    /// 并通过 [`Self::newly_generated_device_token`] 回传，调用方**必须**把它写回设置
    /// （R-6：标识重启后必须稳定）。
    ///
    /// 历史落在 [`super::history::default_history_path`]；需要换地方（如测试隔离）请用
    /// [`Self::start_with_history`]。
    ///
    /// # Errors
    /// 见 [`StartFailure`]（三段的失败都能区分）。
    pub fn start(settings: TransferSettings, machine_name: &str) -> Result<Self, StartFailure> {
        Self::start_with_history(
            settings,
            machine_name,
            super::history::default_history_path(),
        )
    }

    /// 同 [`Self::start`]，但显式指定历史落盘位置。
    ///
    /// 🔴 存在的理由与 `settings_path` 覆盖一样：**测试不该写用户的真实历史**。
    /// 命令层把历史放在**设置文件旁边**，于是"换掉设置路径"这一步自动把历史也隔离了。
    ///
    /// # Errors
    /// 同 [`Self::start`]。
    #[allow(clippy::too_many_lines)]
    // 🔴 允许超长：这是 F-FT-02 的**四步线性拉起**（校验端口 → 建接收目录 → TCP → UDP），
    // 每一步失败都要回滚前一步，而"回滚了没有"必须和"哪一步失败"读在一起。
    // 拆成辅助函数会把这条线性因果链打散——那正是本函数最需要被读懂的东西。
    pub fn start_with_history(
        settings: TransferSettings,
        machine_name: &str,
        history_path: PathBuf,
    ) -> Result<Self, StartFailure> {
        // ── 第 1 步：端口合法性校验（未起任何监听） ──
        let problems = settings::validate_ports(&settings);
        if !problems.is_empty() {
            return Err(StartFailure::InvalidPort { problems });
        }

        // ── 第 2 步：接收目录必须**现在就建出来** ──
        // 🔴 这是 2026-10-04 补的漏洞修复：此前从不创建它，而落盘侧的
        // `infra::safe::join_under_root` 要求根**真实存在**（要 canonicalize），
        // 于是首次运行时**每一次接收都会以 RootUnusable 失败**——功能整个不可用。
        let receive_root = settings
            .receive_directory
            .as_deref()
            .map_or_else(settings::default_receive_directory, PathBuf::from);
        if let Err(cause) = std::fs::create_dir_all(&receive_root) {
            return Err(StartFailure::ReceiveDirectory {
                path: receive_root,
                cause,
            });
        }

        // ── 第 3 步：传输服务（TCP） ──
        let listener = TcpListener::bind(SocketAddr::from(([0, 0, 0, 0], settings.transfer_port)))
            .map_err(|cause| StartFailure::Transfer {
                port: settings.transfer_port,
                cause,
            })?;
        let local_addr = listener
            .local_addr()
            .map_err(|cause| StartFailure::Transfer {
                port: settings.transfer_port,
                cause,
            })?;

        // ── 第 4 步：发现服务（UDP）——失败必须回滚第 3 步 ──
        let discovery_socket =
            match UdpSocket::bind(SocketAddr::from(([0, 0, 0, 0], settings.discovery_port))) {
                Ok(socket) => socket,
                Err(cause) => {
                    drop(listener);
                    return Err(StartFailure::Discovery {
                        port: settings.discovery_port,
                        cause,
                        // `drop` 后端口即刻释放；用一次试探绑定确认回滚真的生效
                        rollback_ok: TcpListener::bind(local_addr).is_ok(),
                    });
                }
            };

        let receive_root = settings
            .receive_directory
            .as_deref()
            .map_or_else(settings::default_receive_directory, PathBuf::from);
        // 先把要在结构体字面量里用到的设置读出来（`settings` 随后被移进 ctx）
        let require_pairing = settings.require_pairing;
        let (identity, _generated_token) =
            SelfIdentity::load_or_create(machine_name, settings.device_token.as_deref());
        // 新生成的令牌通过 `TransferService::device_token()` 暴露给调用方持久化
        // （R-6：不写回的话每次重启都会换标识，对端会看到"同一台机器反复出现又消失"）
        let self_id = identity.device_id();

        let ctx = Arc::new(ServiceContext {
            settings,
            identity,
            receive_root,
            pairing: Mutex::new(PairingGuard::new(require_pairing)),
            confirm: ConfirmGate::default(),
            // 🔴 对端表必须按**自己的设备标识**过滤自身宣告
            tasks: Mutex::new(TaskRegistry::default()),
            // 🟠 历史读盘失败 ⇒ 从空表开始，**不让启动失败**：历史是"最近发生了什么"，
            //    读不出来不该连累收发。坏文件仍原样留在盘上，可人工排查。
            history: Mutex::new(super::history::History::load(&history_path).unwrap_or_default()),
            history_path,
            cancels: Mutex::new(HashMap::new()),
            pauses: Mutex::new(HashMap::new()),
            task_seq: AtomicU64::new(1),
            peers: Mutex::new(PeerTable::new(self_id, OFFLINE_AFTER)),
            last_refresh_ms: AtomicI64::new(now_unix_ms()),
            active_receives: AtomicUsize::new(0),
        });

        let cancel = CancellationToken::new();
        let live = Arc::new(AtomicUsize::new(0));
        let mut threads = Vec::new();

        // 传输：接受循环（用一个"自连"来唤醒阻塞中的 accept，见模块头）
        {
            let ctx = Arc::clone(&ctx);
            let cancel = cancel.clone();
            let live = Arc::clone(&live);
            threads.push(
                std::thread::Builder::new()
                    .name("aegis-ft-accept".to_owned())
                    .spawn(move || accept_loop(listener, cancel, ctx, live))
                    .map_err(|cause| StartFailure::Transfer {
                        port: local_addr.port(),
                        cause,
                    })?,
            );
        }

        // 发现：宣告 + 监听 + 离线清扫
        {
            let ctx = Arc::clone(&ctx);
            let cancel = cancel.clone();
            let discovery_port = discovery_socket
                .local_addr()
                .map_or(ctx.settings.discovery_port, |addr| addr.port());
            threads.push(
                std::thread::Builder::new()
                    .name("aegis-ft-discovery".to_owned())
                    .spawn(move || discovery_loop(discovery_socket, cancel, ctx))
                    .map_err(|cause| StartFailure::Discovery {
                        port: discovery_port,
                        cause,
                        rollback_ok: true,
                    })?,
            );
        }

        Ok(Self {
            local_addr,
            discovery_port: ctx.settings.discovery_port,
            cancel,
            threads,
            ctx,
            live,
        })
    }

    /// 监听地址（传输端口；界面显示"当前服务运行于 …"）。
    #[must_use]
    pub fn local_addr(&self) -> SocketAddr {
        self.local_addr
    }

    /// 发现端口。
    #[must_use]
    pub fn discovery_port(&self) -> u16 {
        self.discovery_port
    }

    /// 当前在册连接数（诊断用）。
    #[must_use]
    pub fn live_connections(&self) -> usize {
        self.live.load(Ordering::SeqCst)
    }

    /// 当前配对码（惰性生成；界面展示用，F-FT-38）。
    #[must_use]
    pub fn current_pairing_code(&self, now_ms: i64) -> String {
        self.ctx.lock_pairing().current_code(now_ms).to_owned()
    }

    /// 距配对码轮换还剩多少毫秒（界面倒计时）。
    #[must_use]
    pub fn pairing_expires_in_ms(&self, now_ms: i64) -> i64 {
        self.ctx.lock_pairing().expires_in_ms(now_ms)
    }

    /// 在线设备快照（按设备标识排序；F-FT-08 的"被动监听快照"）。
    #[must_use]
    pub fn online_peers(&self) -> Vec<Peer> {
        self.ctx.lock_peers().online()
    }

    /// 最近一次"听到设备"的时刻（F-FT-08 界面上的"最近刷新 …"）。
    #[must_use]
    pub fn last_refresh_ms(&self) -> i64 {
        self.ctx.last_refresh_ms.load(Ordering::SeqCst)
    }

    /// 接收目录（启动时已按设置或默认值定下）。
    #[must_use]
    pub fn receive_root(&self) -> &Path {
        &self.ctx.receive_root
    }

    /// 当前等待用户裁决的接收确认（F-FT-14；命令层轮询给界面渲染弹窗）。
    #[must_use]
    pub fn pending_confirms(&self) -> Vec<confirm::PendingConfirmation> {
        self.ctx.confirm.pending()
    }

    /// 取消一个进行中的任务（F-FT-26）。alse = 已收尾或不存在。
    #[must_use]
    pub fn cancel_task(&self, id: &str) -> bool {
        self.ctx.cancel_task(id)
    }

    /// 🔴 暂停一个任务（F-FT-26）。返回 `false` = **未被受理**（任务已收尾 / 本就暂停）。
    ///
    /// `false` **不是错误**，而是"此刻不接受这个动作"——界面必须据此回一句
    /// "未被受理：任务当前状态为 …"（R-14 原文：不允许按钮"点了没反应"）。
    #[must_use]
    pub fn pause_task(&self, id: &str) -> bool {
        self.ctx
            .pause_gate(id)
            .is_some_and(|gate| gate.pause_local())
    }

    /// 🔴 恢复一个任务（F-FT-26）。返回 `false` = 未被受理（任务已收尾 / 本就未暂停）。
    ///
    /// 🔴 只解除**本端**按下的暂停：对端按的暂停由对端恢复（见 `pause` 模块头）。
    #[must_use]
    pub fn resume_task(&self, id: &str) -> bool {
        self.ctx
            .pause_gate(id)
            .is_some_and(|gate| gate.resume_local())
    }

    /// 某个任务当前的暂停状态（界面据此显示"已暂停"／"已暂停（对端）"，F-FT-25）。
    /// `None` = 该任务不存在或已收尾。
    #[must_use]
    pub fn pause_state_of(&self, id: &str) -> Option<super::pause::PauseState> {
        self.ctx.pause_gate(id).map(|gate| gate.state())
    }

    /// 任务台账快照（F-FT-25；只读，命令层不得改写）。
    #[must_use]
    pub fn tasks_snapshot(&self) -> Vec<super::tasks::TransferTask> {
        self.ctx.lock_tasks().snapshot()
    }

    /// 待决确认 + 距超时判拒的剩余毫秒（界面倒计时用）。
    #[must_use]
    pub fn pending_confirms_with_ttl(&self) -> Vec<(confirm::PendingConfirmation, i64)> {
        self.ctx.confirm.pending_with_ttl()
    }

    /// 传输历史快照（F-FT-54；**只读**，命令层不得改写——快照不可变）。
    ///
    /// 🔴 顺序 = [`super::history::History::newest_first`]（**新的在前**，F-FT-58）：
    /// 界面列表直接照这个顺序渲染，不能图省事用内部存储顺序（那是旧的在前的落盘序）。
    #[must_use]
    pub fn history_snapshot(&self) -> Vec<super::history::HistoryEntry> {
        self.ctx.lock_history().newest_first().cloned().collect()
    }

    /// 导出历史为 CSV（F-FT-56）。`ids = None` 为全量；返回 `(CSV, 条数)`。
    ///
    /// 🔴 界面把**当前筛选后的行**的 id 传进来，导出才与屏幕所见一致（"所见即所得"）。
    #[must_use]
    pub fn history_csv_selected(&self, ids: Option<&[String]>) -> (String, usize) {
        self.ctx.lock_history().to_csv_selected(ids)
    }

    /// 清空传输历史（F-FT-57），返回清掉的条数（0 = 本来就没有）。
    ///
    /// 🔴 只清历史，**不动台账**：进行中的任务不受影响（规格原文）。
    /// 清空是用户明确要求的不可逆动作，故落盘失败**必须报出来**——
    /// 否则内存已清、盘上还在，下次启动历史会"复活"，而用户以为已经清干净了。
    pub fn clear_history(&self) -> usize {
        let mut history = self.ctx.lock_history();
        let removed = history.clear();
        if let Err(error) = history.save(&self.ctx.history_path) {
            eprintln!("[transfer] 历史清空后落盘失败（下次启动可能复活）：{error}");
        }
        removed
    }

    /// 回答一条待决确认。返回 `false` 表示**没有这条待决**（已超时、已被答复、或服务已停）。
    ///
    /// 🔴 不能把 `false` 当成功：它意味着用户在界面上点了"接收"但**接收方线程已经走了**
    /// （超时判拒），此时界面必须如实说"该请求已超时"，而不是显示"已接收"。
    #[must_use]
    pub fn resolve_confirm(&self, id: &str, accept: bool) -> bool {
        self.resolve_confirm_with_policy(id, accept, None)
    }

    /// 发送一个文件，并把它**登记进任务台账**（F-FT-25）。
    ///
    /// 🔴 与自由函数 [`send_to`] 的唯一区别就是"登记"：`send_to` 是纯传输（测试直接用），
    /// 本方法是**界面会看到的那个入口**。分开是为了不让"记台账"污染纯传输路径的测试。
    ///
    /// # Errors
    /// 与 [`send_to`] 同：不是文件、连不上、传输期出错、或对端明确拒收。
    pub fn send_file_tracked(
        &self,
        peer: SocketAddr,
        path: &Path,
        chunk_size: u64,
        pair_code: Option<&str>,
        cancel: &CancellationToken,
    ) -> Result<SendOutcome, SendFailure> {
        let file_name = path.file_name().map_or_else(
            || path.display().to_string(),
            |name| name.to_string_lossy().into_owned(),
        );
        let file_size = std::fs::metadata(path).map_or(0, |meta| meta.len());
        let id = self.ctx.next_task_id();
        self.ctx.lock_tasks().begin(
            id.clone(),
            Direction::Send,
            peer.to_string(),
            // 克隆一份：历史要用的那份留在这里（`begin` 会把它移进台账）
            file_name.clone(),
            file_size,
            super::sink::now_unix_ms(),
        );
        // 登记令牌：命令层要能按 id 取消这个任务（F-FT-26）
        self.ctx.register_cancel(&id, cancel.clone());
        // 🔴 登记暂停闸门：与取消令牌同一条纪律（收尾必删）。
        //    `gate` 交给下面的进度闭包等待，命令层按 id 取到的是**同一个** Arc。
        let gate = self.ctx.register_pause(&id);
        let pause_limit = self.ctx.pause_limit();

        let mut progress = super::tasks::ProgressThrottle::default();
        // 🔴 绝不能把台账锁 held 在**阻塞的网络发送**期间：同进程的接收线程也要锁台账，
        //    那会直接死锁（本批真被测试抓到，33 秒正好是传输 IO 超时）。
        //    改为在闭包里**短暂**加锁——节流后最多 10 次/秒，代价可忽略。
        let outcome = send_to(
            peer,
            path,
            chunk_size,
            pair_code,
            &id,
            cancel,
            // 每块如实报一次，按 100ms 合并后写台账（与接收侧同口径）
            |transferred| {
                // 🔴 F-FT-26：暂停就在**这里**兑现——`on_progress` 是"每块之后"被调一次，
                //    故在此阻塞正好等于"停在块边界"，块内绝不被打断（R-14 原文）。
                if gate.wait_if_paused(pause_limit, cancel) == super::pause::PauseExit::TimedOut {
                    // F-FT-27：本地暂停超上限 ⇒ 自动取消这个任务（原因由收尾处判成 PAUSE_TIMEOUT）
                    cancel.cancel();
                }
                if progress.should_report(super::sink::now_unix_ms()) {
                    self.ctx.lock_tasks().advance(&id, transferred);
                }
            },
        );

        let (state, reason) = if gate.timed_out() {
            // 🔴 F-FT-27：**必须是 PAUSE_TIMEOUT，不是 USER_CANCEL**——
            //    两者走的是同一条取消回收路径，不在这里改写的话，
            //    用户会以为"是我按了取消"，而事实是"放太久自动放弃了"。
            (TaskState::Cancelled, Some(reasons::PAUSE_TIMEOUT))
        } else {
            match &outcome {
                Ok(SendOutcome::Delivered) => (TaskState::Completed, None),
                Ok(SendOutcome::Skipped) => (TaskState::Skipped, Some(reasons::CONFLICT_SKIP)),
                Ok(SendOutcome::HashMismatch) => (TaskState::Failed, Some(reasons::HASH_MISMATCH)),
                // 🔴 R-16：对端明确说的话优先原样回传（含"对端拒收"与"自己取消"）
                Err(SendFailure::Transport(
                    TransportError::Rejected { reason_code }
                    | TransportError::Cancelled { reason_code },
                )) => (TaskState::Cancelled, Some(reason_code.as_str())),
                Err(_) => (TaskState::Failed, Some(reasons::PEER_DISCONNECTED)),
            }
        };
        let finished_at_ms = super::sink::now_unix_ms();
        self.ctx
            .lock_tasks()
            .finish(&id, state, reason, finished_at_ms);
        // 收尾即移除令牌与闸门：不移除这两张表会随会话无限增长
        self.ctx.forget_cancel(&id);
        self.ctx.forget_pause(&id);
        // 记进历史（F-FT-54）：**快照**，之后不再改
        self.ctx.record_history(super::history::HistoryEntry {
            id: id.clone(),
            direction: Direction::Send,
            kind: super::history::HistoryKind::File,
            peer_label: peer.to_string(),
            file_name,
            file_size,
            state,
            reason_code: reason.map(str::to_owned),
            finished_at_ms,
        });
        outcome
    }

    /// 发送一条文本（F-FT-33）。
    ///
    /// 🔴 返回的是**对端的真实交付结论**：电脑通道上 [`transport::TextOutcome::Delivered`] 的含义是
    /// "对方的剪贴板写入成功"——写不进去会回 `CLIPBOARD_WRITE_FAILED`，
    /// **而不是"已送达"**（F-FT-34）。
    ///
    /// 🟠 **已知缺口（须登记，不得当作已完成）**：协议里的 `Text` 帧**不带配对码字段**
    /// （见 `protocol.rs` 的 `Text` 变体），故本路径**只受来源白名单约束，配对闸门覆盖不到**。
    /// 手机通道（片2）落地时若沿用同一帧格式，必须一并补上——否则"配对才能收"这句话
    /// 在文本通道上就不成立。
    ///
    /// # Errors
    /// 连不上对端、会话失败；**业务性拒绝走 `Ok(transport::TextOutcome::Rejected { .. })`**（不是错误）。
    pub fn send_text_tracked(
        &self,
        peer: SocketAddr,
        text: &str,
        cancel: &CancellationToken,
    ) -> Result<transport::TextOutcome, SendFailure> {
        let id = self.ctx.next_task_id();
        // 台账的"文件名"列用**120 字预览**：长文本没有可读的短名，
        // 而把正文塞进台账等于把它塞进每一帧进度刷新里。
        let label = super::text::preview(text);
        self.ctx.lock_tasks().begin(
            id.clone(),
            Direction::Send,
            peer.to_string(),
            label,
            super::text::byte_count(text) as u64,
            super::sink::now_unix_ms(),
        );
        self.ctx.register_cancel(&id, cancel.clone());

        let stream = std::net::TcpStream::connect_timeout(&peer, CONNECT_TIMEOUT)
            .map_err(|cause| SendFailure::Connect { peer, cause })?;
        transport::configure_timeout(&stream, transport::IO_TIMEOUT)
            .map_err(|cause| SendFailure::Connect { peer, cause })?;

        let outcome = transport::send_text(
            &stream,
            text,
            &id,
            Some(self.ctx.identity.machine_name()),
            cancel,
        );
        drop(stream);

        let (state, reason) = match &outcome {
            Ok(transport::TextOutcome::Delivered) => (TaskState::Completed, None),
            // 两个臂体相同⇒合并成 or-pattern：对台账而言，"文本被拒收"与
            // "对端明确回绝"都是"未交付 + 原因码"，没必要分成两种终态。
            Ok(transport::TextOutcome::Rejected { reason_code })
            | Err(TransportError::Rejected { reason_code }) => {
                (TaskState::Failed, Some(reason_code.as_str()))
            }
            Err(TransportError::Cancelled { reason_code }) => {
                (TaskState::Cancelled, Some(reason_code.as_str()))
            }
            Err(_) => (TaskState::Failed, Some(reasons::PEER_DISCONNECTED)),
        };
        let finished_at_ms = super::sink::now_unix_ms();
        self.ctx
            .lock_tasks()
            .finish(&id, state, reason, finished_at_ms);
        self.ctx.forget_cancel(&id);
        self.ctx.record_history(super::history::HistoryEntry {
            id,
            direction: Direction::Send,
            kind: super::history::HistoryKind::Text,
            peer_label: peer.to_string(),
            // 🔴 历史里**只留预览**，绝不留正文（F-FT-35 隐私要求）
            file_name: super::text::preview(text),
            file_size: super::text::byte_count(text) as u64,
            state,
            reason_code: reason.map(str::to_owned),
            finished_at_ms,
        });

        outcome.map_err(SendFailure::Transport)
    }

    /// 同 `Self::resolve_confirm`，但可带上**本次传输**的冲突策略覆盖。
    ///
    /// 🔴 只作用于本次接收，**不写设置**。
    #[must_use]
    pub fn resolve_confirm_with_policy(
        &self,
        id: &str,
        accept: bool,
        policy: Option<ConflictPolicy>,
    ) -> bool {
        self.ctx.confirm.resolve_with_policy(
            id,
            if accept {
                confirm::Decision::Accept
            } else {
                confirm::Decision::Reject
            },
            policy,
        )
    }

    #[allow(dead_code)]
    fn __old_resolve_confirm(&self, id: &str, accept: bool) -> bool {
        self.ctx.confirm.resolve(
            id,
            if accept {
                confirm::Decision::Accept
            } else {
                confirm::Decision::Reject
            },
        )
    }

    /// 停止服务。
    ///
    /// 🔴 返回值**必须**被调用方用起来：R-28 要求"停止失败不得宣称已停止"，
    /// 把这句话做成结构上的事实（[`StopReport::all_stopped`]）比靠自觉可靠。
    #[must_use]
    pub fn stop(self) -> StopReport {
        self.cancel.cancel();
        // 🔴 先唤醒所有等待用户确认的接收线程：不唤醒的话它们会卡到 30 秒超时，
        //    stop() 的 join 于是看起来"停不下来"。
        self.ctx.confirm.cancel_all();
        // 用一个"自连"唤醒阻塞在 `accept` 上的线程（无需 sleep，也不违反 sleep 禁令）。
        // 🔴 必须连**回环地址**：`local_addr` 是 `0.0.0.0:port`，连它是无效目标。
        let wake = SocketAddr::from(([127, 0, 0, 1], self.local_addr.port()));
        let _ = TcpStream::connect_timeout(&wake, Duration::from_millis(500));

        let mut threads_joined = 0;
        let mut all_stopped = true;
        for handle in self.threads {
            if handle.join().is_ok() {
                threads_joined += 1;
            } else {
                // 线程 panic 了 ⇒ 不能宣称停住了
                all_stopped = false;
            }
        }
        StopReport {
            threads_joined,
            all_stopped,
        }
    }
}

/// 在册连接的自动减一守卫（线程退出即减，panic 也减）。
struct LiveGuard(Arc<AtomicUsize>);

impl Drop for LiveGuard {
    fn drop(&mut self) {
        self.0.fetch_sub(1, Ordering::SeqCst);
    }
}

/// 接受循环。
///
/// 🔴 取消靠"自连唤醒"：`incoming()` 阻塞时无法检查令牌，故 `stop()` 会连一次本机地址，
/// 让 `accept` 返回一个立刻可丢弃的连接，循环随即看到令牌并退出。
// 四个参数按值传入是**刻意的所有权交接**：它们都会被移进/借给线程内新起的连接线程，
// 按值传递让"这个入口接管了这些资源"在签名上就成立（clippy 的 `needless_pass_by_value`
// 只看函数体是否消费，看不到线程入口的语义）。
#[allow(clippy::needless_pass_by_value)]
fn accept_loop(
    listener: TcpListener,
    cancel: CancellationToken,
    ctx: Arc<ServiceContext>,
    live: Arc<AtomicUsize>,
) {
    for incoming in listener.incoming() {
        if cancel.is_cancelled() {
            break;
        }
        let Ok(stream) = incoming else {
            // 单次 accept 失败（如被中断）不该让整个服务退出
            continue;
        };
        if live.load(Ordering::SeqCst) >= CONNECTION_HARD_CAP {
            // 硬上限：回一条可读原因再关（而不是静默丢弃让对端干等）
            let mut sink_stream = &stream;
            let _ = transport::write_frame(
                &mut sink_stream,
                &TransferMessage::Error {
                    task_id: None,
                    reason_code: reasons::CONCURRENCY_LIMIT.to_owned(),
                    message: None,
                },
                &[],
            );
            continue;
        }
        live.fetch_add(1, Ordering::SeqCst);
        let ctx = Arc::clone(&ctx);
        let cancel = cancel.clone();
        let live = Arc::clone(&live);
        // 🔴 每连接一个专用线程（见 transport.rs 模块头对"阻塞 vs async"的论证）；
        //    线程数上界 = 硬上限，且每个连接有读超时兜底
        let _ = std::thread::Builder::new()
            .name("aegis-ft-conn".to_owned())
            .spawn(move || {
                let _guard = LiveGuard(live);
                let _ = handle_connection(&stream, &ctx, &cancel);
            });
    }
}

/// 一个接收会话（含准入裁决与落盘）。
// 🔴 允许超长，但**这是一笔欠账而不是理想状态**：本函数是一条连接的线性生命周期
// （配超时 → 取对端 → 准入 → 接收确认 → 登记台账 → 收发 → 收尾），文本通道接进来后
// 又长了两段。实测把文本的准入与交付都抽出去仍有 121 行（>100），故本轮不做半吊子拆分；
// 正确的做法是按"文件事务 / 文本事务"拆两个函数，那需要一并搬走 task_id 槽与进度节流器。
#[allow(clippy::too_many_lines)]
fn handle_connection(
    stream: &TcpStream,
    ctx: &ServiceContext,
    cancel: &CancellationToken,
) -> Result<transport::AnyOutcome, TransportError> {
    transport::configure_timeout(stream, transport::IO_TIMEOUT)?;
    let peer = stream.peer_addr()?;
    let peer_token = peer_token_for(peer.ip());
    ctx.active_receives.fetch_add(1, Ordering::SeqCst);
    let _guard = ActiveReceiveGuard(&ctx.active_receives);

    // 磁盘预检（R-12）：🔴 必须在**拿到握手事实之后**判——R-12 的口径是
    // "完整文件大小 + 128 MB 安全余量"，握手之前根本不知道要留多少空间。
    // 探测与判档都在闭包里做，`disk` 只作入参传给准入闸门。
    //
    // 任务 id 的回传槽：准入闭包在握手后登记任务并写入，**进度闭包**在每块之后读它。
    // 🔴 用 `RefCell` 而不是 `Cell`：进度闭包要按引用读（`Cell` 只能整体 take，
    //    那会把 id 从槽里拿走，后续的块就没得读了）。同一线程，无跨线程顾虑。
    let task_id_slot: std::cell::RefCell<Option<String>> = std::cell::RefCell::new(None);
    // F-FT-27：本地暂停上限（设置 0 = 不限 ⇒ None）。一次会话内取一次即可。
    let pause_limit = ctx.pause_limit();
    // 本次接收的进度节流器（01 分册 §3.1：建议 ≥100ms 合并一次）
    let mut progress = super::tasks::ProgressThrottle::default();
    // 本次接收的冲突策略：初值取设置，但准入闭包可在**用户裁决之后**就地覆盖它
    // （弹窗里的「同名文件」三个选项只作用于本次，不写设置）
    let mut policy = ctx.settings.conflict_policy;
    let outcome = transport::receive_any(
        stream,
        &ctx.receive_root,
        &peer_token,
        &mut policy,
        |facts, policy| {
            let available = infra::fs::available_bytes(&ctx.receive_root).ok();
            let disk = admission::classify_disk_space(available, facts.file_size);
            let verdict = ctx.admit(peer.ip(), facts, disk);
            // 🔴 R-12 明写"无法判定则放行**但记录**"——这句话的落点就是这里。
            // 之前 `probe_disk` 恒返回 Unknown 且无人记录，等于放行而不留痕。
            if let Admission::Admitted { disk_unknown: true } = verdict {
                tracing::warn!(
                    module = contracts::LOG_MODULE_TRANSFER,
                    action = "transferReceiveAdmit",
                    result = "Success",
                    exception = "disk-space-unknown",
                    file_size = facts.file_size,
                    "磁盘剩余空间无法判定，按 R-12 放行（disk_unknown）"
                );
            }
            // 🔴 F-FT-18「接收前询问」默认**开**（Q-15 用户裁决）。
            // 位置在**闸门全过之后、建任何文件之前**——正是 R-13 顺序里"建未完成文件"的前一步。
            // R-8：拒绝 / 关闭 / Esc / **超时**四条路径全部按拒绝（超时由 `ConfirmGate` 判）。
            if matches!(verdict, Admission::Admitted { .. })
                && ctx.settings.require_receive_confirmation
            {
                let id = format!("{}|{}", peer.ip(), facts.file_name);
                ctx.confirm.ask(confirm::PendingConfirmation {
                    id: id.clone(),
                    kind: confirm::PendingKind::File,
                    peer_label: peer.to_string(),
                    file_name: facts.file_name.to_owned(),
                    file_size: facts.file_size,
                    save_directory: ctx.receive_root.display().to_string(),
                    available_bytes: available,
                    // F-FT-14：**仅当存在同名文件时**才出现冲突区
                    name_conflict: infra::safe::join_under_root(&ctx.receive_root, facts.file_name)
                        .is_ok_and(|path| path.exists()),
                });
                let (decision, chosen_policy) = ctx.confirm.await_decision(&id);
                if decision == confirm::Decision::Reject {
                    return Admission::Rejected {
                        gate: admission::Gate::Confirm,
                        reason: reasons::USER_REJECT,
                    };
                }
                // 用户在这次弹窗里选了别的冲突策略：就地覆盖（只作用本次）
                if let Some(chosen) = chosen_policy {
                    *policy = chosen;
                }
            }
            // 🔴 登记任务（F-FT-25）：位置在**闸门与确认全过之后**——
            // 被拒的请求不该在台账里留一行"进行中"，那会让并发槽位与界面都失真。
            if matches!(verdict, Admission::Admitted { .. }) {
                let id = ctx.next_task_id();
                ctx.lock_tasks().begin(
                    id.clone(),
                    Direction::Receive,
                    peer.to_string(),
                    facts.file_name.to_owned(),
                    facts.file_size,
                    super::sink::now_unix_ms(),
                );
                ctx.register_cancel(&id, cancel.clone());
                // 🔴 暂停闸门同样按 id 登记（F-FT-26）：命令层按 id 拿到的是**同一个** Arc，
                //    下面的进度闭包用它等待 ⇒ "暂停"才可能生效。
                //    与令牌同一条纪律：收尾必删（见 `forget_pause`）。
                ctx.register_pause(&id);
                task_id_slot.replace(Some(id));
            }
            verdict
        },
        // ── 文本通道的准入（F-FT-33）──
        //
        // 🔴 文本**不适用**文件那些闸门：没有分块大小、没有文件大小上限、也不占磁盘。
        //    适用的是：**来源白名单** + **接收前询问**（Q-15 默认开）。
        //
        // 🟠 **已知缺口**：`Text` 帧**没有配对码字段** ⇒ 配对闸门覆盖不到文本。
        //    这不是这里能补的（要改帧格式），已在 `send_text_tracked` 的文档里登记。
        |facts: &transport::TextFacts<'_>| {
            if ctx.settings.require_known_peer && !ctx.is_known_peer(peer.ip()) {
                return Admission::Rejected {
                    gate: admission::Gate::Whitelist,
                    reason: reasons::PEER_NOT_DISCOVERED,
                };
            }
            if ctx.settings.require_receive_confirmation {
                let id = format!("{}|text|{}", peer.ip(), facts.task_id);
                // 🔴 准入阶段看不到正文（正文在 body 里、由文本事务持有），
                //    故这里只给"这是文本 + 多大"，**不假装**有预览；
                //    120 字预览在 commit 闭包里生成（那里才有正文）。
                let label = format!("文本消息（{} 字节）", facts.bytes);
                ctx.confirm.ask(confirm::PendingConfirmation {
                    id: id.clone(),
                    kind: confirm::PendingKind::Text,
                    peer_label: peer.to_string(),
                    file_name: label,
                    file_size: facts.bytes,
                    // 文本不落盘：目录留空，弹窗按既有规则渲染成"—"
                    save_directory: String::new(),
                    // 文本与磁盘无关：不显示剩余空间（弹窗的磁盘行待按文本隐藏）
                    available_bytes: None,
                    name_conflict: false,
                });
                let (decision, _policy) = ctx.confirm.await_decision(&id);
                if decision == confirm::Decision::Reject {
                    return Admission::Rejected {
                        gate: admission::Gate::Confirm,
                        reason: reasons::USER_REJECT,
                    };
                }
            }
            Admission::Admitted {
                disk_unknown: false,
            }
        },
        // ── 文本的"落交付"：写剪贴板（F-FT-34）──
        // 🔴 失败**必须**变成 CLIPBOARD_WRITE_FAILED 回给发送方，
        //    而不是"已送达"——这是本通道唯一能让发送方察觉真相的地方。
        |text: &str| {
            let result = infra::clipboard::set_text(text).map_err(|error| error.to_string());
            // 文本是一次性事务：台账与历史在这里**一次写齐**（没有进度可报，
            // 故不留"进行中"的行；与文件收尾口径一致，只是没有中间态）。
            let state = if result.is_ok() {
                TaskState::Completed
            } else {
                TaskState::Failed
            };
            let reason = result
                .as_ref()
                .err()
                .map(|_| reasons::CLIPBOARD_WRITE_FAILED);
            let finished_at_ms = super::sink::now_unix_ms();
            let id = ctx.next_task_id();
            let preview = super::text::preview(text);
            ctx.lock_tasks().begin(
                id.clone(),
                Direction::Receive,
                peer.to_string(),
                preview.clone(),
                super::text::byte_count(text) as u64,
                finished_at_ms,
            );
            ctx.lock_tasks().finish(&id, state, reason, finished_at_ms);
            // 🔴 历史里**只留预览**，绝不留正文（F-FT-35 隐私要求）
            ctx.record_history(super::history::HistoryEntry {
                id,
                direction: Direction::Receive,
                kind: super::history::HistoryKind::Text,
                peer_label: peer.to_string(),
                file_name: preview,
                file_size: super::text::byte_count(text) as u64,
                state,
                reason_code: reason.map(str::to_owned),
                finished_at_ms,
            });
            result
        },
        cancel,
        // 分块进度：🔴 每块如实报一次，但**按 100ms 合并**后才写台账——
        // 2 MiB 的块在千兆网上每秒几十次，逐次写会既费锁又让界面抖。
        |transferred| {
            // 🔴 F-FT-26：暂停在**这里**兑现——`on_progress` 是"每块之后"被调一次，
            //    在此阻塞正好等于"停在块边界"，块内绝不被打断（R-14 原文）。
            //
            // 🔴 这段必须在**节流判断之前**：若放在 `should_report` 之后，
            //    被节流吞掉的那些块就不会检查暂停 ⇒ 暂停生效时刻变得不可预期。
            //
            // 🔴 也不能让 `borrow()` 的守卫跨过阻塞点：那会把 RefCell 借走整个暂停时长
            //    （准入闭包还要 borrow_mut 去写回任务 id），故先 clone 成局部变量。
            let current_id = task_id_slot.borrow().clone();
            if let Some(id) = &current_id {
                if let Some(gate) = ctx.pause_gate(id) {
                    if gate.wait_if_paused(pause_limit, cancel) == super::pause::PauseExit::TimedOut
                    {
                        // F-FT-27：本地暂停超上限 ⇒ 自动取消（原因由收尾处判成 PAUSE_TIMEOUT）
                        cancel.cancel();
                    }
                }
            }
            if !progress.should_report(super::sink::now_unix_ms()) {
                return;
            }
            if let Some(id) = &current_id {
                ctx.lock_tasks().advance(id, transferred);
            }
        },
    );

    // ── 收尾：把**实际结果**写成终态（🔴 R-16：写进去就不再改） ──
    // 进度已在过程中按 100ms 合并上报；此处只收尾，并让最后一帧不被节流吞掉。
    progress.force_next();
    if let Some(id) = task_id_slot.borrow_mut().take() {
        // 文本分支的台账与历史在 commit 闭包里已经写齐（那里才有正文），
        // 故这里只可能是文件路径；`task_id_slot` 非空本身就说明登记的是文件任务。
        //
        // 🔴 F-FT-27：本地暂停超上限 ⇒ 终态必须记 PAUSE_TIMEOUT 而**不是** USER_CANCEL。
        //    两者走的是同一条"取消"回收路径，只能靠闸门留下的痕迹来分辨；
        //    不分辩的话，用户会以为"是我按了取消"，而事实是"放太久自动放弃了"。
        let paused_too_long = ctx.pause_gate(&id).is_some_and(|gate| gate.timed_out());
        let (state, reason) = if paused_too_long {
            (
                TaskState::Cancelled,
                Some(reasons::PAUSE_TIMEOUT.to_owned()),
            )
        } else {
            match &outcome {
                Ok(transport::AnyOutcome::File(file)) => receive_terminal_state(file),
                Err(TransportError::Cancelled { reason_code }) => {
                    (TaskState::Cancelled, Some(reason_code.clone()))
                }
                _ => (
                    TaskState::Failed,
                    Some(reasons::PEER_DISCONNECTED.to_owned()),
                ),
            }
        };
        let finished_at_ms = super::sink::now_unix_ms();
        // 🔴 先从**台账**取一份快照：历史要的是当时的事实（文件名/大小/对端），
        //    而它们在准入闭包里，此处已出作用域。台账正好持有它们。
        let snapshot = ctx.lock_tasks().get(&id).cloned();
        ctx.lock_tasks()
            .finish(&id, state, reason.as_deref(), finished_at_ms);
        // 收尾即移除令牌与闸门：不移除这两张表会随会话无限增长
        ctx.forget_cancel(&id);
        ctx.forget_pause(&id);
        if let Some(task) = snapshot {
            ctx.record_history(super::history::HistoryEntry {
                id,
                direction: Direction::Receive,
                kind: super::history::HistoryKind::File,
                peer_label: task.peer_label,
                file_name: task.file_name,
                file_size: task.file_size,
                state,
                reason_code: reason,
                finished_at_ms,
            });
        }
    }

    // `receive_any` 已经直接给出 `AnyOutcome`（文件或文本），无需再包一层。
    outcome
}

/// 接收会话计数守卫。
struct ActiveReceiveGuard<'a>(&'a AtomicUsize);

impl Drop for ActiveReceiveGuard<'_> {
    fn drop(&mut self) {
        self.0.fetch_sub(1, Ordering::SeqCst);
    }
}

/// 未完成文件的按对端隔离令牌（用 IP 派生，稳定且不含端口——客户端端口是临时的）。
#[must_use]
pub fn peer_token_for(ip: IpAddr) -> String {
    match ip {
        IpAddr::V4(v4) => format!("v4{:08x}", u32::from(v4)),
        IpAddr::V6(v6) => {
            // 取末 32 位即可满足"同一局域网内区分对端"的用途（无需可逆，只需稳定）
            let octets = v6.octets();
            let tail = u32::from_be_bytes([octets[12], octets[13], octets[14], octets[15]]);
            format!("v6{tail:08x}")
        }
    }
}
/// 发现循环：定时宣告 + 监听别家宣告 + 清扫离线。
// 同 `accept_loop`：按值接收套接字是刻意的所有权交接（线程活多久它就活多久）。
#[allow(clippy::needless_pass_by_value)]
fn discovery_loop(socket: UdpSocket, cancel: CancellationToken, ctx: Arc<ServiceContext>) {
    if socket.set_read_timeout(Some(DISCOVERY_POLL)).is_err() {
        // 设不上超时会让"检查取消令牌"永不发生 ⇒ 线程无法停止。如实放弃本线程，
        // 由 `stop()` 的 join 结果暴露（不静默假装在跑）。
        return;
    }
    let _ = socket.set_broadcast(true);
    let broadcast = SocketAddr::from(([255, 255, 255, 255], ctx.settings.discovery_port));
    let announce_ms = i64::try_from(ANNOUNCE_INTERVAL.as_millis()).unwrap_or(3_000);
    let mut buffer = vec![0_u8; discovery::MAX_DATAGRAM_BYTES];
    let mut last_announce_ms = i64::MIN;

    while !cancel.is_cancelled() {
        let now = now_unix_ms();
        if now.saturating_sub(last_announce_ms) >= announce_ms {
            let announcement: Announcement =
                ctx.identity.announcement(ctx.settings.transfer_port, now);
            if let Ok(bytes) = discovery::encode(&announcement) {
                // 发不出去（无网卡/被隔离）不算致命：继续监听即可
                let _ = socket.send_to(&bytes, broadcast);
            }
            last_announce_ms = now;
        }

        match socket.recv_from(&mut buffer) {
            Ok((read, from)) => {
                if let Ok(announcement) = discovery::decode(&buffer[..read]) {
                    let event = ctx.lock_peers().observe(&announcement, from, now);
                    if !matches!(event, discovery::PeerEvent::SelfAnnouncement) {
                        ctx.last_refresh_ms.store(now, Ordering::SeqCst);
                    }
                }
            }
            Err(error)
                if error.kind() == io::ErrorKind::WouldBlock
                    || error.kind() == io::ErrorKind::TimedOut =>
            {
                let removed = ctx.lock_peers().sweep(now);
                if !removed.is_empty() {
                    ctx.last_refresh_ms.store(now, Ordering::SeqCst);
                }
            }
            Err(_) => {
                // 其它错误（如某个坏数据报触发的 ICMP 回执）：不因一个包退出循环。
                // 该 match 已是循环体末尾，`continue` 是多余的（clippy 也这么说）
            }
        }
    }
}

/// 发送侧失败。
#[derive(Debug)]
pub enum SendFailure {
    /// 连不上对端（拒绝/超时）
    Connect { peer: SocketAddr, cause: io::Error },
    /// 读盘或算摘要失败
    Source(super::source::SourceError),
    /// 会话失败
    Transport(TransportError),
    /// 路径不是文件
    NotAFile,
}

impl std::fmt::Display for SendFailure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Connect { peer, cause } => write!(f, "无法连接到 {peer}：{cause}"),
            Self::Source(error) => write!(f, "{error}"),
            Self::Transport(error) => write!(f, "{error}"),
            Self::NotAFile => f.write_str("选择的不是文件"),
        }
    }
}

impl std::error::Error for SendFailure {}

/// 发送方入口：连上对端并发一个文件（F-FT-11/12 的"入队发送"最终落到这里）。
///
/// 流程：连接 → 算全文件摘要（握手要用）→ 打开读盘器 → 走 [`transport::send_file`]。
///
/// # Errors
/// 见 [`SendFailure`]。对端拒收（含配对失败）会以 [`TransportError::Rejected`] 出现在
/// [`SendFailure::Transport`] 里，其 `reason_code` 可直接用于界面原因码。
pub fn send_to(
    peer: SocketAddr,
    path: &Path,
    chunk_size: u64,
    pair_code: Option<&str>,
    task_id: &str,
    cancel: &CancellationToken,
    // 每块发成功后的进度回调（累计字节）。🔴 节流由调用方负责。
    on_progress: impl FnMut(u64),
) -> Result<SendOutcome, SendFailure> {
    if !path.is_file() {
        return Err(SendFailure::NotAFile);
    }
    let stream = TcpStream::connect_timeout(&peer, CONNECT_TIMEOUT)
        .map_err(|cause| SendFailure::Connect { peer, cause })?;
    transport::configure_timeout(&stream, transport::IO_TIMEOUT)
        .map_err(|cause| SendFailure::Connect { peer, cause })?;

    let file_hash = source::hash_file(path).map_err(SendFailure::Source)?;
    let modified_at_ms = std::fs::metadata(path)
        .ok()
        .and_then(|meta| meta.modified().ok())
        .and_then(|time| time.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|elapsed| i64::try_from(elapsed.as_millis()).unwrap_or(0));

    let mut send_source =
        source::SendSource::open(path, chunk_size).map_err(SendFailure::Source)?;
    transport::send_file(
        &stream,
        &mut send_source,
        &file_hash,
        task_id,
        modified_at_ms,
        pair_code,
        cancel,
        on_progress,
    )
    .map_err(SendFailure::Transport)
}

/// 接收结果 → 台账终态（F-FT-30 五态；🔴 R-16：写进去就不再改）。
///
/// 抽成**纯函数**有两个好处：`handle_connection` 不再过长（clippy `too_many_lines`），
/// 且这条映射表可以被单测穷举——它是"界面最终看到什么"的**唯一决定处**。
///
/// 🔴 对端明确说的话一律**原样回传**（R-16："以对端回传的原因码为准"）：
/// `Rejected` 与 `Cancelled` 都带自己的 `reason_code`，不要替换成我们自己的措辞。
#[must_use]
fn receive_terminal_state(outcome: &ReceiveOutcome) -> (TaskState, Option<String>) {
    match outcome {
        ReceiveOutcome::Committed { .. } => (TaskState::Completed, None),
        ReceiveOutcome::Skipped => (TaskState::Skipped, Some(reasons::CONFLICT_SKIP.to_owned())),
        ReceiveOutcome::HashMismatch => {
            (TaskState::Failed, Some(reasons::HASH_MISMATCH.to_owned()))
        }
        // 对端明确说的话一律原样回传（R-16）：拒收与取消都带着自己的原因码。
        // 传输层错误（含 `Cancelled`）由调用方判——它才拿得到那个 `Result`。
        ReceiveOutcome::Rejected { reason_code } => {
            (TaskState::Cancelled, Some(reason_code.clone()))
        }
    }
}

/// 让调用方知道"是否需要在启动后把令牌写回设置"。
impl TransferService {
    /// 本机设备标识的令牌（可能本次新生成；R-6 要求写回设置以保证重启后稳定）。
    #[must_use]
    pub fn device_token(&self) -> &str {
        self.ctx.identity.token()
    }

    /// 本机设备标识（机器名 + 令牌）。
    #[must_use]
    pub fn device_id(&self) -> String {
        self.ctx.identity.device_id()
    }

    /// 接收并发上限（界面提示用；取自 `admission`，只有一处定义）。
    #[must_use]
    pub fn max_receive_concurrency() -> usize {
        usize::try_from(MAX_RECEIVE_CONCURRENCY).unwrap_or(8)
    }

    /// 冲突策略（界面显示当前值用）。
    #[must_use]
    pub fn conflict_policy(&self) -> ConflictPolicy {
        self.ctx.settings.conflict_policy
    }
}

#[cfg(test)]
mod tests;
