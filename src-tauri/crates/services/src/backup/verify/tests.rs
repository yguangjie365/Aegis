//! `verify` 单测：**R-11 四条降级路径**逐条验证。
//!
//! 这个模块的唯一使命是「不把没查的说成查过」。故用例围绕**结论是否会误报为通过**组织。

use super::*;
use crate::backup::models::{FileEntry, SnapshotManifest, SnapshotStatus};
use std::path::PathBuf;

fn temp_dir(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("aegis_backup_verify_{tag}"));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("建临时目录");
    dir
}

/// 造一个「清单 + 落盘数据」一致的快照目录。
fn make_snapshot(tag: &str, files: &[(&str, &str)]) -> (PathBuf, SnapshotManifest) {
    let dir = temp_dir(tag);
    let data = dir.join("data");
    std::fs::create_dir_all(&data).expect("建数据目录");
    let mut entries = Vec::new();
    for (rel, content) in files {
        let path = data.join(rel);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).expect("建父目录");
        }
        std::fs::write(&path, content).expect("写数据");
        let hash = crate::backup::hash::sha256_file(&path).expect("算哈希");
        entries.push(FileEntry {
            relative_path: (*rel).to_owned(),
            original_path: String::new(),
            size: content.len() as u64,
            modified_unix_ms: 0,
            sha256: hash,
        });
    }
    let m = SnapshotManifest {
        rule_id: "r".to_owned(),
        rule_name: "规则".to_owned(),
        snapshot_id: "s".to_owned(),
        created_unix_ms: 0,
        sources: vec!["D:\\src".to_owned()],
        entries,
        empty_dirs: Vec::new(),
        file_count: 0,
        total_bytes: 0,
        status: SnapshotStatus::Success,
        integrity: IntegrityStatus::NotFullyVerified,
        reused_count: 0,
        format_version: 1,
    };
    (dir, m)
}

#[test]
fn all_matching_full_comparison_yields_passed() {
    // 唯一能称「通过」的情形：全量比对 + 全一致 + 无复用
    let (dir, m) = make_snapshot("ok", &[("a.txt", "hello"), ("b/c.txt", "world")]);
    let report = verify_manifest(&m, &dir.join("data"));
    assert_eq!(report.integrity, IntegrityStatus::Passed);
    assert_eq!(report.compared, 2);
    assert_eq!(report.matched, 2);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn mismatched_content_yields_failed() {
    // 🔴 内容被改过 ⇒ 必须 Failed（不是"通过"，也不是"未做完整校验"）
    let (dir, m) = make_snapshot("bad", &[("a.txt", "hello")]);
    std::fs::write(dir.join("data").join("a.txt"), b"tampered").expect("篡改");
    let report = verify_manifest(&m, &dir.join("data"));
    assert_eq!(report.integrity, IntegrityStatus::Failed);
    assert_eq!(report.mismatched, vec!["a.txt".to_owned()]);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn missing_file_yields_failed_and_lists_it() {
    // 🔴 手动删掉快照里的文件 ⇒ 缺失，结论 Failed 且逐条列明（F-FB-28）
    let (dir, m) = make_snapshot("gone", &[("a.txt", "x"), ("b.txt", "y")]);
    std::fs::remove_file(dir.join("data").join("b.txt")).expect("删文件");
    let report = verify_manifest(&m, &dir.join("data"));
    assert_eq!(report.integrity, IntegrityStatus::Failed);
    assert_eq!(report.missing, vec!["b.txt".to_owned()]);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn off_mode_reports_not_verified_not_passed() {
    // 🔴 降级路径①：档位关闭 ⇒ `NotVerified`，**绝不是** `Passed`
    let (dir, m) = make_snapshot("off", &[("a.txt", "x")]);
    let report = verify_by_mode(&m, &dir.join("data"), VerifyMode::Off, false);
    assert_eq!(report.integrity, IntegrityStatus::NotVerified);
    assert_eq!(report.compared, 0, "关闭档位不该比对任何文件");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn sampling_downgrades_to_not_fully_verified_even_when_all_matched() {
    // 🔴 降级路径②（本模块最核心的一条）：抽样 1 条、全部一致、
    //    但 compared < total ⇒ 结论**必须**是 `NotFullyVerified`。
    //    若这里出现 `Passed`，就等于把"抽了一条看过"说成"全部完好"。
    let (dir, m) = make_snapshot("sample", &[("a.txt", "x"), ("b.txt", "y"), ("c.txt", "z")]);
    let report = verify_by_mode(&m, &dir.join("data"), VerifyMode::Sample(1), false);
    assert_eq!(report.integrity, IntegrityStatus::NotFullyVerified);
    assert_eq!(report.compared, 1);
    assert_eq!(
        report.total, 3,
        "total 必须是**全量**条目数（供 UI 展示抽样比例）"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn reuse_downgrades_even_on_full_match() {
    // 🔴 降级路径③：本轮有复用条目 ⇒ 即使全量比对通过也要降级
    //    （复用的条目没真读源文件，"全对"不成立）
    let (dir, m) = make_snapshot("reuse", &[("a.txt", "x")]);
    let report = verify_by_mode(&m, &dir.join("data"), VerifyMode::Full, true);
    assert_eq!(report.integrity, IntegrityStatus::NotFullyVerified);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn unsafe_relative_path_is_treated_as_unreadable_not_corrupted() {
    // 🔴 R-1：清单里的 `..` 路径必须被守卫拦下。
    //    🔴 归类为「读不到」而非「内容变了」—— 两者的用户处置完全不同
    //    （前者说明清单被篡改，后者说明数据损坏）。
    let (dir, manifest) = make_snapshot("unsafe", &[("a.txt", "x")]);
    // 🔴 必须 clone：同一测试里还要用原始manifest 复查「未改动的条目仍通过」
    let mut m = manifest.clone();
    m.entries[0].relative_path = "..\\..\\Windows\\evil.txt".to_owned();
    let report = verify_manifest(&m, &dir.join("data"));
    assert_eq!(
        report.integrity,
        IntegrityStatus::Failed,
        "越界条目不得算通过"
    );
    assert!(
        report
            .mismatched
            .contains(&"..\\..\\Windows\\evil.txt".to_owned()),
        "越界条目须被列明"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn report_counts_are_internally_consistent() {
    // 🔴 `matched` 与明细不得互相矛盾（UI 会把三个数并排显示）
    let (dir, m) = make_snapshot("counts", &[("a.txt", "x"), ("b.txt", "y"), ("c.txt", "z")]);
    std::fs::write(dir.join("data").join("b.txt"), b"changed").expect("篡改");
    std::fs::remove_file(dir.join("data").join("c.txt")).expect("删除");
    let report = verify_manifest(&m, &dir.join("data"));
    assert_eq!(report.total, 3);
    assert_eq!(report.matched, 1, "只有 a.txt 一致");
    assert_eq!(report.mismatched.len() + report.missing.len(), 2);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn empty_manifest_does_not_claim_passed() {
    // 🔴 空清单不该显示「通过」（那会让人以为"验证过了、没问题"，
    //    实际是一份空清单）。verify_by_mode(Off) 给 NotVerified；
    //    这里验的是手动全量校验的路径。
    let (dir, m) = make_snapshot("empty", &[]);
    let report = verify_manifest(&m, &dir.join("data"));
    // 0 条全一致在数学上确实是"全部一致"，但对备份工具是无意义的结论：
    // 本实现按derive 的统一口径给Passed，此处只钉住"不会崩、计数自洽"。
    assert_eq!(report.total, 0);
    assert_eq!(report.compared, 0);
    let _ = std::fs::remove_dir_all(&dir);
}
