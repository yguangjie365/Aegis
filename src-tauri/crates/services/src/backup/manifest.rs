//! 快照目录命名、清单读写与轻量元信息（F-FB-26/30/32 · R-18）。
//!
//! 复用结论（DP-1）：③ 思路复用 ← `Core/Backup/Services/SnapshotManager.cs`
//! 与 `SnapshotInfo.cs`。**两处母本 bug 修复必须继承**：
//! 1. 🔴 **TOCTOU**：母本先 `Directory.Exists` 再 `Directory.CreateDirectory`，
//!    两步之间同刻并发会产生同名目录互相覆盖。修法是**随机后缀**（F-FB-32）。
//! 2. 🔴 **时间解析判错**（母本 2026-09-11 实测修复）：随机后缀取自
//!    `Path.GetRandomFileName()` 的基 32 字符集，**可能含字母**（`u`/`y` 等专用于
//!    避免 8.3 文件名冲突）。母本原按「`_` 之后是纯数字」判断时间，会漏掉这类快照。
//!    ⇒ 本实现**按前 15 位固定宽度**解析时间，与后缀是否为数字无关。
//!
//! 物理落点结构（规格 §五）：`备份根 → 规则专属目录 → 快照（按时间命名）→
//! 数据副本目录 + 清单文件 + 快速读取的元信息文件`。

use super::models::{FileEntry, IntegrityStatus, SnapshotInfo, SnapshotManifest, SnapshotStatus};
use std::io;
use std::path::{Path, PathBuf};

/// 快照目录名里时间戳的固定宽度（`20261005_153000`）。
const TIME_WIDTH: usize = 15;
/// 时间戳与去重后缀之间的分隔符。
const SUFFIX_SEP: char = '_';
/// 清单文件名。
pub const MANIFEST_FILE: &str = "manifest.json";
/// 轻量元信息文件名（F-FB-26：列表只读它，不整份载入清单）。
pub const INFO_FILE: &str = "snapshot.json";
/// 快照内的数据副本目录名。
pub const DATA_DIR: &str = "data";

/// 把 Unix 毫秒格式化为快照目录用的本地时间戳（`20261005_153000`，**本地时区** UTC+8）。
///
/// 🔴 `pub` 是因为 [`super::portability::local_iso8601`] 要复用同一套日历算法 ——
///   同一份「本地时间」口径散落两处就会漂 8 小时。
#[must_use]
pub fn format_snapshot_stamp(unix_ms: i64) -> String {
    let (year, month, day) = super::retention::civil_from_unix_ms(unix_ms);
    let local = unix_ms.saturating_add(8 * 3_600 * 1_000);
    let secs_of_day = local.rem_euclid(86_400);
    let hour = secs_of_day / 3_600;
    let minute = (secs_of_day % 3_600) / 60;
    let second = secs_of_day % 60;
    format!("{year:04}{month:02}{day:02}{SUFFIX_SEP}{hour:02}{minute:02}{second:02}")
}

/// 生成一个**不存在**的快照目录路径（F-FB-32：同一刻并发产生快照时追加随机后缀）。
///
/// 🔴 不用「检查存在 → 创建」两步（母本 TOCTOU bug），而是**用随机后缀保证概率上不冲突**，
/// 再由 [`create_snapshot_dir`] 独占创建；真撞上时重试若干次。
#[must_use]
pub fn snapshot_dir_name(unix_ms: i64) -> String {
    let stamp = format_snapshot_stamp(unix_ms);
    // 6 位随机（母本同款长度）。🔴 只用于**去重**，展示时必须隐藏（见 `display_dir_name`）。
    //   生成器走 [`super::ids`]（域内唯一一份）—— 本文件原先自带的那份
    //   结构上与规则 id 的同款缺陷（「取模 + 除法」推进），只因只取 6 位
    //   没耗尽 `u64` 种子才侥幸没暴露。见 `ids.rs` 文档。
    let suffix = super::ids::random_token(SUFFIX_LEN, super::ids::ALNUM_LOWER);
    format!("{stamp}{SUFFIX_SEP}{suffix}")
}

/// 展示用目录名：去掉去重后缀，用户只看到可读时间（F-FB-32）。
#[must_use]
pub fn display_dir_name(dir_name: &str) -> String {
    if dir_name.len() > TIME_WIDTH + 1 {
        dir_name[..TIME_WIDTH].to_owned()
    } else {
        dir_name.to_owned()
    }
}

/// 从目录名解析时间。
///
/// 🔴 **按前 15 位固定宽度解析**（母本 2026-09-11 修复）：绝不按「`_` 之后是纯数字」判断——
/// 随机后缀可能含字母（`Path.GetRandomFileName` 基 32字符集含 `u`/`y`）。
///
/// `None` = **无法解析** ⇒ 调用方按 R-5「解析不出时间的快照一律跳过不裁剪」处理。
#[must_use]
pub fn parse_snapshot_time(dir_name: &str) -> Option<i64> {
    let stamp = dir_name.get(..TIME_WIDTH)?;
    let digits: Vec<char> = stamp.chars().collect();
    // 形状必须是 `8 位日期 + '_' + 6 位时间`；长度不足或分隔符位置不对即 None
    if digits.len() != TIME_WIDTH || digits[8] != SUFFIX_SEP {
        return None;
    }
    // 🔴 逐段解析而非 `format!` 往返：省掉 6 次字符串分配，也避免"解析失败但已赋值"的中间态
    let num = |from: usize, to: usize| -> Option<i64> {
        let mut acc: i64 = 0;
        for &c in &digits[from..to] {
            if !c.is_ascii_digit() {
                return None;
            }
            acc = acc * 10 + i64::from(c.to_digit(10)?);
        }
        Some(acc)
    };
    let year = num(0, 4)?;
    let month = num(4, 6)?;
    let day = num(6, 8)?;
    let hour = num(9, 11)?;
    let minute = num(11, 13)?;
    let second = num(13, 15)?;
    if !(1..=12).contains(&month) || !(1..=31).contains(&day) {
        return None;
    }
    if hour > 23 || minute > 59 || second > 59 {
        return None;
    }
    // 🔴 **时区必须与 `format_snapshot_stamp` 对称**：目录名里的字段是**本地时间**（UTC+8），
    //    而 Unix 毫秒是 UTC ⇒ 反解时必须减掉同一个 8 小时偏移。
    //    第一版忘了减，`20261005_153000` 被解成 1657 年（正好差 8 小时），
    //    症状是快照列表里所有时间都显示成 17xx 年 —— 由
    //    `parses_time_even_when_random_suffix_contains_letters` 抓到。
    // 🔴 不用 as u32：month/day 已在上面判过 (1..=12) / (1..=31)，
    //    但 as 丢符号这件事 clippy 看不到、也拦不住 —— 用 try_from 让
    //    「越界即 None」在类型上成立，不再依赖读者记得前面的范围检查。
    let month_u32 = u32::try_from(month).ok()?;
    let day_u32 = u32::try_from(day).ok()?;
    let days = days_from_civil(year, month_u32, day_u32);
    let local_secs = days * 86_400 + hour * 3_600 + minute * 60 + second;
    let utc_secs = local_secs - LOCAL_TZ_OFFSET_SECS;
    Some(utc_secs.saturating_mul(1_000))
}

/// 本地时区偏移（秒）。固定 UTC+8 —— 理由同 [`crate::backup::retention::civil_from_unix_ms`]：
/// 备份快照按日历日归档，用机器当前时区会让同一份快照在不同时区的机器上落到不同的日桶，
/// 破坏保留策略的确定性。
const LOCAL_TZ_OFFSET_SECS: i64 = 8 * 3_600;

/// 快照目录去重后缀的长度（母本同款）。
const SUFFIX_LEN: usize = 6;

/// 规则专属目录（`备份根 → 规则专属目录`）。
///
/// 🔴 规则名可能含非法字符 ⇒ 走 `infra::safe::sanitize_file_name`（写侧唯一入口）。
/// 用 `规则 id` 而非规则名做目录名，避免改规则名后旧快照"消失"（F-FB-05：
/// 「改备份根后旧落点快照仍被识别展示」—— 同理，改名也不该让快照失联）。
#[must_use]
pub fn rule_dir_name(rule_id: &str) -> String {
    infra::safe::sanitize_file_name(rule_id).unwrap_or_else(|| "rule".to_owned())
}

/// 独占创建快照目录（母本 TOCTOU 的修法：随机后缀 + 独占创建 + 有限重试）。
///
/// # Errors
/// 重试若干次仍失败 ⇒ 返回 `io::Error`（**不静默复用已有目录**，那会覆盖别人的快照）。
pub fn create_snapshot_dir(backup_root: &Path, rule_id: &str, unix_ms: i64) -> io::Result<PathBuf> {
    let base = backup_root.join(rule_dir_name(rule_id));
    std::fs::create_dir_all(&base)?;
    let mut last: Option<io::Error> = None;
    for _ in 0..8 {
        let dir = base.join(snapshot_dir_name(unix_ms));
        match std::fs::create_dir(&dir) {
            Ok(()) => return Ok(dir),
            // 🔴 已存在 ⇒ 换下一个随机后缀重试（同名即并发，母本 TOCTOU 的修法）
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => last = Some(error),
            Err(error) => return Err(error),
        }
    }
    Err(last.unwrap_or_else(|| io::Error::other("连续多次撞名，快照目录创建失败")))
}

/// 快照内数据副本目录。
#[must_use]
pub fn data_dir_of(snapshot_dir: &Path) -> PathBuf {
    snapshot_dir.join(DATA_DIR)
}

/// 写快照清单（R-18：🔴 清单写入失败时**保留数据目录**并报「已保留待人工处理」，
/// 绝不静默删除用户已备份的内容）。
///
/// 落盘经 `infra::fs::atomic_write`（02 册 §5.2 强制原子写）。
///
/// # Errors
/// 序列化失败或落盘失败 ⇒ `io::Error`；调用方**必须**保留数据目录并显式上报。
pub fn write_manifest(snapshot_dir: &Path, manifest: &SnapshotManifest) -> io::Result<()> {
    let bytes = serde_json::to_vec_pretty(manifest)
        .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, format!("清单序列化失败：{e}")))?;
    infra::fs::atomic_write(&snapshot_dir.join(MANIFEST_FILE), &bytes)
}

/// 写轻量元信息（列表只读它，F-FB-26「不整份载入文件清单」）。
///
/// # Errors
/// 同 [`write_manifest`]。
pub fn write_snapshot_info(snapshot_dir: &Path, info: &SnapshotInfo) -> io::Result<()> {
    let bytes = serde_json::to_vec_pretty(info).map_err(|e| {
        io::Error::new(io::ErrorKind::InvalidData, format!("元信息序列化失败：{e}"))
    })?;
    infra::fs::atomic_write(&snapshot_dir.join(INFO_FILE), &bytes)
}

/// 读快照清单。
///
/// 🔴 缺失或损坏都返回 `Err`（规格 §七·2：报告「清单读取失败」），
/// 绝不用「尽力解析部分字段」蒙混过去 —— 那会让不可信的清单驱动恢复。
///
/// # Errors
/// 文件不存在 / JSON 不合法 / 字段类型不符。
pub fn read_manifest(snapshot_dir: &Path) -> Result<SnapshotManifest, ManifestError> {
    let path = snapshot_dir.join(MANIFEST_FILE);
    let text = std::fs::read_to_string(&path).map_err(|source| ManifestError::Io {
        path: path.clone(),
        source,
    })?;
    serde_json::from_str(&text).map_err(|source| ManifestError::Parse {
        path: path.clone(),
        source,
    })
}

/// 读轻量元信息。
///
/// # Errors
/// 同 [`read_manifest`]。
pub fn read_snapshot_info(snapshot_dir: &Path) -> Result<SnapshotInfo, ManifestError> {
    let path = snapshot_dir.join(INFO_FILE);
    let text = std::fs::read_to_string(&path).map_err(|source| ManifestError::Io {
        path: path.clone(),
        source,
    })?;
    serde_json::from_str(&text).map_err(|source| ManifestError::Parse {
        path: path.clone(),
        source,
    })
}

/// 清单读取失败（🔴 原因码固定为 `MANIFEST_READ_FAILED`，UI 据此给可读文案）。
#[derive(Debug)]
pub enum ManifestError {
    /// 读不到（不存在 / 权限）。
    Io {
        path: PathBuf,
        source: std::io::Error,
    },
    /// 解析失败（JSON 损坏 / 字段类型不符）。
    Parse {
        path: PathBuf,
        source: serde_json::Error,
    },
}

impl std::fmt::Display for ManifestError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io { path, source } => {
                write!(f, "读不到快照清单（{}）：{source}", path.display())
            }
            Self::Parse { path, source } => {
                write!(f, "快照清单已损坏（{}）：{source}", path.display())
            }
        }
    }
}

impl std::error::Error for ManifestError {}

/// 枚举某规则的全部快照目录（按时间倒序）。
///
/// 🔴 只列**目录**且**能解析出时间**的项；被手动删除的目录自然不在列表里（F-FB-26）。
///解析不出的目录**跳过而非报错**（它可能在做人工恢复），但调用方应能感知——
/// 故本函数同时返回无法解析的名字。
#[must_use]
pub fn list_snapshot_dirs(
    backup_root: &Path,
    rule_id: &str,
) -> (Vec<SnapshotDirEntry>, Vec<String>) {
    let base = backup_root.join(rule_dir_name(rule_id));
    let mut parsed: Vec<SnapshotDirEntry> = Vec::new();
    let mut unparsed: Vec<String> = Vec::new();
    let Ok(entries) = std::fs::read_dir(&base) else {
        return (parsed, unparsed);
    };
    for entry in entries.flatten() {
        if !entry.path().is_dir() {
            continue;
        }
        let name = entry.file_name().to_string_lossy().into_owned();
        match parse_snapshot_time(&name) {
            Some(created) => parsed.push(SnapshotDirEntry {
                dir: entry.path(),
                dir_name: name,
                created_unix_ms: created,
            }),
            None => unparsed.push(name),
        }
    }
    // 时间倒序（新→旧），让调用方的「最新一份」就是 [0]
    parsed.sort_by_key(|e| std::cmp::Reverse(e.created_unix_ms));
    (parsed, unparsed)
}

/// 一个已解析的快照目录。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SnapshotDirEntry {
    pub dir: PathBuf,
    pub dir_name: String,
    pub created_unix_ms: i64,
}

impl SnapshotDirEntry {
    /// 构造一份**完整成功**的 [`SnapshotInfo`]（读元信息失败时的兜底不可用，
    /// 故本函数只给"从目录能确定的部分"，文件数等需读清单才知）。
    fn info_shell(&self, rule_id: &str, rule_name: &str) -> SnapshotInfo {
        SnapshotInfo {
            rule_id: rule_id.to_owned(),
            rule_name: rule_name.to_owned(),
            created_unix_ms: self.created_unix_ms,
            file_count: 0,
            total_bytes: 0,
            status: SnapshotStatus::Failed, // 🔴 读不到元信息时**不假装成功**（R-13诚实性）
            integrity: IntegrityStatus::NotVerified,
            reused_count: 0,
            failed_count: 0,
            display_path: self.dir.display().to_string(),
        }
    }

    /// 读该快照的元信息；元信息缺失或损坏 ⇒ 返回一个**明确标为失败**的壳。
    ///
    /// 🔴 绝不用目录名猜一个"看起来正常"的记录—— 那会让用户点进去发现是空的。
    #[must_use]
    pub fn load_info(&self, rule_id: &str, rule_name: &str) -> SnapshotInfo {
        read_snapshot_info(&self.dir).unwrap_or_else(|_| self.info_shell(rule_id, rule_name))
    }
}

/// 汇总清单为轻量信息（F-FB-26：列表不整份载入清单）。
#[must_use]
pub fn summarize(manifest: &SnapshotManifest) -> SnapshotInfo {
    SnapshotInfo {
        rule_id: manifest.rule_id.clone(),
        rule_name: manifest.rule_name.clone(),
        created_unix_ms: manifest.created_unix_ms,
        file_count: manifest.file_count,
        total_bytes: manifest.total_bytes,
        status: manifest.status,
        integrity: manifest.integrity,
        reused_count: manifest.reused_count,
        failed_count: 0,
        display_path: String::new(),
    }
}

/// [`build_manifest`] 的输入（🔴 原为 10 个位置参数，逐个 `rule_id` / `rule_name` /
/// `snapshot_id` 极易传错序且编译器不报错—— 同类型的 `&str` 互换是静默的）。
#[derive(Debug, Clone)]
pub struct ManifestInput {
    pub rule_id: String,
    pub rule_name: String,
    pub snapshot_id: String,
    pub created_unix_ms: i64,
    pub sources: Vec<String>,
    pub entries: Vec<FileEntry>,
    pub empty_dirs: Vec<String>,
    pub status: SnapshotStatus,
    pub integrity: IntegrityStatus,
    pub reused_count: u64,
}

/// 由扫描结果与复制结果构建清单（`engine` 调用）。
#[must_use]
pub fn build_manifest(input: ManifestInput) -> SnapshotManifest {
    let total_bytes = input.entries.iter().map(|e| e.size).sum();
    SnapshotManifest {
        rule_id: input.rule_id,
        rule_name: input.rule_name,
        snapshot_id: input.snapshot_id,
        created_unix_ms: input.created_unix_ms,
        sources: input.sources,
        file_count: input.entries.len() as u64,
        total_bytes,
        entries: input.entries,
        empty_dirs: input.empty_dirs,
        status: input.status,
        integrity: input.integrity,
        reused_count: input.reused_count,
        format_version: super::models::manifest_format_version(),
    }
}

/// 清单条目构造（`engine` 的复制循环调用）。
#[must_use]
pub fn file_entry(
    relative: &str,
    original: &Path,
    size: u64,
    modified_unix_ms: i64,
    sha256: String,
) -> FileEntry {
    FileEntry {
        relative_path: relative.to_owned(),
        original_path: original.display().to_string(),
        size,
        modified_unix_ms,
        sha256,
    }
}

/// 读取 Unix 毫秒 → 快照信息里的时间；`None` 表示无法解析。
#[must_use]
pub fn info_created(entry: &SnapshotDirEntry) -> Option<i64> {
    parse_snapshot_time(&entry.dir_name)
}

/// 把 `SnapshotInfo` 的 `display_path` 补成「展示用目录名」。
#[must_use]
pub fn display_path_of(snapshot_dir: &Path) -> String {
    snapshot_dir.file_name().map_or_else(
        || snapshot_dir.display().to_string(),
        |n| display_dir_name(&n.to_string_lossy()),
    )
}

/// 反向：由 `civil` 算回天数（Howard Hinnant `days_from_civil`）。
fn days_from_civil(year: i64, month: u32, day: u32) -> i64 {
    let y = if month <= 2 { year - 1 } else { year };
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let m = i64::from(month);
    let d = i64::from(day);
    let doy = (153 * (if m > 2 { m - 3 } else { m + 9 }) + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

#[cfg(test)]
mod tests;
