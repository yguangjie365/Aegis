//! sidecar 桥状态机（起停/超时/连败停用）：`sensor_bridge` 的子模块。

use super::exe::{find_monitor_exe, FaultKind};
use super::matching::aggregate;
use super::types::{SensorReading, SensorSnapshot, SensorsResponse};
use infra::sidecar::{Sidecar, SidecarError};
use std::path::PathBuf;
use std::time::{Duration, Instant};

/// sidecar 内部 `Computer.Open()` 的上限（`monitor/Program.cs` 同值，改动须两侧同步）。
pub const OPEN_BUDGET: Duration = Duration::from_secs(15);
/// 稳态单次读超时。
pub const READ_TIMEOUT: Duration = Duration::from_secs(8);
/// 首次读的预算：sidecar 在 Open 结束前不会回第一行，故须覆盖 15s + 8s。
pub const FIRST_READ_BUDGET: Duration =
    Duration::from_secs(OPEN_BUDGET.as_secs() + READ_TIMEOUT.as_secs());
/// 一次失败后的重试冷却。
pub const COOLDOWN: Duration = Duration::from_secs(30);
/// 连续失败达此数即本次会话不再重试。
pub const MAX_CONSECUTIVE_FAILURES: u32 = 5;
/// 优雅退出预算（供上层在应用退出时调用）。
pub const SHUTDOWN_GRACE: Duration = Duration::from_secs(2);

const READ_COMMAND: &str = r#"{"cmd":"read"}"#;
pub(super) const EXE_NAME: &str = "AegisMonitor.exe";
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BridgeFault {
    pub kind: FaultKind,
    pub reason: String,
}

impl BridgeFault {
    #[must_use]
    pub fn new(kind: FaultKind, reason: impl Into<String>) -> Self {
        Self {
            kind,
            reason: reason.into(),
        }
    }
}

/// 把传输层错误翻译成本域故障（超时预算按实际使用的预算写进原因，🔴 不写死 8s）。
#[must_use]
pub(super) fn classify_sidecar_error(error: &SidecarError, budget: Duration) -> BridgeFault {
    match error {
        SidecarError::Spawn(reason) => BridgeFault::new(FaultKind::Spawn, reason.clone()),
        SidecarError::Write(reason) => {
            BridgeFault::new(FaultKind::Died, format!("命令管道已失效: {reason}"))
        }
        SidecarError::Timeout => BridgeFault::new(
            FaultKind::Timeout,
            format!("传感器响应超时（预算 {} 秒）", budget.as_secs()),
        ),
        SidecarError::Closed => {
            BridgeFault::new(FaultKind::Died, "子进程输出管道已关闭（进程已退出）")
        }
    }
}

/// sidecar 生命周期状态机（由 `OverviewState` 独占；节拍只读缓存，不驱动它）。
pub struct SensorBridge {
    sidecar: Option<Sidecar>,
    pub(super) exe_path: Option<PathBuf>,
    /// 区分"还没查过"与"查过但没有"，避免每拍重复扫盘
    pub(super) exe_lookup_done: bool,
    pub(super) failures: u32,
    pub(super) cooldown_until: Option<Instant>,
    disabled_reason: Option<String>,
    last_fault: Option<BridgeFault>,
    pub(super) ready_once: bool,
    /// 最近一次成功读取的原始逐条读数（F-11 报告的"全量传感器表"用；聚合会把逐条信息折叠掉）
    last_raw: Option<Vec<SensorReading>>,
}

impl Default for SensorBridge {
    fn default() -> Self {
        Self::new()
    }
}

impl SensorBridge {
    #[must_use]
    pub fn new() -> Self {
        Self {
            sidecar: None,
            exe_path: None,
            exe_lookup_done: false,
            failures: 0,
            cooldown_until: None,
            disabled_reason: None,
            last_fault: None,
            ready_once: false,
            last_raw: None,
        }
    }

    #[must_use]
    pub fn is_disabled(&self) -> bool {
        self.disabled_reason.is_some()
    }

    /// 当前是否处于冷却（供节拍打"跳过"标记，与 R-13 同源判定）。
    #[must_use]
    pub fn cooling_down(&self) -> bool {
        self.cooldown_until
            .is_some_and(|until| Instant::now() < until)
    }

    /// 是否已经成功读到过（决定徽章先显示"接入中"还是直接进"未检测"）。
    #[must_use]
    pub fn has_reads(&self) -> bool {
        self.ready_once
    }

    #[must_use]
    pub fn last_fault(&self) -> Option<&BridgeFault> {
        self.last_fault.as_ref()
    }

    #[must_use]
    pub fn sidecar_pid(&self) -> Option<u32> {
        self.sidecar.as_ref().map(Sidecar::pid)
    }

    /// 最近一次成功读取的逐条原始读数（聚合快照把它折叠了，F-11 报告需要全量表）。
    #[must_use]
    pub fn last_raw(&self) -> Option<&[SensorReading]> {
        self.last_raw.as_deref()
    }

    /// 测试专用：把桥置为"已读到过"，免去真起子进程（`live.rs` 的状态映射用例）。
    #[cfg(test)]
    pub(crate) fn mark_ready_for_tests(&mut self) {
        self.ready_once = true;
        self.failures = 0;
        self.cooldown_until = None;
        self.last_fault = None;
    }

    /// 测试专用：置为"扫过、确认 exe 不在"，并不进入冷却（否则状态映射会读成 `CoolingDown`）。
    #[cfg(test)]
    pub(crate) fn force_exe_missing_for_tests(&mut self) {
        self.exe_lookup_done = true;
        self.exe_path = None;
        self.last_fault = Some(BridgeFault::new(
            FaultKind::ExeMissing,
            format!("未找到 {EXE_NAME}（传感器采集组件未随包提供，或所在路径不可读）"),
        ));
    }

    /// 在调用线程上做一次限时读取。**不得放在节拍线程上调用**（首次预算 23 秒）：
    /// `live.rs` 用独立线程 + 缓存把 2 秒节拍与非阻塞隔开。
    ///
    /// # Errors
    /// 任何一环失败都返回带原因的 [`BridgeFault`]，并在内部推进冷却/停用状态（N-06 留痕）。
    pub fn read_once(&mut self) -> Result<SensorSnapshot, BridgeFault> {
        if let Some(reason) = self.disabled_reason.clone() {
            let fault = BridgeFault::new(FaultKind::Disabled, reason);
            self.last_fault = Some(fault.clone());
            return Err(fault);
        }
        if self.cooling_down() {
            let fault = BridgeFault::new(FaultKind::Cooling, "传感器通道冷却中，稍后自动重试");
            self.last_fault = Some(fault.clone());
            return Err(fault);
        }

        let first_attempt = self.sidecar.is_none();
        if let Err(fault) = self.ensure_started() {
            return Err(self.record_failure(fault));
        }

        let budget = if first_attempt {
            FIRST_READ_BUDGET
        } else {
            READ_TIMEOUT
        };
        let request = match self.sidecar.as_mut() {
            Some(sidecar) => sidecar.request(READ_COMMAND, budget),
            None => Err(SidecarError::Spawn(
                "子进程句柄丢失（内部状态不一致）".to_string(),
            )),
        };
        let line = match request {
            Ok(line) => line,
            Err(error) => {
                self.stop_sidecar();
                return Err(self.record_failure(classify_sidecar_error(&error, budget)));
            }
        };

        let response: SensorsResponse = match serde_json::from_str(&line) {
            Ok(response) => response,
            Err(error) => {
                // 协议不认（含 sidecar 自身报错回包 {"error":"…"}）：留原文片段供诊断
                let snippet: String = line.chars().take(160).collect();
                self.stop_sidecar();
                return Err(self.record_failure(BridgeFault::new(
                    FaultKind::Protocol,
                    format!("传感器回包解析失败: {error}（原文片段：{snippet}）"),
                )));
            }
        };

        self.failures = 0;
        self.cooldown_until = None;
        self.ready_once = true;
        self.last_raw = Some(response.sensors.clone());
        Ok(aggregate(&response))
    }

    fn ensure_started(&mut self) -> Result<(), BridgeFault> {
        if let Some(sidecar) = self.sidecar.as_mut() {
            if sidecar.is_alive() {
                return Ok(());
            }
            self.sidecar = None;
        }
        if !self.exe_lookup_done {
            self.exe_path = find_monitor_exe();
            self.exe_lookup_done = true;
        }
        let Some(exe) = self.exe_path.clone() else {
            return Err(BridgeFault::new(
                FaultKind::ExeMissing,
                format!("未找到 {EXE_NAME}（传感器采集组件未随包提供，或所在路径不可读）"),
            ));
        };
        let sidecar = Sidecar::spawn(&exe, |line| {
            // sidecar 的诊断文本必须留痕（N-06）；W0-10 日志总线就绪前先落 stderr。
            eprintln!("[monitor] {line}");
        })
        .map_err(|error| classify_sidecar_error(&error, READ_TIMEOUT))?;
        self.sidecar = Some(sidecar);
        Ok(())
    }

    pub(super) fn record_failure(&mut self, fault: BridgeFault) -> BridgeFault {
        self.last_fault = Some(fault.clone());
        self.failures += 1;
        if self.failures >= MAX_CONSECUTIVE_FAILURES {
            let reason = format!(
                "传感器通道连续失败 {} 次，本次会话已停用（最后一次：{}）",
                self.failures, fault.reason
            );
            eprintln!("[monitor] {reason}");
            self.disabled_reason = Some(reason.clone());
            self.cooldown_until = None;
            return BridgeFault::new(FaultKind::Disabled, reason);
        }
        self.cooldown_until = Some(Instant::now() + COOLDOWN);
        eprintln!(
            "[monitor] 传感器采集失败（第 {} 次），{} 秒后重试：{}",
            self.failures,
            COOLDOWN.as_secs(),
            fault.reason
        );
        fault
    }

    fn stop_sidecar(&mut self) {
        if let Some(mut sidecar) = self.sidecar.take() {
            sidecar.kill();
        }
    }
}

impl Drop for SensorBridge {
    fn drop(&mut self) {
        self.stop_sidecar();
    }
}
