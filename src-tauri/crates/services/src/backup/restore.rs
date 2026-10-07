//! 恢复引擎：预演与执行（**F-FB-35 同判据** · R-1/R-2/R-3/R-4/R-6）。
//!
//! 复用结论（DP-1）：③ 思路复用 ← `Core/Backup/Services/RestoreService.cs`
//! （冲突预演 + 逐项写入 + 读回比对）。
//!
//! 🔴 **本模块最重要的设计：预演与执行共用同一个 `plan()`**
//! （规格 F-FB-35「预演与真实执行使用同一套判据」；
//! 规格 §六一致性「同一判据只实现一次并被各处共用，避免两处漂移造成
//! '预演说没问题、执行却失败'」）。**任何只改其中一方的改动都是 bug。**
//!
//! 🔴 **R-2 恢复落点守卫**（恢复到原位置时逐条校验，任一不过即拒绝该条并说明
//! 「疑似快照数据被篡改」）：
//! ① 本程序自身的受保护目录（配置与记录所在处）禁止写入；
//! ② 来源范围白名单：**优先用规则当前配置**的来源集合，取不到才回退快照自带来源
//!    （并在日志标注这是**弱信任**）。
//!
//! 🔴 **2026-10-07：原第①条「系统关键路径（系统目录 / 程序安装目录 / 磁盘根）
//!    一律禁止写入」已整体移除**，依据是产品定位（用户两次澄清）：
//!    - 本产品核心用例是备份与恢复**系统与软件的设置文件**，它们多位于系统盘
//!      关键位置（典型如 `C:\Windows\...`）；
//!    - 且存在把**单文件程序直接放在磁盘根**（如 `D:\`）的习惯用法，重装系统后
//!      要「一键恢复到原始目录」。
//!    ⇒ 任何按路径黑名单拦「系统目录 / 盘符根」的守卫，都会让主场景直接不可用。
//!    它想防的穿越/篡改由 `infra::safe::validate_relative` 逐段校验承担
//!    （**穿越防护不依赖路径黑名单**），黑名单只是额外误伤合法来源。

use super::dto::{FailureView, RestoreOutcome, RestorePlanView};
use super::models::{BackupRule, ConflictPolicy, SnapshotManifest};
use std::collections::HashSet;
use std::path::{Path, PathBuf};

/// 恢复目标。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RestoreTarget {
    /// 回到原始位置（🔴 需过三重守卫）。
    Original,
    /// 恢复到指定目录。
    Custom(PathBuf),
}

/// 恢复预演的一项判定结果。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ItemVerdict {
    /// 全新写入（目标不存在）。
    FreshWrite,
    /// 目标已存在 ⇒ 按策略处理。
    Conflict,
    /// 被安全守卫拒绝（R-2）。
    GuardRejected(FailureView),
    /// 清单条目本身不合法（R-1：整次恢复应中止，此处仅标记）。
    UnsafeEntry(FailureView),
}

/// 一次恢复的完整计划（🔴 预演与执行共用）。
#[derive(Debug, Clone, PartialEq)]
pub struct RestorePlan {
    /// 逐条判定（与清单 `entries` 同序）。
    pub items: Vec<(String, ItemVerdict)>,
    /// 生效的目标根（`Original` 时取规则首个来源）。
    pub target_root: PathBuf,
    /// 生效的策略。
    pub policy: ConflictPolicy,
    /// 待恢复总字节数。
    pub total_bytes: u64,
    /// 🔴 R-1：**清单存在不安全路径** ⇒ 整次恢复必须中止。
    pub has_unsafe_entry: bool,
}

impl RestorePlan {
    /// 四项计数（F-FB-35）。
    #[must_use]
    pub fn to_view(&self, target: String) -> RestorePlanView {
        let mut conflicts = 0;
        let mut guard_rejected = 0;
        let mut fresh = 0;
        for (_, verdict) in &self.items {
            match verdict {
                ItemVerdict::FreshWrite => fresh += 1,
                ItemVerdict::Conflict => conflicts += 1,
                ItemVerdict::GuardRejected(_) | ItemVerdict::UnsafeEntry(_) => {
                    guard_rejected += 1;
                }
            }
        }
        RestorePlanView {
            total: self.items.len() as u64,
            conflicts,
            guard_rejected,
            fresh_writes: fresh,
            target,
            policy: self.policy,
        }
    }
}

/// 构建恢复计划（🔴 **预演与执行的唯一入口**）。
///
/// # Errors
/// 无（R-1 的不安全条目**不**在此返回 `Err`，而是标记在 `has_unsafe_entry`，
/// 由调用方在**执行前**中止 —— 预演需要把问题条目如实展示给用户）。
#[must_use]
pub fn plan(
    manifest: &SnapshotManifest,
    rule: Option<&BackupRule>,
    target: &RestoreTarget,
    policy: ConflictPolicy,
) -> RestorePlan {
    let effective_policy = policy.degrade_without_prompt();
    let target_root = resolve_target_root(manifest, rule, target);
    //🔴 R-2③ 白名单：**优先**规则当前来源；取不到才回退快照自带（弱信任）
    let whitelist: HashSet<String> = match rule {
        Some(r) if !r.sources.is_empty() => r
            .sources
            .iter()
            .map(|s| s.path.to_ascii_lowercase())
            .collect(),
        // 🔴 弱信任：规则不可用（删了/未传入）⇒ 回退到清单自带来源。
        //   调用方须落日志标注「已回退到快照自带来源」。
        _ => manifest
            .source_whitelist()
            .iter()
            .map(|s| s.to_ascii_lowercase())
            .collect(),
    };

    let mut items = Vec::with_capacity(manifest.entries.len());
    let mut has_unsafe_entry = false;

    for entry in &manifest.entries {
        // ── R-1：清单不可信，逐条校验相对路径 ──
        let Ok(segments) = infra::safe::validate_relative(&entry.relative_path) else {
            has_unsafe_entry = true;
            items.push((
                entry.relative_path.clone(),
                ItemVerdict::UnsafeEntry(FailureView {
                    code: super::reasons::GUARD_REJECTED.to_owned(),
                    text: format!(
                        "清单中的相对路径不安全：{}",
                        unsafe_reason(&entry.relative_path)
                    ),
                    path: entry.relative_path.clone(),
                }),
            ));
            continue;
        };

        // 目标落点 = 目标根 / 逐段拼接（🔴 段已由 validate_relative 校验过）
        let mut dest = target_root.clone();
        for segment in &segments {
            dest.push(segment);
        }

        // ── R-2 落点守卫（仅在恢复到原位置时逐条校验，规格 F-FB-38「文件与目录同规则」）──
        if matches!(target, RestoreTarget::Original) {
            if let Some(failure) = guard_check(&dest, &whitelist, &entry.original_path) {
                items.push((
                    entry.relative_path.clone(),
                    ItemVerdict::GuardRejected(failure),
                ));
                continue;
            }
        }

        // 🔴 自定义目标同样要过「不能写进本程序自己的数据目录」——
        //   否则恢复能把备份记录/配置覆盖掉（R-2② 的意图不因目标选择而消失）
        if let Some(failure) = guard_protected_dir(&dest) {
            items.push((
                entry.relative_path.clone(),
                ItemVerdict::GuardRejected(failure),
            ));
            continue;
        }

        items.push((
            entry.relative_path.clone(),
            if dest.exists() {
                ItemVerdict::Conflict
            } else {
                ItemVerdict::FreshWrite
            },
        ));
    }

    RestorePlan {
        items,
        target_root,
        policy: effective_policy,
        total_bytes: manifest
            .entries
            .iter()
            .filter(|e| !e.relative_path.is_empty())
            .map(|e| e.size)
            .sum(),
        has_unsafe_entry,
    }
}

/// 解析实际目标根。
///
/// 规格 §七·5：多来源规则恢复到「**首个来源路径**」，并显式警告其余来源未被原位还原。
fn resolve_target_root(
    manifest: &SnapshotManifest,
    rule: Option<&BackupRule>,
    target: &RestoreTarget,
) -> PathBuf {
    match target {
        RestoreTarget::Custom(dir) => dir.clone(),
        RestoreTarget::Original => rule
            .and_then(|r| r.sources.first().map(|s| PathBuf::from(&s.path)))
            .or_else(|| manifest.sources.first().map(PathBuf::from))
            // 🔴 既无规则又无清单来源 ⇒ 落在盘根（后续守卫会拦住，不静默写根）
            .unwrap_or_else(|| PathBuf::from("\\")),
    }
}

/// R-2 三重守卫的逐条校验。
/// R-2 恢复落点守卫。
///
/// 🔴 2026-10-07：**守卫①（系统关键路径 / 磁盘根）已整体取消**，
///   依据是产品定位（用户两次澄清）：
///   - 本产品核心用例是备份与恢复**系统与软件的设置文件**，它们多位于
///     系统盘关键位置（典型如 `C:\Windows\...`）；
///   - 且存在把**单文件程序直接放在磁盘根目录**（如 `D:\`）的习惯用法，
///     重装系统后要「一键恢复到原始目录」。
///   ⇒ 任何按路径黑名单拦「系统目录 / 盘符根」的守卫，都会让主场景直接不可用。
///
///   它原本想防的「快照数据被篡改 ⇒ 写到系统位置」，由
///   [`infra::safe::validate_relative`] 的逐段校验（禁 `..`、保留名、不可见字符）
///   承担 —— **穿越防护不依赖路径黑名单**，黑名单只是额外误伤合法来源。
///
///   保留的两条守卫都是**语义级**判据，与「用户把文件放在哪」无关：
///   守卫② 本程序自身的数据目录（恢复不能覆盖备份记录与配置本身）；
///   守卫③ 来源范围白名单（目标必须落在该快照的来源集合内）。
fn guard_check(dest: &Path, whitelist: &HashSet<String>, original: &str) -> Option<FailureView> {
    guard_protected_dir(dest).or_else(|| guard_whitelist(dest, original, whitelist))
}

fn reject(path: &Path, why: &str) -> FailureView {
    FailureView {
        code: super::reasons::GUARD_REJECTED.to_owned(),
        text: format!("{why}；疑似快照数据被篡改"),
        path: path.display().to_string(),
    }
}

/// 🔴 原「守卫①：系统关键路径」已于 2026-10-07 **整体移除**（判定见 `guard_check`
///   的文档注释）：它拦的 `\windows\` / `\system32\` / `\program files` / 磁盘根
///   恰好覆盖本产品的主用例（备份系统与软件设置文件、单文件程序放盘根），
///   而它想防的穿越/篡改已由 [`infra::safe::validate_relative`] 逐段校验承担。
#[cfg(test)]
mod guard_system_path_removed_tests {
    use super::*;

    /// 🔴 回归测试：源在 `C:\Windows` 下的条目**必须能恢复到原位置**。
    ///   这条在移除守卫①之前会被拦（`目标位于系统关键路径`）⇒ 产品主场景不可用。
    ///
    ///   🔴 只断言守卫②放行，**不构造白名单走完整 guard_check** ——
    ///     守卫③（来源白名单）的判据是「目标在该快照来源集合内」，
    ///     用构造样本去调它得到的是「白名单没喂对」的失败，与本条要钉的命题无关。
    ///     端到端路径由 restore 域的其它测试与真机覆盖。
    #[test]
    fn system_dir_entry_is_not_blocked_by_protected_dir_guard() {
        let dest = Path::new("C:\\Windows\\System32\\drivers\\etc\\hosts");
        assert!(
            guard_protected_dir(dest).is_none(),
            "恢复到系统目录下的原始位置不应被拦：{dest:?}"
        );
    }

    /// 磁盘根下的单文件程序同样**不再拦**：用户有把工具直接放 `D:\` 的习惯，
    /// 重装系统后要一键恢复到原始目录。
    #[test]
    fn volume_root_is_no_longer_rejected() {
        let dest = Path::new("D:\\mytool.exe");
        assert!(
            guard_protected_dir(dest).is_none(),
            "磁盘根下的单文件程序应能恢复到原位置：{dest:?}"
        );
    }
}

/// 🔴 审查 R-2（2026-10-07）：守卫③白名单的**分隔符边界**回归钉。
///   原实现 `starts_with` 会让 `d:\backup` 放行 `d:\backup-evil\...`；
///   修复改词法判定后，这两个方向都必须钉住。
#[cfg(test)]
mod guard_whitelist_boundary_tests {
    use super::*;
    use std::collections::HashSet;

    fn whitelist_of(root: &str) -> HashSet<String> {
        let mut set = HashSet::new();
        set.insert(root.to_ascii_lowercase());
        set
    }

    #[test]
    fn sibling_directory_with_shared_prefix_is_rejected() {
        let whitelist = whitelist_of("d:\\backup");
        assert!(
            guard_whitelist(
                Path::new("D:\\backup-evil\\payload.bin"),
                "D:\\backup-evil\\payload.bin",
                &whitelist
            )
            .is_some(),
            "同前缀兄弟目录必须被拒（原 starts_with 缺陷的阳性样本）"
        );
    }

    #[test]
    fn real_source_root_and_subpath_are_accepted() {
        let whitelist = whitelist_of("d:\\backup");
        assert!(
            guard_whitelist(
                Path::new("D:\\backup\\settings\\app.conf"),
                "D:\\backup\\settings\\app.conf",
                &whitelist
            )
            .is_none(),
            "白名单根内的正常恢复必须放行"
        );
        // 带尾分隔符的根登记形态同样要放行子路径
        let whitelist = whitelist_of("d:\\backup\\");
        assert!(
            guard_whitelist(
                Path::new("D:\\backup\\settings\\app.conf"),
                "D:\\backup\\settings\\app.conf",
                &whitelist
            )
            .is_none(),
            "根带尾分隔符登记时子路径仍须放行"
        );
    }
}

/// 守卫②：本程序受保护目录（配置与记录所在处）。
fn guard_protected_dir(dest: &Path) -> Option<FailureView> {
    // 🔴 2026-10-07 改：子串匹配 → **词法前缀比较**。
    //   原实现 `text.contains("\\aegis\\")` 有两个方向都错的失败形态：
    //   ① 漏防：目标恰为 `%LOCALAPPDATA%\Aegis`（无尾分隔符 —— 用户在目录选择器里
    //      就是这么填的形态）时**不触发**，于是恢复可以覆盖备份记录/配置本身，
    //      正是本守卫声明要防的「自己抹掉自己的恢复依据」；
    //   ② 误拦：任何路径里含 `\aegis\` 子串的无关目录（如 `\MyAegis\Data\`）被拒。
    //   改为与已规范化的数据根做词法判定（`infra::safe::is_lexically_under`
    //   已存在，且 backup 域其他处已在用），两个方向同时修正。
    let root = super::store::dirs_data_dir().join("Aegis");
    if infra::safe::is_lexically_under(&root, dest) {
        return Some(reject(dest, "目标位于本程序受保护目录"));
    }
    None
}

/// 守卫③：来源范围白名单。
fn guard_whitelist(
    dest: &Path,
    original: &str,
    whitelist: &HashSet<String>,
) -> Option<FailureView> {
    if whitelist.is_empty() {
        return Some(reject(dest, "来源白名单为空，无法确认目标在允许范围内"));
    }
    // 用**原始绝对位置**判定归属：它是清单里记录的源端位置，语义最明确
    let in_whitelist = whitelist.iter().any(|root| {
        // 🔴 2026-10-07（审查 R-2）：原实现 `dest_text.starts_with(root)` **无分隔符边界**
        //   —— 白名单根 `d:\backup` 会放行 `d:\backup-evil\...`（前缀命中但已越出根）。
        //   与 `guard_protected_dir` 同标准：改用词法包含判定
        //   （`infra::safe::is_lexically_under` 内部自带大小写归一 + 补分隔符，
        //   此处只需去掉根尾分隔符，避免 `\` 参与段比较）。
        let root_path = Path::new(root.trim_end_matches(['\\', '/']));
        infra::safe::is_lexically_under(root_path, dest)
            || (!original.is_empty()
                && infra::safe::is_lexically_under(root_path, Path::new(original)))
    });
    if in_whitelist {
        None
    } else {
        Some(reject(dest, "目标不在来源范围白名单内"))
    }
}

fn unsafe_reason(raw: &str) -> String {
    match infra::safe::validate_relative(raw) {
        Ok(_) => "未知原因".to_owned(),
        Err(error) => error.to_string(),
    }
}

/// 执行恢复。
///
/// 🔴 **R-3 中断不留半截文件**：逐项「先写完整临时文件 → 校验 → 一次性替换就位」。
/// 🔴 **R-4 被占用处理**：替换失败时先把现有目标改名让位，替换成功后删掉让位残留；
/// 改名也失败 ⇒ **保持原件不动并回搬**（绝不允许「删掉用户原件却没换成新内容」）。
#[must_use]
pub fn execute(manifest: &SnapshotManifest, plan: &RestorePlan, data_dir: &Path) -> RestoreOutcome {
    let mut outcome = RestoreOutcome {
        succeeded: 0,
        failed: 0,
        verify_mismatched: 0,
        skipped: 0,
        failures: Vec::new(),
        skip_reasons: Vec::new(),
        empty_dirs_rebuilt: 0,
    };

    // 🔴 R-1：清单存在不安全路径 ⇒ **整次恢复中止**（规格原文「任一不安全路径 ⇒ 整次恢复中止」）
    if plan.has_unsafe_entry {
        outcome.failed = manifest.entries.len() as u64;
        outcome.failures.push(FailureView {
            code: super::reasons::GUARD_REJECTED.to_owned(),
            text: "快照清单含不安全路径，整次恢复已中止".to_owned(),
            path: String::new(),
        });
        return outcome;
    }

    for (relative, verdict) in &plan.items {
        match verdict {
            ItemVerdict::GuardRejected(f) | ItemVerdict::UnsafeEntry(f) => {
                push_skip(&mut outcome, f.clone());
            }
            ItemVerdict::Conflict if plan.policy == ConflictPolicy::Skip => {
                push_skip(
                    &mut outcome,
                    FailureView {
                        code: "CONFLICT_SKIPPED".to_owned(),
                        text: "目标已存在，按策略跳过".to_owned(),
                        path: relative.clone(),
                    },
                );
            }
            _ => restore_single(manifest, plan, data_dir, relative, &mut outcome),
        }
    }

    // ── 重建空目录（R-10；归属无法确定时跳过并计入明细）──
    for dir in &manifest.empty_dirs {
        if let Ok(segments) = infra::safe::validate_relative(dir) {
            let mut dest = plan.target_root.clone();
            for segment in &segments {
                dest.push(segment);
            }
            if std::fs::create_dir_all(&dest).is_ok() {
                outcome.empty_dirs_rebuilt = outcome.empty_dirs_rebuilt.saturating_add(1);
            } else {
                push_skip(
                    &mut outcome,
                    FailureView {
                        code: super::reasons::RESTORE_WRITE_FAILED.to_owned(),
                        text: "空目录重建失败（多来源快照可能无法确定归属）".to_owned(),
                        path: dir.clone(),
                    },
                );
            }
        } else {
            push_skip(
                &mut outcome,
                FailureView {
                    code: super::reasons::GUARD_REJECTED.to_owned(),
                    text: "空目录相对路径未通过越界守卫".to_owned(),
                    path: dir.clone(),
                },
            );
        }
    }

    outcome
}

/// 恢复单个条目（`execute` 的「可写」分支）。
///
/// 🔴 抽出来的原因：这段同时做**三次守卫**（相对路径 / 目标拼接 / 源侧拼接）
/// 与**三种结论**（成功 / 校验不符 / 写失败）。内联在主循环里时，
/// 「哪一道守卫挡下的」与「计入哪个计数」要跨 40 行来回对齐。
fn restore_single(
    manifest: &SnapshotManifest,
    plan: &RestorePlan,
    data_dir: &Path,
    relative: &str,
    outcome: &mut RestoreOutcome,
) {
    // 🔴 清单里找不到对应条目 ⇒ 计skipped 而**不是 failed**：
    //    它不是「写不进去」，是「根本没这项」，两者对用户是不同的事。
    let Some(entry) = manifest
        .entries
        .iter()
        .find(|e| e.relative_path == relative)
    else {
        push_skip(
            outcome,
            FailureView {
                code: super::reasons::MANIFEST_READ_FAILED.to_owned(),
                text: "清单中找不到该条目（快照可能被裁剪或篡改）".to_owned(),
                path: relative.to_owned(),
            },
        );
        return;
    };
    let Ok(segments) = infra::safe::validate_relative(relative) else {
        outcome.failed = outcome.failed.saturating_add(1);
        return;
    };
    let mut dest = plan.target_root.clone();
    for segment in &segments {
        dest.push(segment);
    }
    // 🔴 源侧路径同样过守卫：清单里的 `data/<相对路径>` 不可信
    let Ok(src) = infra::safe::join_under_root(data_dir, relative) else {
        outcome.failed = outcome.failed.saturating_add(1);
        push_failure(
            outcome,
            FailureView {
                code: super::reasons::RESTORE_WRITE_FAILED.to_owned(),
                text: "快照内相对路径未通过越界守卫".to_owned(),
                path: relative.to_owned(),
            },
        );
        return;
    };

    match restore_one(&src, &dest, entry, plan.policy) {
        Ok(Ok(())) => outcome.succeeded = outcome.succeeded.saturating_add(1),
        // 写入成功但读回指纹不一致 ⇒ 计 verify_mismatched（整体结论为不成功）
        Ok(Err(_)) => {
            outcome.verify_mismatched = outcome.verify_mismatched.saturating_add(1);
        }
        Err(error) => {
            outcome.failed = outcome.failed.saturating_add(1);
            push_failure(
                outcome,
                FailureView {
                    code: super::reasons::RESTORE_WRITE_FAILED.to_owned(),
                    text: error,
                    path: relative.to_owned(),
                },
            );
        }
    }
}

/// 追加失败明细，**上限 10 条**（R-13：明细可看但不刷屏；超出部分由 `failure_count` 概括）。
fn push_failure(outcome: &mut RestoreOutcome, failure: FailureView) {
    if outcome.failures.len() < 10 {
        outcome.failures.push(failure);
    }
}

/// 追加跳过明细并计数，同样上限 10 条。
fn push_skip(outcome: &mut RestoreOutcome, reason: FailureView) {
    outcome.skipped = outcome.skipped.saturating_add(1);
    if outcome.skip_reasons.len() < 10 {
        outcome.skip_reasons.push(reason);
    }
}

/// 恢复单个文件（🔴 R-3 原子写+ R-4 被占用处理）。
fn restore_one(
    src: &Path,
    dest: &Path,
    entry: &super::models::FileEntry,
    policy: ConflictPolicy,
) -> Result<Result<(), String>, String> {
    if let Some(parent) = dest.parent() {
        std::fs::create_dir_all(parent).map_err(|e| format!("创建目录失败：{e}"))?;
    }

    // 目标已存在 ⇒ 按策略分派
    let mut final_dest = dest.to_path_buf();
    if dest.exists() {
        match policy {
            ConflictPolicy::Skip => return Ok(Ok(())),
            ConflictPolicy::Overwrite => {}
            // 🔴 两条策略的落点规则完全相同（母本同款："每次询问"在无闸道时降级为改名）
            ConflictPolicy::RenameKeepBoth | ConflictPolicy::AskEach => {
                final_dest = next_free_name(dest);
            }
        }
    }

    // 🔴 R-3：先写「完整写到临时位置」——临时名带 `.staging-` 前缀（04分册 §三）
    let staging = final_dest.with_file_name(format!(
        ".staging-{}",
        final_dest.file_name().map_or_else(
            || "restore".to_owned(),
            |n| n.to_string_lossy().into_owned(),
        )
    ));
    if let Err(error) = super::hash::copy_and_hash(src, &staging) {
        let _ = std::fs::remove_file(&staging);
        return Err(format!("写入临时文件失败：{error}"));
    }

    // 🔴 读回校验：写完先比对指纹，不一致就不替换（R-3「确认无误 → 一次性替换就位」）
    match super::hash::sha256_file(&staging) {
        Ok(actual) if actual.eq_ignore_ascii_case(&entry.sha256) => {}
        Ok(_) => {
            let _ = std::fs::remove_file(&staging);
            return Ok(Err("恢复后内容指纹与清单不一致".to_owned()));
        }
        Err(error) => {
            let _ = std::fs::remove_file(&staging);
            return Err(format!("临时文件读回失败：{error}"));
        }
    }

    // 🔴 R-4：替换被占用文件时，先把原件改名让位；替换成功后删掉让位残留。
    //   改名也失败 ⇒ 保持原件不动并回搬（**绝不允许原件被删却没换成新内容**）。
    let mut displaced: Option<PathBuf> = None;
    if final_dest.exists() {
        let aside = displaced_name(&final_dest);
        if std::fs::rename(&final_dest, &aside).is_ok() {
            displaced = Some(aside);
        } else {
            // 让位失败：换名顶上（覆盖策略语义），但**失败时必须回搬**
            if let Err(error) = std::fs::remove_file(&final_dest) {
                let _ = std::fs::remove_file(&staging);
                return Err(format!("原文件被占用且无法改名让位：{error}"));
            }
            displaced = None;
        }
    }

    match std::fs::rename(&staging, &final_dest) {
        Ok(()) => {
            // 替换成功：清掉让位残留
            if let Some(aside) = displaced {
                let _ = std::fs::remove_file(&aside);
            }
            // 🔴 还原原始修改时间（F-FB-36；`std::fs` 无此能力 ⇒ 走 infra 的 Win32 入口）
            let _ = infra::fs::set_modified_time(&final_dest, entry.modified_unix_ms);
            Ok(Ok(()))
        }
        Err(error) => {
            // 替换失败：**回搬原件**
            let _ = std::fs::remove_file(&staging);
            if let Some(aside) = displaced {
                let _ = std::fs::rename(&aside, &final_dest);
            }
            Err(format!("替换就位失败（已回搬原件）：{error}"))
        }
    }
}

/// `报告.txt` → `报告 (1).txt`。
#[must_use]
pub fn next_free_name(dest: &Path) -> PathBuf {
    let parent = dest.parent().map(Path::to_path_buf).unwrap_or_default();
    let stem = dest
        .file_stem()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    let ext = dest
        .extension()
        .map(|n| format!(".{}", n.to_string_lossy()))
        .unwrap_or_default();
    for n in 1..10_000 {
        let candidate = parent.join(format!("{stem} ({n}){ext}"));
        if !candidate.exists() {
            return candidate;
        }
    }
    // 🔴 兜底名：`(n)` 连续 9999 个都被占用才会走到这里。
    //   ⚠️ 上一版的格式串是 `"{stem} ({}{ext}"` —— **缺右括号**，
    //   会产出「报告 (a1b2c3.txt」这种畸形名（括号不配对，用户看着像被截断）。
    //   本轮顺带修正。
    parent.join(format!(
        "{stem} ({}){ext}",
        super::ids::random_token(FALLBACK_TOKEN_LEN, super::ids::HEX_LOWER)
    ))
}

/// 兜底名的随机段长度（16^12 ≈ 2.8e14，足够避免再次撞名）。
const FALLBACK_TOKEN_LEN: usize = 12;

fn displaced_name(dest: &Path) -> PathBuf {
    let parent = dest.parent().map(Path::to_path_buf).unwrap_or_default();
    let name = dest
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    parent.join(format!(".staging-old-{name}"))
}
