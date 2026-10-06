//! `scan` 单测：排除匹配（R-8①）与多来源去重（R-8③）。

use super::*;
use std::path::PathBuf;

fn temp_dir(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("aegis_backup_scan_{tag}"));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("建临时目录");
    dir
}

#[test]
fn glob_matches_question_and_star() {
    // 🔴 语义对齐 .NET `FileSystemName.MatchesSimpleExpression`：
    //    `*` 匹配**零个或多个**、`?` 匹配**恰好一个**
    assert!(glob_match("*.tmp", "a.tmp"));
    assert!(glob_match("*.tmp", ".tmp"), "`*` 可匹配零个字符");
    assert!(glob_match("node_modules", "NODE_MODULES"), "大小写不敏感");
    assert!(glob_match("a?c", "abc"));
    assert!(!glob_match("a?c", "ac"), "`?` 必须恰好一个");
    assert!(!glob_match("a?c", "abbc"));
    assert!(glob_match("*", "任意东西"));
    assert!(!glob_match("*.tmp", "a.txt"));
}

#[test]
fn exclusion_matches_any_path_segment() {
    // 🔴 R-8①：按"路径任一段命中"判定
    let m = ExclusionMatcher::new(&["node_modules".to_owned(), "*.tmp".to_owned()]);
    assert!(m.is_excluded("proj/node_modules/pkg/index.js"));
    assert!(m.is_excluded("node_modules"));
    assert!(m.is_excluded("a/b/c.tmp"));
    assert!(!m.is_excluded("proj/src/index.js"));
}

#[test]
fn exclusion_matches_multi_segment_prefix() {
    // 🔴 R-8①：多段相对路径前缀（`bin/Debug`）
    let m = ExclusionMatcher::new(&["bin/Debug".to_owned()]);
    assert!(m.is_excluded("proj/bin/Debug/app.exe"));
    assert!(m.is_excluded("BIN/DEBUG/app.exe"), "分隔符与大小写不敏感");
    assert!(!m.is_excluded("proj/bin/Release/app.exe"));
    assert!(!m.is_excluded("proj/Debug/app.exe"), "必须整段前缀命中");
}

#[test]
fn exclusion_dedups_and_trims() {
    // F-FB-13：自动去空白与重复
    let m = ExclusionMatcher::new(&[
        "  node_modules  ".to_owned(),
        "NODE_MODULES".to_owned(),
        String::new(),
        "   ".to_owned(),
    ]);
    assert!(
        m.is_excluded("x/node_modules"),
        "重复项应被去重（否则只是白占内存，但不得影响结果）"
    );
}

#[test]
fn contained_sources_are_dropped() {
    // 🔴 R-8③：多来源中某来源被另一来源包含 ⇒ 移除内侧，
    //    否则同一物理文件被备份两遍（体积翻倍、清单出现重复条目）。
    let sources = vec![
        BackupSource::new("D:\\Data", SourceType::Directory),
        BackupSource::new("D:\\Data\\sub", SourceType::Directory),
        BackupSource::new("E:\\Other", SourceType::Directory),
    ];
    let kept = drop_contained_sources(&sources);
    let paths: Vec<&str> = kept.iter().map(|s| s.path.as_str()).collect();
    assert!(paths.contains(&"D:\\Data"));
    assert!(!paths.contains(&"D:\\Data\\sub"), "内侧来源应被移除");
    assert!(paths.contains(&"E:\\Other"), "无关来源不得被误删");
}

#[test]
fn files_are_never_dropped_by_containment_rule() {
    // 🔴 包含判定只应作用于目录来源；单文件来源即使用户写在别的来源内部也要保留
    let sources = vec![
        BackupSource::new("D:\\Data", SourceType::Directory),
        BackupSource::new("D:\\Data\\note.txt", SourceType::File),
    ];
    let kept = drop_contained_sources(&sources);
    assert_eq!(kept.len(), 2, "单文件来源不应被包含规则剔除");
}

#[test]
fn scan_finds_files_and_skips_excluded() {
    let dir = temp_dir("walk");
    std::fs::create_dir_all(dir.join("src")).expect("建子目录");
    std::fs::create_dir_all(dir.join("node_modules")).expect("建排除目录");
    std::fs::write(dir.join("src").join("a.txt"), b"aaa").expect("写文件");
    std::fs::write(dir.join("node_modules").join("x.js"), b"x").expect("写文件");

    let source = BackupSource::new(dir.to_str().expect("路径"), SourceType::Directory);
    let matcher = ExclusionMatcher::new(&["node_modules".to_owned()]);
    let (files, _empty, _skipped) = scan_source(&source, &matcher);

    let relatives: Vec<&str> = files.iter().map(|f| f.relative.as_str()).collect();
    assert!(relatives.contains(&"src/a.txt"));
    assert!(
        !relatives.iter().any(|r| r.contains("node_modules")),
        "被排除的目录不得进入结果：{relatives:?}"
    );

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn missing_source_is_reported_not_panicked() {
    // 🔴 F-FB-04 允许来源缺失（外置盘未接）；此时必须**记入 skipped** 而非崩掉
    let source = BackupSource::new("Z:\\绝对不存在的盘\\x", SourceType::Directory);
    let matcher = ExclusionMatcher::new(&[]);
    let (files, _empty, skipped) = scan_source(&source, &matcher);
    assert_eq!(files.len(), 0, "单文件来源被目录包含时不该产出条目");
    assert_eq!(skipped, vec!["Z:\\绝对不存在的盘\\x".to_owned()]);
}

#[test]
fn empty_directories_are_recorded() {
    // 🔴 R-10：空目录作为独立信息记入清单，恢复时重建
    let dir = temp_dir("empty");
    std::fs::create_dir_all(dir.join("empty-one")).expect("建空目录");
    std::fs::write(dir.join("has.txt"), b"x").expect("写文件");

    let source = BackupSource::new(dir.to_str().expect("路径"), SourceType::Directory);
    let matcher = ExclusionMatcher::new(&[]);
    let (files, empty_dirs, _skipped) = scan_source(&source, &matcher);

    assert_eq!(files.len(), 1);
    assert!(
        empty_dirs.iter().any(|d| d.contains("empty-one")),
        "空目录应被记入：{empty_dirs:?}"
    );

    let _ = std::fs::remove_dir_all(&dir);
}
