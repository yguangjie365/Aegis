//! `reasons` 码表稳定性测试（与 `models::tests` 里的表级检查互补：
//! 这里钉**具体某几个码的拼写与中文**，防重构时被顺手改掉）。

use super::*;

#[test]
fn user_cancel_is_not_failure() {
    // 🔴 R-14：取消**不是**失败。码与中文都不得被改成含"失败"字样的表述，
    //    否则日志面板会把用户的取消操作呈现成一次错误。
    let (code, text) = pair(USER_CANCELLED);
    assert_eq!(code, "USER_CANCELLED");
    assert_eq!(text, "已取消");
    assert!(
        !text.contains("失败"),
        "取消的中文说明不得含「失败」：{text}"
    );
}

#[test]
fn disk_space_unknown_is_distinct_from_insufficient() {
    // 🔴 R-6 红线：禁止把"未知"当"充足"，也不得把"未知"混进"不足"
    //    （用户需要知道是"确定不够"还是"查不到"—— 后者要警告而非阻断）。
    assert_ne!(DISK_SPACE_UNKNOWN, INSUFFICIENT_DISK);
    assert_eq!(
        describe(DISK_SPACE_UNKNOWN),
        Some("无法确认目标磁盘剩余空间")
    );
    assert_eq!(describe(INSUFFICIENT_DISK), Some("目标磁盘空间不足"));
}

#[test]
fn shadow_and_schedule_codes_exist_for_the_pending_adrs() {
    // 🟡 K-002 / K-003 未拍板 ⇒ 这两个码**必须存在**：
    // 否则 UI 会把"能力不可用"显示成"未知错误"，用户理解不了为什么勾了无效。
    assert!(describe(SHADOW_COPY_UNAVAILABLE).is_some());
    assert!(describe(SCHEDULE_FORM_UNDECIDED).is_some());
}

#[test]
fn guard_rejection_mentions_tampering() {
    // 🔴 R-2：被守卫拒绝时必须说明"疑似快照数据被篡改"——
    //    让用户知道这不是普通的权限问题，而是数据可信度问题。
    let text = describe(GUARD_REJECTED).unwrap_or_default();
    assert!(
        text.contains("安全守卫") || text.contains("篡改"),
        "守卫拒绝的说明须点明性质：{text}"
    );
}
