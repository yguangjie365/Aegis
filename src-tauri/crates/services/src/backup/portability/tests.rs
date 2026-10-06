use super::*;
use crate::backup::models::{BackupTarget, RetentionPolicy, SourceType};
use std::path::PathBuf;

/// 造一条规则（只填与本组测试相关的字段）。
fn rule(id: &str, name: &str, sources: &[&str]) -> BackupRule {
    BackupRule {
        id: id.to_owned(),
        name: name.to_owned(),
        note: String::new(),
        enabled: true,
        sources: sources
            .iter()
            .map(|p| BackupSource::new(p, SourceType::Directory))
            .collect(),
        target: BackupTarget::GlobalRoot,
        retention: RetentionPolicy::Count(7),
        exclusions: Vec::new(),
        daily_time: None,
        last_scheduled_date: None,
        use_shadow_copy: false,
        order: 0,
    }
}

fn temp_file(tag: &str, name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("aegis-backup-port-{tag}"));
    std::fs::create_dir_all(&dir).expect("建临时目录");
    dir.join(name)
}

// ── F-FB-15 判据一：同标识且首次命中 = 原地覆盖 ──────────────────────────

#[test]
fn same_id_first_hit_overwrites_in_place() {
    let existing = vec![rule("r1", "旧名", &["C:\\A"])];
    let incoming = vec![rule("r1", "新名", &["C:\\B"])];
    let report = merge_import(&existing, incoming);
    assert_eq!(report.updated, 1, "同标识首次命中应覆盖");
    assert_eq!(report.appended, 0, "覆盖不算新增");
    assert_eq!(report.merged.len(), 1, "覆盖不改变条数");
    assert_eq!(report.merged[0].name, "新名", "应取导入的字段");
    assert_eq!(report.merged[0].sources[0].path, "C:\\B");
}

// ── F-FB-15 判据二：批内重复标识 / 重复命中 = 分配新标识追加 ──────────────

#[test]
fn duplicate_id_within_batch_gets_new_id() {
    let existing = vec![rule("r1", "已有", &["C:\\A"])];
    // 文件里两条同 id：第 1 条覆盖，第 2 条必须换新 id
    let incoming = vec![rule("r1", "甲", &["C:\\X"]), rule("r1", "乙", &["C:\\Y"])];
    let report = merge_import(&existing, incoming);
    assert_eq!(report.updated, 1);
    assert_eq!(report.appended, 1);
    assert_eq!(report.merged.len(), 2);
    let new_id = &report.merged[1].id;
    assert_ne!(new_id, "r1", "第二条必须换新标识");
    assert!(!new_id.trim().is_empty(), "新标识不能为空");
}

#[test]
fn unknown_id_is_appended_verbatim() {
    let existing = vec![rule("r1", "已有", &["C:\\A"])];
    let incoming = vec![rule("r-new", "新来的", &["C:\\Z"])];
    let report = merge_import(&existing, incoming);
    assert_eq!(report.updated, 0);
    assert_eq!(report.appended, 1);
    // 🔴 换机场景：id 必须原样保留，否则「规则 → 旧快照」的对应会断（F-FB-05）
    assert_eq!(report.merged[1].id, "r-new", "未见过的标识应原样保留");
}

// ── F-FB-15 判据三：来源集合完全相同 = 加「（导入）」后缀 ──────────────────

#[test]
fn identical_source_set_gets_import_suffix() {
    let existing = vec![rule("r1", "文档", &["C:\\A", "C:\\B"])];
    let incoming = vec![rule("r2", "文档副本", &["C:\\B", "C:\\A"])];
    let report = merge_import(&existing, incoming);
    assert_eq!(report.renamed, 1, "来源集合相同应改名");
    assert_eq!(report.appended, 1);
    assert_eq!(report.merged[1].name, "文档副本（导入）");
    assert!(
        !report.issues.is_empty(),
        "改名必须留痕（F-FB-15 逐条明细）"
    );
}

#[test]
fn source_set_comparison_ignores_case_and_order() {
    // 🔴 Windows 路径大小写不敏感 ⇒ `C:\Data` 与 `c:\data` 是同一处
    let mixed_case = vec![BackupSource::new("C:\\Data", SourceType::Directory)];
    let lower_case = vec![BackupSource::new("c:\\data", SourceType::Directory)];
    assert!(
        same_source_set(&mixed_case, &lower_case),
        "大小写不同但同路径应判为相同"
    );
    let ab = vec![
        BackupSource::new("C:\\A", SourceType::Directory),
        BackupSource::new("C:\\B", SourceType::Directory),
    ];
    let ba = vec![
        BackupSource::new("C:\\B", SourceType::Directory),
        BackupSource::new("C:\\A", SourceType::Directory),
    ];
    assert!(same_source_set(&ab, &ba), "顺序不同应判为相同");
    let only_a = vec![BackupSource::new("C:\\A", SourceType::Directory)];
    assert!(!same_source_set(&ab, &only_a), "条数不同不能判为相同");
}

#[test]
fn overwrite_does_not_get_renamed() {
    // 🔴 覆盖更新时**不**改名：否则用户改过的规则名会被「（导入）」污染
    let existing = vec![rule("r1", "文档", &["C:\\A"])];
    let incoming = vec![rule("r1", "文档", &["C:\\A"])];
    let report = merge_import(&existing, incoming);
    assert_eq!(report.updated, 1);
    assert_eq!(report.renamed, 0, "覆盖不应触发改名");
    assert_eq!(report.merged[0].name, "文档");
}

// ── F-FB-14 导出 ────────────────────────────────────────────────────────

#[test]
fn export_rejects_wrong_extension() {
    let dir = temp_file("ext", "rules.txt");
    let err = export_rules(&[rule("r1", "A", &["C:\\A"])], &dir).expect_err("扩展名不合规必须拒绝");
    assert!(err.contains("json"), "错误须说明合规扩展名：{err}");
    assert!(!dir.exists(), "被拒绝时不得留下文件");
}

#[test]
fn export_writes_envelope_and_reads_back() {
    let dir = temp_file("rt", "rules.json");
    let original = vec![rule("r1", "文档", &["C:\\A"])];
    let count = export_rules(&original, &dir).expect("导出");
    assert_eq!(count, 1);

    let text = std::fs::read_to_string(&dir).expect("读回");
    let envelope: RuleExportEnvelope = serde_json::from_str(&text).expect("信封可解析");
    assert_eq!(envelope.schema_version, SCHEMA_VERSION);
    assert_eq!(envelope.tool, TOOL_ID);
    assert!(
        envelope.exported_at.contains('T'),
        "导出时刻应为 ISO 文本：{}",
        envelope.exported_at
    );
    // 🔴 id 必须原样带出（换机后仍能寻址旧快照）
    assert_eq!(envelope.rules[0].id, "r1");

    let back = read_import(&dir).expect("读回解析");
    assert_eq!(back.len(), 1);
    assert_eq!(back[0].name, "文档");
    let _ = std::fs::remove_dir_all(dir.parent().expect("父目录"));
}

// ── 导入兼容与版本闸门 ───────────────────────────────────────────────────

#[test]
fn bare_array_is_accepted() {
    // 兼容母本旧格式与用户手写的裸数组
    let text = r#"[{"id":"r1","name":"裸数组","enabled":true,"sources":[],"target":{"mode":"global_root"},"retention":{"count":7},"use_shadow_copy":false,"order":0}]"#;
    let rules = parse_import(text).expect("裸数组应可解析");
    assert_eq!(rules.len(), 1);
    assert_eq!(rules[0].name, "裸数组");
}

#[test]
fn future_schema_version_is_rejected_with_reason() {
    let text = r#"{"schemaVersion":99,"tool":"x","exportedAt":"","rules":[]}"#;
    let err = parse_import(text).expect_err("高版本必须拒绝");
    assert!(
        err.contains('9') && err.contains("升级"),
        "错误须说明版本与对策：{err}"
    );
}

#[test]
fn garbage_text_is_rejected() {
    let err = parse_import("{ 这不是合法 JSON").expect_err("坏文件必须报错");
    assert!(!err.is_empty(), "错误须可读");
}

// ── F-FB-08 重排 ────────────────────────────────────────────────────────

#[test]
fn reorder_syncs_order_field() {
    let mut rules = vec![
        rule("r1", "甲", &["C:\\A"]),
        rule("r2", "乙", &["C:\\B"]),
        rule("r3", "丙", &["C:\\C"]),
    ];
    reorder(
        &mut rules,
        &["r3".to_owned(), "r1".to_owned(), "r2".to_owned()],
    )
    .expect("重排应成功");
    let ids: Vec<&str> = rules.iter().map(|r| r.id.as_str()).collect();
    assert_eq!(ids, vec!["r3", "r1", "r2"]);
    for (index, r) in rules.iter().enumerate() {
        assert_eq!(
            r.order,
            u32::try_from(index).unwrap_or(u32::MAX),
            "order 字段必须与新位置一致"
        );
    }
}

#[test]
fn reorder_rejects_inconsistent_id_set() {
    let mut rules = vec![rule("r1", "甲", &["C:\\A"]), rule("r2", "乙", &["C:\\B"])];
    let err = reorder(&mut rules, &["r1".to_owned()]).expect_err("缺条目必须拒绝");
    assert!(err.contains("r2"), "错误须指出缺了哪条：{err}");
    let err2 = reorder(
        &mut rules,
        &["r1".to_owned(), "r2".to_owned(), "r9".to_owned()],
    )
    .expect_err("多条目必须拒绝");
    assert!(err2.contains("r9"), "错误须指出多了哪条：{err2}");
}

#[test]
fn reorder_is_idempotent_for_same_order() {
    let mut rules = vec![rule("r1", "甲", &["C:\\A"]), rule("r2", "乙", &["C:\\B"])];
    let before: Vec<String> = rules.iter().map(|r| r.id.clone()).collect();
    reorder(&mut rules, &before).expect("同序重排应成功");
    let after: Vec<String> = rules.iter().map(|r| r.id.clone()).collect();
    assert_eq!(before, after);
}
