//! `confirm` 的测试。
//!
//! 🔴 涉及"等待"的用例一律用**极短超时**（10–50ms）+ 真线程，不依赖真实 30 秒；
//! 而"超时即拒"这条本身换成了对 [`ConfirmGate::new`] 注入超时来验，
//! 不引入 sleep（本仓 `clippy.toml` 禁用 `std::thread::sleep`）。

use super::*;
use std::sync::Arc;
use std::time::Duration;

fn snapshot(id: &str) -> PendingConfirmation {
    PendingConfirmation {
        id: id.to_owned(),
        peer_label: "192.168.1.24:18889".to_owned(),
        kind: PendingKind::File,
        file_name: "报告.pdf".to_owned(),
        file_size: 1_000,
        save_directory: r"D:\收件".to_owned(),
        available_bytes: Some(500_000),
        name_conflict: false,
    }
}

#[test]
fn ask_registers_a_pending_that_the_command_layer_can_see() {
    let gate = ConfirmGate::default();
    let id = gate.ask(snapshot("t1"));
    assert_eq!(id, "t1");
    let pending = gate.pending();
    assert_eq!(pending.len(), 1);
    assert_eq!(pending[0].file_name, "报告.pdf");
    assert_eq!(pending[0].save_directory, r"D:\收件");
    assert_eq!(pending[0].available_bytes, Some(500_000));
    assert!(!pending[0].name_conflict);
}

#[test]
fn a_resolved_confirmation_leaves_the_pending_list() {
    let gate = ConfirmGate::default();
    gate.ask(snapshot("t1"));
    assert!(gate.resolve("t1", Decision::Accept));
    assert_eq!(
        gate.pending(),
        Vec::<PendingConfirmation>::new(),
        "已裁决的不该继续出现在待决列表里"
    );
}

#[test]
fn resolving_an_unknown_id_is_reported_not_silently_ok() {
    let gate = ConfirmGate::default();
    assert!(!gate.resolve("nope", Decision::Accept));
}

#[test]
fn a_second_resolve_cannot_flip_the_first_decision() {
    let gate = ConfirmGate::default();
    gate.ask(snapshot("t1"));
    assert!(gate.resolve("t1", Decision::Accept));
    // 🔴 用户点了"接收"之后，一个迟到的"拒绝"不该把它翻过来
    assert!(
        !gate.resolve("t1", Decision::Reject),
        "已回答过的不得被改写（终态不可回退）"
    );
}

#[test]
fn await_returns_the_users_decision_when_resolved() {
    let gate = Arc::new(ConfirmGate::new(Duration::from_secs(5)));
    gate.ask(snapshot("t1"));
    let waiter = Arc::clone(&gate);
    let handle = std::thread::spawn(move || waiter.await_decision("t1"));
    // 给等待线程一点时间登入等待，再裁决
    std::thread::yield_now();
    assert!(gate.resolve("t1", Decision::Accept));
    assert_eq!(handle.join().unwrap().0, Decision::Accept);
}

/// 🔴 R-8 四条路径之一：**超时按拒绝**。
#[test]
fn timeout_denies_by_default() {
    let gate = ConfirmGate::new(Duration::from_millis(20));
    gate.ask(snapshot("t1"));
    let started = std::time::Instant::now();
    let (decision, _) = gate.await_decision("t1");
    assert_eq!(
        decision,
        Decision::TimedOut,
        "没人回答 ⇒ 到期（本端同样按拒绝），但变体必须与「用户拒绝」区分开：对端要收到 CONFIRM_TIMEOUT"
    );
    assert!(
        started.elapsed() >= Duration::from_millis(15),
        "应当真的等过（而不是立刻返回）"
    );
    // 超时后待决被清理，不再占着列表
    assert_eq!(gate.pending(), Vec::<PendingConfirmation>::new());
}

/// 默认裁决是拒绝——这条常量意义上的保证值得单独钉住（R-8 的根）。
#[test]
fn the_default_decision_is_reject() {
    assert_eq!(Decision::default(), Decision::Reject);
}

#[test]
fn reject_is_delivered_as_is() {
    let gate = Arc::new(ConfirmGate::new(Duration::from_secs(5)));
    gate.ask(snapshot("t1"));
    let waiter = Arc::clone(&gate);
    let handle = std::thread::spawn(move || waiter.await_decision("t1"));
    std::thread::yield_now();
    assert!(gate.resolve("t1", Decision::Reject));
    assert_eq!(handle.join().unwrap().0, Decision::Reject);
}

#[test]
fn awaiting_an_unknown_id_denies_conservatively() {
    let gate = ConfirmGate::new(Duration::from_millis(10));
    assert_eq!(gate.await_decision("ghost").0, Decision::Reject);
}

/// 🔴 服务停止时 `cancel_all` 必须**唤醒**等待者：否则 `stop()` 的 join 会卡到超时。
#[test]
fn cancel_all_wakes_waiters_and_denies() {
    let gate = Arc::new(ConfirmGate::new(Duration::from_secs(30)));
    gate.ask(snapshot("t1"));
    gate.ask(snapshot("t2"));
    let waiter = Arc::clone(&gate);
    let handle = std::thread::spawn(move || waiter.await_decision("t1"));
    std::thread::yield_now();

    let started = std::time::Instant::now();
    gate.cancel_all();
    assert_eq!(handle.join().unwrap().0, Decision::Reject);
    assert!(
        started.elapsed() < Duration::from_secs(2),
        "必须被唤醒，而不是等满 30 秒超时（实测 {:?}）",
        started.elapsed()
    );
    // 两条都按拒绝收场（第二条没人等，但也已被置为拒绝）
    assert!(
        !gate.resolve("t2", Decision::Accept),
        "已取消的不得再被接受"
    );
}

#[test]
fn pending_list_is_sorted_for_a_stable_ui() {
    let gate = ConfirmGate::default();
    gate.ask(snapshot("c"));
    gate.ask(snapshot("a"));
    gate.ask(snapshot("b"));
    let ids: Vec<String> = gate.pending().into_iter().map(|p| p.id).collect();
    assert_eq!(ids, vec!["a", "b", "c"]);
}

#[test]
fn re_asking_the_same_id_overwrites_the_old_slot() {
    let gate = ConfirmGate::default();
    gate.ask(snapshot("t1"));
    let mut second = snapshot("t1");
    second.file_name = "另一个.bin".to_owned();
    gate.ask(second);
    let pending = gate.pending();
    assert_eq!(pending.len(), 1, "同 id 只应有一条");
    assert_eq!(pending[0].file_name, "另一个.bin");
}

#[test]
fn timeout_rejects_even_if_a_decision_arrives_immediately_after() {
    let gate = ConfirmGate::new(Duration::from_millis(20));
    gate.ask(snapshot("t1"));
    assert_eq!(
        gate.await_decision("t1").0,
        Decision::TimedOut,
        "到期才判的：即使裁决紧接着到达，也已经过点了（对端要收到 CONFIRM_TIMEOUT）"
    );
    // 迟到很久的"接收"没有对象可改（那条待决已被清理）
    assert!(!gate.resolve("t1", Decision::Accept));
}
