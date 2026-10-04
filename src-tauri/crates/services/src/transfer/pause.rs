//! 一次传输的**暂停闸门**（F-FT-26 / F-FT-27 / R-14 / R-15）。
//!
//! ## 它解决什么
//!
//! "暂停"在两端的含义不是"停止这个线程"，而是：**到块边界就停下来等**，并且
//! **已经收/发的那一块不许被撕开**（R-14："暂停只在块边界生效以不打断正在写入的块"）。
//! 因此闸门只在**每块之间**被查询一次，块内绝不查询——这正是 R-14 要的语义。
//!
//! ## 三种状态与两种来源
//!
//! * `Run`：正常。
//! * `LocalPaused`：**本端**按下的暂停。
//! * `PeerPaused`：**对端**发来 Pause。
//!
//! 🔴 界面必须区分后两者（F-FT-25："已暂停"与"已暂停（对端）"），因为
//! **"谁按的"决定了"谁能继续"**——对端按的暂停，本端点"继续"是不该生效的。
//!
//! ## F-FT-27：本地暂停有明确终点
//!
//! [`PauseGate::wait_if_paused`] 带超时上限：本地暂停超过上限就**自己走人**，
//! 回一个 [`PauseExit::TimedOut`]，由调用方把任务收成 `PAUSE_TIMEOUT`
//! （🔴 不用 `USER_CANCEL` —— 用户要知道是"放太久自动放弃了"，而不是"我按了取消"）。
//! 上限设为 0 表示不限（规格原文）。
//!
//! ## 为什么不直接 `thread::park`
//!
//! 因为**取消也必须能唤醒它**：用户按取消时任务可能正卡在暂停里，
//! 若闸门只认"恢复"信号，取消就会永远等下去。故这里是
//! `Condvar::wait_timeout` + 取消标志轮询的组合。

use std::sync::{Condvar, Mutex};
use std::time::Duration;

use tokio_util::sync::CancellationToken;

/// 闸门状态（见模块头"三种状态与两种来源"）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PauseState {
    Run,
    LocalPaused,
    PeerPaused,
}

/// 从暂停中醒来的原因。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PauseExit {
    /// 已恢复，继续跑
    Resumed,
    /// 本地暂停超上限 ⇒ 调用方收成 `PAUSE_TIMEOUT`
    TimedOut,
    /// 取消（用户取消或连接断开）⇒ 调用方收成取消
    Cancelled,
}

/// 暂停闸门（每任务一个）。
#[derive(Debug)]
pub struct PauseGate {
    inner: Mutex<PauseState>,
    signal: Condvar,
    /// 🔴 是否因**超上限**醒来的。
    ///
    /// 必须有这份记忆：超时后调用方要**取消**该传输，而取消与"用户按取消"走同一条
    /// 回收路径 ⇒ 若不记下来，终态会变成 `USER_CANCEL` ✗，而 F-FT-27 明写要固定为
    /// `PAUSE_TIMEOUT`——"让用户知道是放太久自动放弃了，而不是我按了取消"。
    timed_out: std::sync::atomic::AtomicBool,
    /// 累计被暂停的毫秒数（F-FT-25 / R-14：暂停时长**不计入速度**）。
    ///
    /// 🔴 每进入一次等待就累加那一趟的实际时长 ⇒ 多次暂停正确累加；
    ///    恢复后重新计时，正对应"恢复时重置速度基线"。
    paused_ms: std::sync::atomic::AtomicU64,
}

impl Default for PauseGate {
    fn default() -> Self {
        Self::new()
    }
}

/// 轮询取消的间隔：够小（用户按取消后最多这么久就有反应），又不必空转。
/// 🔴 它不是"暂停多久"，而是"多久看一眼有没有人取消"。
const CANCEL_POLL: Duration = Duration::from_millis(120);

impl PauseGate {
    #[must_use]
    pub fn new() -> Self {
        Self {
            inner: Mutex::new(PauseState::Run),
            signal: Condvar::new(),
            timed_out: std::sync::atomic::AtomicBool::new(false),
            paused_ms: std::sync::atomic::AtomicU64::new(0),
        }
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, PauseState> {
        self.inner
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    /// 把一趟等待的时长记进累计暂停时长（毫秒）。
    fn add_paused(&self, waited: Duration) {
        let ms = u64::try_from(waited.as_millis()).unwrap_or(u64::MAX);
        self.paused_ms
            .fetch_add(ms, std::sync::atomic::Ordering::SeqCst);
    }

    /// 该任务**累计被暂停的毫秒数**（F-FT-25："暂停时长不计入速度"）。
    #[must_use]
    pub fn paused_ms(&self) -> u64 {
        self.paused_ms.load(std::sync::atomic::Ordering::SeqCst)
    }

    /// 本次暂停是否**因超上限**结束（F-FT-27：终态据此判 `PAUSE_TIMEOUT`）。
    #[must_use]
    pub fn timed_out(&self) -> bool {
        self.timed_out.load(std::sync::atomic::Ordering::SeqCst)
    }

    #[must_use]
    pub fn state(&self) -> PauseState {
        *self.lock()
    }

    /// 本端按下暂停。返回 `false` = **未被受理**（已经处于暂停），
    /// 调用方必须据此回一句"未被受理：任务当前状态为 …"（R-14：不许点了没反应）。
    pub fn pause_local(&self) -> bool {
        let mut state = self.lock();
        if *state != PauseState::Run {
            return false;
        }
        *state = PauseState::LocalPaused;
        true
    }

    /// 对端发来暂停（幂等：重复的 Pause 不当成"未受理"，因为对端可能重发）。
    pub fn pause_peer(&self) {
        let mut state = self.lock();
        // 本端已暂停时，保持 LocalPaused：**谁先按的，谁的状态优先**，
        // 这样界面不会因为对端一句 Pause 就把"我按的暂停"改写成"对端按的"。
        if *state == PauseState::Run {
            *state = PauseState::PeerPaused;
        }
        self.signal.notify_all();
    }

    /// 恢复。返回 `false` = 未被受理（本来就没暂停）。
    ///
    /// 🔴 只解除 [`PauseState::LocalPaused`]：**对端按下的暂停，本端点"继续"不该生效**
    /// （对端才是暂停的发起方，它恢复时会再发 Resume）。
    pub fn resume_local(&self) -> bool {
        let mut state = self.lock();
        if *state != PauseState::LocalPaused {
            return false;
        }
        *state = PauseState::Run;
        self.signal.notify_all();
        true
    }

    /// 对端发来恢复。
    ///
    /// 🔴 不会解除 `LocalPaused`：本端按的暂停只能由本端解除，
    /// 否则"我暂停了，对端一句 Resume 就把我的暂停撤了"。
    pub fn resume_peer(&self) {
        let mut state = self.lock();
        if *state == PauseState::PeerPaused {
            *state = PauseState::Run;
        }
        self.signal.notify_all();
    }

    /// 🔴 **在块边界调用**：处于暂停时阻塞，直到恢复 / 超时 / 取消。
    ///
    /// `limit = None` 表示不限（设置里 `pause_timeout_minutes = 0`，F-FT-27 原文）。
    /// 计时**只统计暂停时长**（每次进入等待重新起算），故恢复后再暂停会重新计。
    pub fn wait_if_paused(&self, limit: Option<Duration>, cancel: &CancellationToken) -> PauseExit {
        // 取消优先：已经取消了就别再问暂停的事
        if cancel.is_cancelled() {
            return PauseExit::Cancelled;
        }
        let mut waited = Duration::ZERO;
        loop {
            {
                let state = self.lock();
                if *state == PauseState::Run {
                    // 恢复（或本就没暂停）：把这一趟实际等待记进暂停总时长
                    self.add_paused(waited);
                    return PauseExit::Resumed;
                }
            }
            if cancel.is_cancelled() {
                self.add_paused(waited);
                return PauseExit::Cancelled;
            }
            if let Some(cap) = limit {
                if waited >= cap {
                    // 🔴 记下来：调用方据此把终态收成 PAUSE_TIMEOUT 而不是 USER_CANCEL
                    self.timed_out
                        .store(true, std::sync::atomic::Ordering::SeqCst);
                    self.add_paused(waited);
                    return PauseExit::TimedOut;
                }
            }
            // 每次最多睡 CANCEL_POLL，醒来再看一眼状态与取消
            let slice = match limit {
                Some(cap) => CANCEL_POLL.min(cap.saturating_sub(waited)),
                None => CANCEL_POLL,
            };
            let guard = self.lock();
            let (guard, _timeout) = self
                .signal
                .wait_timeout(guard, slice)
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            drop(guard);
            waited += slice;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn starts_running() {
        let gate = PauseGate::new();
        assert_eq!(gate.state(), PauseState::Run);
    }

    #[test]
    fn local_pause_is_not_accepted_twice() {
        let gate = PauseGate::new();
        assert!(gate.pause_local(), "第一次应受理");
        assert_eq!(gate.state(), PauseState::LocalPaused);
        assert!(
            !gate.pause_local(),
            "已暂停 ⇒ **未被受理**（R-14 要求如实回报）"
        );
        assert!(!gate.resume_local() || gate.state() == PauseState::Run);
    }

    #[test]
    fn peer_resume_must_not_cancel_my_own_pause() {
        let gate = PauseGate::new();
        assert!(gate.pause_local());
        gate.resume_peer();
        assert_eq!(
            gate.state(),
            PauseState::LocalPaused,
            "本端按的暂停只能由本端解除"
        );
    }

    #[test]
    fn local_resume_must_not_cancel_a_peer_pause() {
        let gate = PauseGate::new();
        gate.pause_peer();
        assert!(!gate.resume_local(), "对端按的暂停，本端点继续不生效");
        assert_eq!(gate.state(), PauseState::PeerPaused);
        gate.resume_peer();
        assert_eq!(gate.state(), PauseState::Run);
    }

    #[test]
    fn a_peer_pause_does_not_overwrite_my_local_pause() {
        let gate = PauseGate::new();
        assert!(gate.pause_local());
        gate.pause_peer();
        assert_eq!(
            gate.state(),
            PauseState::LocalPaused,
            "谁先按的，状态就显示谁的——否则界面会把'我按的'说成'对端按的'"
        );
    }

    #[test]
    fn wait_returns_immediately_when_running() {
        let gate = PauseGate::new();
        let cancel = CancellationToken::new();
        assert_eq!(
            gate.wait_if_paused(Some(Duration::from_secs(1)), &cancel),
            PauseExit::Resumed
        );
    }

    #[test]
    fn wait_times_out_at_the_limit() {
        let gate = PauseGate::new();
        let cancel = CancellationToken::new();
        assert!(gate.pause_local());
        let exit = gate.wait_if_paused(Some(Duration::from_millis(300)), &cancel);
        assert_eq!(
            exit,
            PauseExit::TimedOut,
            "本地暂停超上限 ⇒ TimedOut，调用方据此收成 PAUSE_TIMEOUT（F-FT-27）"
        );
    }

    #[test]
    fn wait_wakes_up_on_resume() {
        let gate = std::sync::Arc::new(PauseGate::new());
        let cancel = CancellationToken::new();
        assert!(gate.pause_local());
        let waker = std::sync::Arc::clone(&gate);
        // 🔴 不睡：顺序无关紧要——先恢复则下面的等待立刻返回，先等待则被这次恢复叫醒。
        //    （而且本仓 `clippy.toml` 明令禁用 `std::thread::sleep`。）
        let handle = std::thread::spawn(move || {
            assert!(waker.resume_local());
        });
        let exit = gate.wait_if_paused(Some(Duration::from_secs(5)), &cancel);
        assert_eq!(
            exit,
            PauseExit::Resumed,
            "恢复必须把它叫醒（而不是干等到超时）"
        );
        handle.join().unwrap();
    }

    #[test]
    fn wait_is_woken_by_cancellation() {
        let gate = std::sync::Arc::new(PauseGate::new());
        let cancel = CancellationToken::new();
        assert!(gate.pause_local());
        let gate2 = std::sync::Arc::clone(&gate);
        let cancel2 = cancel.clone();
        // 🔴 不睡：上限设为"不限"（None），故**任何**返回都只可能来自取消
        //    ⇒ 无论取消发生在等待前还是等待中，断言都成立。
        let handle = std::thread::spawn(move || {
            cancel2.cancel();
            let _ = &gate2;
        });
        let exit = gate.wait_if_paused(None, &cancel);
        assert_eq!(exit, PauseExit::Cancelled, "取消必须能唤醒暂停中的等待");
        handle.join().unwrap();
    }

    #[test]
    fn unlimited_gate_only_exits_by_resume_or_cancel() {
        let gate = PauseGate::new();
        let cancel = CancellationToken::new();
        assert!(gate.pause_local());
        cancel.cancel();
        assert_eq!(
            gate.wait_if_paused(None, &cancel),
            PauseExit::Cancelled,
            "limit=None（设置 0 = 不限）时唯一出路是恢复或取消"
        );
    }

    #[test]
    fn resume_before_wait_means_no_wait() {
        let gate = PauseGate::new();
        let cancel = CancellationToken::new();
        assert!(gate.pause_local());
        assert!(gate.resume_local());
        assert_eq!(
            gate.wait_if_paused(Some(Duration::from_secs(5)), &cancel),
            PauseExit::Resumed
        );
    }
    /// 🔴 F-FT-27 的关键一环：超时**必须留下痕迹**，否则终态会被写成 `USER_CANCEL`，
    /// 用户会以为"是我按了取消"，而事实是"放太久自动放弃了"。
    #[test]
    fn timing_out_is_remembered_so_the_terminal_state_can_say_pause_timeout() {
        let gate = PauseGate::new();
        let cancel = CancellationToken::new();
        assert!(!gate.timed_out(), "还没暂停过，不该有超时痕迹");
        assert!(gate.pause_local());
        assert_eq!(
            gate.wait_if_paused(Some(Duration::from_millis(200)), &cancel),
            PauseExit::TimedOut
        );
        assert!(
            gate.timed_out(),
            "超时后必须记得：调用方据此收成 PAUSE_TIMEOUT 而不是 USER_CANCEL"
        );
    }

    #[test]
    fn a_normal_resume_leaves_no_timeout_mark() {
        let gate = PauseGate::new();
        let cancel = CancellationToken::new();
        assert!(gate.pause_local());
        assert!(gate.resume_local());
        assert_eq!(
            gate.wait_if_paused(Some(Duration::from_secs(5)), &cancel),
            PauseExit::Resumed
        );
        assert!(!gate.timed_out(), "正常恢复不该留超时痕迹");
    }
}
