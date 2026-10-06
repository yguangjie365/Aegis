//! `models` 与 `reasons` 的单测。
//!
//! 🔴 **R-11 三态推导是本域最重要的一条不变量**，故对它做**穷举**式验证
//! （3 个布尔量的全部 8 种组合），而不是挑几个case —— 挑 case 漏掉组合就会
//! 让「抽样通过被显示成通过」这种失真溜过去。

use super::*;

// ── R-11 完整性三态：8 种组合全覆盖 ──────────────────────────────────────

#[test]
fn integrity_derive_covers_all_eight_combinations() {
    // (has_failed, all_compared, has_reused, 期望结论)
    let cases: [(bool, bool, bool, IntegrityStatus); 8] = [
        (false, true, false, IntegrityStatus::Passed),
        // 抽样（未全量比对）⇒ 降级
        (false, false, false, IntegrityStatus::NotFullyVerified),
        // 有复用条目 ⇒ 降级
        (false, true, true, IntegrityStatus::NotFullyVerified),
        (false, false, true, IntegrityStatus::NotFullyVerified),
        // 🔴 有失败项 ⇒ **一律Failed**，不管其它两个是什么
        (true, true, false, IntegrityStatus::Failed),
        (true, true, true, IntegrityStatus::Failed),
        (true, false, false, IntegrityStatus::Failed),
        (true, false, true, IntegrityStatus::Failed),
    ];
    for (has_failed, all_compared, has_reused, expected) in cases {
        let got = IntegrityStatus::derive(has_failed, all_compared, has_reused);
        assert_eq!(
            got, expected,
            "derive(failed={has_failed}, all={all_compared}, reused={has_reused}) 应为 {expected:?}，实得 {got:?}"
        );
    }
}

#[test]
fn only_passed_is_trustworthy() {
    // 🔴 R-11：`通过` 才可作为可信基准；其余三态都**不能**支撑「可恢复」结论
    assert!(IntegrityStatus::Passed.is_trustworthy());
    for status in [
        IntegrityStatus::NotFullyVerified,
        IntegrityStatus::Failed,
        IntegrityStatus::NotVerified,
    ] {
        assert!(!status.is_trustworthy(), "{status:?} 不得被当作可信基准");
    }
}

// ── 冲突策略（R-19 回落 / 降级） ────────────────────────────────────────

#[test]
fn illegal_conflict_policy_falls_back_to_the_safest() {
    // 🔴 R-19：非法值回落「重命名保留两者」—— 最不易毁数据的策略，**不是** Overwrite
    for illegal in ["", "  ", "nonsense", "delete", "OVERWRITE_ALL"] {
        assert_eq!(
            ConflictPolicy::from_setting(illegal),
            ConflictPolicy::RenameKeepBoth,
            "非法值「{illegal}」应回落为 RenameKeepBoth"
        );
    }
}

#[test]
fn conflict_policy_roundtrips_through_settings_form() {
    for policy in [
        ConflictPolicy::Overwrite,
        ConflictPolicy::RenameKeepBoth,
        ConflictPolicy::Skip,
        ConflictPolicy::AskEach,
    ] {
        assert_eq!(
            ConflictPolicy::from_setting(policy.as_setting()),
            policy,
            "{policy:?} 的落盘串必须能原样读回"
        );
    }
}

#[test]
fn ask_each_degrades_to_rename_without_prompt() {
    // 🔴 无界面执行时没有交互通道 ⇒ 「每次询问」降级为改名，绝不默认覆盖
    assert_eq!(
        ConflictPolicy::AskEach.degrade_without_prompt(),
        ConflictPolicy::RenameKeepBoth
    );
    // 其余策略不受影响
    for p in [
        ConflictPolicy::Overwrite,
        ConflictPolicy::RenameKeepBoth,
        ConflictPolicy::Skip,
    ] {
        assert_eq!(p.degrade_without_prompt(), p);
    }
}

// ── 保留策略钳制（R-5 / R-20） ──────────────────────────────────────────

#[test]
fn count_is_clamped_into_legal_range() {
    // 0 → 1（规格 R-20：非法即拒绝；数据层是第二道防线）
    assert_eq!(
        RetentionPolicy::Count(0).clamped(),
        RetentionPolicy::Count(1)
    );
    // 超上限 → 100（防被设成极大值导致无限累积）
    assert_eq!(
        RetentionPolicy::Count(9_999).clamped(),
        RetentionPolicy::Count(100)
    );
    // 合法值原样保留
    assert_eq!(
        RetentionPolicy::Count(7).clamped(),
        RetentionPolicy::Count(7)
    );
}

#[test]
fn gfs_buckets_are_clamped_to_100() {
    let gfs = GfsRetention {
        daily_keep: 500,
        weekly_keep: 0,
        monthly_keep: 1_000,
    }
    .clamped();
    assert_eq!(gfs.daily_keep, 100);
    assert_eq!(gfs.weekly_keep, 0);
    assert_eq!(gfs.monthly_keep, 100);
}

#[test]
fn gfs_all_zero_means_keep_latest_only() {
    // 🔴 R-5：三项全 0 = 只保最新一份（**不是**「不限制」）
    let none = GfsRetention::default();
    assert!(none.is_keep_latest_only());
    assert!(!GfsRetention {
        daily_keep: 1,
        ..Default::default()
    }
    .is_keep_latest_only());
}

// ── 设置钳制（R-19） ────────────────────────────────────────────────────

#[test]
fn settings_are_clamped_on_load() {
    let raw = BackupSettings {
        default_keep_count: 0, // → 1
        concurrency: 99,       // → 8
        ..Default::default()
    };
    let clamped = raw.clamped();
    assert_eq!(clamped.default_keep_count, 1);
    assert_eq!(clamped.concurrency, 8, "并发上限为 8（规格 §五）");
}

#[test]
fn verify_mode_maps_negatives_to_off() {
    // 🔴 档位语义：-1 关闭 / 0 全量 / >0 抽样
    //    用结构体更新语法而非逐字段赋值：后者在新增字段时会静默保留默认值，
    //    而这里三个case 恰恰只在**一个**字段上有差别。
    let settings = BackupSettings::default();
    assert_eq!(
        BackupSettings {
            verify_sample: -1,
            ..settings.clone()
        }
        .verify_mode(),
        VerifyMode::Off
    );
    assert_eq!(
        BackupSettings {
            verify_sample: 0,
            ..settings.clone()
        }
        .verify_mode(),
        VerifyMode::Full
    );
    assert_eq!(
        BackupSettings {
            verify_sample: 200,
            ..settings
        }
        .verify_mode(),
        VerifyMode::Sample(200)
    );
}

#[test]
fn reuse_is_off_by_default() {
    // 🔴 R-12：未变文件复用**默认关闭**（时间戳可被人为改回会造成静默漏备）
    assert!(
        !BackupSettings::default().reuse_unchanged,
        "复用特性必须出厂关闭"
    );
    assert_eq!(
        BackupSettings::default().default_conflict_policy,
        ConflictPolicy::RenameKeepBoth
    );
}

// ── 来源规范化（F-FB-02） ───────────────────────────────────────────────

#[test]
fn source_input_is_normalized() {
    // 去成对引号 + 去首尾空白
    assert_eq!(normalize_source_input(r#"  "D:\工作"  "#), "D:\\工作");
    // 合并重复分隔符
    assert_eq!(normalize_source_input("D:\\\\工作"), "D:\\工作");
    // 去掉结尾分隔符（盘符根除外）
    assert_eq!(normalize_source_input("D:\\工作\\"), "D:\\工作");
    assert_eq!(normalize_source_input("D:\\"), "D:\\");
    // 未加引号的原样保留
    assert_eq!(normalize_source_input("D:\\工作"), "D:\\工作");
}

#[test]
fn source_dedup_is_case_insensitive() {
    // 🔴 F-FB-02：去重按大小写不敏感（Windows 语义）
    assert_eq!(
        source_dedup_key(r#" "D:\Work" "#),
        source_dedup_key("d:\\work")
    );
}

// ── 原因码表（transfer 域同款稳定性纪律） ───────────────────────────────

#[test]
fn reason_codes_are_unique_and_stable() {
    // 🔴 码是**跨端契约**：历史条目里存的就是这些字符串，改拼写即破坏兼容
    let mut seen: Vec<&str> = Vec::new();
    for reason in super::super::reasons::ALL {
        assert!(!seen.contains(&reason.code), "原因码重复：{}", reason.code);
        assert!(!reason.text.is_empty(), "{} 的中文说明为空", reason.code);
        seen.push(reason.code);
    }
    assert!(
        seen.len() >= 18,
        "码表不该被悄悄删减（当前 {} 条）",
        seen.len()
    );
}

#[test]
fn unknown_reason_code_is_not_swallowed() {
    // 🔴 未知码返回 None（调用方原样展示），**不**回一个"未知错误"
    assert_eq!(super::super::reasons::describe("SOME_NEW_CODE"), None);
    // 已知码能查到中文
    assert_eq!(
        super::super::reasons::describe(super::super::reasons::USER_CANCELLED),
        Some("已取消")
    );
    // pair 对未登记码返回码本身当说明（对齐母本 Describe() 的不吞语义）
    let (code, text) = super::super::reasons::pair("SOME_NEW_CODE");
    assert_eq!(code, "SOME_NEW_CODE");
    assert_eq!(text, "SOME_NEW_CODE");
}

// ── 清单格式版本 ────────────────────────────────────────────────────────

#[test]
fn manifest_format_version_is_one() {
    // 🔴 格式版本用于识别旧清单；升版时这个断言会提醒同步改读入逻辑
    assert_eq!(MANIFEST_FORMAT_VERSION, 1);
    assert_eq!(manifest_format_version(), 1);
}
