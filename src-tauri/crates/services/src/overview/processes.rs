//! 实时占用进程榜（M-01 片1 命令面 / F-04 采集侧，2026-10-03）。
//!
//! 复用结论（DP-1）：
//! - 取数走 `sysinfo`（`NexBox` `hardware.rs` 同库；本域只用 `refresh_processes_specifics`）；
//! - 排名口径沿用 `SystemToolkit` 的 `TopProcessSampler` **思路**（③跨栈思路复用，非代码移植）：
//!   差分两轮才有 CPU 值，🔴 拒绝 WMI `PerfProc` 路线（母本实测冷启动 6291ms）。
//!
//! 🔴 三条口径纪律：
//! 1. **归一化**：sysinfo 在 Windows 上把进程 CPU 算成 `100 × 进程增量/系统增量 × 逻辑核数`
//!    （核对 `sysinfo-0.30.13/src/windows/process.rs::compute_cpu_usage`），即"单核满载 ≈ 100%"；
//!    F-04 要的是**占整机百分比**，故展示前除以 `cpus().len()`（R-17，与任务管理器一致）；
//! 2. **无证据不给数**（R-08）：首轮进程还没有计数器上一值（sysinfo 此时写 0，与真空闲不可区分，L-02）、
//!    PID 被复用、逻辑核数未知 → 一律 `None`；
//! 3. **PID 0 不入榜**（R-17：系统空闲伪进程）。

use std::collections::HashSet;
use std::sync::Arc;

use sysinfo::ProcessRefreshKind;

use super::dto::{clamp_percent, mb_value, ProcessRowDto, TopProcessesDto};
use super::state::{OverviewInner, ProcessObservation};

/// 榜单固定行数（R-18：不足由前端补空行，行高不随数据量跳动）。
pub const TOP_ROWS: usize = 5;

/// 榜单类别（同一份候选集，两种排序键）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Board {
    Cpu,
    Memory,
}

/// 排序键：无证据的行沉到底（`-1` 哨兵恒低于任何真实读数）。
#[must_use]
pub fn sort_key(row: &ProcessRowDto, board: Board) -> f64 {
    match board {
        Board::Cpu => row.cpu_percent.unwrap_or(-1.0),
        Board::Memory => row.memory_mb.unwrap_or(-1.0),
    }
}

/// 榜单排序 + 截断 + 重编名次（F-04：键降序，同值按 PID 升序，行序稳定）。
#[must_use]
pub fn sorted_rows(rows: &[ProcessRowDto], board: Board, limit: usize) -> Vec<ProcessRowDto> {
    let mut picked: Vec<ProcessRowDto> = rows.to_vec();
    picked.sort_by(
        |left, right| match sort_key(right, board).total_cmp(&sort_key(left, board)) {
            std::cmp::Ordering::Equal => left.pid.cmp(&right.pid),
            other => other,
        },
    );
    picked.truncate(limit);
    for (index, row) in picked.iter_mut().enumerate() {
        row.rank = index + 1;
    }
    picked
}

/// 进程 CPU 值归一化为占整机百分比并钳制 0–100（R-07/R-17）。
///
/// `logical_cores == 0`（核数还没拿到）时返回 `None`：🔴 此时任何百分比都无法解释，
/// 不得退化成"直接显示原值"（那是单核口径，最高能报到 100×N%）。
#[must_use]
#[allow(clippy::cast_precision_loss)] // 逻辑核数远小于 2^53，转换无损精度
pub fn machine_percent(raw: f64, logical_cores: usize) -> Option<f64> {
    if logical_cores == 0 {
        return None;
    }
    // sysinfo 给的是"占一个核"口径（满载一核 ≈100，N 核满载 ≈100×N），故先除核数、后钳制。
    // 🔴 顺序不能反：先钳到 100 再除会把多核进程压成 1/N（整机 50% 显示成 5%）。
    let value = clamp_percent(raw / logical_cores as f64);
    Some((value * 10.0).round() / 10.0)
}

/// 一轮进程采样的中间结果。
struct Round {
    rows: Vec<ProcessRowDto>,
    has_baseline: bool,
    cores_known: bool,
}

/// 刷新进程表并产出候选行（含差分基线判定）。
fn sample_round(inner: &OverviewInner) -> Round {
    let mut sys = inner.system();
    // 只要 CPU 与工作集：磁盘读数与 exe 路径采集代价高且 F-04 用不到
    sys.refresh_processes_specifics(ProcessRefreshKind::new().with_cpu().with_memory());
    let logical_cores = sys.cpus().len();
    let has_baseline = inner.note_process_refresh() >= 2;

    let mut rows: Vec<ProcessRowDto> = Vec::with_capacity(sys.processes().len());
    let mut live: HashSet<u32> = HashSet::with_capacity(sys.processes().len());
    for (pid, process) in sys.processes() {
        let pid_u32 = pid.as_u32();
        // R-17：系统空闲伪进程不进榜，也不进差分基线
        if pid_u32 == 0 {
            continue;
        }
        live.insert(pid_u32);
        let observation = inner.observe_process(pid_u32, process.start_time());
        let cpu_percent = match (has_baseline, observation) {
            (true, ProcessObservation::Ongoing) => {
                machine_percent(f64::from(process.cpu_usage()), logical_cores)
            }
            _ => None,
        };
        let memory = process.memory();
        rows.push(ProcessRowDto {
            rank: 0,
            name: process.name().to_string(),
            pid: pid_u32,
            cpu_percent,
            // 工作集读不到时是 0（受保护进程查询失败），与真 0 不可区分 → 按无证据处理（R-08）
            memory_mb: if memory > 0 {
                Some(mb_value(memory))
            } else {
                None
            },
        });
    }
    drop(sys);
    // 基线映射只留现役 PID，否则长期运行会无界增长
    inner.prune_process_baseline(|pid| live.contains(&pid));
    Round {
        rows,
        has_baseline,
        cores_known: logical_cores > 0,
    }
}

/// 采集两榜（`overview_top_processes` 载荷）。
///
/// 与 `live_sample` 共用 `OverviewInner::system`：本函数要做一轮进程刷新，
/// 故同一拍里两条命令串行时后一条读到更新后的表（两者都是 2 秒节拍消费方，可接受）。
#[must_use]
pub fn top_processes(inner: &Arc<OverviewInner>) -> TopProcessesDto {
    let Round {
        rows,
        has_baseline,
        cores_known,
    } = sample_round(inner);

    let reason = if rows.is_empty() {
        Some("未获取到进程列表（系统进程枚举无返回）".to_string())
    } else if !has_baseline {
        Some("首次采样中：CPU 占比需两轮差分才能计算，本表暂按 PID 升序占位".to_string())
    } else if !cores_known {
        Some("未取到逻辑处理器数：CPU 占比列暂不可用".to_string())
    } else {
        None
    };

    TopProcessesDto {
        has_baseline,
        cpu: sorted_rows(&rows, Board::Cpu, TOP_ROWS),
        memory: sorted_rows(&rows, Board::Memory, TOP_ROWS),
        reason,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row(pid: u32, name: &str, cpu: Option<f64>, memory_mb: Option<f64>) -> ProcessRowDto {
        ProcessRowDto {
            rank: 0,
            name: name.to_string(),
            pid,
            cpu_percent: cpu,
            memory_mb,
        }
    }

    #[test]
    fn machine_percent_divides_by_logical_cores() {
        // 单核满载（sysinfo 侧 ≈ 100）在 20 线程机上 = 占整机 5%
        assert_eq!(machine_percent(100.0, 20), Some(5.0));
        assert_eq!(machine_percent(1000.0, 20), Some(50.0));
        assert_eq!(machine_percent(45.0, 20), Some(2.3));
    }

    #[test]
    fn machine_percent_refuses_to_guess_without_core_count() {
        // 🔴 核数未知时不得退回单核口径（会报到 100×N%）
        assert_eq!(machine_percent(60.0, 0), None);
    }

    #[test]
    fn machine_percent_stays_inside_zero_hundred() {
        // 计数器回退等异常下 sysinfo 可能给出超大值：R-07 的 0–100 钳制在除法之后生效
        assert_eq!(machine_percent(100.0, 1), Some(100.0));
        assert_eq!(machine_percent(-8.0, 4), Some(0.0));
        assert_eq!(machine_percent(9_000.0, 4), Some(100.0));
    }

    #[test]
    fn rows_without_evidence_sink_to_the_bottom() {
        let rows = vec![
            row(10, "no-baseline", None, Some(50.0)),
            row(20, "light", Some(1.0), Some(10.0)),
            row(30, "heavy", Some(42.0), Some(1.0)),
        ];
        let cpu = sorted_rows(&rows, Board::Cpu, TOP_ROWS);
        assert_eq!(
            cpu.iter().map(|item| item.pid).collect::<Vec<_>>(),
            vec![30, 20, 10],
            "无值行必须排在有值行之后"
        );
        assert_eq!(cpu[0].rank, 1, "名次须在截断后重编");
        // 内存榜同一候选集、另一排序键
        let memory = sorted_rows(&rows, Board::Memory, TOP_ROWS);
        assert_eq!(memory[0].pid, 10, "工作集最大的在前");
    }

    #[test]
    fn equal_values_are_ordered_by_pid_for_stable_layout() {
        let rows = vec![
            row(300, "c", Some(5.0), None),
            row(100, "a", Some(5.0), None),
            row(200, "b", Some(5.0), None),
        ];
        let cpu = sorted_rows(&rows, Board::Cpu, TOP_ROWS);
        assert_eq!(
            cpu.iter().map(|item| item.pid).collect::<Vec<_>>(),
            vec![100, 200, 300],
            "同值按 PID 升序（F-04 行序稳定）"
        );
    }

    #[test]
    fn board_is_truncated_to_five_rows() {
        let rows: Vec<ProcessRowDto> = (0..30)
            .map(|index| row(index, "p", Some(f64::from(index)), None))
            .collect();
        let cpu = sorted_rows(&rows, Board::Cpu, TOP_ROWS);
        assert_eq!(cpu.len(), TOP_ROWS);
        assert_eq!(cpu[0].pid, 29);
        assert_eq!(cpu.last().expect("应有末行").rank, TOP_ROWS);
    }

    #[test]
    fn empty_candidate_set_sorts_to_empty() {
        assert!(sorted_rows(&[], Board::Cpu, TOP_ROWS).is_empty());
        assert_eq!(sort_key(&row(1, "x", None, None), Board::Cpu), -1.0);
    }

    #[test]
    fn exited_pids_are_pruned_from_the_baseline() {
        let inner = OverviewInner::new();
        inner.observe_process(11, 100);
        inner.observe_process(22, 200);
        inner.prune_process_baseline(|pid| pid == 22);
        // 11 被清掉 → 再次见到回到 New 语义；22 保留 → Ongoing
        assert_eq!(inner.observe_process(11, 900), ProcessObservation::New);
        assert_eq!(inner.observe_process(22, 200), ProcessObservation::Ongoing);
    }

    /// 真机冒烟：本机两榜须排得出来，且第二轮起 CPU 列才有值。
    #[cfg(windows)]
    #[test]
    fn top_processes_on_this_machine_rank_within_two_rounds() {
        let inner = OverviewInner::new();
        let first = top_processes(&inner);
        assert!(!first.has_baseline, "首轮不得宣称有基线（R-08）");
        assert!(
            first
                .reason
                .as_deref()
                .unwrap_or_default()
                .contains("首次采样中"),
            "占位榜必须写明原因：{:?}",
            first.reason
        );
        assert!(
            first.cpu.iter().all(|item| item.cpu_percent.is_none()),
            "首拍 CPU 列不得有值"
        );

        // 首拍的排序基准（PID 升序占位）须与实现一致
        let pids: Vec<u32> = first.cpu.iter().map(|item| item.pid).collect();
        let mut sorted = pids.clone();
        sorted.sort_unstable();
        assert_eq!(pids, sorted, "无基线时按 PID 升序占位");

        let second = top_processes(&inner);
        assert!(second.has_baseline, "第二轮起应有差分基线");
        assert!(!second.cpu.is_empty(), "本机应至少排出一个进程");
        assert!(second.cpu.len() <= TOP_ROWS);
        assert!(
            second.reason.is_none(),
            "有基线后不应再挂降级原因：{:?}",
            second.reason
        );
        for item in &second.cpu {
            if let Some(value) = item.cpu_percent {
                assert!(
                    (0.0..=100.0).contains(&value),
                    "占整机比例须落在 0–100，实得 {value}"
                );
            }
        }
        let memory = &second.memory;
        assert!(!memory.is_empty(), "内存榜须有行");
        assert!(
            memory
                .windows(2)
                .all(|pair| sort_key(&pair[0], Board::Memory) >= sort_key(&pair[1], Board::Memory)),
            "内存榜须降序"
        );
    }
}
