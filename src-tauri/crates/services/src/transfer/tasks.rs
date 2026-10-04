//! 传输任务台账（F-FT-25 / F-FT-29 / F-FT-30；红线 R-16）。
//!
//! ## 这个模块存在的理由
//!
//! 界面要显示"哪些文件正在传、传到哪、最后成没成"，而这些事实**分散在多个线程里产生**：
//! 发送侧一个线程、接收侧每连接一个线程、取消/断开可能来自任何一方。
//! 若各处自行往界面推状态，就会出现 R-16 明令禁止的那件事：
//!
//! > **R-16 终态不可回退**：一旦进入完成/跳过/失败/取消，**迟到的消息不允许翻转结论**；
//! > 以对端回传的原因码为准。
//!
//! 具体会长成什么样：一个"已完成"的任务被 100ms 后才到的旧进度帧改回"进行中"，
//! 界面于是显示"正在传输"而文件其实已经落盘——用户看到的是**不可能的状态**。
//! 故把"谁是终态、谁能改它"收进本模块一处：
//! [`TaskRegistry::advance`] / [`TaskRegistry::finish`] 对终态任务**一律拒绝并返回 `false`**。
//!
//! ## 进度节流
//!
//! 分块 2 MiB 的传输每秒可能产生几十次进度更新，逐次推给界面既浪费又抖动。
//! `01-编码与命名规范.md` §3.1 的口径是"进度经事件上报，频率受限（**建议 ≥100ms 合并一次**）"，
//! [`ProgressThrottle`] 就是那条纪律的落点（**纯函数式**，时刻由调用方传入，可单测）。
//!
//! ## 本模块不做的事
//!
//! * **不写日志**：与 `transport` 一致，只产出可区分的结果；八字段日志由调用方落。
//! * **不发事件**：事件发射是命令层的职责（本 crate 不引用 Tauri 宿主类型做业务耦合）。
//! * **不持久化**：这是**运行期**台账；历史（200 条、可导出）是另一件事（F-FT-54/58）。

use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::collections::VecDeque;

/// 运行期台账的容量上限（超限丢最旧的**终态**任务；进行中的永不丢）。
///
/// 与历史（F-FT-54 的 200 条）分开取值：台账是"当前会话的事"，
/// 200 条足够覆盖一次大目录拖拽，又不至于让界面渲染太久。
pub const TASK_CAPACITY: usize = 200;

/// 任务方向。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Direction {
    Send,
    Receive,
}

/// 任务状态（F-FT-30 的五态）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TaskState {
    /// 进行中（唯一**非终态**）
    Active,
    Completed,
    Skipped,
    Failed,
    Cancelled,
}

impl TaskState {
    /// 是否终态。🔴 R-16：终态一旦写入，任何后续消息都不得改写它。
    #[must_use]
    pub fn is_terminal(self) -> bool {
        !matches!(self, Self::Active)
    }

    /// 界面用的稳定标识（camelCase 之外取小写单词，便于 TS 侧直接分支）。
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Active => "active",
            Self::Completed => "completed",
            Self::Skipped => "skipped",
            Self::Failed => "failed",
            Self::Cancelled => "cancelled",
        }
    }
}

/// 一条传输任务（**快照**；`TaskRegistry` 内部持有的就是它）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TransferTask {
    pub id: String,
    pub direction: Direction,
    /// 对端标识（`ip:port`；界面直接显示）
    pub peer_label: String,
    pub file_name: String,
    pub file_size: u64,
    pub transferred: u64,
    pub state: TaskState,
    /// 终态的原因码（取 `reasons` 的 16 码冻结表；进行中为 `None`）
    pub reason_code: Option<String>,
    pub started_at_ms: i64,
    pub finished_at_ms: Option<i64>,
}

impl TransferTask {
    /// 进度百分比（0..=100）。文件大小为 0 时返回 100（空文件传完即完成）。
    #[must_use]
    pub fn percent(&self) -> u32 {
        if self.file_size == 0 {
            return 100;
        }
        let raw = self.transferred.saturating_mul(100) / self.file_size;
        u32::try_from(raw.min(100)).unwrap_or(100)
    }
}

/// 运行期任务台账。
#[derive(Debug)]
pub struct TaskRegistry {
    tasks: VecDeque<TransferTask>,
    /// id → 在 `tasks` 中的位置**不**缓存（会随淘汰漂移），改存引用计数用途的活跃集合
    active: HashSet<String>,
    capacity: usize,
}

impl Default for TaskRegistry {
    fn default() -> Self {
        Self::new(TASK_CAPACITY)
    }
}

impl TaskRegistry {
    #[must_use]
    pub fn new(capacity: usize) -> Self {
        Self {
            tasks: VecDeque::new(),
            active: HashSet::new(),
            capacity: capacity.max(1),
        }
    }

    /// 建一条任务并返回它的 id。
    ///
    /// 🔴 调用方**必须**最终用 [`Self::finish`] 收尾；只 `begin` 不收尾会让
    /// [`Self::active_count`] 永久偏高（并发闸门据此放行，等于槽位泄漏）。
    pub fn begin(
        &mut self,
        id: impl Into<String>,
        direction: Direction,
        peer_label: impl Into<String>,
        file_name: impl Into<String>,
        file_size: u64,
        now_ms: i64,
    ) -> String {
        let id = id.into();
        self.tasks.push_back(TransferTask {
            id: id.clone(),
            direction,
            peer_label: peer_label.into(),
            file_name: file_name.into(),
            file_size,
            transferred: 0,
            state: TaskState::Active,
            reason_code: None,
            started_at_ms: now_ms,
            finished_at_ms: None,
        });
        self.active.insert(id.clone());
        self.evict_if_needed();
        id
    }

    /// 推进进度。返回 `false` 表示**被忽略**（任务不存在，或已是终态）。
    ///
    /// 🔴 R-16 的落点：终态任务的进度一律丢弃。乱序到达的旧进度帧因此无法把
    /// "已完成"改回"进行中"。
    pub fn advance(&mut self, id: &str, transferred: u64) -> bool {
        let Some(task) = self.tasks.iter_mut().find(|task| task.id == id) else {
            return false;
        };
        if task.state.is_terminal() {
            return false;
        }
        // 进度只增不减：同一条任务的旧帧（数值更小）同样不该让界面倒退
        task.transferred = transferred.min(task.file_size).max(task.transferred);
        true
    }

    /// 写入终态。返回 `false` 表示**被忽略**（任务不存在，或**已经是终态**）。
    ///
    /// 🔴 R-16 的核心：这是唯一能写终态的入口，且它拒绝覆盖已有终态。
    /// 于是"取消后对端才回执完成"这类迟到消息不会把结论翻转。
    pub fn finish(
        &mut self,
        id: &str,
        state: TaskState,
        reason_code: Option<&str>,
        now_ms: i64,
    ) -> bool {
        if !state.is_terminal() {
            // 用 `finish` 写非终态是调用方的用法错误；拒绝而不是静默接受
            return false;
        }
        let Some(task) = self.tasks.iter_mut().find(|task| task.id == id) else {
            return false;
        };
        if task.state.is_terminal() {
            return false;
        }
        task.state = state;
        task.reason_code = reason_code.map(str::to_owned);
        task.finished_at_ms = Some(now_ms);
        if state == TaskState::Completed {
            // 完成即"收满"：避免末帧进度因节流被丢而显示 99%
            task.transferred = task.file_size;
        }
        self.active.remove(id);
        true
    }

    /// 进行中的任务数（并发闸门用）。
    #[must_use]
    pub fn active_count(&self) -> usize {
        self.active.len()
    }

    /// 全部任务的快照（按开始时间序；界面直接渲染）。
    #[must_use]
    pub fn snapshot(&self) -> Vec<TransferTask> {
        self.tasks.iter().cloned().collect()
    }

    /// 取一条（诊断/测试用）。
    #[must_use]
    pub fn get(&self, id: &str) -> Option<&TransferTask> {
        self.tasks.iter().find(|task| task.id == id)
    }

    /// 超容量时淘汰**最旧的终态任务**；若全是进行中则不淘汰（宁可超一点也不能丢活跃任务）。
    fn evict_if_needed(&mut self) {
        while self.tasks.len() > self.capacity {
            let Some(position) = self.tasks.iter().position(|task| task.state.is_terminal()) else {
                return; // 全是进行中：不淘汰
            };
            self.tasks.remove(position);
        }
    }
}

/// 进度上报节流（`01-编码与命名规范.md` §3.1：建议 ≥100ms 合并一次）。
///
/// 🔴 **纯逻辑**：时刻由调用方传入（`now_ms`），故可用单测穷举边界，不必真的等时间流逝
/// （本仓 `clippy.toml` 亦禁用 `std::thread::sleep`）。
#[derive(Debug, Clone, Copy)]
pub struct ProgressThrottle {
    interval_ms: i64,
    last_reported_ms: i64,
}

/// 默认合并间隔（规范建议的 100ms）。
pub const PROGRESS_INTERVAL_MS: i64 = 100;

impl Default for ProgressThrottle {
    fn default() -> Self {
        Self::new(PROGRESS_INTERVAL_MS)
    }
}

impl ProgressThrottle {
    #[must_use]
    pub fn new(interval_ms: i64) -> Self {
        Self {
            // 间隔非正时退化为"每次都报"，而不是"永远不报"——
            // 后者会让界面**永远不动**，且不报错，是最难查的一种
            interval_ms: interval_ms.max(1),
            last_reported_ms: i64::MIN,
        }
    }

    /// 现在该不该报一次？该报则**顺带记下时刻**（所以是 `&mut self`）。
    pub fn should_report(&mut self, now_ms: i64) -> bool {
        let elapsed = now_ms.saturating_sub(self.last_reported_ms);
        if elapsed >= self.interval_ms {
            self.last_reported_ms = now_ms;
            return true;
        }
        false
    }

    /// 强制下一次必报（终态收尾时用：**最后一帧不能被节流吞掉**）。
    pub fn force_next(&mut self) {
        self.last_reported_ms = i64::MIN;
    }
}

#[cfg(test)]
mod tests;
