//! `schedule` 单测：R-15 合法性判据（**本批唯一已实现的部分**）+ R-16 记账。

use super::*;
use crate::backup::models::{BackupSource, SourceType};

fn rule_with_time(time: Option<&str>, enabled: bool, last: Option<&str>) -> BackupRule {
    BackupRule {
        id: "r1".to_owned(),
        name: "测试".to_owned(),
        note: String::new(),
        enabled,
        sources: vec![BackupSource::new("D:\\a", SourceType::Directory)],
        target: crate::backup::models::BackupTarget::GlobalRoot,
        retention: crate::backup::models::RetentionPolicy::default(),
        exclusions: Vec::new(),
        daily_time: time.map(str::to_owned),
        last_scheduled_date: last.map(str::to_owned),
        use_shadow_copy: false,
        order: 0,
    }
}

/// 🔴 R-15 踩坑背景：格式对但越界（如小时填 25）会**永不到点且无提示**。
/// 所以「格式合法」与「取值合法」是两关，缺一即静默失效。
#[test]
fn time_validity_checks_shape_and_range() {
    for good in ["00:00", "09:30", "23:59", "0:00", "9:05", " 09:00 "] {
        assert!(
            is_valid_daily_time(good),
            "「{good}」应被判为合法（允许省略前导零与首尾空白）"
        );
    }
    for bad in [
        "24:00",     // 🔴 越界：永不到点
        "25:00",     // 母本踩过的坑
        "09:60",     // 分钟越界
        "9",         // 形状不对
        "09:0",      // 形状不对
        "09-00",     // 分隔符错
        "ab:cd",     // 非 ASCII 数字
        "０９:００", // 全角数字：scheduled_at 不认
        "",
    ] {
        assert!(
            !is_valid_daily_time(bad),
            "「{bad}」应被判为非法（否则会静默永不到点）"
        );
    }
}

#[test]
fn parse_returns_hour_and_minute() {
    assert_eq!(parse_daily_time("09:05"), Some((9, 5)));
    assert_eq!(parse_daily_time("23:59"), Some((23, 59)));
    assert_eq!(parse_daily_time("24:00"), None);
}

#[test]
fn should_run_respects_every_gate() {
    // 2026-10-05 10:00 本地（UTC+8）
    let at_ten = 1_790_000_000_000;
    let today = "2026-10-05";

    // 正常：启用 + 定时 09:00 + 未记账 + 已过点
    assert!(should_run_now(
        &rule_with_time(Some("09:00"), true, None),
        at_ten,
        today
    ));
    // 停用规则不参与
    assert!(!should_run_now(
        &rule_with_time(Some("09:00"), false, None),
        at_ten,
        today
    ));
    // 未配定时
    assert!(!should_run_now(
        &rule_with_time(None, true, None),
        at_ten,
        today
    ));
    // 时间非法
    assert!(!should_run_now(
        &rule_with_time(Some("25:00"), true, None),
        at_ten,
        today
    ));
    // 同日已记账 ⇒ 不重复（R-16 / F-FB-46）
    assert!(!should_run_now(
        &rule_with_time(Some("09:00"), true, Some(today)),
        at_ten,
        today
    ));
    // 未到点（定时 23:00，此刻 10:00）
    assert!(!should_run_now(
        &rule_with_time(Some("23:00"), true, None),
        at_ten,
        today
    ));
}

#[test]
fn only_success_without_cancel_marks_executed() {
    // 🔴 R-16：失败与取消**不记账** —— 记了就等于"今天不用再试了"
    assert!(should_mark_executed(true, false), "成功且未取消 ⇒ 记账");
    assert!(!should_mark_executed(true, true), "取消不记账");
    assert!(!should_mark_executed(false, false), "失败不记账");
    assert!(!should_mark_executed(false, true), "失败且取消不记账");
}

#[test]
fn task_name_is_derived_from_rule_id() {
    // 🔴 R-17：任务名按规则标识生成；重复注册为覆盖式 ⇒ 幂等
    assert_eq!(task_name("abc-123"), "Aegis\\FileBackup\\abc-123");
    // 同一 id 两次生成同名 ⇒ 覆盖即幂等
    assert_eq!(task_name("abc-123"), task_name("abc-123"));
}

#[test]
fn undecided_registrar_rejects_illegal_time_first() {
    // 🔴 顺序很重要：即便形态未定，**非法时间也必须先被拒**，
    //    否则"永远不执行"的原因会被误报成"形态待定"。
    let registrar = UndecidedTaskRegistrar;
    let bad = rule_with_time(Some("25:00"), true, None);
    let err = registrar.register(&bad).expect_err("应被拒");
    assert!(
        matches!(err, ScheduleError::InvalidTime { .. }),
        "非法时间须报 InvalidTime，实得 {err:?}"
    );

    let good = rule_with_time(Some("09:00"), true, None);
    let err = registrar.register(&good).expect_err("形态未定仍应失败");
    assert!(
        matches!(err, ScheduleError::Failed { .. }),
        "合法时间下应报形态待定，实得 {err:?}"
    );
}

#[test]
fn undecided_registrar_reports_undecided_capability() {
    let registrar = UndecidedTaskRegistrar;
    assert_eq!(
        registrar.query("r1"),
        ScheduleCapability::Undecided,
        "K-003 未拍板 ⇒ 状态恒为「形态待定」，不得显示成「未注册」"
    );
}
