//! 帧收发与会话编排（F-FT-13/19/21/29；阻塞 IO + 取消令牌）。
//!
//! ## 为什么是阻塞 IO 而不是 async
//!
//! 本工程要求"禁止阻塞 tokio 运行时"（`03-开发规范/01-编码与命名规范.md` §3.1），
//! 而那条规则**自己给了出口**：
//!
//! > `async fn` 内直接 `std::fs::*`… | 一次性/短任务：`spawn_blocking`；
//! > **长任务：放 `crates/worker` 独立进程/独立线程**
//!
//! 且"长任务"清单里**点名包含"大文件传输"**。本通道同时满足：分块读写是阻塞文件 IO、
//! SHA-256 是 CPU 阻塞、并发上限只有发送 3 + 接收 8（至多 11 个线程，可忽略）。
//! 故采**每连接一个专用线程 + 阻塞 socket**：既避开阻塞运行时的红线，
//! 又不必把已经测得比较透的 `source`/`sink`（同步）改写成 async。
//!
//! 规范真正强制的是**取消令牌贯通**，本模块照办：[`CancellationToken`] 在每个可能
//! 长阻塞的位置被检查，取消时**主动发 `Cancel` 告知对端**（不让对端干等）。
//!
//! ## 中断与"要么完整、要么不存在"
//!
//! * 摘要不符 / 对端跳过 → `sink::finish` 自己会删掉未完成文件（F-FT-20）
//! * **取消 / 断连 → 未完成文件保留**（这正是 R-21「7 天清扫未完成文件」存在的前提：
//!   中断后 `.part` 留着，下次可续传）。要彻底放弃请显式调 `sink::discard`
//!
//! ## 本模块不做的事
//!
//! * **不写日志**：本模块是纯协议机制，只返回**可区分的**结果/错误；
//!   八字段 JSONL 由命令层按结果落（"禁止静默失败"的落点在调用方，不是这里）。
//!   🔴 调用方**必须**对每个 `Err` 与每个非成功 `Outcome` 记一条。
//! * **不做准入裁决**：接收侧的裁决由调用方以 `admit` 闭包注入
//!   （白名单/配对/并发/磁盘都在它里面，见 `admission::evaluate`）。
//! * **不做暂停闸门**（F-FT-26 接收侧发起 + F-FT-27 超时自动取消）：本轮只实现
//!   **发送侧遵守**对端发来的 `Pause`/`Resume`；接收侧发起暂停需会话状态机，留下批。

use std::io::{Read, Write};
use std::net::TcpStream;
use std::path::{Path, PathBuf};
use std::time::Duration;

use tokio_util::sync::CancellationToken;

use super::admission::Admission;
use super::conflict::ConflictPolicy;
use super::protocol::{self, TransferMessage};
use super::sink::{now_unix_ms, FinishOutcome, ReceiveSink, SinkError};
use super::source::{SendSource, SourceError};

/// 单次 `read` 的超时（不是"整帧"的超时）。
///
/// 🔴 没有它，一个"连上就不发数据的对端"会**永久占住一个线程**；
/// 有了它，死对端最多占用 [`IO_TIMEOUT`]。
/// 取值宽松是因为它约束的是"两次读到数据之间的间隔"，慢速局域网下 2 MiB 分块也远用不到 30 秒。
pub const IO_TIMEOUT: Duration = Duration::from_secs(30);

/// 一次会话的失败原因。
///
/// 🔴 各变体**可区分**是为了让命令层能落出精确的八字段日志与原因码，
/// 而不是只知道"传输失败"。
#[derive(Debug)]
pub enum TransportError {
    Io(std::io::Error),
    /// 单次读超时（对端长时间不发数据）
    TimedOut,
    /// 帧层错误（魔数/版本/长度/头解析）
    Frame(protocol::FrameError),
    /// 落盘错误
    Sink(SinkError),
    /// 读盘/摘要错误
    Source(SourceError),
    /// 对端在握手里拒收
    Rejected {
        reason_code: String,
    },
    /// 对端报告的终态失败（收尾回执 `ok=false`）
    RemoteFailed {
        reason_code: Option<String>,
    },
    /// 对端在当前阶段发了不该发的消息，或**自相矛盾**（如两次声明的摘要不一致）
    Unexpected(&'static str),
    /// 收到对端的取消
    Cancelled {
        reason_code: String,
    },
    /// 本机取消令牌被触发
    Aborted,
    /// 会话未结束时对端关闭了连接
    PeerClosed,
}

impl std::fmt::Display for TransportError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io(error) => write!(f, "网络 IO 失败：{error}"),
            Self::TimedOut => f.write_str("对端长时间无数据（读超时）"),
            Self::Frame(error) => write!(f, "帧解析失败：{error}"),
            Self::Sink(error) => write!(f, "落盘失败：{error}"),
            Self::Source(error) => write!(f, "读盘失败：{error}"),
            Self::Rejected { reason_code } => {
                write!(f, "对端拒收（{reason_code}）")
            }
            Self::RemoteFailed { reason_code } => match reason_code {
                Some(code) => write!(f, "对端报告失败（{code}）"),
                None => f.write_str("对端报告失败（未给原因码）"),
            },
            Self::Unexpected(what) => write!(f, "对端违反会话顺序：{what}"),
            Self::Cancelled { reason_code } => write!(f, "对端取消（{reason_code}）"),
            Self::Aborted => f.write_str("本机取消"),
            Self::PeerClosed => f.write_str("对端在会话结束前关闭了连接"),
        }
    }
}

impl std::error::Error for TransportError {}

impl From<std::io::Error> for TransportError {
    fn from(error: std::io::Error) -> Self {
        if error.kind() == std::io::ErrorKind::WouldBlock
            || error.kind() == std::io::ErrorKind::TimedOut
        {
            Self::TimedOut
        } else {
            Self::Io(error)
        }
    }
}

impl From<protocol::FrameError> for TransportError {
    fn from(error: protocol::FrameError) -> Self {
        Self::Frame(error)
    }
}

impl From<SinkError> for TransportError {
    fn from(error: SinkError) -> Self {
        Self::Sink(error)
    }
}

impl From<SourceError> for TransportError {
    fn from(error: SourceError) -> Self {
        Self::Source(error)
    }
}

/// 发送侧结论。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SendOutcome {
    /// 对端已完整收下并通过校验
    Delivered,
    /// 对端按冲突策略跳过（终态是「已跳过」而非「已完成」）
    Skipped,
    /// 对端收尾时报摘要不符
    HashMismatch,
}

/// 接收侧结论。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReceiveOutcome {
    /// 已入位（含因撞名改名）
    Committed { path: PathBuf, renamed: bool },
    /// 按策略跳过（未写入新文件）
    Skipped,
    /// 摘要不符，损坏的未完成文件已删除
    HashMismatch,
    /// 已明确拒收（原因码见 `reason_code`），未创建任何文件
    Rejected { reason_code: String },
}

/// 等**握手回执**的上限。
///
/// 🔴 它必须**严格大于**接收端的"接收前询问"窗口（[`super::confirm::CONFIRM_TIMEOUT`]）。
/// 理由是结构性的而不是拍脑袋的：接收端的 `HandshakeAck` 是在**用户裁决之后**才发出的，
/// 所以发送方这一侧的等待要覆盖「接收端问用户（30 秒）+ 之后真正收发（常规 IO 超时）」。
/// 若沿用 [`IO_TIMEOUT`]（同为 30 秒），开启确认时发送方会几乎必然先超时——
/// 真机端到端跑出来过：发送方报 `PEER_DISCONNECTED（对端长时间无数据）`，
/// 而接收端的确认弹窗刚刚弹出，**两端结论不一致**。
pub const HANDSHAKE_WAIT: Duration =
    Duration::from_secs(super::confirm::CONFIRM_TIMEOUT.as_secs() + IO_TIMEOUT.as_secs());

/// 给流设读超时（对 `TcpStream` 生效；其它 `Read` 实现忽略失败）。///
/// # Errors
/// 设置失败时返回底层错误——不静默忽略，否则"防死对端"的保证就成了空话。
pub fn configure_timeout(stream: &TcpStream, timeout: Duration) -> std::io::Result<()> {
    stream.set_read_timeout(Some(timeout))?;
    stream.set_write_timeout(Some(timeout))?;
    Ok(())
}

/// 写一帧（`header` 与 `body` 连写，避免只写出一半）。
///
/// # Errors
/// 编码失败或底层写失败时返回 [`TransportError`]。
pub fn write_frame<W: Write>(
    writer: &mut W,
    message: &TransferMessage,
    body: &[u8],
) -> Result<(), TransportError> {
    let frame = protocol::encode_frame(message, body)?;
    writer.write_all(&frame)?;
    // 立刻刷出：否则对端可能长时间收不到握手（TCP 会攒包）
    writer.flush()?;
    Ok(())
}

/// 帧读取器：内部持缓冲，处理"一次读到的字节不到一帧"的情况。
#[derive(Debug)]
pub struct FrameReader<R: Read> {
    inner: R,
    buffer: Vec<u8>,
}

impl<R: Read> FrameReader<R> {
    pub fn new(inner: R) -> Self {
        Self {
            inner,
            buffer: Vec::with_capacity(protocol::PREFIX_LEN + 512),
        }
    }

    /// 读满一帧；返回消息与 body（body 已从缓冲中摘除）。
    ///
    /// # Errors
    /// 帧错误、IO 错误、读超时、或会话未结束即遇 EOF 时返回 [`TransportError`]。
    pub fn read_frame(&mut self) -> Result<(TransferMessage, Vec<u8>), TransportError> {
        loop {
            match protocol::parse_head(&self.buffer)? {
                Some((head, body_offset)) => {
                    let need = body_offset + head.body_len as usize;
                    self.fill_to(need)?;
                    let body = self.buffer[body_offset..need].to_vec();
                    self.buffer.drain(..need);
                    return Ok((head.message, body));
                }
                None => self.fill_more()?,
            }
        }
    }

    /// 再读一块；EOF 即 [`TransportError::PeerClosed`]。
    ///
    /// 🔴 **没有"读到 0 字节就正常结束"这条路**：帧边界上遇到 EOF 一定是会话被截断，
    /// 而不是"对端说完了"——协议靠 `Complete`/`Cancel` 显式收尾，不靠关闭连接。
    fn fill_more(&mut self) -> Result<(), TransportError> {
        let mut chunk = [0_u8; 16 * 1024];
        let read = self.inner.read(&mut chunk)?;
        if read == 0 {
            return Err(TransportError::PeerClosed);
        }
        self.buffer.extend_from_slice(&chunk[..read]);
        Ok(())
    }

    /// 读到至少 `need` 字节。
    fn fill_to(&mut self, need: usize) -> Result<(), TransportError> {
        while self.buffer.len() < need {
            self.fill_more()?;
        }
        Ok(())
    }
}

/// 检查取消令牌；被触发即向对端发一条 `Cancel` 再返回。
///
/// 🔴 取消时**主动告知对端**：否则对端会一直等在那里直到它自己的读超时。
fn check_cancel<W: Write>(
    cancel: &CancellationToken,
    writer: &mut W,
    task_id: &str,
    reason_code: &str,
) -> Result<(), TransportError> {
    if cancel.is_cancelled() {
        let _ = write_frame(
            writer,
            &TransferMessage::Cancel {
                task_id: task_id.to_owned(),
                reason_code: reason_code.to_owned(),
            },
            &[],
        );
        return Err(TransportError::Aborted);
    }
    Ok(())
}

/// 握手给调用方看的事实 —— **准入裁决的全部输入**。
///
/// 单独成结构（而不是一串位置参数）是为了让接收侧的闭包一次拿全：
/// 白名单要地址（在闭包外捕获）、配对要 [`HandshakeFacts::pair_code`]、
/// 分块约定与大小合法性要另两个字段。
#[derive(Debug, Clone, Copy)]
pub struct HandshakeFacts<'a> {
    pub file_name: &'a str,
    pub file_size: u64,
    pub file_hash: &'a str,
    /// 对端声明的分块大小（R-13 第 4 关）
    pub chunk_size: u64,
    /// 对端随握手带来的配对码（R-2；`None` = 没带）
    pub pair_code: Option<&'a str>,
}

/// 发送侧一次完整会话：握手 → 发块 → 收尾。
///
/// `file_hash` 必须是 [`super::source::hash_file`] 的结果（握手要用它）。
/// `admit` 不参与发送侧（准入是接收方的职责）。
///
/// # Errors
/// 见 [`TransportError`]。对端明确拒收返回 [`TransportError::Rejected`]。
#[allow(clippy::too_many_arguments)]
pub fn send_file(
    stream: &TcpStream,
    source: &mut SendSource,
    file_hash: &str,
    task_id: &str,
    file_modified_at_ms: Option<i64>,
    pair_code: Option<&str>,
    cancel: &CancellationToken,
    // 每块发成功后的进度回调（**累计已发送字节数**）。
    //
    // 🔴 节流由调用方负责：本层"每块如实报一次"，
    // 合并频率是界面的事（见 [`super::tasks::ProgressThrottle`]）。
    mut on_progress: impl FnMut(u64),
) -> Result<SendOutcome, TransportError> {
    let mut writer = stream;
    let mut reader = FrameReader::new(stream);

    write_frame(
        &mut writer,
        &TransferMessage::Handshake {
            task_id: task_id.to_owned(),
            file_name: source.file_name().to_owned(),
            file_size: source.file_size(),
            file_hash: file_hash.to_owned(),
            chunk_size: source.chunk_size(),
            file_modified_at_ms,
            pair_code: pair_code.map(str::to_owned),
        },
        &[],
    )?;

    // ── 握手回执：拿到续传起点与最终块大小 ──
    //
    // 🔴 这次读取必须**放宽**到 [`HANDSHAKE_WAIT`]：接收端是在**用户裁决之后**
    // 才发 `HandshakeAck` 的（F-FT-14 的"接收前询问"），若这里沿用 `IO_TIMEOUT`（同为 30 秒），
    // 发送方会几乎必然先读超时，而接收端还在等用户点按钮 ⇒ **两端结论不一致**。
    // 真机端到端跑出来的现象就是：发送方报 `PEER_DISCONNECTED 对端长时间无数据`，
    // 而接收端的确认弹窗刚刚弹出。
    let _ = stream.set_read_timeout(Some(HANDSHAKE_WAIT));
    let handshake_read = reader.read_frame();
    // 无论成败都恢复常规超时：后续的块传输不该享有这个宽限
    let _ = stream.set_read_timeout(Some(IO_TIMEOUT));
    let (message, _) = handshake_read?;
    let (resume_from, chunk_size) = match message {
        TransferMessage::HandshakeAck {
            accepted,
            reason_code,
            resume_from,
            chunk_size,
            ..
        } => {
            if !accepted {
                return Err(TransportError::Rejected {
                    reason_code: reason_code.unwrap_or_else(|| "UNKNOWN".to_owned()),
                });
            }
            (resume_from.unwrap_or(0), chunk_size)
        }
        TransferMessage::Cancel { reason_code, .. } => {
            return Err(TransportError::Cancelled { reason_code })
        }
        TransferMessage::Error { reason_code, .. } => {
            return Err(TransportError::RemoteFailed {
                reason_code: Some(reason_code),
            })
        }
        _ => return Err(TransportError::Unexpected("握手时期待 HandshakeAck")),
    };
    // 对端可以指定更小的块（F-FT-28）；本模块只接受"不小于 1"的值，其余交由上层复审
    if let Some(size) = chunk_size {
        if size > 0 && size != source.chunk_size() {
            // 不擅自改变块大小（source 已在 open 时钉死），只把差异如实报出
            return Err(TransportError::Unexpected("对端要求的分块大小与本端不一致"));
        }
    }
    source.seek_to(resume_from)?;

    // ── 发块 ──
    loop {
        check_cancel(cancel, &mut writer, task_id, super::reasons::USER_CANCEL)?;
        match source.next_chunk()? {
            Some(chunk) => {
                write_frame(
                    &mut writer,
                    &TransferMessage::Chunk {
                        task_id: task_id.to_owned(),
                        index: chunk.index,
                        offset: chunk.offset,
                    },
                    &chunk.bytes,
                )?;
                // 进度回调：**发成功之后**才报（写失败时不该报"已发出"）。
                // 节流由调用方负责——本层只管"每块如实报一次"。
                on_progress(chunk.offset + chunk.bytes.len() as u64);
            }
            None => break,
        }
    }

    // 发送期间文件被改动 → 如实报错（不发"半新半旧"的内容）
    source.verify_unchanged()?;

    // ── 收尾 ──
    write_frame(
        &mut writer,
        &TransferMessage::Complete {
            task_id: task_id.to_owned(),
            file_hash: file_hash.to_owned(),
        },
        &[],
    )?;
    let (message, _) = reader.read_frame()?;
    match message {
        TransferMessage::CompleteAck {
            ok, reason_code, ..
        } => {
            if ok {
                Ok(SendOutcome::Delivered)
            } else {
                match reason_code.as_deref() {
                    Some(super::reasons::HASH_MISMATCH) => Ok(SendOutcome::HashMismatch),
                    Some(super::reasons::CONFLICT_SKIP) => Ok(SendOutcome::Skipped),
                    _ => Err(TransportError::RemoteFailed { reason_code }),
                }
            }
        }
        TransferMessage::Cancel { reason_code, .. } => {
            Err(TransportError::Cancelled { reason_code })
        }
        _ => Err(TransportError::Unexpected("收尾时期待 CompleteAck")),
    }
}

/// 接收侧通过握手的会话事实（`admit_handshake` 的产物）。
#[derive(Debug)]
struct HandshakeAdmitted {
    task_id: String,
    file_name: String,
    file_size: u64,
    declared_hash: String,
}

/// 握手阶段的两条出路。
#[derive(Debug)]
enum HandshakePhase {
    /// 准入通过，可以打开落盘器了
    Admitted(HandshakeAdmitted),
    /// 已回执拒收，会话到此为止
    Rejected(ReceiveOutcome),
}

/// 读握手 → 跑调用方的准入闭包 → 回执。抽出来是为了让 [`receive_file`] 保持在可读长度内。
///
/// 🔴 拒收路径**不碰文件系统**：R-13 要求闸门在"建未完成文件"之前，本函数是那条顺序的落点。
fn admit_handshake<R: Read, W: Write>(
    reader: &mut FrameReader<R>,
    writer: &mut W,
    policy: &mut ConflictPolicy,
    admit: impl FnOnce(&HandshakeFacts<'_>, &mut ConflictPolicy) -> Admission,
    // 🔴 首帧由调用方读好传进来：接收侧要先看**第一帧是什么**才能决定
    //    走文件事务还是文本事务（F-FT-33），读掉就还不回去了。
    (message, _body): (TransferMessage, Vec<u8>),
) -> Result<HandshakePhase, TransportError> {
    let (task_id, file_name, file_size, declared_hash, declared_chunk, pair_code) = match message {
        TransferMessage::Handshake {
            task_id,
            file_name,
            file_size,
            file_hash,
            chunk_size,
            pair_code,
            ..
        } => (
            task_id, file_name, file_size, file_hash, chunk_size, pair_code,
        ),
        TransferMessage::Cancel { reason_code, .. } => {
            return Err(TransportError::Cancelled { reason_code })
        }
        _ => return Err(TransportError::Unexpected("首帧必须是 Handshake")),
    };

    let facts = HandshakeFacts {
        file_name: &file_name,
        file_size,
        file_hash: &declared_hash,
        chunk_size: declared_chunk,
        pair_code: pair_code.as_deref(),
    };
    // 裁决点唯一：`admission::evaluate`，由调用方的闭包执行（白名单/配对/并发/磁盘都在里面）
    let verdict = admit(&facts, policy);
    // 防御：`evaluate` 的第 4 关本应拦住非法分块大小；这里再拦一次是因为
    // "分块大小"直接决定后续每次读取的缓冲，不该只依赖调用方把参数配对。
    let verdict = match verdict {
        Admission::Admitted { .. }
            if super::chunk::chunk_size_mismatch(declared_chunk).is_some() =>
        {
            Admission::Rejected {
                gate: super::admission::Gate::ChunkSize,
                reason: super::reasons::CHUNK_SIZE_MISMATCH,
            }
        }
        other => other,
    };

    if let Admission::Rejected { gate, reason } = verdict {
        write_frame(
            writer,
            &TransferMessage::HandshakeAck {
                task_id: task_id.clone(),
                accepted: false,
                reason_code: Some(reason.to_owned()),
                resume_from: None,
                chunk_size: None,
            },
            &[],
        )?;
        return Ok(HandshakePhase::Rejected(ReceiveOutcome::Rejected {
            reason_code: format!("{}:{reason}", gate.as_str()),
        }));
    }

    Ok(HandshakePhase::Admitted(HandshakeAdmitted {
        task_id,
        file_name,
        file_size,
        declared_hash,
    }))
}

/// 接收侧一次完整会话：握手 → 准入 → 收块 → 校验入位。
///
/// `admit` 收到 `(file_size, file_hash)` 后返回 [`Admission`]——
/// 白名单、配对、并发、磁盘预检都在它里面（见 `admission::evaluate`）。
///
/// # Errors
/// 见 [`TransportError`]。对端取消/断连返回 [`TransportError::Cancelled`] / `PeerClosed`，
/// **此时未完成文件保留**（可续传）。
#[allow(clippy::too_many_arguments)]
pub fn receive_file(
    stream: &TcpStream,
    root: &Path,
    peer_token: &str,
    policy: &mut ConflictPolicy,
    admit: impl FnOnce(&HandshakeFacts<'_>, &mut ConflictPolicy) -> Admission,
    cancel: &CancellationToken,
    on_progress: impl FnMut(u64),
) -> Result<ReceiveOutcome, TransportError> {
    // 🔴 先读掉首帧再交给 `_with_first`：`FrameReader` **恰好**读一帧的字节
    //    （13 字节前缀 + body），不预读，故新建一个 reader 能从原位置继续。
    let first = FrameReader::new(stream).read_frame()?;
    receive_file_with_first(
        stream,
        root,
        peer_token,
        policy,
        admit,
        cancel,
        on_progress,
        first,
    )
}

/// 同 [`receive_file`]，但**首帧已由调用方读出**（见 [`admit_handshake`] 的同名参数）。
///
/// 存在的理由：接收侧必须在读第一帧时就分辨"这是文件还是文本"（F-FT-33），
/// 而读掉的帧还不回去。
///
/// # Errors
/// 同 [`receive_file`]。
pub fn receive_file_with_first(
    stream: &TcpStream,
    root: &Path,
    peer_token: &str,
    // 🔴 &mut 而不是按值：准入闭包要在**拿到用户裁决之后**就地覆盖本次的冲突策略
    //    （「同名文件」三个选项只在本次生效，不该改动持久设置）
    policy: &mut ConflictPolicy,
    admit: impl FnOnce(&HandshakeFacts<'_>, &mut ConflictPolicy) -> Admission,
    cancel: &CancellationToken,
    // 每块落盘成功后的进度回调（**累计已接收字节数**）。
    //
    // 🔴 与发送侧同口径：本层每块如实报一次，节流由调用方负责。
    mut on_progress: impl FnMut(u64),
    first: (TransferMessage, Vec<u8>),
) -> Result<ReceiveOutcome, TransportError> {
    let mut writer = stream;
    let mut reader = FrameReader::new(stream);

    // ── 握手 + 准入裁决（已抽出，见 `admit_handshake`） ──
    let admitted = match admit_handshake(&mut reader, &mut writer, policy, admit, first)? {
        HandshakePhase::Admitted(admitted) => admitted,
        // 🔴 拒收即不建任何文件（R-13 的顺序：闸门在"建未完成文件"之前）
        HandshakePhase::Rejected(outcome) => return Ok(outcome),
    };
    let HandshakeAdmitted {
        task_id,
        file_name,
        file_size,
        declared_hash,
    } = admitted;

    // ── 打开落盘器（续传起点即未完成文件的真实长度） ──
    let mut sink = ReceiveSink::open(root, &file_name, peer_token, file_size)?;
    let resume_from = sink.received();
    write_frame(
        &mut writer,
        &TransferMessage::HandshakeAck {
            task_id: task_id.clone(),
            accepted: true,
            reason_code: None,
            resume_from: Some(resume_from),
            chunk_size: None,
        },
        &[],
    )?;

    // ── 收块 ──
    loop {
        check_cancel(cancel, &mut writer, &task_id, super::reasons::USER_CANCEL)?;
        let (message, body) = reader.read_frame()?;
        match message {
            TransferMessage::Chunk { offset, .. } => {
                sink.append(offset, &body)?;
                // 进度回调：**落盘成功之后**才报（append 失败时不该报"已收到"）
                on_progress(offset + body.len() as u64);
            }
            TransferMessage::Complete { file_hash, .. } => {
                // 🔴 两次声明的摘要必须一致：不一致 = 对端文件中途被改动
                // （本工程发送侧自己会 `verify_unchanged`，这里是**纵深防御**，
                // 拦住不检查的对端。理由由 `Unexpected` 承载，不新造原因码——
                // `reasons` 是 16 码冻结契约。）
                if file_hash != declared_hash {
                    return Err(TransportError::Unexpected(
                        "Complete 声明的摘要与握手不一致（对端文件中途被改动？）",
                    ));
                }
                let outcome = sink.finish(&file_hash, *policy, None, now_unix_ms())?;
                let (ok, reason_code) = match &outcome {
                    FinishOutcome::Committed { .. } => (true, None),
                    FinishOutcome::Skipped => {
                        (false, Some(super::reasons::CONFLICT_SKIP.to_owned()))
                    }
                    FinishOutcome::HashMismatch => {
                        (false, Some(super::reasons::HASH_MISMATCH.to_owned()))
                    }
                };
                write_frame(
                    &mut writer,
                    &TransferMessage::CompleteAck {
                        task_id: task_id.clone(),
                        ok,
                        reason_code,
                    },
                    &[],
                )?;
                return Ok(match outcome {
                    FinishOutcome::Committed { path, renamed } => {
                        ReceiveOutcome::Committed { path, renamed }
                    }
                    FinishOutcome::Skipped => ReceiveOutcome::Skipped,
                    FinishOutcome::HashMismatch => ReceiveOutcome::HashMismatch,
                });
            }
            TransferMessage::Cancel { reason_code, .. } => {
                // 🔴 不删未完成文件：中断留下的 .part 正是续传的前提（R-21 亦然）
                return Err(TransportError::Cancelled { reason_code });
            }
            TransferMessage::Pause { .. } | TransferMessage::Resume { .. } => {
                // 对端暂停/恢复不影响接收侧（本来就在等它发数据）；
                // 接收侧**发起**暂停属会话状态机，见模块头"本模块不做的事"
            }
            _ => {
                return Err(TransportError::Unexpected(
                    "收块时期待 Chunk/Complete/Cancel",
                ))
            }
        }
    }
}

/// 文本事务的事实（准入判定用；文件侧对应 [`HandshakeFacts`]）。
///
/// 🔴 文本**没有**文件那些闸门（分块大小 / 文件大小上限 / 磁盘余量都不适用），
/// 适用的是：**来源白名单**、**配对**、以及**接收前询问**。
pub struct TextFacts<'a> {
    pub task_id: &'a str,
    /// 发送方声明的字符数（F-FT-32 的"137 字"，按 Unicode 标量）
    pub chars: u64,
    /// 发送方声明的 UTF-8 字节数
    pub bytes: u64,
    pub from_device: Option<&'a str>,
    /// 本次运行期所需的配对凭据（免码地址可省）
    pub pair_code: Option<&'a str>,
}

/// 文本事务的结论。
///
/// 🔴 [`TextOutcome::Delivered`] 在**电脑通道**上的含义是**剪贴板写入成功**——
/// 这正是 F-FT-34 那条红线：写不进去就必须是 `Rejected`，**不许回"已送达"**。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TextOutcome {
    Delivered,
    Rejected { reason_code: String },
}

/// 发送一条文本（F-FT-33）。
///
/// 🔴 等回执用的是 [`HANDSHAKE_WAIT`] 而不是 [`IO_TIMEOUT`]：接收端要先把
/// "接收前询问"问完才回执，30 秒的常规超时会让发送方在用户还没点之前就失败
/// （文件侧踩过这个坑，见 [`HANDSHAKE_WAIT`] 的文档）。
///
/// # Errors
/// 编码失败、底层写失败、或读回执失败。
pub fn send_text(
    stream: &TcpStream,
    text: &str,
    task_id: &str,
    from_device: Option<&str>,
    cancel: &CancellationToken,
) -> Result<TextOutcome, TransportError> {
    let mut writer = stream;
    let mut reader = FrameReader::new(stream);
    let chars = super::text::char_count(text) as u64;
    let bytes = super::text::byte_count(text) as u64;

    write_frame(
        &mut writer,
        &TransferMessage::Text {
            task_id: task_id.to_owned(),
            chars,
            bytes,
            from_device: from_device.map(str::to_owned),
        },
        text.as_bytes(),
    )?;

    if cancel.is_cancelled() {
        return Ok(TextOutcome::Rejected {
            reason_code: super::reasons::USER_CANCEL.to_owned(),
        });
    }

    let _ = stream.set_read_timeout(Some(HANDSHAKE_WAIT));
    let ack = reader.read_frame();
    let _ = stream.set_read_timeout(Some(IO_TIMEOUT));
    let (message, _) = ack?;
    match message {
        TransferMessage::TextAck {
            ok, reason_code, ..
        } => {
            if ok {
                Ok(TextOutcome::Delivered)
            } else {
                Ok(TextOutcome::Rejected {
                    reason_code: reason_code.unwrap_or_else(|| "UNKNOWN".to_owned()),
                })
            }
        }
        TransferMessage::Error {
            reason_code,
            message,
            ..
        } => Err(TransportError::Unexpected(Box::leak(
            format!("对端报错 {reason_code} {}", message.unwrap_or_default()).into_boxed_str(),
        ))),
        _ => Err(TransportError::Unexpected("等文本回执时收到别的帧")),
    }
}

/// 接收一条文本（F-FT-34：**成功定义就是 `commit` 成功**）。
///
/// `commit` 是"把文本变成交付"的那一步（电脑通道上是写剪贴板）。
/// 🔴 之所以注入而不是直接调 `infra::clipboard`：单测不该动**用户真实的剪贴板**
/// （第 45 轮那次堆损坏就是并发碰真实剪贴板造成的）。
///
/// # Errors
/// 读帧失败、写回执失败。业务性拒绝走 `Ok(TextOutcome::Rejected { .. })`。
pub fn receive_text(
    stream: &TcpStream,
    admit: impl FnOnce(&TextFacts<'_>) -> Admission,
    commit: impl FnOnce(&str) -> Result<(), String>,
    cancel: &CancellationToken,
) -> Result<TextOutcome, TransportError> {
    // 🔴 与 `receive_file` 同款：先读首帧，再交给 `_with_first`（FrameReader 不预读）
    let first = FrameReader::new(stream).read_frame()?;
    receive_text_with_first(stream, admit, commit, cancel, first)
}

/// 同 [`receive_text`]，但**首帧已由调用方读出**（见 [`receive_any`] 的分流）。
///
/// # Errors
/// 读帧失败、写回执失败。业务性拒绝走 `Ok(TextOutcome::Rejected { .. })`。
pub fn receive_text_with_first(
    stream: &TcpStream,
    admit: impl FnOnce(&TextFacts<'_>) -> Admission,
    commit: impl FnOnce(&str) -> Result<(), String>,
    cancel: &CancellationToken,
    (message, body): (TransferMessage, Vec<u8>),
) -> Result<TextOutcome, TransportError> {
    let mut writer = stream;

    let TransferMessage::Text {
        task_id,
        chars,
        bytes,
        from_device,
    } = message
    else {
        return Err(TransportError::Unexpected("文本事务的第一帧必须是 Text"));
    };

    let text = String::from_utf8_lossy(&body).into_owned();
    let facts = TextFacts {
        task_id: &task_id,
        chars,
        bytes,
        from_device: from_device.as_deref(),
        pair_code: None,
    };

    let reply = |writer: &mut &TcpStream, ok: bool, reason: Option<String>| {
        write_frame(
            writer,
            &TransferMessage::TextAck {
                task_id: task_id.clone(),
                ok,
                reason_code: reason.clone(),
            },
            &[],
        )
        .map(|()| reason)
    };

    // 声明的字节数与实际 body 不符 ⇒ 拒收（🔴 不按声明值裁剪，宁可拒绝也不交付半个文本）
    if bytes as usize != body.len() {
        let reason = super::reasons::TEXT_TOO_LONG.to_owned();
        let _ = reply(&mut writer, false, Some(reason.clone()));
        return Ok(TextOutcome::Rejected {
            reason_code: reason,
        });
    }

    let verdict = admit(&facts);
    if let Admission::Rejected { reason, .. } = verdict {
        let reason = reason.to_owned();
        let _ = reply(&mut writer, false, Some(reason.clone()));
        return Ok(TextOutcome::Rejected {
            reason_code: reason,
        });
    }

    // 与文件侧同口径：取消/断开也要如实收场
    if cancel.is_cancelled() {
        let reason = super::reasons::USER_CANCEL.to_owned();
        let _ = reply(&mut writer, false, Some(reason.clone()));
        return Ok(TextOutcome::Rejected {
            reason_code: reason,
        });
    }

    // 准入通过 ⇒ 交付。🔴 交付失败的原因码是 CLIPBOARD_WRITE_FAILED，
    //    而不是"已送达"——这正是 F-FT-34 的落点。
    match commit(&text) {
        Ok(()) => {
            let _ = reply(&mut writer, true, None);
            Ok(TextOutcome::Delivered)
        }
        Err(_detail) => {
            let reason = super::reasons::CLIPBOARD_WRITE_FAILED.to_owned();
            let _ = reply(&mut writer, false, Some(reason.clone()));
            Ok(TextOutcome::Rejected {
                reason_code: reason,
            })
        }
    }
}

/// 连接的第一帧决定走哪条事务。
#[derive(Debug)]
pub enum AnyOutcome {
    File(ReceiveOutcome),
    Text(TextOutcome),
}

/// **按首帧分流**：`Text` 走文本事务（F-FT-33），其余按文件事务交给 `admit_file`。
///
/// 🔴 这是接收侧唯一的分流点。之所以要在这里分：`Text` 是**顶层消息**（与 `Handshake`
/// 平级，见 [`TransferMessage`]），故文本连接根本**没有文件握手**——
/// 不能让文件路径先把首帧读掉再报"首帧必须是 Handshake"。
///
/// `commit_text` 是文本的"落交付"动作（电脑通道上是写剪贴板，见 [`receive_text`]）。
///
/// # Errors
/// 读首帧失败、或两条事务各自的失败。
#[allow(clippy::too_many_arguments)]
pub fn receive_any(
    stream: &TcpStream,
    root: &Path,
    peer_token: &str,
    policy: &mut ConflictPolicy,
    admit_file: impl FnOnce(&HandshakeFacts<'_>, &mut ConflictPolicy) -> Admission,
    admit_text: impl FnOnce(&TextFacts<'_>) -> Admission,
    commit_text: impl FnOnce(&str) -> Result<(), String>,
    cancel: &CancellationToken,
    on_progress: impl FnMut(u64),
) -> Result<AnyOutcome, TransportError> {
    let first = FrameReader::new(stream).read_frame()?;
    if matches!(first.0, TransferMessage::Text { .. }) {
        return receive_text_with_first(stream, admit_text, commit_text, cancel, first)
            .map(AnyOutcome::Text);
    }
    receive_file_with_first(
        stream,
        root,
        peer_token,
        policy,
        admit_file,
        cancel,
        on_progress,
        first,
    )
    .map(AnyOutcome::File)
}

#[cfg(test)]
mod tests;
