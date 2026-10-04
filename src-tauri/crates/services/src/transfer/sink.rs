//! 接收侧落盘器：未完成文件 → 全文件校验 → 原子改名 → 还原时间（F-FT-19/20/21/24）。
//!
//! ## 它守护的那条承诺
//!
//! 规格 R-11 原文：**"要么完整、要么不存在"**——目录里绝不允许出现"半截成品"。
//! 手段是把这三件事绑成一串：**写入只落未完成文件** → **全文件摘要比对通过** → **一次性改名入位**。
//! 任何一环失败都回到"目录里没有这个文件"，而不是"目录里有个坏文件"。
//!
//! ## 两处容易做错、且做错了很难查的地方
//!
//! 1. 🔴 **续传时哈希必须重灌前缀**（F-FT-19 原文："先重算已收部分作为校验起点"）。
//!    续传只算新到的字节，摘要必然对不上，表现为"传完了但报 `HASH_MISMATCH`"——
//!    而重传一次又会成功（因为那时没有前缀），于是变成一个**间歇性**缺陷。
//!    本模块在 [`ReceiveSink::open`] 里无条件把已落盘前缀喂进 hasher。
//! 2. 🔴 **摘要不符时必须删掉未完成文件**（F-FT-20 原文："避免『永远续传一个坏文件』的死循环"）。
//!    不删的话下次续传会从这段坏字节上继续，永远失败。
//!
//! ## 未完成文件的命名与清理
//!
//! 名字是 `{清洗后的文件名}.{对端令牌}.part`——**按对端隔离**，两个对端同时传同名文件互不干扰
//! （母本同口径）。服务启动与每次传输完成时清扫**属于其它对端**且**超过 7 天**的遗留
//! （F-FT-19 / R-21：清扫只针对未完成文件，绝不触碰成品）。
//!
//! 🔴 所有路径都经 [`infra::safe::join_under_root`]——本域不自己拼路径（`transfer::guard` 会扫）。

use std::fs::{self, File, OpenOptions};
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use sha2::{Digest, Sha256};

use super::{chunk, conflict};
use conflict::{ConflictDecision, ConflictPolicy};

/// 未完成文件的扩展名（母本同 `.part`）。
pub const PART_EXTENSION: &str = "part";

/// 遗留未完成文件的保留期（F-FT-19 / R-21：**7 天**）。
pub const ORPHAN_RETENTION: Duration = Duration::from_secs(7 * 24 * 60 * 60);

/// 未完成文件名里**不可歧义的标记**：`{名}.aegisft-{对端令牌}.part`。
///
/// ## 🔴 为什么必须有这个标记（本轮实测抓出的真缺陷）
///
/// 最初的命名是 `{名}.{令牌}.part`，于是 [`peer_token_of`] 只能取"最后一个点之后、
/// `.part` 之前"的那一段。问题：**用户完全可能有一个合法的、就叫 `report.pdf.part` 的文件**
/// （很多下载器就用 `.part` 后缀），它会被解析成"对端 `pdf` 的未完成文件"，
/// 进而在孤儿清理里**过了 7 天就被删掉**——**静默删除用户的合法文件**，属数据丢失级缺陷。
///
/// 加一个只可能由本程序写出的标记之后歧义消失：标记不匹配即"这不是我们的未完成文件"，
/// 孤儿清理会跳过它。
pub const PART_MARKER: &str = "aegisft-";

/// 修改时间还原窗口的下界：**1980-01-01**（F-FT-24）。
///
/// 上界是"今日 + 1 天"，由 [`is_acceptable_mtime`] 按调用方给的 `now_ms` 判。
pub const MTIME_LOWER_BOUND_MS: i64 = 315_532_800_000;

/// 改名冲突候选的尝试上限（到 `{name} (9999).ext` 仍撞名就放弃，避免死循环）。
const MAX_RENAME_ATTEMPTS: u32 = 9999;

/// 落盘失败原因。
#[derive(Debug)]
pub enum SinkError {
    /// 路径被守卫拒绝（越界/非法名）——**这是拒绝，不是"清洗后继续"**
    Path(infra::safe::PathEscape),
    /// 底层 IO 失败
    Io(std::io::Error),
    /// 追加的偏移与已收长度不符（对端自称的起点与真实进度不一致）
    OffsetMismatch { expected: u64, got: u64 },
    /// 写入会越过声明的文件大小
    Oversize { declared: u64, attempted: u64 },
    /// 收尾时发现收满的字节数与声明不符
    Incomplete { declared: u64, received: u64 },
    /// 撞名重试次数耗尽
    RenameExhausted,
}

impl std::fmt::Display for SinkError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Path(escape) => write!(f, "路径被拒绝：{escape}"),
            Self::Io(error) => write!(f, "落盘 IO 失败：{error}"),
            Self::OffsetMismatch { expected, got } => {
                write!(f, "块起点不符：应为 {expected}，对端给 {got}")
            }
            Self::Oversize {
                declared,
                attempted,
            } => write!(f, "写入越界：声明 {declared} 字节，试图写到 {attempted}"),
            Self::Incomplete { declared, received } => {
                write!(f, "收尾时长度不符：声明 {declared}，实收 {received}")
            }
            Self::RenameExhausted => write!(f, "改名尝试次数耗尽"),
        }
    }
}

impl std::error::Error for SinkError {}

impl From<std::io::Error> for SinkError {
    fn from(error: std::io::Error) -> Self {
        Self::Io(error)
    }
}

impl From<infra::safe::PathEscape> for SinkError {
    fn from(escape: infra::safe::PathEscape) -> Self {
        Self::Path(escape)
    }
}

/// 收尾结论（对应 F-FT-30 的四类结局里的三类；"取消"由调用方另行处置）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FinishOutcome {
    /// 已入位。`renamed` 为真表示因撞名而改了名（F-FT-23）
    Committed { path: PathBuf, renamed: bool },
    /// 按策略跳过，未写入新文件，未完成文件已删除（F-FT-23：终态是"已跳过"而非"已完成"）
    Skipped,
    /// 全文件摘要不符，**损坏的未完成文件已删除**（F-FT-20）
    HashMismatch,
}

/// 未完成文件名：`{清洗后的文件名}.aegisft-{对端令牌}.part`。
///
/// 返回 `None` 表示文件名清洗后不可用（调用方应拒收该名字，而不是替它编一个）。
#[must_use]
pub fn part_file_name(file_name: &str, peer_token: &str) -> Option<String> {
    let safe = infra::safe::sanitize_file_name(file_name)?;
    let token: String = peer_token
        .chars()
        .filter(char::is_ascii_alphanumeric)
        .take(32)
        .collect();
    if token.is_empty() {
        return None;
    }
    Some(format!("{safe}.{PART_MARKER}{token}.{PART_EXTENSION}"))
}

/// 从文件名里取回对端令牌（[`part_file_name`] 的逆操作）。
///
/// 🔴 **只认带 [`PART_MARKER`] 的名字**：`report.pdf.part` 这种"用户自己的 `.part` 文件"
/// 必须返回 `None`，否则孤儿清理会去删用户的东西（见 [`PART_MARKER`] 的说明）。
#[must_use]
pub fn peer_token_of(file_name: &str) -> Option<&str> {
    let stem = file_name.strip_suffix(&format!(".{PART_EXTENSION}"))?;
    let (_, token) = stem.rsplit_once('.')?;
    let token = token.strip_prefix(PART_MARKER)?;
    if token.is_empty() {
        None
    } else {
        Some(token)
    }
}

/// 修改时间是否落在可还原窗口内（F-FT-24：**1980 年至今日 + 1 天**）。
///
/// 窗口外一律不还原（静默保持当前时间），**不报错**——规格明写"不为难用户"。
#[must_use]
pub fn is_acceptable_mtime(unix_ms: i64, now_ms: i64) -> bool {
    const DAY_MS: i64 = 24 * 60 * 60 * 1000;
    unix_ms >= MTIME_LOWER_BOUND_MS && unix_ms <= now_ms.saturating_add(DAY_MS)
}

/// 当前 Unix 毫秒（供窗口判定与孤儿清理用）。
#[must_use]
pub fn now_unix_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| i64::try_from(d.as_millis()).unwrap_or(i64::MAX))
}

/// 一次接收的落盘器。
///
/// 生命周期：`open` → 若干 `append` → `finish` 或 `discard`。
#[derive(Debug)]
pub struct ReceiveSink {
    root: PathBuf,
    part_path: PathBuf,
    final_name: String,
    file_size: u64,
    received: u64,
    hasher: Sha256,
    file: File,
}

impl ReceiveSink {
    /// 打开（或续开）一个接收。
    ///
    /// 已存在同名未完成文件时按 [`chunk::resume_from`] 裁决：
    /// * 可续传 → 定位到已收长度，并**把已落盘前缀重灌进 hasher**（见模块头 ①）
    /// * 不可信（`existing >= file_size`）→ 截断归零重传
    ///
    /// # Errors
    /// 路径被守卫拒绝、或 IO 失败时返回 [`SinkError`]。
    pub fn open(
        root: &Path,
        file_name: &str,
        peer_token: &str,
        file_size: u64,
    ) -> Result<Self, SinkError> {
        let safe_name = infra::safe::sanitize_file_name(file_name)
            .ok_or(SinkError::Path(infra::safe::PathEscape::EmptySegment))?;
        let part_name = part_file_name(&safe_name, peer_token)
            .ok_or(SinkError::Path(infra::safe::PathEscape::EmptySegment))?;
        let part_path = infra::safe::join_under_root(root, &part_name)?;

        let mut file = OpenOptions::new()
            .create(true)
            .read(true)
            .write(true)
            .truncate(false)
            .open(&part_path)?;
        let existing = file.metadata()?.len();

        let mut hasher = Sha256::new();
        let start = if let Some(offset) = chunk::resume_from(existing, file_size) {
            // 🔴 重灌前缀：漏了这一步，续传必然报 HASH_MISMATCH（模块头 ①）
            let mut prefix = vec![0_u8; usize::try_from(offset).unwrap_or(0)];
            file.seek(SeekFrom::Start(0))?;
            file.read_exact(&mut prefix)?;
            hasher.update(&prefix);
            offset
        } else {
            // 不可信的残留：归零重传（不是"从 0 接着写"，是清掉重来）
            file.set_len(0)?;
            0
        };
        file.seek(SeekFrom::Start(start))?;

        Ok(Self {
            root: root.to_path_buf(),
            part_path,
            final_name: safe_name,
            file_size,
            received: start,
            hasher,
            file,
        })
    }

    /// 已收字节数（续传时为前缀长度）。
    #[must_use]
    pub fn received(&self) -> u64 {
        self.received
    }

    /// 未完成文件的路径（排障用；**不要**把它展示给用户）。
    #[must_use]
    pub fn part_path(&self) -> &Path {
        &self.part_path
    }

    /// 在 `offset` 处追加一块。
    ///
    /// `offset` **必须**等于当前已收长度：不符即 [`SinkError::OffsetMismatch`]，
    /// 让对端"重新查询真实进度"（与手机通道的 409 语义同源，此处是电脑通道的对应物）。
    ///
    /// # Errors
    /// 起点不符、越界、或 IO 失败时返回 [`SinkError`]。
    pub fn append(&mut self, offset: u64, bytes: &[u8]) -> Result<(), SinkError> {
        if offset != self.received {
            return Err(SinkError::OffsetMismatch {
                expected: self.received,
                got: offset,
            });
        }
        let attempted = self.received.saturating_add(bytes.len() as u64);
        if attempted > self.file_size {
            return Err(SinkError::Oversize {
                declared: self.file_size,
                attempted,
            });
        }
        self.file.write_all(bytes)?;
        self.hasher.update(bytes);
        self.received = attempted;
        Ok(())
    }

    /// 收尾：校验 → 改名入位 → 还原时间。
    ///
    /// * 摘要不符 → 删除未完成文件，返回 [`FinishOutcome::HashMismatch`]
    /// * 冲突策略判为跳过 → 删除未完成文件，返回 [`FinishOutcome::Skipped`]
    /// * 通过 → 原子改名（撞名则按策略改名，必要时取 ` (n)` 候选）→ 按窗口还原 mtime
    ///
    /// 🔴 `ask_channel_available` 恒为 `false`：逐次询问发生在**握手期**，
    /// 走到这里时用户已经决定过了；若此处再问一次，就等于让同一个决定被问两遍。
    ///
    /// # Errors
    /// 收尾时长度不符、路径被拒、或 IO 失败时返回 [`SinkError`]。
    pub fn finish(
        self,
        expected_hash: &str,
        policy: ConflictPolicy,
        modified_at_ms: Option<i64>,
        now_ms: i64,
    ) -> Result<FinishOutcome, SinkError> {
        if self.received != self.file_size {
            return Err(SinkError::Incomplete {
                declared: self.file_size,
                received: self.received,
            });
        }
        let actual = hex::encode(self.hasher.clone().finalize());
        let expected = expected_hash.trim().to_ascii_lowercase();
        if actual != expected {
            // F-FT-20：必须删掉，否则下次续传会从这个坏文件上继续、永远失败
            drop(self.file);
            let _ = fs::remove_file(&self.part_path);
            return Ok(FinishOutcome::HashMismatch);
        }

        // 校验通过才决定落点（F-FT-21：改名前再确认目标名不冲突）
        let target = infra::safe::join_under_root(&self.root, &self.final_name)?;
        let decision = conflict::decide(policy, target.exists(), false);
        if decision == ConflictDecision::Skip {
            drop(self.file);
            let _ = fs::remove_file(&self.part_path);
            return Ok(FinishOutcome::Skipped);
        }

        // 先把内容刷到磁盘，再改名——否则改名后崩溃会留下一个长度对但内容空的成品
        self.file.sync_all()?;
        drop(self.file);

        let (final_path, renamed) = match decision {
            ConflictDecision::Overwrite | ConflictDecision::Fresh => (target, false),
            // `Skip` 已在上面提前返回（返回 `Skipped`），此处不可达；
            // `AskUser` 在 `ask_channel_available=false` 下也不会由 `decide` 给出。
            // 两者一并按**改名**兜底——即便逻辑将来被改动，也绝不会走到"覆盖"。
            ConflictDecision::Renamed | ConflictDecision::Skip | ConflictDecision::AskUser => {
                let candidate = free_rename_candidate(&self.root, &self.final_name)?;
                (candidate, true)
            }
        };

        // 同卷 rename 是原子的：目录里要么没有、要么是完整的
        fs::rename(&self.part_path, &final_path)?;

        // F-FT-24：窗口外静默不还原（不报错、也不谎称已还原）
        if let Some(ms) = modified_at_ms {
            if is_acceptable_mtime(ms, now_ms) {
                // 还原失败不算传输失败——规格明写"静默保持当前时间"
                let _ = infra::fs::set_modified_time(&final_path, ms);
            }
        }

        Ok(FinishOutcome::Committed {
            path: final_path,
            renamed,
        })
    }

    /// 放弃本次接收：关闭句柄并删除未完成文件（服务端主动取消路径用）。
    ///
    /// # Errors
    /// 删除失败时返回 `io::Error`（文件不存在不算失败）。
    pub fn discard(self) -> std::io::Result<()> {
        drop(self.file);
        match fs::remove_file(&self.part_path) {
            Ok(()) => Ok(()),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(error) => Err(error),
        }
    }
}

/// 找第一个空闲的 ` (n)` 候选（F-FT-23 的改名后缀）。
fn free_rename_candidate(root: &Path, file_name: &str) -> Result<PathBuf, SinkError> {
    let original = Path::new(file_name);
    for index in 1..=MAX_RENAME_ATTEMPTS {
        let candidate_name = conflict::candidate_with_index(original, index);
        let candidate_name = candidate_name.to_string_lossy().into_owned();
        let candidate = infra::safe::join_under_root(root, &candidate_name)?;
        if !candidate.exists() {
            return Ok(candidate);
        }
    }
    Err(SinkError::RenameExhausted)
}

/// 清扫**属于其它对端**且**超过保留期**的遗留未完成文件（F-FT-19 / R-21）。
///
/// 返回清扫条数。
///
/// 🔴 只碰 `*.part`：成品文件无论如何都不动。
/// 🔴 保留期按**文件自身的修改时间**判，不用创建时间（未完成文件是持续写入的，
/// 其修改时间即"最后一次有人管它"的时刻）。
///
/// # Errors
/// 根目录不可读时返回 `io::Error`。
pub fn cleanup_orphans(
    root: &Path,
    keep_token: &str,
    now_ms: i64,
    retention: Duration,
) -> std::io::Result<usize> {
    let retention_ms = i64::try_from(retention.as_millis()).unwrap_or(i64::MAX);
    let mut removed = 0;
    for entry in fs::read_dir(root)? {
        let entry = entry?;
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        let name = entry.file_name().to_string_lossy().into_owned();
        let Some(token) = peer_token_of(&name) else {
            continue;
        };
        if token == keep_token {
            continue;
        }
        let modified_ms = entry
            .metadata()
            .and_then(|meta| meta.modified())
            .ok()
            .and_then(|time| time.duration_since(UNIX_EPOCH).ok())
            .map_or(0, |d| i64::try_from(d.as_millis()).unwrap_or(0));
        if now_ms.saturating_sub(modified_ms) < retention_ms {
            continue;
        }
        fs::remove_file(&path)?;
        removed += 1;
    }
    Ok(removed)
}

#[cfg(test)]
mod tests;
