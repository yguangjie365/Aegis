//! `admission` 单测：R-6 空间三态 + **R-7 重叠守卫**（含母本 2026-09-27 的纠正）。

use super::*;
use std::path::PathBuf;

#[test]
fn headroom_is_applied_and_never_overflows() {
    // 🔴 母本 2026-09-27 实测教训：加法溢出后回退 int.MaxValue，
    //    否则"检查空间充足"会因为溢出变成假阳性。
    let small = required_with_headroom(1_000);
    assert!(small > 1_000, "必须上浮（含绝对下限）");
    // 极大值不得 panic、不得回绕
    let huge = required_with_headroom(u64::MAX);
    assert_eq!(huge, u64::MAX, "溢出须饱和而不是回绕");
}

#[test]
fn verdict_blocks_only_when_insufficient() {
    // 🔴 R-6：`Unknown` **不阻断**（无法确认≠不足），但需警告
    assert!(!DiskVerdict::Unknown { required: 1 }.blocks());
    assert!(DiskVerdict::Unknown { required: 1 }.needs_warning());
    assert!(DiskVerdict::Insufficient {
        required: 10,
        available: 5
    }
    .blocks());
    assert!(!DiskVerdict::Enough {
        required: 5,
        available: 10
    }
    .blocks());
    //🔴 足够时**不产生失败码**（套上"空间不足"的码会让日志自相矛盾）
    let (code, _) = DiskVerdict::Enough {
        required: 1,
        available: 2,
    }
    .reason();
    assert!(code.is_empty(), "Enough 不该带失败码");
}

#[test]
fn unknown_disk_is_reported_distinctly() {
    // 🔴 R-6 红线：禁止把"未知"当"充足"，故码与中文都必须与「不足」可区分
    let (unknown_code, unknown_text) = DiskVerdict::Unknown { required: 1 }.reason();
    let (short_code, short_text) = DiskVerdict::Insufficient {
        required: 2,
        available: 1,
    }
    .reason();
    assert_ne!(unknown_code, short_code);
    assert_ne!(unknown_text, short_text);
    assert!(unknown_text.contains("无法确认"));
}

#[test]
fn overlap_detects_both_directions() {
    // 🔴 R-7：任一方向互为包含都要拒
    let root = PathBuf::from("D:\\Backup");
    // 来源在落点之内
    assert_eq!(
        check_overlap(&["D:\\Backup\\docs".to_owned()], &root),
        OverlapVerdict::Overlapping
    );
    // 落点在来源之内（更危险：备份到自己的源里）
    assert_eq!(
        check_overlap(&["D:\\".to_owned()], &root),
        OverlapVerdict::Overlapping
    );
    // 完全不相干 ⇒ 放行
    assert_eq!(
        check_overlap(&["E:\\Docs".to_owned()], &root),
        OverlapVerdict::Clear
    );
}

#[test]
fn unnormalizable_path_is_rejected_not_allowed() {
    // 🔴 母本 2026-09-27 的纠正：「无法归一化」从**放行**改为**阻断**。
    //    放行 = 递归自我备份导致无限膨胀。空来源即触发此路径。
    assert_eq!(
        check_overlap(&[String::new()], Path::new("D:\\Backup")),
        OverlapVerdict::NotNormalizable
    );
    assert!(
        OverlapVerdict::NotNormalizable.blocks(),
        "无法归一化必须阻断（母本 bug 已修，不得回退）"
    );
}

#[test]
fn normalization_makes_case_and_slash_variants_compare_equal() {
    //🔴 不归一化就会漏判：`D:\a` 与 `d:\A\` 指向同一目录
    let a = normalize_for_compare(Path::new("D:\\Data")).expect("归一化 D:\\Data");
    let b = normalize_for_compare(Path::new("d:\\data\\")).expect("归一化 d:\\data\\");
    assert_eq!(
        a.to_string_lossy().to_lowercase(),
        b.to_string_lossy().to_lowercase(),
        "大小写与结尾分隔符差异不应导致漏判重叠"
    );
}

#[test]
fn overlap_reason_codes_are_distinct() {
    // 两种阻断必须给不同原因码，UI 才能给出不同的可读文案
    assert_eq!(
        OverlapVerdict::Overlapping.reason_code(),
        crate::backup::reasons::SOURCE_TARGET_OVERLAP
    );
    assert_eq!(
        OverlapVerdict::NotNormalizable.reason_code(),
        crate::backup::reasons::PATH_NOT_NORMALIZABLE
    );
    assert_eq!(OverlapVerdict::Clear.reason_code(), "");
}
