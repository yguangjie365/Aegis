//! 接收确认闸门（F-FT-14 / F-FT-18 / Q-15；红线 R-8、R-9）。
//!
//! ## 它解决的问题
//!
//! F-FT-18「接收前询问」**默认开启**（Q-15 用户 2026-10-04 裁决，全链路统一），
//! 但接收路径若不停下来问一句，文件就会**悄悄进来**——这正是母本被诟病的行为。
//!
//! 本模块是"接收线程停下来等用户决定"的那半段：
//!
//! ```text
//! 接收线程：admit() → [需确认] ask() ──等待──> 收到裁决 / 超时
//! 命令层  ：收到 pending 列表 → 用户点"接收"/"拒绝" → resolve()
//! ```
//!
//! ## 🔴 R-8：四条路径全部按**拒绝**处理
//!
//! 原文：*"接收确认弹窗在『拒绝 / 关闭 / Esc / 超时』四条路径上**全部按拒绝**处理"*。
//! 这里的落点：**默认值是拒绝**，且**超时也判拒绝**。
//! 本模块的 `PendingConfirmation::default()` 是 [`Decision::Reject`]，
//! 于是"没人回答"与"用户拒绝"走到同一个终点——**不存在"没人管就默认收下"这条路**。
//!
//! ## 判定与等待分离
//!
//! * [`ConfirmGate::ask`] 登记一条待决并返回它的 id（**纯状态操作**，不阻塞）
//! * [`ConfirmGate::await_decision`] 阻塞等待，**超时即拒**
//!
//! 分成两步是为了让"登记/裁决/超时/取消"这些**状态迁移**能脱离真实时间做单测
//! （本仓 `clippy.toml` 禁用 `std::thread::sleep`，故等待用 `Condvar::wait_timeout`，
//! 且超时值可注入）。

use std::collections::HashMap;
use std::sync::{Condvar, Mutex};
use std::time::Duration;

/// 待决确认的默认等待上限（F-FT-14：确认超时 **30 秒**）。
pub const CONFIRM_TIMEOUT: Duration = Duration::from_secs(30);

/// 用户裁决（R-8：默认与超时都是「拒绝」）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Decision {
    /// 🔴 **默认值就是拒绝**——"没人回答"与"用户拒绝"必须走到同一个终点
    #[default]
    Reject,
    Accept,
}

/// 待决确认的对外快照（命令层拿它渲染弹窗；F-FT-14 要求展示的那些字段）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PendingConfirmation {
    pub id: String,
    /// 对端标识（`ip:port`）
    pub peer_label: String,
    pub file_name: String,
    pub file_size: u64,
    /// 保存目录（未设置时为空串，界面显示"—"）
    pub save_directory: String,
    /// 磁盘剩余（`None` = 无法判定，界面显示"无法判定剩余空间"）
    pub available_bytes: Option<u64>,
    /// 是否已存在同名文件（**仅当存在时才展示冲突区**，F-FT-14 原文）
    pub name_conflict: bool,
}

/// 内部的待决槽。
#[derive(Debug)]
struct Slot {
    snapshot: PendingConfirmation,
    decision: Option<Decision>,
    /// 登记时刻（**单调时钟**）：用来算"还剩多少毫秒"。
    ///
    /// 🔴 用 `Instant` 而不是墙上时钟：用户改系统时间不该让倒计时跳一下或直接到期。
    asked_at: std::time::Instant,
}

/// 接收确认闸门（一个服务实例一个）。
#[derive(Debug)]
pub struct ConfirmGate {
    inner: Mutex<HashMap<String, Slot>>,
    signal: Condvar,
    /// 等待上限（生产用 [`CONFIRM_TIMEOUT`]；测试可缩短）
    timeout: Duration,
}

impl Default for ConfirmGate {
    fn default() -> Self {
        Self::new(CONFIRM_TIMEOUT)
    }
}

impl ConfirmGate {
    #[must_use]
    pub fn new(timeout: Duration) -> Self {
        Self {
            inner: Mutex::new(HashMap::new()),
            signal: Condvar::new(),
            timeout,
        }
    }

    /// 登记一条待决，返回 id。**不阻塞**。
    ///
    /// 同一 id 重复登记会**覆盖**旧的（调用方用 `task_id` 作 id 时天然唯一）。
    pub fn ask(&self, snapshot: PendingConfirmation) -> String {
        let id = snapshot.id.clone();
        let mut map = self.lock();
        map.insert(
            id.clone(),
            Slot {
                snapshot,
                decision: None,
                asked_at: std::time::Instant::now(),
            },
        );
        id
    }

    /// 命令层回答一条待决。返回 `false` 表示**没有这条待决**（可能已超时并清理）。
    ///
    /// 🔴 已回答过的不再被改写：这是"终态不可回退"在确认路径上的对应物
    /// （用户点了接收之后，一个迟到的"拒绝"不该把它翻过来）。
    pub fn resolve(&self, id: &str, decision: Decision) -> bool {
        let mut map = self.lock();
        let Some(slot) = map.get_mut(id) else {
            return false;
        };
        if slot.decision.is_some() {
            return false;
        }
        slot.decision = Some(decision);
        self.signal.notify_all();
        true
    }

    /// 阻塞等待裁决；**超时即拒**（R-8），并在返回前清理该条待决。
    ///
    /// 被 `resolve` 唤醒 → 返回用户的选择；超时 → [`Decision::Reject`]。
    pub fn await_decision(&self, id: &str) -> Decision {
        let mut map = self.lock();
        let deadline = std::time::Instant::now() + self.timeout;
        loop {
            let Some(slot) = map.get(id) else {
                // 已被清理（不该发生）——保守判拒
                return Decision::Reject;
            };
            if let Some(decision) = slot.decision {
                map.remove(id);
                return decision;
            }
            let now = std::time::Instant::now();
            if now >= deadline {
                // 🔴 超时按拒绝（R-8 四条路径之一），并清理待决
                map.remove(id);
                return Decision::Reject;
            }
            let (guard, timeout_result) = self
                .signal
                .wait_timeout(map, deadline.saturating_duration_since(now))
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            map = guard;
            // 超时与否都回到循环顶部：那里统一走"超时即拒"分支（一处逻辑，不写两遍）
            let _ = timeout_result;
        }
    }

    /// 当前待决快照（命令层轮询给界面渲染弹窗）。
    #[must_use]
    pub fn pending(&self) -> Vec<PendingConfirmation> {
        self.pending_with_ttl()
            .into_iter()
            .map(|(pending, _)| pending)
            .collect()
    }

    /// 待决快照 + **距超时判拒还剩多少毫秒**（R-8：超时按拒绝，故界面必须有这个倒计时）。
    ///
    /// 🔴 剩余时间是**算出来的**，不是拿 `CONFIRM_TIMEOUT` 当常量回给界面——
    /// 后者会让倒计时永远从 30 秒开始，用户看到的是一个**不会动的假倒计时**。
    #[must_use]
    pub fn pending_with_ttl(&self) -> Vec<(PendingConfirmation, i64)> {
        let now = std::time::Instant::now();
        let mut list: Vec<(PendingConfirmation, i64)> = self
            .lock()
            .values()
            .filter(|slot| slot.decision.is_none())
            .map(|slot| {
                let remaining = self
                    .timeout
                    .saturating_sub(now.saturating_duration_since(slot.asked_at));
                let millis = i64::try_from(remaining.as_millis()).unwrap_or(i64::MAX);
                (slot.snapshot.clone(), millis)
            })
            .collect();
        // 稳定顺序：界面不该因为 HashMap 顺序抖动而重排序
        list.sort_by(|a, b| a.0.id.cmp(&b.0.id));
        list
    }

    /// 取消全部待决（服务停止时用）：全部按拒绝，且**唤醒**所有等待者。
    ///
    /// 🔴 不唤醒的话，停止服务会让接收线程卡到超时（最长 30 秒），
    /// `stop()` 的 join 于是看起来"停不下来"。
    pub fn cancel_all(&self) {
        let mut map = self.lock();
        for slot in map.values_mut() {
            if slot.decision.is_none() {
                slot.decision = Some(Decision::Reject);
            }
        }
        drop(map);
        self.signal.notify_all();
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, HashMap<String, Slot>> {
        // 毒锁恢复：一次 panic 不该让整个确认机制永久失效
        self.inner
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }
}

#[cfg(test)]
mod tests;
