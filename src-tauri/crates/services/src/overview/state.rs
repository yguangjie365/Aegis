//! 概览域运行态（M-01 片1，2026-10-03）。
//!
//! 🔴 由 `src-tauri` 壳 `app.manage()` 托管，是本域唯一的可变全局点：
//! sidecar 桥、传感器缓存、sysinfo 实例、慢变量缓存都挂在这里，随应用生命周期释放
//! （母本 `NexBox` 用 `lazy_static` 全局，已在 `sensor_bridge.rs` 头注登记为有意偏离）。
//!
//! 节拍与非阻塞的分工：
//! - `sensors` 只存**上一次成功读到的快照**，`live_sample` 只读它，绝不等待；
//! - `read_in_flight` 是 R-13 的重入闸门：同一时刻至多一个采集在途；
//! - `bridge` 由后台读线程独占，`system` 由采样调用共用（毫秒级，允许短暂排队）。

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};

use sysinfo::System;

use super::dto::DiskHealthRowDto;
use super::sensor_bridge::{SensorBridge, SensorSnapshot};

/// 一次全量/慢变量采集确定的存储汇总（R-12：不随 2 秒节拍变化）。
#[derive(Debug, Clone, Default, PartialEq)]
pub struct StorageSummary {
    /// R-06 按介质聚合的副标题，如 "SSD 476.9 GB + HDD 931.5 GB"
    pub media_text: String,
    /// 已用百分比（多卷取最大值，沿用 `SystemToolkit` 2026-09-02 起 `DriveInfo` 单一来源口径）
    pub used_percent: f64,
    pub total_bytes: u64,
    pub free_bytes: u64,
    /// 容量最大卷的盘符（如 "C:"），片2 换为物理盘型号形态（R-06 回退支）
    pub largest_volume: String,
    /// 卷数（多分区同盘时的提示，不参与展示数值）
    pub volume_count: usize,
    pub unavailable_reason: Option<String>,
}

/// 会话内不变的系统身份项（页头条与卡片型号用，首次采样后缓存）。
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SystemIdentity {
    pub computer_name: String,
    pub os_name: String,
    pub cpu_brand: String,
    pub physical_cores: Option<usize>,
    pub logical_cores: usize,
}

/// 域内部状态（`OverviewState` 经 `Arc` 托管，内部再持 `Arc<OverviewInner>` 供后台线程用）。
pub struct OverviewInner {
    pub bridge: Mutex<SensorBridge>,
    pub sensors: Mutex<Option<SensorSnapshot>>,
    pub read_in_flight: AtomicBool,
    pub system: Mutex<System>,
    /// CPU 占用刷新次数：sysinfo 在 Windows 上首轮拿不到 PDH 值（0 填充与真空闲不可区分，L-02），
    /// 故首拍必须判为"无基线"而不是 0%（R-08）。
    pub cpu_refreshes: AtomicU32,
    /// PID → 上次见到的进程启动时刻；变了说明 PID 被复用，🔴 该行的 CPU 宁可不给（R-17）。
    pub process_baseline: Mutex<HashMap<u32, u64>>,
    /// 进程 CPU 差分轮次（首拍无上一值 → 榜单显示"首次采样中…"，R-08）。
    pub process_refreshes: AtomicU32,
    pub storage: Mutex<Option<StorageSummary>>,
    /// F-06 磁盘健康面板数据（慢变量，只在首次/全量采集时更新）
    pub disk_health: Mutex<Vec<DiskHealthRowDto>>,
    pub identity: Mutex<Option<SystemIdentity>>,
}

/// 一个进程样本在本拍的观测结论（F-04 的"能不能给 CPU 数字"判定，R-17）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProcessObservation {
    /// 本轮首次纳管：sysinfo 没有上一值，差分为 0 填充（不可信）
    New,
    /// 同一进程再次见到：差分可用
    Ongoing,
    /// PID 换了主人：数字属上一个进程（不可信）
    Reused,
}

impl OverviewInner {
    #[must_use]
    pub fn new() -> Arc<Self> {
        Arc::new(Self {
            bridge: Mutex::new(SensorBridge::new()),
            sensors: Mutex::new(None),
            read_in_flight: AtomicBool::new(false),
            system: Mutex::new(System::new()),
            cpu_refreshes: AtomicU32::new(0),
            process_baseline: Mutex::new(HashMap::new()),
            process_refreshes: AtomicU32::new(0),
            storage: Mutex::new(None),
            disk_health: Mutex::new(Vec::new()),
            identity: Mutex::new(None),
        })
    }

    /// 取锁并在中毒时恢复。
    ///
    /// 策略说明：锁中毒只可能由本域代码 panic 造成，属缺陷而非运行期条件；
    /// 让一次 panic 把整个概览永久锁死（R-10 反例）比恢复更糟，故一律取回内部值继续。
    fn guard<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
        match mutex.lock() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        }
    }

    pub fn bridge(&self) -> MutexGuard<'_, SensorBridge> {
        Self::guard(&self.bridge)
    }

    #[must_use]
    pub fn sensors(&self) -> Option<SensorSnapshot> {
        Self::guard(&self.sensors).clone()
    }

    pub fn store_sensors(&self, snapshot: SensorSnapshot) {
        *Self::guard(&self.sensors) = Some(snapshot);
    }

    pub fn system(&self) -> MutexGuard<'_, System> {
        Self::guard(&self.system)
    }

    /// R-13 重入闸门：抢到才允许起后台读线程。
    #[must_use]
    pub fn try_begin_read(&self) -> bool {
        !self.read_in_flight.swap(true, Ordering::AcqRel)
    }

    pub fn end_read(&self) {
        self.read_in_flight.store(false, Ordering::Release);
    }

    #[must_use]
    pub fn read_in_flight(&self) -> bool {
        self.read_in_flight.load(Ordering::Acquire)
    }

    #[must_use]
    pub fn cpu_refresh_count(&self) -> u32 {
        self.cpu_refreshes.load(Ordering::Acquire)
    }

    pub fn note_cpu_refresh(&self) -> u32 {
        self.cpu_refreshes.fetch_add(1, Ordering::AcqRel) + 1
    }

    /// 记录并返回该 PID 上一次的启动时刻；`true` = 本次换了进程（PID 复用）。
    pub fn pid_reused(&self, pid: u32, start_time: u64) -> bool {
        self.observe_process(pid, start_time) == ProcessObservation::Reused
    }

    /// 观测一个进程样本：首次见到 / 同一进程再次见到 / PID 已被复用。
    ///
    /// `New` 与 `Reused` 两种情况下本拍 `Process::cpu_usage()` 都不可信
    /// （新纳管者没有计数器上一值，复用者属两个不同进程），🔴 一律不给数字（R-17）。
    pub fn observe_process(&self, pid: u32, start_time: u64) -> ProcessObservation {
        let mut baseline = Self::guard(&self.process_baseline);
        match baseline.insert(pid, start_time) {
            None => ProcessObservation::New,
            Some(previous) if previous == start_time => ProcessObservation::Ongoing,
            Some(_) => ProcessObservation::Reused,
        }
    }

    /// 清掉已消失进程的基线条目（每轮榜单调用，防长期运行下映射无界增长）。
    pub fn prune_process_baseline(&self, is_live: impl Fn(u32) -> bool) {
        Self::guard(&self.process_baseline).retain(|pid, _| is_live(*pid));
    }

    /// 进程 CPU 差分轮次：首轮所有进程都没有上一值，故 ≥2 轮才有基线（R-08）。
    pub fn note_process_refresh(&self) -> u32 {
        self.process_refreshes.fetch_add(1, Ordering::AcqRel) + 1
    }

    #[must_use]
    pub fn has_process_baseline(&self) -> bool {
        self.process_refreshes.load(Ordering::Acquire) >= 2
    }

    #[must_use]
    pub fn storage(&self) -> Option<StorageSummary> {
        Self::guard(&self.storage).clone()
    }

    pub fn store_storage(&self, summary: StorageSummary) {
        *Self::guard(&self.storage) = Some(summary);
    }

    #[must_use]
    pub fn identity(&self) -> Option<SystemIdentity> {
        Self::guard(&self.identity).clone()
    }

    pub fn store_identity(&self, identity: SystemIdentity) {
        *Self::guard(&self.identity) = Some(identity);
    }

    /// F-06 磁盘健康面板数据（慢变量，只在首次/全量采集时更新）
    #[must_use]
    pub fn disk_health(&self) -> Vec<DiskHealthRowDto> {
        Self::guard(&self.disk_health).clone()
    }

    pub fn store_disk_health(&self, rows: Vec<DiskHealthRowDto>) {
        *Self::guard(&self.disk_health) = rows;
    }
}

/// Tauri 托管入口（`app.manage(OverviewState::new())`）。
#[derive(Clone)]
pub struct OverviewState {
    inner: Arc<OverviewInner>,
}

impl OverviewState {
    #[must_use]
    pub fn new() -> Self {
        Self {
            inner: OverviewInner::new(),
        }
    }

    /// 取域内部状态的 `Arc`（后台线程与命令共用）。
    ///
    /// 🔴 名字刻意不叫 `inner()`：`tauri::State` 自带同名 inherent 方法且返回 `&T`，
    /// 经 `State<'_, OverviewState>` 调用时 inherent 优先，会静默拿到 `&OverviewState`。
    #[must_use]
    pub fn shared(&self) -> Arc<OverviewInner> {
        Arc::clone(&self.inner)
    }
}

impl Default for OverviewState {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reentry_gate_allows_exactly_one_reader() {
        let inner = OverviewInner::new();
        assert!(!inner.read_in_flight());
        assert!(inner.try_begin_read(), "首次应抢到闸门");
        assert!(!inner.try_begin_read(), "在途时第二次必须抢不到（R-13）");
        assert!(inner.read_in_flight());
        inner.end_read();
        assert!(inner.try_begin_read(), "释放后应可再次进入");
    }

    #[test]
    fn cpu_baseline_requires_two_refreshes() {
        let inner = OverviewInner::new();
        assert_eq!(inner.cpu_refresh_count(), 0);
        assert_eq!(inner.note_cpu_refresh(), 1, "首拍计数应为 1（无基线）");
        assert_eq!(inner.note_cpu_refresh(), 2);
        assert_eq!(inner.cpu_refresh_count(), 2);
    }

    #[test]
    fn pid_reuse_is_detected_from_start_time_change() {
        let inner = OverviewInner::new();
        assert!(!inner.pid_reused(4242, 1_000), "首次见到不应判为复用");
        assert!(
            !inner.pid_reused(4242, 1_000),
            "同一进程再次采样不应判为复用"
        );
        assert!(
            inner.pid_reused(4242, 2_000),
            "启动时刻变化即 PID 复用（R-17）"
        );
    }

    #[test]
    fn process_observation_distinguishes_new_ongoing_reused() {
        let inner = OverviewInner::new();
        assert_eq!(
            inner.observe_process(7, 100),
            ProcessObservation::New,
            "首轮没有上一值：本拍差分不可信"
        );
        assert_eq!(inner.observe_process(7, 100), ProcessObservation::Ongoing);
        assert_eq!(
            inner.observe_process(7, 500),
            ProcessObservation::Reused,
            "换主人即不可信（R-17 宁缺毋滥）"
        );
        assert_eq!(inner.observe_process(7, 500), ProcessObservation::Ongoing);
    }

    #[test]
    fn process_baseline_needs_two_rounds_and_prunes_exited_pids() {
        let inner = OverviewInner::new();
        assert!(!inner.has_process_baseline(), "零轮采样不得宣称有基线");
        assert_eq!(inner.note_process_refresh(), 1);
        assert!(
            !inner.has_process_baseline(),
            "首轮全部进程无上一值（R-08）"
        );
        assert_eq!(inner.note_process_refresh(), 2);
        assert!(inner.has_process_baseline(), "第二轮起差分可用");

        inner.observe_process(1, 10);
        inner.observe_process(2, 20);
        inner.prune_process_baseline(|pid| pid == 2);
        assert_eq!(
            inner.observe_process(1, 999),
            ProcessObservation::New,
            "被清理的 PID 应回到首轮语义，不得误判为复用"
        );
        assert_eq!(inner.observe_process(2, 20), ProcessObservation::Ongoing);
    }

    #[test]
    fn empty_caches_read_as_none_not_zero() {
        let inner = OverviewInner::new();
        assert!(inner.sensors().is_none());
        assert!(inner.storage().is_none());
        assert!(inner.identity().is_none());
    }

    #[test]
    fn stored_snapshot_is_readable_from_another_thread() {
        let inner = OverviewInner::new();
        let shared = Arc::clone(&inner);
        let handle = std::thread::spawn(move || {
            shared.store_storage(StorageSummary {
                media_text: "SSD 476.9 GB".to_string(),
                used_percent: 78.0,
                total_bytes: 512,
                free_bytes: 112,
                largest_volume: "C:".to_string(),
                volume_count: 1,
                unavailable_reason: None,
            });
        });
        handle.join().expect("写入线程不应 panic");
        let summary = inner.storage().expect("慢变量应可跨线程读回");
        assert_eq!(summary.media_text, "SSD 476.9 GB");
        assert_eq!(summary.largest_volume, "C:");
    }

    #[test]
    fn state_is_send_and_sync_for_tauri_managed() {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<OverviewState>();
        // 托管对象必须能跨 Tauri 工作线程使用，否则整域无法 manage
        assert_send_sync::<OverviewInner>();
    }
}
