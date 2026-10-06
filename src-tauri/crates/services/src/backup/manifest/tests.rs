//! `manifest` 单测：**母本两个已修 bug 的回归钉桩** + 清单读写。
//!
//! 这两个 bug 若复发，症状是"某些快照在列表里不出现"或"两份快照互相覆盖"，
//! 都不会报错、只会静默少数据 —— 所以必须有专门的用例守着。

use super::*;
use std::path::PathBuf;

fn temp_dir(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("aegis_backup_manifest_{tag}"));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("建临时目录");
    dir
}

/// 🔴 母本 bug #1 回归（2026-09-11 实测修复）：随机后缀取自 `Path.GetRandomFileName()`
/// 的基 32 字符集，**可能含字母**（`u`/`y` 等专用于避开 8.3 文件名冲突）。
/// 母本原按「`_` 之后是纯数字」判断时间 ⇒ 遇到含字母的后缀就解析失败 ⇒
/// 该快照在列表里不出现、也不被裁剪（用户会以为快照丢了）。
/// 本实现按**前 15 位固定宽度**解析，与后缀是否含字母无关。
#[test]
fn parses_time_even_when_random_suffix_contains_letters() {
    let base = "20261005_153000";
    for suffix in ["ab12cd", "u7y2z9q", "000001", "zzz"] {
        let name = format!("{base}_{suffix}");
        let parsed = parse_snapshot_time(&name);
        assert!(
            parsed.is_some(),
            "后缀含非数字的「{name}」应仍能解析出时间（母本 bug #1 回归）"
        );
        let (y, mo, d) = super::super::retention::civil_from_unix_ms(parsed.expect("已解析"));
        assert_eq!((y, mo, d), (2026, 10, 5), "{name} 的日期解析错误");
    }
}

#[test]
fn rejects_malformed_names() {
    // 形状不对 / 越界一律 None（⇒ 上层按 R-5「跳过不裁剪」处理，绝不猜）
    for name in [
        "",
        "2026",
        "20261005",        // 少时间部分
        "20261005_1530",   // 秒位缺失
        "20261005_253000", // 小时 25 越界
        "20261005_156000", // 分钟 60 越界
        "20261305_153000", // 月份 13 越界
        "abcdefgh_123456", // 完全不是时间
        "20261005-153000", // 分隔符错
    ] {
        assert_eq!(parse_snapshot_time(name), None, "「{name}」不该被解析成功");
    }
}

#[test]
fn display_name_hides_the_dedup_suffix() {
    // 🔴 F-FB-32：用户只看到可读时间，后缀对其无意义
    let full = "20261005_153000_ab12cd";
    assert_eq!(display_dir_name(full), "20261005_153000");
    // 已是展示名则原样返回
    assert_eq!(display_dir_name("20261005_153000"), "20261005_153000");
}

#[test]
fn stamp_format_is_fixed_width() {
    // 宽度必须恒为 15 —— parse_snapshot_time 靠 `get(..15)` 取值
    let stamp = format_snapshot_stamp(1_790_000_000_000);
    assert_eq!(stamp.chars().count(), 15, "时间戳必须是 15 字符");
    assert_eq!(&stamp[8..9], "_", "第 9 位必须是下划线");
}

/// 🔴 母本 bug #2 回归：母本先 `Directory.Exists` 再 `CreateDirectory`，
/// 两步之间同刻并发会同名 ⇒ 互相覆盖。本实现用随机后缀 + 独占创建。
#[test]
fn concurrent_creates_never_collide() {
    let dir = temp_dir("toctou");
    // 同一毫秒连建 20 次，全部必须得到不同目录
    let mut created: Vec<PathBuf> = Vec::new();
    for _ in 0..20 {
        let snapshot = create_snapshot_dir(&dir, "rule-1", 1_790_000_000_000).expect("建目录");
        assert!(
            !created.contains(&snapshot),
            "同一毫秒产出了重复目录：{}",
            snapshot.display()
        );
        created.push(snapshot);
    }
    assert_eq!(created.len(), 20);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn manifest_roundtrips_through_disk() {
    let dir = temp_dir("roundtrip");
    let snapshot = dir.join("snap");
    std::fs::create_dir_all(&snapshot).expect("建快照目录");

    let m = build_manifest(ManifestInput {
        rule_id: "rule-1".to_owned(),
        rule_name: "文档备份".to_owned(),
        snapshot_id: "snap".to_owned(),
        created_unix_ms: 1_790_000_000_000,
        sources: vec!["D:\\工作".to_owned()],
        entries: vec![file_entry(
            "src/a.txt",
            std::path::Path::new("D:\\工作\\src\\a.txt"),
            12,
            100,
            "abc".to_owned(),
        )],
        empty_dirs: vec!["src/empty".to_owned()],
        status: SnapshotStatus::Success,
        integrity: IntegrityStatus::Passed,
        reused_count: 0,
    });
    write_manifest(&snapshot, &m).expect("写清单");
    let loaded = read_manifest(&snapshot).expect("读清单");
    assert_eq!(loaded, m, "清单往返后必须逐字段相等");
    assert!(
        !snapshot.join(".staging-manifest.json").exists(),
        "不得留临时件"
    );

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn corrupt_manifest_reports_error_not_partial_parse() {
    // 🔴 规格 §七·2：清单缺失或损坏时报告"清单读取失败"，
    //    **不得**尽力解析部分字段 —— 不可信的清单绝不能驱动恢复。
    let dir = temp_dir("corrupt");
    let snapshot = dir.join("snap");
    std::fs::create_dir_all(&snapshot).expect("建目录");
    std::fs::write(snapshot.join(MANIFEST_FILE), b"{ not json").expect("写坏清单");
    let err = read_manifest(&snapshot).expect_err("损坏清单必须报错");
    assert!(matches!(err, ManifestError::Parse { .. }));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn missing_manifest_reports_io_error() {
    let dir = temp_dir("absent");
    let snapshot = dir.join("snap");
    std::fs::create_dir_all(&snapshot).expect("建目录");
    let err = read_manifest(&snapshot).expect_err("缺失清单必须报错");
    assert!(matches!(err, ManifestError::Io { .. }));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn list_dirs_separates_parsable_from_unparsable() {
    // 🔴 无法解析的目录**不报错但要被单独报出** —— 它们可能正在被人工恢复，
    //    悄悄处理掉会让用户以为快照丢了。
    let dir = temp_dir("list");
    let rule_dir = dir.join(rule_dir_name("r1"));
    std::fs::create_dir_all(&rule_dir).expect("建规则目录");
    for name in [
        "20261005_153000_aaaaaa",
        "20261004_090000_bbbbbb",
        "手工放的目录",
    ] {
        std::fs::create_dir_all(rule_dir.join(name)).expect("建子目录");
    }
    let (parsed, unparsed) = list_snapshot_dirs(&dir, "r1");
    assert_eq!(parsed.len(), 2, "应解析出 2 个快照");
    assert_eq!(unparsed, vec!["手工放的目录".to_owned()]);
    // 时间倒序 ⇒ [0] 是最新
    assert!(parsed[0].created_unix_ms > parsed[1].created_unix_ms);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn rule_dir_name_is_sanitized() {
    // 🔴 规则 id 可能含非法字符 ⇒ 走 infra 的写侧清洗，不自己拼
    assert_eq!(rule_dir_name("normal-id"), "normal-id");
    let dirty = rule_dir_name("id/with\\bad:chars");
    assert!(!dirty.contains('/') && !dirty.contains('\\') && !dirty.contains(':'));
}
