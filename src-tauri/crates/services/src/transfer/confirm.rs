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

use super::conflict::ConflictPolicy;
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
    /// 🔴 到期没人回答（F-FT-16：**必须与"用户拒绝"区分**）。
    ///
    /// 两者在**本端**的处理相同（都按拒绝），但对端收到的原因码不同：
    /// 用户拒绝 ⇒ `USER_REJECT`；到期 ⇒ `CONFIRM_TIMEOUT`（"对方没响应"，不是"对方不要"）。
    /// 不区分的话，发送方会把"对方没来得及点"误读成"对方明确拒绝了"。
    TimedOut,
}

/// 待决确认的对外快照（命令层拿它渲染弹窗；F-FT-14 要求展示的那些字段）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PendingConfirmation {
    pub id: String,
    /// 对端标识（`ip:port`）
    pub peer_label: String,
    /// 🔴 **这次要确认的是什么**。
    ///
    /// 界面据此只显示**与该类型有关**的行：文件要问"存到哪、空间够不够"，
    /// 文本要问"要不要覆盖你当前的剪贴板"——把文件的磁盘信息摆给文本看，
    /// 是用无关信息干扰一个**不可逆**的决定（写入剪贴板会覆盖既有内容）。
    ///
    /// 🔴 用**显式字段**而不是让界面从"目录为空且磁盘未知"去猜：
    /// 猜测会在某天因为某个合法取值而悄悄失效，而这里错不起。
    pub kind: PendingKind,
    pub file_name: String,
    pub file_size: u64,
    /// 保存目录（未设置时为空串，界面显示"—"；**文本传输不适用**）
    pub save_directory: String,
    /// 磁盘剩余（`None` = 无法判定；**文本传输不适用**）
    pub available_bytes: Option<u64>,
    /// 是否已存在同名文件（**仅当存在时才展示冲突区**，F-FT-14 原文）
    pub name_conflict: bool,
}

/// 待决确认的种类（见 [`PendingConfirmation::kind`]）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PendingKind {
    /// 文件：要落盘，故问目录与磁盘
    File,
    /// 文本：写入剪贴板，与磁盘无关
    Text,
}

/// 内部的待决槽。
#[derive(Debug)]
struct Slot {
    snapshot: PendingConfirmation,
    decision: Option<Decision>,
    /// 用户在本次弹窗里选的冲突策略（只作用于本次传输，不落设置）
    policy: Option<ConflictPolicy>,
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
                policy: None,
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
        self.resolve_with_policy(id, decision, None)
    }

    /// 同 [`Self::resolve`]，但可一并带上**本次传输**的冲突策略（「同名文件」三个选项）。
    ///
    /// 🔴 只作用于本次接收，**不写设置**：用户在这次弹窗里选"覆盖"，
    /// 不该把他下次的默认行为也一并改掉。
    pub fn resolve_with_policy(
        &self,
        id: &str,
        decision: Decision,
        policy: Option<ConflictPolicy>,
    ) -> bool {
        let mut map = self.lock();
        let Some(slot) = map.get_mut(id) else {
            return false;
        };
        if slot.decision.is_some() {
            return false;
        }
        slot.decision = Some(decision);
        slot.policy = policy;
        self.signal.notify_all();
        true
    }

    /// 阻塞等待裁决；**超时即拒**（R-8），并在返回前清理该条待决。
    ///
    /// 被 `resolve` 唤醒 → 返回用户的选择与（可选的）本次冲突策略；
    /// 超时/无此条 → [`Decision::Reject`] 且无策略。
    pub fn await_decision(&self, id: &str) -> (Decision, Option<ConflictPolicy>) {
        let mut map = self.lock();
        let deadline = std::time::Instant::now() + self.timeout;
        loop {
            let Some(slot) = map.get(id) else {
                // 已被清理（不该发生）——保守判拒
                return (Decision::Reject, None);
            };
            if let Some(decision) = slot.decision {
                let policy = slot.policy;
                map.remove(id);
                return (decision, policy);
            }
            let now = std::time::Instant::now();
            if now >= deadline {
                // 🔴 超时按拒绝（R-8 四条路径之一），并清理待决。
                //    返回独立变体：本端同样按拒绝，但**对端要用 CONFIRM_TIMEOUT** 而不是 USER_REJECT。
                map.remove(id);
                return (Decision::TimedOut, None);
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
