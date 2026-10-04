//! `history` 的测试。
//!
//! 重点全在**导出的可用性**与**容量/幂等**这两件事上：CSV 转义写错不会报错，
//! 只会让导出的表在某些文件名下悄悄串列——那是最难被发现的坏法。

use super::*;

fn entry(id: &str, file_name: &str, state: TaskState, reason: Option<&str>) -> HistoryEntry {
    HistoryEntry {
        id: id.to_owned(),
        direction: Direction::Receive,
        peer_label: "192.168.1.24:18889".to_owned(),
        file_name: file_name.to_owned(),
        file_size: 3112,
        state,
        reason_code: reason.map(str::to_owned),
        finished_at_ms: 1_700_000_000_000,
    }
}

#[test]
fn push_appends_in_order() {
    let mut history = History::new();
    history.push(entry("a", "一.bin", TaskState::Completed, None));
    history.push(entry("b", "二.bin", TaskState::Completed, None));
    let names: Vec<&str> = history
        .entries()
        .iter()
        .map(|item| item.file_name.as_str())
        .collect();
    assert_eq!(names, vec!["一.bin", "二.bin"]);
}

#[test]
fn the_same_id_overwrites_instead_of_duplicating() {
    let mut history = History::new();
    history.push(entry(
        "a",
        "旧名.bin",
        TaskState::Cancelled,
        Some("USER_CANCEL"),
    ));
    history.push(entry("a", "新名.bin", TaskState::Completed, None));
    assert_eq!(history.len(), 1, "一次传输只该在历史里占一行");
    let only = history.entries().front().expect("有一条");
    assert_eq!(only.file_name, "新名.bin");
    assert_eq!(only.state, TaskState::Completed);
}

#[test]
fn capacity_drops_the_oldest() {
    let mut history = History::new();
    for index in 0..(HISTORY_CAPACITY + 5) {
        history.push(entry(
            &format!("t{index}"),
            "x.bin",
            TaskState::Completed,
            None,
        ));
    }
    assert_eq!(history.len(), HISTORY_CAPACITY);
    // 最旧的 5 条应被丢掉
    assert_eq!(history.entries().front().expect("有头").id, "t5");
    assert_eq!(
        history.entries().back().expect("有尾").id,
        format!("t{}", HISTORY_CAPACITY + 4)
    );
}

#[test]
fn from_entries_truncates_to_the_capacity_keeping_the_newest() {
    let many: Vec<HistoryEntry> = (0..(HISTORY_CAPACITY + 3))
        .map(|index| entry(&format!("t{index}"), "x.bin", TaskState::Completed, None))
        .collect();
    let history = History::from_entries(many);
    assert_eq!(history.len(), HISTORY_CAPACITY);
    assert_eq!(history.entries().front().expect("有头").id, "t3", "丢最旧");
}

// ── CSV 转义（写错的后果是"悄悄串列"） ──────────────────────────────────

#[test]
fn csv_escape_quotes_only_when_needed() {
    assert_eq!(csv_escape("普通文件名.bin"), "普通文件名.bin");
    assert_eq!(csv_escape(""), "");
    // 含逗号 → 整体加引号
    assert_eq!(csv_escape("报告,终稿.pdf"), "\"报告,终稿.pdf\"");
    // 含引号 → 加引号 + 内部翻倍
    assert_eq!(csv_escape("他说\"好\".txt"), "\"他说\"\"好\"\".txt\"");
    // 含换行 → 加引号（否则一行会被撑成两行）
    assert_eq!(csv_escape("两行\n文件.txt"), "\"两行\n文件.txt\"");
    assert_eq!(csv_escape("带回车\r.txt"), "\"带回车\r.txt\"");
}

#[test]
fn csv_has_bom_header_and_crlf() {
    let mut history = History::new();
    history.push(entry("a", "一件.bin", TaskState::Completed, None));
    let csv = history.to_csv();
    assert!(
        csv.starts_with('\u{feff}'),
        "必须带 BOM，否则 Excel 中文乱码"
    );
    assert!(csv.contains(CSV_HEADER), "表头要按 F-FT-58 固定");
    assert!(csv.ends_with("\r\n"), "RFC 4180 用 CRLF");
    // 表头一行 + 数据一行（数据行末尾也是 CRLF）
    assert_eq!(csv.matches("\r\n").count(), 2);
}

#[test]
fn csv_keeps_a_comma_filename_in_one_line() {
    let mut history = History::new();
    history.push(entry("a", "报告,终稿.pdf", TaskState::Completed, None));
    let csv = history.to_csv();
    let data_line = csv.lines().nth(1).expect("有数据行");
    // `lines()` 按 \n 切；CRLF 下末尾会带 \r，这里关心的是**没有被逗号撑成两行/多列**
    assert!(
        data_line.contains("\"报告,终稿.pdf\""),
        "含逗号的文件名必须整体加引号，实得：{data_line}"
    );
    assert_eq!(csv.matches("\r\n").count(), 2, "仍然只有两行");
}

#[test]
fn csv_uses_the_frozen_reason_text_and_passes_unknown_codes_through() {
    let mut history = History::new();
    history.push(entry(
        "a",
        "x.bin",
        TaskState::Cancelled,
        Some("USER_REJECT"),
    ));
    history.push(entry(
        "b",
        "y.bin",
        TaskState::Failed,
        Some("SOMETHING_NEW"),
    ));
    let csv = history.to_csv();
    assert!(csv.contains("接收方用户拒绝"), "已知码用冻结表的中文");
    // 🔴 F-FT-31 末句：未知码原样显示，不硬翻成「未知错误」
    assert!(csv.contains("SOMETHING_NEW"));
    assert!(!csv.contains("未知错误"));
}

#[test]
fn a_completed_entry_has_an_empty_reason_column() {
    let mut history = History::new();
    history.push(entry("a", "x.bin", TaskState::Completed, None));
    let csv = history.to_csv();
    let line = csv.lines().nth(1).expect("有数据行");
    let cells: Vec<&str> = line.trim_end_matches('\r').split(',').collect();
    assert_eq!(cells.len(), 8, "八列：{cells:?}");
    assert_eq!(cells[6], "", "原因码为空");
    assert_eq!(cells[7], "", "原因说明为空");
}

#[test]
fn direction_and_state_labels_are_chinese() {
    let mut received = entry("a", "x.bin", TaskState::Skipped, Some("CONFLICT_SKIP"));
    assert_eq!(received.direction_label(), "接收");
    assert_eq!(received.state_label(), "已跳过");
    received.direction = Direction::Send;
    assert_eq!(received.direction_label(), "发送");
}

#[test]
fn format_time_renders_a_readable_stamp() {
    // 1_700_000_000_000 = 2023-11-14T22:13:20Z
    let text = format_time(1_700_000_000_000);
    assert_eq!(text.len(), 19, "YYYY-MM-DD HH:MM:SS，实得 {text}");
    assert!(text.starts_with("2023-11-1"), "实得 {text}");
}

#[test]
fn format_time_returns_the_raw_number_for_absurd_values() {
    // 超出 chrono 可表示范围 → 返回原始毫秒串（而不是空白：空白会被读成"没有时间"）
    assert_eq!(format_time(i64::MIN), i64::MIN.to_string());
}

// ── 落盘往返 ────────────────────────────────────────────────────────────

#[test]
fn save_then_load_round_trips() {
    let dir = std::env::temp_dir().join(format!("aegis-hist-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join(HISTORY_FILE_NAME);

    let mut history = History::new();
    history.push(entry("a", "一件.bin", TaskState::Completed, None));
    history.push(entry(
        "b",
        "两件,含逗号.bin",
        TaskState::Failed,
        Some("HASH_MISMATCH"),
    ));
    history.save(&path).expect("写盘");

    let loaded = History::load(&path).expect("读盘");
    assert_eq!(loaded.entries(), history.entries(), "往返后逐条相同");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_missing_file_is_an_empty_history_not_an_error() {
    let path = std::env::temp_dir().join("aegis-hist-does-not-exist-xyz.json");
    let history = History::load(&path).expect("首次运行不该报错");
    assert!(history.is_empty());
}

#[test]
fn a_corrupt_file_is_reported_instead_of_silently_empty() {
    let dir = std::env::temp_dir().join(format!("aegis-hist-bad-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join(HISTORY_FILE_NAME);
    std::fs::write(&path, b"{ this is not json").unwrap();
    // 🔴 静默返回空表会让"历史莫名清空"无从排查
    assert!(History::load(&path).is_err());
    let _ = std::fs::remove_dir_all(&dir);
}

/// 🔴 钉住：默认路径的最后一段必须与 [`HISTORY_FILE_NAME`] 一致。
///
/// `default_history_path()` 里**故意写了字面量**（架构守卫不允许带常量参数的 `.join(`），
/// 于是"字面量与常量"就有了分家的可能——分家不会报错，只会让读历史的和写历史的
/// 指向两个文件。故用测试把它们钉在一起。
#[test]
fn default_path_ends_with_the_constant() {
    let path = default_history_path();
    assert_eq!(
        path.file_name().and_then(|name| name.to_str()),
        Some(HISTORY_FILE_NAME),
        "默认路径末段与常量不一致：{path:?}"
    );
    assert!(path.to_string_lossy().contains("aegis"));
}
