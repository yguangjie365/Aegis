//! 保留策略裁剪（GFS；F-FB-31 / **R-5**）。
//!
//! 复用结论（DP-1）：③ 思路复用 ← `Core/Backup/Services/SnapshotManager.cs:376-428`
//! （`ShouldDelete` + `BuildGfsBuckets`）。判据逐条对照，**三条硬约束原样继承**：
//! 1. 🔴 **最新一份快照永不被删除**；
//! 2. 🔴 **无法从名称解析出时间的快照一律跳过不裁剪**（宁可不删，不可错删）；
//! 3. 🔴 **一旦出现删除失败即停止本轮后续删除**（避免连锁误删）。
//!
//! 本模块是**纯函数**：只判「哪些该删」，不碰文件系统（IO 在 `manifest.rs`）——
//! 这样判据能被独立单测，且「判据」与「执行」不会两处漂移（规格 §六一致性条款）。

use super::models::{GfsRetention, RetentionPolicy};

/// 单份快照参与裁剪判断所需的**最小信息**（只两样，不把整个 `SnapshotInfo` 拖进来）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RetentionCandidate {
    /// 快照时间（Unix 毫秒）；无法解析时为 `None`。
    pub created_unix_ms: Option<i64>,
    /// 是否为最新一份（由调用方按时间比较后标记；🔴 它是"永不删"的执行者）。
    pub is_newest: bool,
}

/// 一次裁剪的判定结果。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RetentionDecision {
    /// 应删除的索引（对应入参顺序）。
    pub delete: Vec<usize>,
    /// 跳过的索引及原因（🔴 跳过必须可解释，规格 §六「跳过…必须可见留痕」）。
    pub skipped: Vec<(usize, &'static str)>,
}

/// GFS 判据：按「日/周/月」三个桶各保最近 N 份。
///
/// 时间桶的划分沿用母本口径：**按本地日期**归日桶，周起始为周一，月按自然月。
/// 🔴 `latest_kept` 是三桶共同保底的最新一份，与 [`plan_retention`] 的"最新永不删"同源。
#[must_use]
pub fn build_gfs_buckets(candidates: &[RetentionCandidate], gfs: GfsRetention) -> Vec<usize> {
    let gfs = gfs.clamped();
    let quotas = [gfs.daily_keep, gfs.weekly_keep, gfs.monthly_keep];

    // 时间倒序（新→旧），使"保留最近 N 份"就是"前 N 个"
    let mut keyed: Vec<(usize, i64, i64, i64)> = Vec::new();
    for (index, candidate) in candidates.iter().enumerate() {
        let Some(ms) = candidate.created_unix_ms else {
            continue;
        };
        let (year, month, day) = civil_from_unix_ms(ms);
        let day_key = i64::from(year) * 10_000 + i64::from(month) * 100 + i64::from(day);
        // 粗粒度周：以「年内日序」除 7 为键。🔴 刻意不做ISO 周历 —— 周的起点一旦随
        // 跨年漂移，同一份快照会在不同年份落到不同的"最近 4 周"里，破坏可预期性。
        let day_of_year = i64::from(month) * 31 + i64::from(day);
        let week_key = i64::from(year) * 100 + day_of_year / 7;
        let month_key = i64::from(year) * 100 + i64::from(month);
        keyed.push((index, day_key, week_key, month_key));
    }
    keyed.sort_by_key(|k| std::cmp::Reverse(k.1));

    let mut daily: Vec<i64> = Vec::new();
    let mut weekly: Vec<i64> = Vec::new();
    let mut monthly: Vec<i64> = Vec::new();
    let mut kept: Vec<usize> = Vec::new();

    for (index, day_key, week_key, month_key) in keyed {
        // 依次尝试三个桶，取**第一个未满**的放进去 —— 配额大的桶优先
        // 🔴 用 `u32::try_from(..).unwrap_or(u32::MAX)` 而非 `as u32`：clippy pedantic 的
        //   cast_possible_truncation 就是为这类「值域已知但类型系统不知道」的转换准备的；
        //   且截断成 0 会让「超长列表」被误判为「桶空了」—— 那正是本函数不该有的行为。
        let daily_len = u32::try_from(daily.len()).unwrap_or(u32::MAX);
        let weekly_len = u32::try_from(weekly.len()).unwrap_or(u32::MAX);
        let monthly_len = u32::try_from(monthly.len()).unwrap_or(u32::MAX);
        if daily_len < quotas[0] && !daily.contains(&day_key) {
            daily.push(day_key);
            kept.push(index);
        } else if weekly_len < quotas[1] && !weekly.contains(&week_key) {
            weekly.push(week_key);
            kept.push(index);
        } else if monthly_len < quotas[2] && !monthly.contains(&month_key) {
            monthly.push(month_key);
            kept.push(index);
        }
    }

    // 三项全 0 ⇒ 语义是"只保最新一份"（R-5）
    if gfs.is_keep_latest_only() {
        kept.truncate(1);
    }
    kept
}

/// 统一的裁剪判据入口（🔴 预演与执行共用同一函数，规格 §六「同一判据只实现一次」）。
///
/// 返回值里`skipped` 的每一条都带原因，调用方须逐条落日志。
#[must_use]
pub fn plan_retention(
    candidates: &[RetentionCandidate],
    policy: RetentionPolicy,
) -> RetentionDecision {
    let mut delete = Vec::new();
    let mut skipped = Vec::new();

    // 🔴 硬约束 1：最新一份永不删，先标记
    for (index, candidate) in candidates.iter().enumerate() {
        if candidate.is_newest {
            skipped.push((index, "最新一份快照永不被删除"));
        }
    }
    let protected = |i: usize| candidates[i].is_newest;

    match policy.clamped() {
        RetentionPolicy::Count(keep) => {
            // 🔴 硬约束 2 在此分支同样成立：先把**不可解析**的显式记为跳过。
            //    （第一版只遍历 `ordered`，那些 `None` 索引压根没进循环 ⇒
            //    既不在 delete 也不在 skipped ⇒ 被静默漏掉，违反「跳过必须可见留痕」。
            //    由 `unparsable_time_is_never_pruned` 抓到。）
            for (index, candidate) in candidates.iter().enumerate() {
                if candidate.created_unix_ms.is_none() && !protected(index) {
                    skipped.push((index, "快照名无法解析出时间，跳过裁剪"));
                }
            }
            // 固定条数：保留时间最新的 keep 份
            let mut ordered: Vec<usize> = (0..candidates.len())
                .filter(|i| candidates[*i].created_unix_ms.is_some())
                .collect();
            ordered.sort_by(|a, b| {
                candidates[*b]
                    .created_unix_ms
                    .cmp(&candidates[*a].created_unix_ms)
            });
            for (rank, index) in ordered.iter().enumerate() {
                if protected(*index) {
                    continue;
                }
                // 🔴 排名与 `keep`（u32）比较：用 `u32::try_from` 而非 `as`，
                //   避免「排在 2^32 之后」被截断成小数后误判为"在保留范围内"。
                if u32::try_from(rank).unwrap_or(u32::MAX) < keep {
                    skipped.push((*index, "在保留条数范围内"));
                } else {
                    delete.push(*index);
                }
            }
        }
        RetentionPolicy::Gfs(gfs) => {
            let keep_set = build_gfs_buckets(candidates, gfs);
            for (index, candidate) in candidates.iter().enumerate() {
                if protected(index) {
                    continue;
                }
                // 🔴 硬约束 2：无法解析时间的快照一律跳过，不裁剪
                if candidate.created_unix_ms.is_none() {
                    skipped.push((index, "快照名无法解析出时间，跳过裁剪"));
                    continue;
                }
                if keep_set.contains(&index) {
                    skipped.push((index, "命中时间深度配额"));
                } else {
                    delete.push(index);
                }
            }
        }
    }

    delete.sort_unstable();
    delete.dedup();
    RetentionDecision { delete, skipped }
}

/// Unix 毫秒 → `(年, 月, 日)`（本地时区，UTC+8 固定偏移）。
///
/// 🔴 母本用 `DateTimeOffset.FromUnixTimeMilliseconds(...).ToLocalTime()`，本工程取
/// **固定 UTC+8偏移**而非读系统时区：备份快照按日历日归档，用机器当前时区会让同一份快照
/// 在不同时区的机器上落到不同的日桶，破坏GFS 的确定性。代价是本机时区非UTC+8 时
/// 日界线落在 08:00/16:00 —— 已在布局稿与变更记录登记为已知取舍。
#[must_use]
pub fn civil_from_unix_ms(unix_ms: i64) -> (i32, u32, u32) {
    const TZ_OFFSET_MS: i64 = 8 * 3_600 * 1_000;
    let local_ms = unix_ms.saturating_add(TZ_OFFSET_MS);
    let secs = local_ms.div_euclid(1000);
    let days = secs.div_euclid(86_400);
    let (year, month, day) = civil_from_days(days);
    (year, month, day)
}

/// Howard Hinnant 的 `civil_from_days` 算法（days since 1970-01-01 → 年月日）。
#[must_use]
pub fn civil_from_days(days: i64) -> (i32, u32, u32) {
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    // 🔴 用 `try_from().unwrap_or(饱和值)` 而非 `as`：clippy pedantic 的 cast_sign_loss /
    //   cast_possible_truncation 就是为这类「值域已知但类型系统不知道」的转换准备的。
    //   🔴 饱和值取 `i32::MAX` / `u32::MAX` 而非 0：日历字段非法时应表现为「极大」，
    //   由 `parse_snapshot_time` 的范围校验拒掉；若饱和成 0 会被误读成 1970-01-01。
    (
        i32::try_from(if m <= 2 { y + 1 } else { y }).unwrap_or(i32::MAX),
        u32::try_from(m).unwrap_or(u32::MAX),
        u32::try_from(d).unwrap_or(u32::MAX),
    )
}

#[cfg(test)]
mod tests;
