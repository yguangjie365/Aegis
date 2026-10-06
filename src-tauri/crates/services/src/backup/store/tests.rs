//! `store` 单测：**R-18 损坏另存**（不静默丢弃用户数据）+ 原子写往返。

use super::*;
use crate::backup::models::{BackupSource, BackupTarget, RetentionPolicy, SourceType};
use std::path::PathBuf;

fn temp_dir(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("aegis_backup_store_{tag}"));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("建临时目录");
    dir
}

fn sample_rule(id: &str) -> BackupRule {
    BackupRule {
        id: id.to_owned(),
        name: "规则".to_owned(),
        note: String::new(),
        enabled: true,
        sources: vec![BackupSource::new("D:\\a", SourceType::Directory)],
        target: BackupTarget::GlobalRoot,
        retention: RetentionPolicy::Count(7),
        exclusions: Vec::new(),
        daily_time: None,
        last_scheduled_date: None,
        use_shadow_copy: false,
        order: 0,
    }
}

#[test]
fn rules_roundtrip_through_disk() {
    let dir = temp_dir("roundtrip");
    let rules = vec![sample_rule("r1"), sample_rule("r2")];
    save_rules(&dir, &rules).expect("写规则");
    let loaded = load_rules(&dir);
    assert!(loaded.warning.is_none(), "正常读不应有警告");
    assert_eq!(loaded.value.expect("应读到规则"), rules);
    // 原子写不得留临时件
    assert!(!dir.join(".staging-backup.json").exists());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn missing_file_is_not_a_warning() {
    // 🔴 首次运行没有记录是**正常态**，不该弹警告
    let dir = temp_dir("absent");
    let loaded = load_rules(&dir);
    assert!(loaded.value.is_none());
    assert!(
        loaded.warning.is_none(),
        "文件不存在属正常态，不该警告：{:?}",
        loaded.warning
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn corrupted_rules_are_quarantined_not_discarded() {
    // 🔴 R-18：规则记录损坏 ⇒ **先另存留痕**再按可读部分继续，
    //    绝不静默丢弃（丢了用户就没法恢复自己的备份规则）。
    let dir = temp_dir("corrupt");
    let path = dir.join(RULES_FILE);
    // 🔴 用 ASCII 损坏内容：`b"..."` 字节串不能含非ASCII 字符
    std::fs::write(&path, b"{ broken json").expect("写坏记录");

    let loaded = load_rules(&dir);
    assert!(loaded.value.is_none(), "损坏记录不得被猜出内容");
    let quarantined = loaded.quarantined.expect("必须另存留痕");
    assert!(quarantined.exists(), "留痕文件应真实存在");
    assert!(
        std::fs::read_to_string(&quarantined)
            .expect("读留痕")
            .contains("broken json"),
        "留痕内容应与原损坏文件一致"
    );
    assert!(
        loaded.warning.expect("须有警告").contains("已损坏"),
        "警告须说明「已损坏」"
    );
    // 🔴 关键：**原文件不得被删除**（用户数据）
    assert!(path.exists(), "损坏的原文件不得被删除");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn repeated_corruption_does_not_overwrite_previous_quarantine() {
    // 两次损坏 ⇒ 两份留痕（后一次不能盖掉前一次的证据）
    let dir = temp_dir("twice");
    let path = dir.join(RULES_FILE);
    std::fs::write(&path, b"broken once").expect("写坏");
    let first = load_rules(&dir).quarantined.expect("第一次留痕");
    std::fs::write(&path, b"broken twice").expect("再写坏");
    let second = load_rules(&dir).quarantined.expect("第二次留痕");
    assert_ne!(first, second, "第二次损坏不得覆盖第一次的留痕");
    assert!(first.exists() && second.exists());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn settings_are_clamped_on_load() {
    let dir = temp_dir("clamp");
    let raw = BackupSettings {
        default_keep_count: 9999,
        concurrency: 999,
        ..Default::default()
    };
    save_settings(&dir, &raw).expect("写设置");
    let loaded = load_settings(&dir);
    let settings = loaded.value.expect("应读到设置");
    assert_eq!(settings.default_keep_count, 100, "保留份数须钳到上限");
    assert_eq!(settings.concurrency, 8, "并发须钳到 8");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn snake_case_field_names_are_preserved() {
    // 🔴 母本落盘用 `JsonNamingPolicy.SnakeCaseLower` ⇒ Rust 侧必须同为 snake_case，
    //    否则用户的既有记录读不出来。查**多词字段**——单词字段（id/name）区分不出来。
    let dir = temp_dir("snake");
    let mut rule = sample_rule("r1");
    rule.last_scheduled_date = Some("2026-10-05".to_owned());
    rule.daily_time = Some("09:00".to_owned());
    save_rules(&dir, &[rule]).expect("写规则");
    let text = std::fs::read_to_string(dir.join(RULES_FILE)).expect("读回");

    for snake in ["last_scheduled_date", "daily_time", "use_shadow_copy"] {
        assert!(
            text.contains(&format!("\"{snake}\"")),
            "字段「{snake}」须以 snake_case 落盘：{text}"
        );
    }
    for camel in ["lastScheduledDate", "dailyTime", "useShadowCopy"] {
        assert!(
            !text.contains(camel),
            "不得落成 camelCase（会与母本记录不兼容）：{camel}"
        );
    }
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn global_root_falls_back_with_an_explicit_warning() {
    // 🔴 R-19：枚举不到可用磁盘时兜底到文档目录，**且必须留警告**
    //    （静默兜底会让用户以为备份真的落在全局根上）
    let settings = BackupSettings::default();
    let (root, warning) = resolve_global_root(&settings);
    assert!(warning.is_some(), "兜底落点必须有警告，不能静默");
    assert!(root.to_string_lossy().contains("backups"));
    // 显式配置时不警告
    let configured = BackupSettings {
        global_root: "E:\\Backups".to_owned(),
        ..Default::default()
    };
    let (root2, warning2) = resolve_global_root(&configured);
    assert_eq!(root2, PathBuf::from("E:\\Backups"));
    assert!(warning2.is_none());
}
