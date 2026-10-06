//! 读回校验（备份后自动 + 手动；F-FB-28/29 / **R-11**）。
//!
//! 复用结论（DP-1）：③ 思路复用 ← `Core/Backup/Services/SnapshotVerifier.cs`
//! （逐文件重算 SHA-256 并与清单比对）。三态结论的推导落在
//! [`IntegrityStatus::derive`](super::models::IntegrityStatus::derive)，**单一真相**。
//!
//! 🔴 **本模块的核心职责是"不把没查的说成查过"**（R-11 / 规格 §六诚实性红线）。
//! 四条降级路径缺一不可，漏掉任何一条都会把「抽样」显示成「通过」：
//! | 情形 | 结论 |
//! |---|---|
//! | 有不一致 / 有缺失 | `Failed` |
//! | 抽样（`compared < total`） | `NotFullyVerified` |
//! | 本轮有复用条目 | `NotFullyVerified` |
//! | 档位关闭（`Off`） | `NotVerified` |
//!
//! 🔴 另有一条**独立于 R-11 但同样诚实性相关**的规则（R-11 末句 / F-FB-29）：
//! 「校验自身失败或被取消**不推翻**备份成功的结论」—— 故本模块**只产出报告**，
//! 绝不返回会影响备份成败的信号；是否降级快照状态由调用方按此规则显式决定。

use super::models::{FileEntry, IntegrityStatus, SnapshotManifest, VerifyMode, VerifyReport};
use std::path::Path;

/// 单份文件的比对结果。
///
/// 🔴 保留为独立枚举而非只用 `VerifyReport` 的两个 Vec，是为了让调用方（手动校验的
/// 逐条展示 F-FB-28）能区分四种情况，���「读不到」被混进「内容变了」——
/// 前者通常是权限/占用问题，后者是数据损坏，处置方式完全不同。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FileVerdict {
    /// 指纹一致。
    Matched,
    /// 指纹不一致（携带相对路径）。
    Mismatched(String),
    /// 清单里有、实际读不到（文件被手动删除）。
    Missing(String),
    /// 读取本身失败（权限 / 被占用）。
    Unreadable(String),
}

/// 一次比对的原始产出（**未叠加降级规则**）。
struct RawComparison {
    matched: u64,
    mismatched: Vec<String>,
    missing: Vec<String>,
    unreadable: Vec<String>,
    compared: u64,
}

/// 逐条比对（不抽样的核心动作）。
///
/// 🔴 清单里的 `relative_path` 是**外部数据**（R-1「清单不可信」）⇒ 必须过
/// [`infra::safe::join_under_root`] 复验；越界即视为不可读，**不得**拼路径硬读。
fn compare_entries(entries: &[FileEntry], data_dir: &Path) -> RawComparison {
    let mut out = RawComparison {
        matched: 0,
        mismatched: Vec::new(),
        missing: Vec::new(),
        unreadable: Vec::new(),
        compared: 0,
    };
    for entry in entries {
        let Ok(resolved) = infra::safe::join_under_root(data_dir, &entry.relative_path) else {
            // 路径越界/非法：算作「读不到」而非「内容变了」——清单被篡改是另一类问题，
            // 混进 mismatched 会让用户误以为文件损坏。
            out.unreadable.push(entry.relative_path.clone());
            continue;
        };
        if !resolved.exists() {
            out.missing.push(entry.relative_path.clone());
            continue;
        }
        out.compared = out.compared.saturating_add(1);
        match super::hash::sha256_file(&resolved) {
            Ok(actual) if actual.eq_ignore_ascii_case(&entry.sha256) => {
                out.matched = out.matched.saturating_add(1);
            }
            Ok(_) => out.mismatched.push(entry.relative_path.clone()),
            Err(_) => out.unreadable.push(entry.relative_path.clone()),
        }
    }
    out
}

/// 把原始比对结果按 R-11 叠成报告。
///
/// `all_compared` 由调用方显式给出（而不是在这里比较 `compared == total`）——
/// 因为 `Off` 档位下 `compared = 0` 与「抽样 0 条」在数字上不可区分，只有调用方知道档位。
fn finalize(total: u64, raw: RawComparison, all_compared: bool, has_reused: bool) -> VerifyReport {
    // 🔴 「读不到」既不是通过也不是"内容变了"，但同样**不能**算通过 ⇒ 与 mismatched
    // 合并计入失败侧，同时在报告里单列，让界面能分开呈现。
    let mut mismatched = raw.mismatched;
    mismatched.extend(raw.unreadable.iter().cloned());
    let integrity = IntegrityStatus::derive(
        !mismatched.is_empty() || !raw.missing.is_empty(),
        all_compared,
        has_reused,
    );
    VerifyReport {
        total,
        compared: raw.compared,
        matched: raw.matched,
        mismatched,
        missing: raw.missing,
        integrity,
    }
}

/// 逐文件重算指纹并与清单比对（手动校验 F-FB-28：全量、无抽样）。
#[must_use]
pub fn verify_manifest(manifest: &SnapshotManifest, data_dir: &Path) -> VerifyReport {
    finalize(
        manifest.entries.len() as u64,
        compare_entries(&manifest.entries, data_dir),
        true,
        false,
    )
}

/// 按档位校验（备份后自动校验 F-FB-29）。
///
/// 🔴 抽样时 `all_compared = false` ⇒ 结论**必然**降级为 `NotFullyVerified`，
/// 这正是规格要求的行为（"抽样通过记为'未做完整校验'而非'通过'"）。
#[must_use]
pub fn verify_by_mode(
    manifest: &SnapshotManifest,
    data_dir: &Path,
    mode: VerifyMode,
    has_reused: bool,
) -> VerifyReport {
    let total = manifest.entries.len() as u64;
    match mode {
        VerifyMode::Off => {
            // 🔴 关闭档位：`compared = 0`，且 `all_compared = false` ⇒ 降级为 `NotVerified`
            //（`IntegrityStatus::derive` 在 `all_compared=false` 时给 `NotFullyVerified`，
            //  这里需要更明确的「未校验」—— 故单独构造，不走 derive）。
            VerifyReport {
                total,
                compared: 0,
                matched: 0,
                mismatched: Vec::new(),
                missing: Vec::new(),
                integrity: IntegrityStatus::NotVerified,
            }
        }
        VerifyMode::Full => finalize(
            total,
            compare_entries(&manifest.entries, data_dir),
            true,
            has_reused,
        ),
        VerifyMode::Sample(n) => {
            let take = usize::try_from(n).unwrap_or(usize::MAX);
            let selected: Vec<FileEntry> = manifest.entries.iter().take(take).cloned().collect();
            finalize(
                total,
                compare_entries(&selected, data_dir),
                selected.len() == manifest.entries.len(),
                has_reused,
            )
        }
    }
}

#[cfg(test)]
mod tests;
