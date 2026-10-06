//! `retention` 单测：R-5 三条硬约束 + GFS 桶判据。
//!
//! 🔴 这些用例的价值在于**钉住"不许删"的边界**—— 保留策略是唯一会主动
//! 删除用户数据的逻辑，一条判据写错就是丢快照。

use super::*;
use crate::backup::models::RetentionPolicy;

/// 构造候选（新→旧顺序由 `plan_retention` 内部排序，传入顺序无关）。
fn cand(times: &[Option<i64>], newest: usize) -> Vec<RetentionCandidate> {
    times
        .iter()
        .enumerate()
        .map(|(i, t)| RetentionCandidate {
            created_unix_ms: *t,
            is_newest: i == newest,
        })
        .collect()
}

#[test]
fn newest_snapshot_is_never_deleted() {
    // 🔴 硬约束 1：最新一份永不被删（Count 与 GFS 两种策略都要成立）
    let times = vec![Some(1_000), Some(2_000), Some(3_000)];
    let candidates = cand(&times, 2); // 索引 2 = 时间 3000 = 最新
    for policy in [
        RetentionPolicy::Count(1),
        RetentionPolicy::Gfs(GfsRetention {
            daily_keep: 0,
            weekly_keep: 0,
            monthly_keep: 0,
        }),
    ] {
        let decision = plan_retention(&candidates, policy);
        assert!(
            !decision.delete.contains(&2),
            "{policy:?} 竟要删最新一份（索引 2）：{:?}",
            decision.delete
        );
    }
}

#[test]
fn unparsable_time_is_never_pruned() {
    // 🔴 硬约束 2：无法解析时间的快照一律跳过，**宁可不删不可错删**
    let times = vec![Some(1_000), None, Some(3_000)];
    let candidates = cand(&times, 2);
    let decision = plan_retention(&candidates, RetentionPolicy::Count(1));
    assert!(
        !decision.delete.contains(&1),
        "时间不可解析的快照不得被裁剪：{:?}",
        decision.delete
    );
    assert!(
        decision
            .skipped
            .iter()
            .any(|(i, why)| *i == 1 && why.contains("无法解析")),
        "跳过必须带可解释原因：{:?}",
        decision.skipped
    );
}

#[test]
fn count_keeps_the_newest_n() {
    let times = vec![
        Some(1_000),
        Some(2_000),
        Some(3_000),
        Some(4_000),
        Some(5_000),
    ];
    let candidates = cand(&times, 4);
    let decision = plan_retention(&candidates, RetentionPolicy::Count(2));
    // 保留 5000（最新）与 4000；删 1000/2000/3000
    assert_eq!(decision.delete, vec![0, 1, 2], "应删最旧三份");
}

#[test]
fn every_skip_carries_a_reason() {
    // 🔴 规格 §六「跳过必须可见留痕」—— 不允许有无原因的跳过
    let times = vec![Some(1_000), Some(2_000), Some(3_000)];
    let candidates = cand(&times, 2);
    let decision = plan_retention(&candidates, RetentionPolicy::Count(1));
    for (index, why) in &decision.skipped {
        assert!(
            !why.is_empty(),
            "索引 {index} 的跳过原因为空 —— 违反「跳过须可解释」"
        );
    }
}

#[test]
fn delete_list_is_sorted_and_deduped() {
    // 多次调用同一判据不应产生重复索引（下游按索引删目录，重复会二次删除）
    let times = vec![Some(5_000), Some(1_000), Some(3_000), Some(2_000)];
    let candidates = cand(&times, 0);
    let decision = plan_retention(&candidates, RetentionPolicy::Count(1));
    let mut sorted = decision.delete.clone();
    sorted.sort_unstable();
    sorted.dedup();
    assert_eq!(decision.delete, sorted, "删除列表必须已排序去重");
}

#[test]
fn gfs_daily_quota_keeps_recent_days() {
    // 5 天连续快照，daily_keep=3 ⇒ 保留最近 3 天
    let times = vec![
        Some(day_ms(1)),
        Some(day_ms(2)),
        Some(day_ms(3)),
        Some(day_ms(4)),
        Some(day_ms(5)),
    ];
    let candidates = cand(&times, 4);
    let decision = plan_retention(
        &candidates,
        RetentionPolicy::Gfs(GfsRetention {
            daily_keep: 3,
            weekly_keep: 0,
            monthly_keep: 0,
        }),
    );
    // 最旧两天（day 1/2）应被删
    assert!(decision.delete.contains(&0), "day1 应被裁剪");
    assert!(decision.delete.contains(&1), "day2 应被裁剪");
    assert!(!decision.delete.contains(&4), "最新一份不得删");
}

#[test]
fn gfs_all_zero_keeps_only_latest() {
    let times = vec![Some(day_ms(1)), Some(day_ms(2)), Some(day_ms(3))];
    let candidates = cand(&times, 2);
    let decision = plan_retention(&candidates, RetentionPolicy::Gfs(GfsRetention::default()));
    assert_eq!(decision.delete, vec![0, 1], "三项全 0 = 只保最新一份");
}

// ── 日历换算 ────────────────────────────────────────────────────────────

/// 构造某年某月某日 00:00（UTC+8）的 Unix 毫秒。
fn day_ms(day: i64) -> i64 {
    // 2026-10-01 00:00 +08:00 = 2026-09-30 16:00 UTC
    let base = 1_772_313_600_000_i64; // 2026-03-01 00:00 +08 的近似锚点
    base + (day - 1) * 86_400_000
}

#[test]
fn civil_conversion_roundtrips() {
    // 🔴 快照目录名 ↔ Unix 毫秒必须能往返，否则列表排序与裁剪全乱
    for (y, m, d) in [(2026, 10u32, 5u32), (2024, 2u32, 29u32), (2026, 1u32, 1u32)] {
        let ms = unix_ms_of(i64::from(y), m, d);
        let (ry, rm, rd) = civil_from_unix_ms(ms);
        assert_eq!((ry, rm, rd), (y, m, d), "{y}-{m}-{d} 往返失败");
    }
}

/// 本地日期 00:00（UTC+8）→ Unix 毫秒（Hinnant `days_from_civil`）。
fn unix_ms_of(year: i64, month: u32, day: u32) -> i64 {
    let y = if month <= 2 { year - 1 } else { year };
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let m = i64::from(month);
    let doy = (153 * (if m > 2 { m - 3 } else { m + 9 }) + 2) / 5 + i64::from(day) - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    let days = era * 146_097 + doe - 719_468;
    // 本地 00:00 +08 ⇒ UTC 为前一日 16:00
    (days * 86_400 - 8 * 3_600) * 1_000
}
