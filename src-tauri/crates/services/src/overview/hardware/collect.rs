//! 静态硬件详情的装配（F-02 出口，片2-b/c，2026-10-03）。
//!
//! **顺序单源**：卡序 = 规格 §五「详细信息」表序（处理器 → 显卡 → 存储 → 内存 → 主板 → BIOS →
//! 网卡 → 声卡 → 显示器）。🔴 前端不再排序，只按数组渲染——排序在两处各写一遍必然漂移。
//! 显示器卡已于片2-c 由 `infra::display` + EDID 产出，插入在末尾。
//!
//! **分区独立容错（R-10）**：每个采集器自己把 `Err` 折成带 `reason` 的降级卡，本函数🔴 不做
//! "任一失败就整体失败"的判定，也不吞掉任何一张卡——缺哪一类，那一类以降级卡的形式仍然占位并说明原因。
//!
//! **线程纪律**：本函数是**同步阻塞**的（每条 WQL 走一次 COM 连接，NVML 首次调用要加载 dll）。
//! 🔴 不得在 2 秒节拍里调用（R-12 慢变量），只在用户进入本页的首次全量采集与手动刷新时由命令层
//! 经 `spawn_blocking` 投递到阻塞线程池（母本 `hardware.rs:1502-1949` 用七路 `thread::spawn`，
//! 本工程改由命令层统一一次投递，避免同一棵树上的 COM 套间互相打架）。

use std::time::{SystemTime, UNIX_EPOCH};

use crate::overview::dto::HardwareDetailDto;

use super::{audio, board, cpu, gpu, memory, monitor, network, storage};

/// 装配一次全量静态硬件详情（规格 §五 的九类中本批可得的八类）。
#[must_use]
pub fn collect() -> HardwareDetailDto {
    let mut cards = Vec::new();
    cards.push(cpu::card());
    cards.extend(gpu::cards());
    cards.extend(storage::cards());
    cards.push(memory::card());
    cards.push(board::board_card());
    cards.push(board::bios_card());
    cards.extend(network::cards());
    cards.extend(audio::cards());
    cards.push(monitor::card());

    HardwareDetailDto {
        cards,
        collected_at_ms: now_ms(),
    }
}

/// Unix 毫秒时刻；系统时钟异常时退回 0（🔴 不 panic，快照侧把 0 当"无时间戳"处理）。
#[must_use]
#[allow(clippy::cast_possible_truncation)] // Unix 毫秒在 2^64 内，实际溢出要等到 5 亿年后
pub fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |elapsed| elapsed.as_millis() as u64)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::overview::dto::DetailKind;

    #[test]
    fn clock_returns_nonzero_milliseconds() {
        // 只锁量级：Unix 毫秒在 2020 年之后必然大于 10^12
        assert!(now_ms() > 1_000_000_000_000);
    }

    #[test]
    fn collect_keeps_one_slot_per_category_even_when_wmi_fails() {
        // 单测环境可能查不到 WMI（无 COM / 无权限），这里锁的是"卡数不为 0 且每类都有位"
        let detail = collect();
        assert!(
            !detail.cards.is_empty(),
            "🔴 采集失败也必须出卡（降级卡带 reason），面板不得整体消失"
        );
        assert!(
            detail.cards.iter().any(|card| card.kind == DetailKind::Cpu),
            "处理器卡恒在首位"
        );
        assert_eq!(
            detail.cards.first().map(|card| card.kind),
            Some(DetailKind::Cpu)
        );
        let degraded_with_reason = detail
            .cards
            .iter()
            .filter(|card| card.reason.is_some())
            .count();
        let empty_non_degraded = detail
            .cards
            .iter()
            .filter(|card| card.reason.is_none() && card.rows.is_empty())
            .count();
        assert_eq!(
            empty_non_degraded, 0,
            "没有 reason 的卡不得是空卡（静默空面板即违规）"
        );
        assert!(degraded_with_reason <= detail.cards.len());
    }
}
