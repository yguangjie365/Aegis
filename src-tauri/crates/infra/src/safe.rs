//! 路径越界守卫 —— **外部路径拼接的唯一入口**（02 专题 §5.1；04 分册 §一 目录树）。
//!
//! ```text
//! API：safe::join_under_root(allowed_root: &Path, external: &str) -> Result<PathBuf, PathEscape>
//! ```
//!
//! 规格原文（`Docs/开发文档/02-专题设计/02-安全与特权通道.md` §5.1）：
//! * 规则：拒绝 `..`、绝对路径、盘符、UNC、设备保留名；join 后 `canonicalize` 复验仍在允许根内
//!   （防符号链接逃逸）；越界即**拒绝请求**、不静默裁剪替换（05 R-19）。
//! * 必须走守卫的场景：接收文件的 `relativePath`、恢复包/备份包内路径、手机侧共享目录浏览与打包、
//!   解压目标、**一切"外部字符串 + 根目录"拼接**。
//! * 依赖审计：各域禁止 `Path::join` 直接消费外部输入。
//!
//! ## 为什么"不静默裁剪"是硬要求
//!
//! 把 `..\..\Windows\System32\x.dll` 清洗成 `Windows\System32\x.dll` 看似"更友好"，
//! 实则把一个**攻击尝试**变成一次**正常写入**：日志里看不出异常，而文件已经落在共享根里，
//! 用户还会以为是自己传的。故本模块只有两条出口：**原样通过**，或 **`Err`**。
//!
//! ## 两处实现要点（都有实测出处）
//!
//! 1. 🔴 **`canonicalize` 会泄漏 `\\?\` 前缀**（K-S01 探针实测：`D:\a` 52 字符 → `\\?\D:\a` 56 字符）。
//!    本模块内部比对前一律用 [`strip_verbatim_prefix`] 归一，**返回值也是干净路径**——
//!    否则 `\\?\D:\...` 会一路显示到界面上（接收目录、历史、日志）。
//! 2. 🔴 **目标文件常常还不存在**（"接收文件"就是刚创建）。此时 `canonicalize(目标)` 必失败，
//!    故本模块改为：先把**根**规范化，再**逐段**校验后做词法拼接，最后把
//!    **已存在的最深祖先**规范化复验仍在根内。这样既不要求目标已存在，又拦得住符号链接逃逸。
//!
//! ## 本模块**不做**的事
//!
//! * **不**给新文件名做"Windows 名称怪癖"清洗（结尾点/空格、非法字符、超长段）——
//!   那会误杀磁盘上**已经存在**的合法名字（浏览共享目录时就会拒绝掉真实文件）。
//!   产出新名字请用 [`sanitize_file_name`]（同一模块，职责分开）。
//! * **不**碰文件系统以外的任何东西（不做 IO 读写、不建目录）。

use std::path::{Component, Path, PathBuf};

/// 相对路径的最大段数（03 思路复用自母本 `SanitizeRelativePath`：段数上限 1–32）。
pub const MAX_SEGMENTS: usize = 32;

/// 单段最大字符数（母本同值 255；Windows 单文件名上限）。
pub const MAX_SEGMENT_CHARS: usize = 255;

/// 拒绝原因。**可区分**是为了让调用方能落出可排查的日志（规格要求每次拒绝落 LP-10 warn），
/// 而不是只知道"被拒了"。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PathEscape {
    /// 空串或全是空白
    Empty,
    /// 含 NUL 字节（Win32 会在 NUL 处截断，是经典的绕过手法）
    NulByte,
    /// 绝对路径（含 `\` 或 `/` 开头、或以根目录开头）
    Absolute,
    /// 盘符前缀（`C:`）—— 注意 `Path::components` 在前缀处即给出 `Prefix`
    DriveLetter,
    /// UNC（`\\server\share` 或 `//server/share`）
    Unc,
    /// 长路径/设备前缀（`\\?\` / `\\.\`）
    VerbatimPrefix,
    /// 含 `..` 段
    ParentDir,
    /// 含 `.` 段（不裁剪，直接拒）
    CurDir,
    /// 段数为 0 或超过 [`MAX_SEGMENTS`]
    SegmentCount(usize),
    /// 某段为空（连续分隔符）
    EmptySegment,
    /// 某段超长
    SegmentTooLong,
    /// Windows 设备保留名（`CON` / `NUL` / `COM1` …，**带扩展名也算**）
    ReservedName,
    /// 词法拼接后不在允许根内
    OutsideRoot,
    /// `canonicalize` 复验后发现真实落点在允许根外（符号链接/junction 逃逸）
    SymlinkEscape,
    /// 允许根本身无法规范化（不存在或无权限）
    RootUnusable,
}

impl std::fmt::Display for PathEscape {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let text = match self {
            Self::Empty => "路径为空",
            Self::NulByte => "路径含 NUL 字节",
            Self::Absolute => "不允许绝对路径",
            Self::DriveLetter => "不允许盘符",
            Self::Unc => "不允许 UNC 路径",
            Self::VerbatimPrefix => "不允许 \\\\?\\ 或 \\\\.\\ 前缀",
            Self::ParentDir => "路径含上级目录（..）",
            Self::CurDir => "路径含当前目录（.）段",
            Self::SegmentCount(_) => "路径段数超限",
            Self::EmptySegment => "路径含空段（连续分隔符）",
            Self::SegmentTooLong => "路径段过长",
            Self::ReservedName => "路径含 Windows 设备保留名",
            Self::OutsideRoot => "拼接结果不在允许根内",
            Self::SymlinkEscape => "真实落点经符号链接逃出允许根",
            Self::RootUnusable => "允许根不可用（不存在或无权限）",
        };
        f.write_str(text)
    }
}

impl std::error::Error for PathEscape {}

/// Windows 设备保留名（不含扩展名，大写比对）。母本 `IsWindowsReservedDeviceName` 同表。
const RESERVED_NAMES: &[&str] = &[
    "CON", "PRN", "AUX", "NUL", "COM1", "COM2", "COM3", "COM4", "COM5", "COM6", "COM7", "COM8",
    "COM9", "LPT1", "LPT2", "LPT3", "LPT4", "LPT5", "LPT6", "LPT7", "LPT8", "LPT9",
];

/// 剥掉 `\\?\` / `\\.\` 前缀（K-S01 实测：`canonicalize` 必加）。
///
/// 只在**确实是 verbatim 前缀**时剥；普通路径原样返回。
#[must_use]
pub fn strip_verbatim_prefix(path: &Path) -> PathBuf {
    let text = path.to_string_lossy();
    for prefix in ["\\\\?\\", "\\\\.\\"] {
        if let Some(rest) = text.strip_prefix(prefix) {
            // `\\?\UNC\server\share` → `\\server\share`（verbatim UNC 的还原形态）
            if let Some(unc) = rest.strip_prefix("UNC\\") {
                return PathBuf::from(format!("\\\\{unc}"));
            }
            return PathBuf::from(rest);
        }
    }
    path.to_path_buf()
}

/// 该段是否为设备保留名（`NUL`、`CON`、`COM1.txt` 都算）。
#[must_use]
pub fn is_reserved_name(segment: &str) -> bool {
    let stem = segment.split('.').next().unwrap_or(segment);
    RESERVED_NAMES
        .iter()
        .any(|name| stem.eq_ignore_ascii_case(name))
}

/// 逐段校验一个**外部来源的相对路径**，返回规范化后的段列表。
///
/// 🔴 只校验、不改写：任一非法即 `Err`，**绝不**把 `..\a` 变成 `a`。
///
/// # Errors
/// 见 [`PathEscape`] 各变体。
pub fn validate_relative(external: &str) -> Result<Vec<String>, PathEscape> {
    if external.contains('\0') {
        return Err(PathEscape::NulByte);
    }
    if external.trim().is_empty() {
        return Err(PathEscape::Empty);
    }
    let lowered = external.to_ascii_lowercase();
    if lowered.starts_with("\\\\?\\") || lowered.starts_with("\\\\.\\") {
        return Err(PathEscape::VerbatimPrefix);
    }
    // UNC：`\\server` 或 `//server`（先于"绝对路径"判，以便给出更准的原因）
    if external.starts_with("\\\\") || external.starts_with("//") {
        return Err(PathEscape::Unc);
    }
    let path = Path::new(external);
    // 🔴 **不能只看 `is_absolute()`**（2026-10-04 实测踩到）：Windows 上 `/etc/passwd` 与
    // `\Windows` 的 `is_absolute()` 是 **false**（它们"有根无盘符"），于是会漏进后面的逐段循环，
    // 最终被 `RootDir` 分支误报成"盘符"。改为**只看 components 的首个**，语义才准确：
    // `Prefix` = 有盘符（`C:\x` / `C:x`）→ DriveLetter；`RootDir` = 只有根分隔符 → Absolute。
    match path.components().next() {
        Some(Component::Prefix(_)) => return Err(PathEscape::DriveLetter),
        Some(Component::RootDir) => return Err(PathEscape::Absolute),
        _ => {}
    }
    if path.is_absolute() {
        // 兜底：理论上上面的 components 判定已覆盖，保留以防平台差异
        return Err(PathEscape::Absolute);
    }

    let mut segments = Vec::new();
    for component in path.components() {
        match component {
            Component::ParentDir => return Err(PathEscape::ParentDir),
            Component::CurDir => return Err(PathEscape::CurDir),
            Component::Normal(os) => {
                let Some(text) = os.to_str() else {
                    return Err(PathEscape::EmptySegment);
                };
                if text.is_empty() {
                    return Err(PathEscape::EmptySegment);
                }
                if text.chars().count() > MAX_SEGMENT_CHARS {
                    return Err(PathEscape::SegmentTooLong);
                }
                if is_reserved_name(text) {
                    return Err(PathEscape::ReservedName);
                }
                segments.push(text.to_owned());
            }
            Component::Prefix(_) | Component::RootDir => return Err(PathEscape::DriveLetter),
        }
    }
    // 连续分隔符（`a//b`）不会产生空 `Normal` 段，需显式查
    if external.contains("//") || external.contains("\\\\") {
        return Err(PathEscape::EmptySegment);
    }
    if segments.is_empty() {
        return Err(PathEscape::Empty);
    }
    if segments.len() > MAX_SEGMENTS {
        return Err(PathEscape::SegmentCount(segments.len()));
    }
    Ok(segments)
}

/// **外部路径拼接的唯一入口**（02 专题 §5.1）。
///
/// 把外部来源的相对路径 `external` 拼到 `allowed_root` 之下，并保证结果**确实落在根内**。
/// 越界即 `Err`（**不裁剪、不替换**）。
///
/// 返回的是**干净路径**（已剥 `\\?\`）——可直接用于 IO，也可直接显示给用户。
///
/// # Errors
/// * [`PathEscape::RootUnusable`] —— 根不存在或无法规范化
/// * 其余变体 —— 见 [`validate_relative`] 与符号链接复验
pub fn join_under_root(allowed_root: &Path, external: &str) -> Result<PathBuf, PathEscape> {
    let segments = validate_relative(external)?;

    // 根必须真实可用；规范化后剥 verbatim 前缀（K-S01 T8）
    let root = allowed_root
        .canonicalize()
        .map_err(|_| PathEscape::RootUnusable)?;
    let root = strip_verbatim_prefix(&root);

    // 词法拼接（此时每段都已被白名单校验过）
    let mut target = root.clone();
    for segment in &segments {
        target.push(segment);
    }

    // 词法上必须仍在根内（`join` 不会引入 `..`，但根若是相对路径就会出岔子，故双保险）
    if !is_lexically_under(&root, &target) {
        return Err(PathEscape::OutsideRoot);
    }

    // 🔴 canonicalize 复验：目标常常还不存在，故找**已存在的最深祖先**来验
    let deepest = deepest_existing_ancestor(&target);
    match deepest {
        Some(existing) => {
            let real = existing
                .canonicalize()
                .map_err(|_| PathEscape::RootUnusable)?;
            let real = strip_verbatim_prefix(&real);
            if !is_lexically_under(&root, &real) {
                return Err(PathEscape::SymlinkEscape);
            }
        }
        // 连根都不存在 —— 不该发生（根刚规范化过），保守拒绝
        None => return Err(PathEscape::RootUnusable),
    }

    Ok(target)
}

/// 词法包含判定：`child` 是否等于 `root` 或位于 `root` 之下（大小写不敏感，Windows 语义）。
#[must_use]
pub fn is_lexically_under(root: &Path, child: &Path) -> bool {
    let root_text = root.to_string_lossy().to_lowercase();
    let child_text = child.to_string_lossy().to_lowercase();
    if child_text == root_text {
        return true;
    }
    let mut prefix = root_text;
    if !prefix.ends_with('\\') && !prefix.ends_with('/') {
        prefix.push('\\');
    }
    child_text.starts_with(&prefix)
}

/// 目标自身或**最深的已存在祖先**（目标尚不存在时用）。
fn deepest_existing_ancestor(target: &Path) -> Option<PathBuf> {
    let mut current = Some(target);
    while let Some(path) = current {
        if path.exists() {
            return Some(path.to_path_buf());
        }
        current = path.parent();
    }
    None
}

/// 去掉**不可见字符**（零宽、BOM、软连字符、方向控制符）。
///
/// 与 [`sanitize_file_name`] 共用同一张字符表——"外部来的字符串"（文件名、设备显示名、
/// 对端自报的名字）都需要这一步，故抽成公开原语而非各处重抄一张表（DP-1 复用优先）。
///
/// 🔴 只去不可见字符，**不做**其它清洗：调用方按自己的语义决定后续（文件名要替换非法字符，
/// 设备名可能只需要 trim 与回退）。
#[must_use]
pub fn strip_invisible(raw: &str) -> String {
    raw.chars().filter(|c| !is_invisible(*c)).collect()
}

/// 产出一个**可安全落盘的新文件名**（写侧专用；母本 `SanitizeFileName` ③思路复用）。
///
/// 与 [`join_under_root`] 的分工：后者管"能不能落在根内"，本函数管"这个名字在 Windows 上
/// 会不会被悄悄改写"。三处处理：
/// 1. 去掉不可见字符（零宽/BOM/控制符）——母本实证：不剥会落盘成 `a\u{200B}b.txt`
/// 2. 非法字符（`\ / : * ? " < > |` 与控制符）替换为 `_`
/// 3. 保留名加 `_` 前缀；结尾的点与空格去掉（Win32 会静默裁掉，导致日志名 ≠ 实际名）
///
/// 返回 `None` 表示清洗后为空（调用方应拒绝该名字，而不是自己编一个）。
#[must_use]
pub fn sanitize_file_name(raw: &str) -> Option<String> {
    let stripped: String = strip_invisible(raw)
        // 结尾的点与空格由 Win32 静默裁掉 → 日志名与实际名不一致
        .trim_end_matches(['.', ' '])
        .to_owned();
    if stripped.is_empty() {
        return None;
    }
    let mapped: String = stripped
        .chars()
        .map(|c| {
            if matches!(c, '\\' | '/' | ':' | '*' | '?' | '"' | '<' | '>' | '|') || c.is_control() {
                '_'
            } else {
                c
            }
        })
        .collect();
    if mapped == "." || mapped == ".." || mapped.is_empty() {
        return None;
    }
    let result = if is_reserved_name(&mapped) {
        format!("_{mapped}")
    } else {
        mapped
    };
    if result.chars().count() > MAX_SEGMENT_CHARS {
        return None;
    }
    Some(result)
}

/// 不可见字符（零宽、BOM、软连字符、方向控制符）。
fn is_invisible(c: char) -> bool {
    matches!(
        c,
        '\u{200B}'..='\u{200F}' | '\u{202A}'..='\u{202E}' | '\u{2060}'..='\u{2064}' | '\u{FEFF}' | '\u{00AD}'
    )
}

#[cfg(test)]
mod tests;
