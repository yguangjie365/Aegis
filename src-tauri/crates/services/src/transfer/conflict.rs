//! 同名冲突的**唯一裁决点**（F-FT-17 / F-FT-23 / R-10）。
//!
//! 复用结论（DP-1）：③思路复用 ←
//! `Source/SystemToolkit/src/SystemToolkit.Core/FileTransfer/Models/TransferConflictPolicy.cs`
//! 与 `.../Services/TransferConflictResolver.cs`（GPL-3.0 同源，ADR-003 五步登记见 `DEPENDENCIES.md §三`）。
//!
//! ## 🔴 为什么必须是"唯一裁决点"
//!
//! 规格 R-10 原文：*"两条通道共用同一个裁决点，同一文件 + 同一策略在任何入口得到同一结果"*。
//! 若电脑通道与手机通道各写一份，必然漂移成"同一次操作、两台设备上结果不同"。
//! 故**任何**落盘前的同名判定都必须调 [`decide`]，不得在调用方另写 if-else。
//!
//! ## 🔴 出厂默认 = 自动改名（house policy「绝不覆盖」）
//!
//! 规格 F-FT-23 写"默认『自动改名』"并明示"『绝不覆盖』作为出厂默认"，母本
//! `TransferSettings.ConflictPolicy` 实测默认值也是 `Rename`（枚举判别值 0）。三处一致，故
//! [`ConflictPolicy::default`] 取 `Rename`。
//! 🟠 规格 F-FT-17 另有一句"默认策略为『每次询问』时……默认选中『改名保存』" —— 那是描述
//! **Ask 模式下弹窗内的默认选项**，不是出厂策略；两者不矛盾，已在 TASKS M-05 备注登记。
//!
//! ## 🔴 "每次询问"在无确认闸道的通道上必须降级为改名
//!
//! F-FT-23 / R-10 / 已知限制 6：手机通道（含"发文件到手机"与手机上传）没有逐次询问能力；
//! F-FT-18 又规定"接收前询问"关闭时逐次询问同样不可用。两种情形都由
//! `ask_channel_available = false` 表达，裁决统一降级为**改名**——
//! *宁可不覆盖，也不静默覆盖*。

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use super::reasons;

/// 同名冲突策略（F-FT-23；四值，两条通道共用）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ConflictPolicy {
    /// 自动改名保存（**出厂默认**；后缀 ` (n)`，原文件保持不变）
    #[default]
    Rename,
    /// 每次询问（仅在**有确认闸道**时成立，否则降级为 [`ConflictPolicy::Rename`]）
    Ask,
    /// 跳过不接收（不留新文件；任务终态为「已跳过」）
    Skip,
    /// 覆盖原文件（**原内容不可恢复**；house policy 不推荐，但用户可选）
    Overwrite,
}

/// 裁决结论（落盘前一次性得出）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConflictDecision {
    /// 目标不存在同名文件 → 直接以原名落盘
    Fresh,
    /// 改名保存（原文件保持不变）
    Renamed,
    /// 覆盖原文件（原内容不可恢复）
    Overwrite,
    /// 跳过不接收（不留新文件；任务终态为「已跳过」）
    Skip,
    /// 需先弹窗询问用户（**仅电脑通道 + 策略 Ask + 确有同名**）
    AskUser,
}

impl ConflictDecision {
    /// 是否写入文件（`Skip` 与 `AskUser` 不写）。
    ///
    /// 与母本 `ConflictPlan.WritesFile` 同语义；调用方据此决定是否建未完成文件。
    #[must_use]
    pub fn writes_file(self) -> bool {
        !matches!(self, Self::Skip | Self::AskUser)
    }

    /// 该决策是否应把任务终态定为**「已跳过」**（F-FT-23 原文：
    /// *"跳过时任务终态为『已跳过』而非已完成"* —— 报「已完成」就是状态欺骗）。
    #[must_use]
    pub fn is_skipped(self) -> bool {
        self == Self::Skip
    }
}

/// **唯一裁决点**（R-10）。两条通道、所有入口（电脑接收 / 手机上传 / 发文件到手机）都调它。
///
/// * `policy` —— 用户当前策略（两通道共用同一份设置）
/// * `target_exists` —— 目标目录是否已有同名文件
/// * `ask_channel_available` —— 本通道此刻**是否具备逐次询问能力**：
///   手机通道恒 `false`；电脑通道在"接收前询问"关闭时也是 `false`（F-FT-18）
///
/// 判定顺序（与母本一致，改动须同步两侧）：
/// 1. 无同名 → [`ConflictDecision::Fresh`]（策略与闸道都不参与）
/// 2. 有同名 + `Ask` + 有闸道 → [`ConflictDecision::AskUser`]
/// 3. 有同名 + `Ask` + **无闸道** → 降级 [`ConflictDecision::Renamed`]
/// 4. 其余按策略直落
#[must_use]
pub fn decide(
    policy: ConflictPolicy,
    target_exists: bool,
    ask_channel_available: bool,
) -> ConflictDecision {
    if !target_exists {
        return ConflictDecision::Fresh;
    }
    match policy {
        ConflictPolicy::Rename => ConflictDecision::Renamed,
        ConflictPolicy::Skip => ConflictDecision::Skip,
        ConflictPolicy::Overwrite => ConflictDecision::Overwrite,
        ConflictPolicy::Ask => {
            if ask_channel_available {
                ConflictDecision::AskUser
            } else {
                // 无闸道 → 降级改名（绝不静默覆盖）
                ConflictDecision::Renamed
            }
        }
    }
}

/// 本次裁决是否发生了"Ask 降级"（用于回传 [`reasons::CONFLICT_ASK_UNAVAILABLE`] 提示）。
///
/// 单独一个函数而不是让 [`decide`] 返回更复杂的类型：降级是**提示性事实**，
/// 与落盘动作正交，调用方按需取用。
#[must_use]
pub fn degraded_from_ask(
    policy: ConflictPolicy,
    target_exists: bool,
    ask_channel_available: bool,
) -> bool {
    policy == ConflictPolicy::Ask && target_exists && !ask_channel_available
}

/// 该决策对应的原因码（无则为 `None`）：
/// * [`ConflictDecision::Skip`] → [`reasons::CONFLICT_SKIP`]
/// * [`ConflictDecision::Renamed`] 且发生了 Ask 降级 → [`reasons::CONFLICT_ASK_UNAVAILABLE`]
///
/// 🔴 其余决策**不产生原因码**：它们不是失败，不该在"原因"列里出现东西。
#[must_use]
pub fn reason_code_for(
    decision: ConflictDecision,
    policy: ConflictPolicy,
    target_exists: bool,
    ask_channel_available: bool,
) -> Option<&'static str> {
    match decision {
        ConflictDecision::Skip => Some(reasons::CONFLICT_SKIP),
        ConflictDecision::Renamed
            if degraded_from_ask(policy, target_exists, ask_channel_available) =>
        {
            Some(reasons::CONFLICT_ASK_UNAVAILABLE)
        }
        _ => None,
    }
}

/// 改名候选名：`a.txt` → `a (1).txt`；无扩展名 → `a (1)`（F-FT-23 后缀格式 ` (n)`）。
///
/// 纯函数（不探盘）：调用方从 `index = 1` 递增，直到 [`candidate_with_index`] 给出的名字不存在为止。
/// 🔴 母本后缀是**空格 + 括号**（不是 `_1`），照抄以保持用户可预期。
#[must_use]
pub fn candidate_with_index(target: &Path, index: u32) -> PathBuf {
    let stem = target
        .file_stem()
        .map_or_else(String::new, |s| s.to_string_lossy().into_owned());
    let renamed = match target.extension() {
        Some(ext) => format!("{stem} ({index}).{}", ext.to_string_lossy()),
        None => format!("{stem} ({index})"),
    };
    target.with_file_name(renamed)
}

/// 策略说明文案（F-FT-17 原文措辞；`AskUser` 与 `Fresh` 无此文案，返回 `None`）。
///
/// 🔴 调用方展示时应**原样**使用，不要改写措辞——规格把这四条列为用户的"知情"依据
/// （覆盖类必须写明"原内容不可恢复"）。
#[must_use]
pub fn policy_effect_text(decision: ConflictDecision) -> Option<&'static str> {
    match decision {
        ConflictDecision::Overwrite => Some("按当前策略：将覆盖原文件（原内容不可恢复）"),
        ConflictDecision::Skip => Some("按当前策略：将跳过不接收（不留新文件）"),
        ConflictDecision::Renamed => Some("按当前策略：将改名保存（原文件保持不变）"),
        ConflictDecision::AskUser | ConflictDecision::Fresh => None,
    }
}

/// 自动策略下的补句（F-FT-17 原文："这是当前策略下将会发生的动作。"）。
pub const AUTO_EFFECT_SUFFIX: &str = "这是当前策略下将会发生的动作。";

/// Ask 模式弹窗的三选项及其**默认选中项**（F-FT-17 原文：默认选中"改名保存"——它是唯一不会丢东西的选项）。
pub const ASK_OPTIONS: [(&str, ConflictDecision); 3] = [
    ("改名保存（保留原文件）", ConflictDecision::Renamed),
    ("覆盖原文件（原内容不可恢复）", ConflictDecision::Overwrite),
    ("跳过不接收（不留新文件）", ConflictDecision::Skip),
];

/// Ask 模式弹窗的默认选中项（F-FT-17；🔴 取安全侧）。
pub const ASK_DEFAULT: ConflictDecision = ConflictDecision::Renamed;

/// Ask 模式弹窗的提示句（F-FT-17 原文）。
pub const ASK_HINT: &str = "请选择同名处理方式后点「接收」；本次选择不会改变默认策略。";

#[cfg(test)]
mod tests {
    use super::*;

    /// F-FT-23 + house policy + 母本实测：出厂默认必须是**自动改名**，不是询问、更不是覆盖。
    #[test]
    fn factory_default_is_rename() {
        assert_eq!(ConflictPolicy::default(), ConflictPolicy::Rename);
    }

    #[test]
    fn no_conflict_short_circuits_policy_and_channel() {
        // 无同名时，四种策略、闸道有无，结论一律 Fresh
        for policy in [
            ConflictPolicy::Rename,
            ConflictPolicy::Ask,
            ConflictPolicy::Skip,
            ConflictPolicy::Overwrite,
        ] {
            for ask in [true, false] {
                assert_eq!(
                    decide(policy, false, ask),
                    ConflictDecision::Fresh,
                    "无同名时策略 {policy:?} / 闸道 {ask} 不该改变结论"
                );
            }
        }
    }

    /// 🔴 核心安全口径：无确认闸道时 Ask **必须降级为改名**，绝不能落到覆盖。
    #[test]
    fn ask_degrades_to_rename_without_channel_never_to_overwrite() {
        assert_eq!(
            decide(ConflictPolicy::Ask, true, false),
            ConflictDecision::Renamed,
            "手机通道（无闸道）的「每次询问」必须降级为改名"
        );
        assert!(degraded_from_ask(ConflictPolicy::Ask, true, false));
        assert_eq!(
            reason_code_for(ConflictDecision::Renamed, ConflictPolicy::Ask, true, false),
            Some(reasons::CONFLICT_ASK_UNAVAILABLE)
        );
        // 有闸道时不降级、也不产生该提示
        assert_eq!(
            decide(ConflictPolicy::Ask, true, true),
            ConflictDecision::AskUser
        );
        assert!(!degraded_from_ask(ConflictPolicy::Ask, true, true));
        assert_eq!(
            reason_code_for(ConflictDecision::AskUser, ConflictPolicy::Ask, true, true),
            None
        );
    }

    #[test]
    fn explicit_policies_are_honoured_regardless_of_channel() {
        for ask in [true, false] {
            assert_eq!(
                decide(ConflictPolicy::Rename, true, ask),
                ConflictDecision::Renamed
            );
            assert_eq!(
                decide(ConflictPolicy::Overwrite, true, ask),
                ConflictDecision::Overwrite
            );
            assert_eq!(
                decide(ConflictPolicy::Skip, true, ask),
                ConflictDecision::Skip
            );
        }
    }

    /// F-FT-23：跳过时终态是「已跳过」而非「已完成」；且跳过/询问都不写文件。
    #[test]
    fn skip_is_not_completion_and_writes_nothing() {
        assert!(ConflictDecision::Skip.is_skipped());
        assert!(!ConflictDecision::Skip.writes_file());
        assert!(!ConflictDecision::AskUser.writes_file());
        assert!(ConflictDecision::Fresh.writes_file());
        assert!(ConflictDecision::Renamed.writes_file());
        assert!(ConflictDecision::Overwrite.writes_file());
        // 只有 Skip 进"已跳过"
        assert!(!ConflictDecision::Renamed.is_skipped());
        assert_eq!(
            reason_code_for(ConflictDecision::Skip, ConflictPolicy::Skip, true, true),
            Some(reasons::CONFLICT_SKIP)
        );
    }

    /// 后缀格式必须是 ` (n)`（空格 + 括号），且插在扩展名之前 —— 母本格式，用户可预期。
    #[test]
    fn rename_candidates_use_space_paren_index_before_extension() {
        let target = Path::new(r"C:\Downloads\report.tar.gz");
        assert_eq!(
            candidate_with_index(target, 1),
            PathBuf::from(r"C:\Downloads\report.tar (1).gz"),
            "Rust 的 extension 只取最后一段，与母本 Path.GetExtension 语义一致"
        );
        assert_eq!(
            candidate_with_index(Path::new(r"C:\Downloads\noext"), 2),
            PathBuf::from(r"C:\Downloads\noext (2)")
        );
        assert_eq!(
            candidate_with_index(Path::new(r"C:\Downloads\照片.png"), 12),
            PathBuf::from(r"C:\Downloads\照片 (12).png"),
            "非 ASCII 文件名与母本一致地保留原名"
        );
        // 同一目标不同 index 必须互不相同（否则改名会自撞）
        let t = Path::new("a.txt");
        assert_ne!(candidate_with_index(t, 1), candidate_with_index(t, 2));
    }

    /// F-FT-17：覆盖类后果必须写明"原内容不可恢复"——这是"后果在点之前可见"（R-9）的载体。
    #[test]
    fn effect_texts_match_spec_wording_and_warn_on_overwrite() {
        assert_eq!(
            policy_effect_text(ConflictDecision::Overwrite),
            Some("按当前策略：将覆盖原文件（原内容不可恢复）")
        );
        assert_eq!(
            policy_effect_text(ConflictDecision::Skip),
            Some("按当前策略：将跳过不接收（不留新文件）")
        );
        assert_eq!(
            policy_effect_text(ConflictDecision::Renamed),
            Some("按当前策略：将改名保存（原文件保持不变）")
        );
        assert!(policy_effect_text(ConflictDecision::Overwrite)
            .unwrap()
            .contains("不可恢复"));
        // 询问与无冲突不该有"按当前策略"文案（否则用户以为会被自动处置）
        assert_eq!(policy_effect_text(ConflictDecision::AskUser), None);
        assert_eq!(policy_effect_text(ConflictDecision::Fresh), None);
    }

    /// F-FT-17：逐次选择的三选项与**默认选中改名保存**（唯一不丢东西的选项）。
    #[test]
    fn ask_options_and_default_are_safe_side() {
        assert_eq!(ASK_OPTIONS.len(), 3);
        assert_eq!(ASK_DEFAULT, ConflictDecision::Renamed);
        assert!(
            ASK_OPTIONS.iter().any(|(_, d)| *d == ASK_DEFAULT),
            "默认项必须在三选项之列"
        );
        assert!(ASK_OPTIONS[0].0.contains("保留原文件"));
        assert!(ASK_OPTIONS[1].0.contains("不可恢复"));
        assert!(ASK_OPTIONS[2].0.contains("不留新文件"));
        assert!(ASK_HINT.contains("不会改变默认策略"));
    }
}
