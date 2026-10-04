//! `tasks` 的测试。
//!
//! 全部纯逻辑：时刻由调用方传入，故 R-16「终态不可回退」与进度节流的边界都能穷举，
//! 不必等真实时间流逝（本仓 `clippy.toml` 禁用 `std::thread::sleep`）。

use super::*;

const T0: i64 = 1_760_000_000_000;

fn begun() -> (TaskRegistry, String) {
    let mut registry = TaskRegistry::default();
    let id = registry.begin(
        "t1",
        Direction::Receive,
        "192.168.1.24:18889",
        "报告.pdf",
        1_000,
        T0,
    );
    (registry, id)
}

// ── 建任务与进度 ──────────────────────────────────────────────────────────

#[test]
fn begin_creates_an_active_task_and_counts_toward_concurrency() {
    let (registry, id) = begun();
    assert_eq!(registry.active_count(), 1);
    let task = registry.get(&id).expect("应能取到");
    assert_eq!(task.state, TaskState::Active);
    assert_eq!(task.transferred, 0);
    assert_eq!(task.file_size, 1_000);
    assert_eq!(task.peer_label, "192.168.1.24:18889");
    assert_eq!(task.started_at_ms, T0);
    assert_eq!(task.finished_at_ms, None);
    assert_eq!(task.reason_code, None);
}

#[test]
fn progress_percent_handles_zero_size_and_clamps() {
    let mut registry = TaskRegistry::default();
    let zero = registry.begin("z", Direction::Send, "p", "empty.bin", 0, T0);
    assert_eq!(
        registry.get(&zero).unwrap().percent(),
        100,
        "空文件传完即完成"
    );

    let id = registry.begin("t", Direction::Send, "p", "a.bin", 1000, T0);
    assert_eq!(registry.get(&id).unwrap().percent(), 0);
    registry.advance(&id, 250);
    assert_eq!(registry.get(&id).unwrap().percent(), 25);
    registry.advance(&id, 1000);
    assert_eq!(registry.get(&id).unwrap().percent(), 100);
    // 超出声明大小也不得越过 100
    registry.advance(&id, 9_999);
    assert_eq!(registry.get(&id).unwrap().percent(), 100);
}

#[test]
fn advance_is_monotonic_and_clamped_to_the_declared_size() {
    let (mut registry, id) = begun();
    assert!(registry.advance(&id, 400));
    assert_eq!(registry.get(&id).unwrap().transferred, 400);
    // 旧帧（更小的值）不得让界面倒退
    assert!(registry.advance(&id, 100));
    assert_eq!(registry.get(&id).unwrap().transferred, 400, "进度只增不减");
    // 超过声明大小按声明大小截断
    assert!(registry.advance(&id, 999_999));
    assert_eq!(registry.get(&id).unwrap().transferred, 1_000);
}

#[test]
fn advance_on_an_unknown_task_is_ignored() {
    let (mut registry, _) = begun();
    assert!(!registry.advance("nope", 100), "未知任务应被忽略而非 panic");
}

// ── R-16：终态不可回退（本模块的核心承诺） ─────────────────────────────────

#[test]
fn terminal_states_cannot_be_overwritten_by_late_messages() {
    for terminal in [
        TaskState::Completed,
        TaskState::Skipped,
        TaskState::Failed,
        TaskState::Cancelled,
    ] {
        let (mut registry, id) = begun();
        assert!(
            registry.finish(&id, terminal, Some("X"), T0 + 10),
            "首次写终态应成功"
        );
        // 🔴 迟到的第二条终态必须被拒（"取消后对端才回执完成"就是这一条）
        assert!(
            !registry.finish(&id, TaskState::Completed, None, T0 + 99),
            "[{terminal:?}] 终态不得被改写"
        );
        let task = registry.get(&id).unwrap();
        assert_eq!(task.state, terminal, "结论必须保持第一次写入的那个");
        assert_eq!(
            task.reason_code.as_deref(),
            Some("X"),
            "原因码同样不得被改写"
        );
        assert_eq!(task.finished_at_ms, Some(T0 + 10));
    }
}

#[test]
fn progress_arriving_after_a_terminal_state_is_discarded() {
    let (mut registry, id) = begun();
    assert!(registry.finish(&id, TaskState::Cancelled, Some("USER_CANCEL"), T0 + 10));
    // 🔴 乱序到达的旧进度帧不得把"已取消"改回"进行中"
    assert!(!registry.advance(&id, 500), "终态任务的进度必须被丢弃");
    let task = registry.get(&id).unwrap();
    assert_eq!(task.state, TaskState::Cancelled);
    assert_eq!(task.transferred, 0, "进度也不该被改动");
}

#[test]
fn finish_rejects_a_non_terminal_state() {
    let (mut registry, id) = begun();
    assert!(
        !registry.finish(&id, TaskState::Active, None, T0),
        "用 finish 写非终态是用法错误，应拒绝"
    );
    assert_eq!(registry.get(&id).unwrap().state, TaskState::Active);
    assert_eq!(registry.active_count(), 1, "活跃计数不该被误清");
}

#[test]
fn completing_marks_the_task_as_fully_transferred() {
    let (mut registry, id) = begun();
    registry.advance(&id, 990);
    assert!(registry.finish(&id, TaskState::Completed, None, T0 + 10));
    // 末帧进度可能被节流丢掉；完成态直接补满，避免界面停在 99%
    assert_eq!(registry.get(&id).unwrap().transferred, 1_000);
    assert_eq!(registry.get(&id).unwrap().percent(), 100);
}

#[test]
fn finishing_frees_a_concurrency_slot() {
    let (mut registry, id) = begun();
    assert_eq!(registry.active_count(), 1);
    registry.finish(&id, TaskState::Completed, None, T0);
    assert_eq!(registry.active_count(), 0, "终态必须释放并发槽");
    // 重复收尾不再重复减（否则计数会下溢成天文数字）
    assert!(!registry.finish(&id, TaskState::Failed, None, T0));
    assert_eq!(registry.active_count(), 0);
}

// ── 容量与淘汰 ────────────────────────────────────────────────────────────

#[test]
fn eviction_drops_the_oldest_terminal_task_but_never_an_active_one() {
    let mut registry = TaskRegistry::new(3);
    let a = registry.begin("a", Direction::Send, "p", "a", 1, T0);
    let b = registry.begin("b", Direction::Send, "p", "b", 1, T0);
    let c = registry.begin("c", Direction::Send, "p", "c", 1, T0);
    registry.finish(&a, TaskState::Completed, None, T0);

    // 再加一条：a 是最旧的**终态**任务，应被淘汰；b/c 进行中，必须留
    let d = registry.begin("d", Direction::Send, "p", "d", 1, T0);
    let ids: Vec<String> = registry.snapshot().into_iter().map(|t| t.id).collect();
    assert_eq!(ids, vec!["b", "c", "d"]);
    assert!(registry.get(&a).is_none(), "最旧的终态应被淘汰");
    assert!(registry.get(&b).is_some() && registry.get(&c).is_some());
    assert!(registry.get(&d).is_some());
}

#[test]
fn eviction_gives_up_when_everything_is_active() {
    let mut registry = TaskRegistry::new(2);
    registry.begin("a", Direction::Send, "p", "a", 1, T0);
    registry.begin("b", Direction::Send, "p", "b", 1, T0);
    registry.begin("c", Direction::Send, "p", "c", 1, T0);
    // 宁可暂时超容量，也不能丢掉活跃任务（丢了就永远收不到它的终态）
    assert_eq!(registry.snapshot().len(), 3);
    assert_eq!(registry.active_count(), 3);
}

#[test]
fn snapshot_is_ordered_by_start_time() {
    let mut registry = TaskRegistry::default();
    registry.begin("first", Direction::Send, "p", "1", 1, T0);
    registry.begin("second", Direction::Send, "p", "2", 1, T0 + 1);
    let ids: Vec<String> = registry.snapshot().into_iter().map(|t| t.id).collect();
    assert_eq!(ids, vec!["first", "second"]);
}

// ── 进度节流 ──────────────────────────────────────────────────────────────

#[test]
fn throttle_merges_updates_within_the_interval() {
    let mut throttle = ProgressThrottle::new(100);
    assert!(throttle.should_report(T0), "第一次必报（上次是 i64::MIN）");
    assert!(!throttle.should_report(T0 + 50), "间隔内不报");
    assert!(!throttle.should_report(T0 + 99), "差 1ms 仍不报");
    assert!(throttle.should_report(T0 + 100), "到点即报");
    // 报完之后重新计时（不是"每次都比最初那次"）
    assert!(!throttle.should_report(T0 + 150));
    assert!(throttle.should_report(T0 + 200));
}

#[test]
fn throttle_records_the_time_it_reported_so_cadence_does_not_drift() {
    let mut throttle = ProgressThrottle::new(100);
    assert!(throttle.should_report(T0));
    // 迟到的调用（间隔 350ms）之后，下一次应在它自己 +100ms
    assert!(throttle.should_report(T0 + 350));
    assert!(!throttle.should_report(T0 + 400));
    assert!(throttle.should_report(T0 + 450));
}

#[test]
fn throttle_never_starves_when_the_interval_is_not_positive() {
    // 🔴 间隔非法时退化为"最小间隔 1ms"，而不是"永远不报"——
    // 后者会让界面永远不动且不报错，是最难查的一种。
    // 注意语义：夹到 1ms 之后**同一毫秒内的两次调用仍会合并**（首版断言"每次都报"是错的）
    for bad in [0, -1, i64::MIN] {
        let mut throttle = ProgressThrottle::new(bad);
        assert!(throttle.should_report(T0), "间隔 {bad} 时首报");
        assert!(
            !throttle.should_report(T0),
            "间隔 {bad} 时同一毫秒内仍合并（夹到 1ms 的必然结果）"
        );
        assert!(
            throttle.should_report(T0 + 1),
            "间隔 {bad} 时下一毫秒即报——不得饿死界面"
        );
    }
}

#[test]
fn force_next_lets_the_final_frame_through() {
    let mut throttle = ProgressThrottle::new(100);
    assert!(throttle.should_report(T0));
    assert!(!throttle.should_report(T0 + 10), "间隔内本会吞掉");
    throttle.force_next();
    assert!(
        throttle.should_report(T0 + 10),
        "🔴 终态收尾的那一帧不能被节流吞掉"
    );
}

#[test]
fn state_strings_are_stable_identifiers_for_the_frontend() {
    assert_eq!(TaskState::Active.as_str(), "active");
    assert_eq!(TaskState::Completed.as_str(), "completed");
    assert_eq!(TaskState::Skipped.as_str(), "skipped");
    assert_eq!(TaskState::Failed.as_str(), "failed");
    assert_eq!(TaskState::Cancelled.as_str(), "cancelled");
    assert!(!TaskState::Active.is_terminal());
    for terminal in [
        TaskState::Completed,
        TaskState::Skipped,
        TaskState::Failed,
        TaskState::Cancelled,
    ] {
        assert!(terminal.is_terminal(), "{terminal:?} 必须是终态");
    }
}
