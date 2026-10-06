//! 扫描：来源枚举 + 排除匹配 + 多来源去重（F-FB-02/13 · **R-8 / R-9 / R-10**）。
//!
//! 复用结论（DP-1）：③ 思路复用 ← `Core/Backup/Services/DirectoryScanner.cs:26-65`
//! （`Directory.EnumerateFileSystemEntries` + `ExcludedPathMatcher`）。
//! 母本用 .NET `FileSystemName.MatchesSimpleExpression` 做通配匹配，Rust 无对应物 ⇒
//! 本实现**自己写**通配匹配，但语义逐条对齐（见 [`glob_match`] 的注释）。
//!
//! **R-8 扫描语义（四条，全部继承）**：
//! 1. 排除式按「路径任一段命中」**或**「多段相对路径前缀命中」判定，大小写不敏感；
//! 2. 遇到链接/装载点**不跟随**并留警告（防环与防越界重复备份）；
//! 3. 多来源中若某来源被另一来源包含 ⇒ **移除内侧来源**（避免同一物理文件被备份两遍）；
//! 4. 同层枚举先目录后文件并按名称排序，保证顺序稳定（快照可比对）。
//!
//! **R-9 相对路径唯一性**：多来源合并时以「来源显示名」作前缀；前缀重名时追加来源短指纹后缀，
//! 仍冲突再追加一次 ⇒ **任何情况下都不允许两个文件在快照里互相覆盖**。

use super::models::{BackupSource, SourceType};
use std::collections::HashSet;
use std::path::{Path, PathBuf};

/// 一个待备份的文件（已解析出快照内的相对路径）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScannedFile {
    /// 绝对来源路径。
    pub absolute: PathBuf,
    /// 快照内的相对路径（🔴 恢复时须再过一次 `infra::safe::validate_relative`）。
    pub relative: String,
    /// 字节数。
    pub size: u64,
    /// 修改时间（Unix 毫秒）。
    pub modified_unix_ms: i64,
}

/// 扫描产出。
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ScanResult {
    /// 待备份文件。
    pub files: Vec<ScannedFile>,
    /// 空目录的相对路径（R-10：作为独立信息记入，恢复时重建）。
    pub empty_dirs: Vec<String>,
    /// 未跟随的链接/装载点（🔴 规格 §七·13：不跟随但**必须留警告**）。
    pub skipped_links: Vec<String>,
    /// 未产出任何内容的来源（不存在 / 全被排除）。
    ///
    /// 🔴 单独记而不混进 `skipped_links`：前者是「配置了但取不到」（用户可修），
    /// 后者是「源里有但我们不跟」（用户须知）。两者的处置完全不同。
    pub skipped_sources: Vec<String>,
    /// 因被其他来源包含而被移除的内侧来源（可解释性，R-8③）。
    pub dropped_contained_sources: Vec<String>,
    /// 总字节数。
    pub total_bytes: u64,
}

/// 排除式匹配器（F-FB-13：命中即不参与备份；大小写不敏感）。
#[derive(Debug, Clone)]
pub struct ExclusionMatcher {
    /// 单段 / 通配式（`node_modules` / `*.tmp` / `.git`）。
    segments: Vec<String>,
    /// 多段相对路径前缀（`bin/Debug`）。
    prefixes: Vec<Vec<String>>,
}

impl ExclusionMatcher {
    /// 从规则的 `exclusions` 构造。自动去空白、去重复（规格 F-FB-13）。
    #[must_use]
    pub fn new(raw: &[String]) -> Self {
        let mut segments = Vec::new();
        let mut prefixes = Vec::new();
        let mut seen = HashSet::new();
        for item in raw {
            let line = item.trim().replace('\\', "/");
            if line.is_empty() || !seen.insert(line.to_ascii_lowercase()) {
                continue;
            }
            let parts: Vec<String> = line
                .split('/')
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .map(ToOwned::to_owned)
                .collect();
            if parts.is_empty() {
                continue;
            }
            if parts.len() == 1 {
                segments.push(parts[0].to_ascii_lowercase());
            } else {
                prefixes.push(parts.iter().map(|s| s.to_ascii_lowercase()).collect());
            }
        }
        Self { segments, prefixes }
    }

    /// 该相对路径是否应被排除。
    #[must_use]
    pub fn is_excluded(&self, relative: &str) -> bool {
        let normalized = relative.replace('\\', "/").to_ascii_lowercase();
        let parts: Vec<&str> = normalized.split('/').filter(|s| !s.is_empty()).collect();

        // ① 路径任一段命中
        for part in &parts {
            if self
                .segments
                .iter()
                .any(|pattern| glob_match(pattern, part))
            {
                return true;
            }
        }
        // ② 多段相对路径在**任意层级**出现即命中
        //🔴 必须是滑窗而不是「从头对齐」：用户写 `bin/Debug` 的意图是排除**所有**
        //    `bin/Debug`，而不是只排除路径**开头**的那一处。第一版用 `zip` 对齐
        //    头部，实测 `proj/bin/Debug/app.exe` 不被排除 —— 由
        //    `exclusion_matches_multi_segment_prefix` 抓到。
        self.prefixes.iter().any(|prefix| {
            if parts.len() < prefix.len() {
                return false;
            }
            parts.windows(prefix.len()).any(|window| {
                prefix
                    .iter()
                    .zip(window.iter())
                    .all(|(w, g)| glob_match(w, g))
            })
        })
    }
}

/// `*` / `?` 通配匹配（大小写不敏感）。
///
/// 🔴 语义对齐 .NET `FileSystemName.MatchesSimpleExpression`（母本用的就是它）：
/// `*` 匹配**零个或多个**字符、`?` 匹配**恰好一个**。
/// 🔴 递归式实现（母本同款）而非转成正则——转正则会有转义字符差异，
/// 且 `?` 在 Windows 文件名里非法，用正则反而要处理无意义转义。
#[must_use]
pub fn glob_match(pattern: &str, text: &str) -> bool {
    let p: Vec<char> = pattern.to_ascii_lowercase().chars().collect();
    let t: Vec<char> = text.to_ascii_lowercase().chars().collect();
    glob_inner(&p, &t)
}

fn glob_inner(p: &[char], t: &[char]) -> bool {
    if p.is_empty() {
        return t.is_empty();
    }
    match p[0] {
        '*' => {
            // 试 0..=len 个字符全部可能（母本同款剪枝）
            for skip in 0..=t.len() {
                if glob_inner(&p[1..], &t[skip..]) {
                    return true;
                }
            }
            false
        }
        '?' => !t.is_empty() && glob_inner(&p[1..], &t[1..]),
        c => !t.is_empty() && t[0] == c && glob_inner(&p[1..], &t[1..]),
    }
}

/// 枚举一个来源下的全部文件（递归，不跟随链接）。
///
/// # Errors
/// 来源不存在 ⇒ 返回 `Ok((vec![], vec![来源路径]))`（**不报错**）——
/// F-FB-04 允许来源缺失时把规则存为草稿，执行时该来源计入失败项而非让整次备份崩掉。
#[must_use]
pub fn scan_source(
    source: &BackupSource,
    matcher: &ExclusionMatcher,
) -> (Vec<ScannedFile>, Vec<String>, Vec<String>) {
    let mut files = Vec::new();
    let mut empty_dirs = Vec::new();
    let mut skipped = Vec::new();
    let absolute = PathBuf::from(&source.path);

    match source.kind {
        SourceType::File => {
            // 单文件来源：排除式对它无效（用户已显式点名，F-FB-02）；
            // 相对路径 = 文件名本身（`strip_prefix(root)` 在 root==path 时得到空串，
            // 故这里直接用文件名，避免产出 `prefix/` 这种空相对路径）。
            if absolute.is_file() {
                if let Some(name) = absolute
                    .file_name()
                    .map(|n| n.to_string_lossy().into_owned())
                {
                    if let Ok(meta) = std::fs::metadata(&absolute) {
                        let modified = meta
                            .modified()
                            .ok()
                            .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                            .map_or(0, |d| i64::try_from(d.as_millis()).unwrap_or(i64::MAX));
                        files.push(ScannedFile {
                            absolute: absolute.clone(),
                            relative: name,
                            size: meta.len(),
                            modified_unix_ms: modified,
                        });
                    }
                }
            } else {
                skipped.push(source.path.clone());
            }
        }
        SourceType::Directory => {
            if !absolute.is_dir() {
                skipped.push(source.path.clone());
                return (files, empty_dirs, skipped);
            }
            let prefix = source_display_name(&source.path);
            walk_dir(
                &absolute,
                &absolute,
                &prefix,
                matcher,
                &mut files,
                &mut empty_dirs,
                &mut skipped,
            );
        }
    }
    (files, empty_dirs, skipped)
}

fn walk_dir(
    root: &Path,
    dir: &Path,
    prefix: &str,
    matcher: &ExclusionMatcher,
    files: &mut Vec<ScannedFile>,
    empty_dirs: &mut Vec<String>,
    skipped: &mut Vec<String>,
) {
    // 先按名称排序（R-8④：顺序稳定），再「先目录后文件」
    let Ok(entries) = std::fs::read_dir(dir) else {
        skipped.push(dir.display().to_string());
        return;
    };
    let mut names: Vec<PathBuf> = entries.flatten().map(|e| e.path()).collect();
    names.sort_by_key(|p| {
        (
            !p.is_dir(), // 目录在前
            p.file_name()
                .map(|n| n.to_string_lossy().to_ascii_lowercase()),
        )
    });

    let mut has_content = false;
    for path in names {
        let Some(name) = path.file_name().map(|n| n.to_string_lossy().into_owned()) else {
            continue;
        };
        let relative = format!("{prefix}/{name}");
        if matcher.is_excluded(&relative) {
            // 被排除也算"这里有东西" ⇒ 不能把父目录误记成空目录（R-10 的空目录
            // 会在恢复时被重建，若把「只含被排除项」的目录记成空目录，用户会看到
            // 源端明明没有的目录凭空出现）。
            has_content = true;
            continue;
        }

        // 🔴 R-8②：链接/装载点不跟随（防环、防越界重复备份）
        let is_symlink = std::fs::symlink_metadata(&path).is_ok_and(|m| m.file_type().is_symlink());
        if is_symlink {
            skipped.push(path.display().to_string());
            has_content = true;
            continue;
        }

        if path.is_dir() {
            let before_files = files.len();
            let before_dirs = empty_dirs.len();
            walk_dir(root, &path, &relative, matcher, files, empty_dirs, skipped);
            if files.len() == before_files && empty_dirs.len() == before_dirs {
                // 子目录整体为空 ⇒ 记入空目录清单（R-10）
                empty_dirs.push(relative.clone());
            } else {
                has_content = true;
            }
        } else if let Some(entry) = to_scanned(root, &path) {
            files.push(entry);
            has_content = true;
        }
    }
    // 🔴 本目录自身为空（无文件、无子目录、全部被排除）⇒ 记入空目录清单
    if !has_content && !prefix.contains('/') {
        // 根层不记（根是用户指定的存在物，不需要被"重建"）
    } else if !has_content {
        empty_dirs.push(prefix.to_owned());
    }
}

fn to_scanned(root: &Path, path: &Path) -> Option<ScannedFile> {
    let meta = std::fs::metadata(path).ok()?;
    let relative = path
        .strip_prefix(root)
        .ok()
        .map(|p| p.to_string_lossy().replace('\\', "/"))?;
    let modified = meta
        .modified()
        .ok()
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map_or(0, |d| i64::try_from(d.as_millis()).unwrap_or(i64::MAX));
    Some(ScannedFile {
        absolute: path.to_path_buf(),
        relative,
        size: meta.len(),
        modified_unix_ms: modified,
    })
}

/// 来源显示名（R-9：用作相对路径前缀，保证快照内路径可追溯到来源）。
#[must_use]
pub fn source_display_name(path: &str) -> String {
    // 🔴 `Path` 没有 `and_then`（那是 Option 的方法）；`file_name()` 返回 `Option<&OsStr>`，
    //    直接对它map/and_then 即可。
    Path::new(path)
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .filter(|n| !n.is_empty())
        .unwrap_or_else(|| "root".to_owned())
}

/// 移除被其他来源包含的内侧来源（R-8③）。
///
/// 同一物理文件被两个来源覆盖时，备份两遍既浪费时间又让快照体积翻倍。
#[must_use]
pub fn drop_contained_sources(sources: &[BackupSource]) -> Vec<BackupSource> {
    let mut kept: Vec<BackupSource> = Vec::new();
    for candidate in sources {
        // 🔴 **只有「目录被另一个目录包含」才剔除**。用户显式点名的单文件
        //    即便位于某个目录来源内部，也必须保留 —— 否则用户配的「这个文件也要备份」
        //    会被静默吞掉（由 `files_are_never_dropped_by_containment_rule` 抓到）。
        let contained = candidate.kind == SourceType::Directory
            && sources.iter().any(|other| {
                other.path != candidate.path
                    && other.kind == SourceType::Directory
                    && Path::new(&candidate.path).starts_with(Path::new(&other.path))
            });
        if !contained {
            kept.push(candidate.clone());
        }
    }
    kept
}

#[cfg(test)]
mod tests;
