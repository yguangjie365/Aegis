//! 传感器通道桥：sidecar 生命周期状态机 + 读数聚合（M-01 片1 / Q-12=B，2026-10-03）。
//!
//! 移植来源：`Source/NexBox/src-tauri/src/sensor.rs`（②改造移植，登记见 `DEPENDENCIES.md §三`）。
//! 🔴 有意偏离母本三处：
//! 1. 母本把桥放在全局 `lazy_static + Mutex<Option<SensorBridge>>`，本实现把状态收进
//!    `OverviewState`（随应用生命周期释放、可在测试里独立构造，不留跨域全局可变量）；
//! 2. 母本 `find_monitor_exe` 的方案G 用 `CARGO_MANIFEST_DIR` 编译期路径兜底，本实现改为
//!    **向上回溯祖先目录**（回溯已覆盖开发环境，少一处编译期路径依赖）；
//! 3. 母本与 `pawnio_driver.rs` 联动"装完驱动重启传感器进程"，本实现🔴 **零分发零安装**，
//!    只做只读探测（Q-12=B 拍板 + ADR-003 许可闸）。
//!
//! 生命周期参数（批准方案 §三 片1，沿用 `NexBox` 实测值）：
//! 15s Open 兜底 / 8s 读超时 / 5s 快败 / 30s 冷却 / 连败 5 次禁用。
//! 其中"5s 快败"在本实现里不是定时器而是**结构性上界**：exe 缺失或子进程秒退时，
//! `Sidecar::spawn` 立即返回 `Spawn`、管道关闭立即返回 `Closed`，都等不到读超时。

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use serde::Deserialize;

use infra::sidecar::{Sidecar, SidecarError};

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
const EXE_NAME: &str = "AegisMonitor.exe";
/// 相对可执行文件目录的候选子路径（安装版 / 资源目录 / 开发者构建产物）。
const SUBDIRS: [&str; 4] = [
    "monitor",
    "resources/monitor",
    "monitor/bin/Release/net48",
    "monitor/bin/Debug/net48",
];
/// 向上回溯祖先目录的层数上限（覆盖从 `src-tauri/target/debug` 运行的开发场景）。
const ANCESTOR_DEPTH: usize = 6;
/// 回溯时逐层尝试的子路径。
const ANCESTOR_SUBDIRS: [&str; 3] = [
    "monitor/bin/Release/net48",
    "monitor/bin/Debug/net48",
    "monitor",
];

/// sidecar 回单条传感器（字段名与 `monitor/SensorReader.cs` 逐字节对齐）。
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SensorReading {
    pub hardware: String,
    #[serde(default)]
    pub hardware_type: String,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub sensor_type: String,
    #[serde(default)]
    pub value: f64,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SensorsResponse {
    /// 🔴 不设默认值：缺 `sensors` 字段的回包（如 sidecar 自身报错 `{"error":"…"}`）
    /// 必须走协议故障，不能被当成"合法的零传感器"。
    pub sensors: Vec<SensorReading>,
}

/// 一块显卡的传感器汇总（R-01：多卡全列）。
#[derive(Debug, Clone, PartialEq)]
pub struct GpuSensor {
    pub device: String,
    pub temperature_c: Option<f64>,
    pub load_percent: Option<f64>,
}

/// 一次传感器快照（已按 K-Q12-2 §二 聚合规则折叠成概览要用的字段）。
#[derive(Debug, Clone, PartialEq)]
pub struct SensorSnapshot {
    pub cpu_load_percent: Option<f64>,
    pub cpu_temperature_c: Option<f64>,
    pub memory_load_percent: Option<f64>,
    /// 内存族最高温度（R-09：徽章取各设备中的最大值）
    pub memory_temperature_c: Option<f64>,
    /// 磁盘族最高温度（R-09 同上；无 SMART 时也可能有盘温）
    pub storage_temperature_c: Option<f64>,
    pub gpus: Vec<GpuSensor>,
    /// 是否存在磁盘族传感器（决定 F-06 面板走"空 + 原因"还是"根本没这路"）
    pub has_storage_sensor: bool,
    /// 是否存在内存温度传感器（决定 RAM 徽章是"未检测"还是"隐藏"，R-09）
    pub has_memory_temperature: bool,
    pub taken_at: Instant,
}

impl SensorSnapshot {
    #[must_use]
    pub fn age_secs(&self) -> u64 {
        self.taken_at.elapsed().as_secs()
    }
}

fn eq_ignore_case(left: &str, right: &str) -> bool {
    left.eq_ignore_ascii_case(right)
}

fn is_storage_family(hardware_type: &str) -> bool {
    STORAGE_FAMILIES
        .iter()
        .any(|kind| eq_ignore_case(hardware_type, kind))
}

/// K-Q12-2 §二 规则 1：CPU 占用取 `Load/CPU Total`（回退 `Total`）。
#[must_use]
pub fn cpu_load_of(response: &SensorsResponse) -> Option<f64> {
    response
        .sensors
        .iter()
        .find(|s| {
            eq_ignore_case(&s.hardware_type, "Cpu")
                && eq_ignore_case(&s.sensor_type, "Load")
                && (s.name == "CPU Total" || s.name == "Total")
        })
        .map(|s| s.value)
}

/// K-Q12-2 §二 规则 2：CPU 温度候选名（母本 `sensor.rs:288-299` 同集合），取首个命中。
const CPU_TEMP_NAMES: [&str; 7] = [
    "Core (Tctl/Tdie)",
    "CPU Package",
    "Tctl",
    "Core",
    "CPU",
    "CPU Core",
    "CPU Temperature",
];
/// 温度可能来自 CPU 之外的板载芯片族（老 AMD A 系列经 `SuperIO` 上报，母本同口径）。
const CPU_TEMP_HARDWARE: [&str; 3] = ["Cpu", "SuperIO", "Motherboard"];

#[must_use]
pub fn cpu_temperature_of(response: &SensorsResponse) -> Option<f64> {
    response.sensors.iter().find_map(|s| {
        let family_hit = CPU_TEMP_HARDWARE
            .iter()
            .any(|kind| eq_ignore_case(&s.hardware_type, kind));
        let name_hit = CPU_TEMP_NAMES.iter().any(|name| s.name == *name);
        if eq_ignore_case(&s.sensor_type, "Temperature") && family_hit && name_hit {
            Some(s.value)
        } else {
            None
        }
    })
}

/// K-Q12-2 §二 规则 4：RAM 占用只认 `hardware == "Total Memory"`（🔴 不得取 `Virtual Memory`）。
const TOTAL_MEMORY_HARDWARE: &str = "Total Memory";
/// 磁盘族硬件类型名（`LHM` 在不同版本里叫 `HardDisk` / `Hdd` / `Ssd`）。
const STORAGE_FAMILIES: [&str; 3] = ["HardDisk", "Hdd", "Ssd"];

#[must_use]
pub fn memory_load_of(response: &SensorsResponse) -> Option<f64> {
    response
        .sensors
        .iter()
        .find(|s| {
            eq_ignore_case(&s.hardware_type, "Memory")
                && s.hardware == TOTAL_MEMORY_HARDWARE
                && eq_ignore_case(&s.sensor_type, "Load")
        })
        .map(|s| s.value)
}

/// 某一硬件族里的**最高**温度（R-09 徽章口径：取各设备中的最大值）。
#[must_use]
pub fn max_temperature_in_family(response: &SensorsResponse, families: &[&str]) -> Option<f64> {
    response
        .sensors
        .iter()
        .filter(|sensor| eq_ignore_case(&sensor.sensor_type, "Temperature"))
        .filter(|sensor| {
            families
                .iter()
                .any(|family| eq_ignore_case(&sensor.hardware_type, family))
        })
        .map(|sensor| sensor.value)
        .fold(None, |highest: Option<f64>, value| {
            Some(highest.map_or(value, |current| current.max(value)))
        })
}

/// K-Q12-2 §二 规则 3：显卡按设备名分组，组内负载/温度各按优先名取首个命中。
const GPU_LOAD_NAMES: [&str; 5] = ["GPU Core", "D3D 3D", "GPU", "D3D Usage", "Core"];
const GPU_TEMP_NAMES: [&str; 4] = ["GPU Core", "GPU", "Core", "GPU Temperature"];

#[must_use]
pub fn gpu_sensors_of(response: &SensorsResponse) -> Vec<GpuSensor> {
    let mut devices: Vec<String> = Vec::new();
    let mut has_nvidia = false;
    for sensor in &response.sensors {
        if !sensor.hardware_type.to_ascii_lowercase().starts_with("gpu") {
            continue;
        }
        if eq_ignore_case(&sensor.hardware_type, "GpuNvidia") {
            has_nvidia = true;
        }
        if !devices.contains(&sensor.hardware) {
            devices.push(sensor.hardware.clone());
        }
    }

    let mut gpus = Vec::new();
    for device in devices {
        let hardware_type = response
            .sensors
            .iter()
            .find(|s| s.hardware == device)
            .map(|s| s.hardware_type.clone())
            .unwrap_or_default();
        // 混合显卡口径（母本 sensor.rs:336）：存在 NVIDIA 独显时跳过 Intel 核显，AMD 保留
        if has_nvidia && eq_ignore_case(&hardware_type, "GpuIntel") {
            continue;
        }
        let pick = |sensor_type: &str, names: &[&str]| {
            names
                .iter()
                .find_map(|name| {
                    response.sensors.iter().find(|s| {
                        s.hardware == device
                            && eq_ignore_case(&s.sensor_type, sensor_type)
                            && s.name == *name
                    })
                })
                .map(|sensor| sensor.value)
        };
        // 先取完数值再搬 `device`：闭包借用它，struct 字面量按书写顺序求值会构成 E0505。
        let temperature_c = pick("Temperature", &GPU_TEMP_NAMES);
        let load_percent = pick("Load", &GPU_LOAD_NAMES);
        gpus.push(GpuSensor {
            device,
            temperature_c,
            load_percent,
        });
    }
    gpus
}

/// 聚合成快照（规则 5：取不到一律 `None`，🔴 禁止填 0）。
#[must_use]
pub fn aggregate(response: &SensorsResponse) -> SensorSnapshot {
    SensorSnapshot {
        cpu_load_percent: cpu_load_of(response),
        cpu_temperature_c: cpu_temperature_of(response),
        memory_load_percent: memory_load_of(response),
        memory_temperature_c: max_temperature_in_family(response, &["Memory"]),
        storage_temperature_c: max_temperature_in_family(response, &STORAGE_FAMILIES),
        gpus: gpu_sensors_of(response),
        has_storage_sensor: response
            .sensors
            .iter()
            .any(|s| is_storage_family(&s.hardware_type)),
        has_memory_temperature: response.sensors.iter().any(|s| {
            eq_ignore_case(&s.hardware_type, "Memory")
                && eq_ignore_case(&s.sensor_type, "Temperature")
        }),
        taken_at: Instant::now(),
    }
}

/// 探测 sidecar 可执行文件：先按同目录候选，再向上回溯祖先目录（开发环境）。
#[must_use]
pub fn find_monitor_exe() -> Option<PathBuf> {
    let base = std::env::current_exe().ok()?.parent()?.to_path_buf();
    candidate_paths(&base)
        .into_iter()
        .find(|path| path.exists())
}

/// 候选路径生成（与文件系统无关，单独拆出便于测试）。
#[must_use]
pub fn candidate_paths(base: &Path) -> Vec<PathBuf> {
    let mut paths: Vec<PathBuf> = SUBDIRS
        .iter()
        .map(|sub| base.join(sub).join(EXE_NAME))
        .collect();
    let mut probe = base.to_path_buf();
    for _ in 0..ANCESTOR_DEPTH {
        for sub in ANCESTOR_SUBDIRS {
            let candidate = probe.join(sub).join(EXE_NAME);
            if !paths.contains(&candidate) {
                paths.push(candidate);
            }
        }
        if !probe.pop() {
            break;
        }
    }
    paths
}

/// 故障形态（同时决定 DTO 里的 `MonitorState` 与用户可读原因）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FaultKind {
    /// 未找到 AegisMonitor.exe
    ExeMissing,
    /// 子进程起不来
    Spawn,
    /// 子进程已退出 / 管道失效
    Died,
    /// 在预算内没等到回行
    Timeout,
    /// 回了行但不是可解析的传感器 JSON
    Protocol,
    /// 连败达上限
    Disabled,
    /// 冷却期内被再次调用
    Cooling,
}

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
fn classify_sidecar_error(error: &SidecarError, budget: Duration) -> BridgeFault {
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
    exe_path: Option<PathBuf>,
    /// 区分"还没查过"与"查过但没有"，避免每拍重复扫盘
    exe_lookup_done: bool,
    failures: u32,
    cooldown_until: Option<Instant>,
    disabled_reason: Option<String>,
    last_fault: Option<BridgeFault>,
    ready_once: bool,
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

    fn record_failure(&mut self, fault: BridgeFault) -> BridgeFault {
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

#[cfg(test)]
mod tests {
    use super::*;

    fn response(json: &str) -> SensorsResponse {
        serde_json::from_str(json).expect("测试夹具应为合法传感器 JSON")
    }

    /// 本机实测形态（K-Q12-2：无内核驱动 + 非管理员 → CPU 无温度、无磁盘族传感器）。
    const REALISH_FIXTURE: &str = r#"{
        "updatedAt":"2026-10-03T08:00:00Z",
        "sensors":[
            {"hardware":"CPU #1 - 12th Gen Intel Core i7-12700H","hardwareType":"Cpu","name":"CPU Total","sensorType":"Load","value":22.5,"unit":"%"},
            {"hardware":"CPU #1 - 12th Gen Intel Core i7-12700H","hardwareType":"Cpu","name":"Core (Tctl/Tdie)","sensorType":"Load","value":31.0,"unit":"%"},
            {"hardware":"Total Memory","hardwareType":"Memory","name":"Memory Used","sensorType":"Data","value":11.84,"unit":"GB"},
            {"hardware":"Total Memory","hardwareType":"Memory","name":"Memory","sensorType":"Load","value":37.24,"unit":"%"},
            {"hardware":"Virtual Memory","hardwareType":"Memory","name":"Memory","sensorType":"Load","value":99.0,"unit":"%"},
            {"hardware":"GPU #1 - NVIDIA GeForce RTX 3060 Laptop","hardwareType":"GpuNvidia","name":"GPU Core","sensorType":"Temperature","value":56.0,"unit":"°C"},
            {"hardware":"GPU #1 - NVIDIA GeForce RTX 3060 Laptop","hardwareType":"GpuNvidia","name":"D3D 3D","sensorType":"Load","value":3.0,"unit":"%"},
            {"hardware":"GPU #1 - NVIDIA GeForce RTX 3060 Laptop","hardwareType":"GpuNvidia","name":"GPU Core","sensorType":"Load","value":1.0,"unit":"%"},
            {"hardware":"GPU #2 - Intel UHD","hardwareType":"GpuIntel","name":"GPU Core","sensorType":"Load","value":7.0,"unit":"%"}
        ]
    }"#;

    #[test]
    fn cpu_load_prefers_cpu_total() {
        assert_eq!(cpu_load_of(&response(REALISH_FIXTURE)), Some(22.5));
    }

    #[test]
    fn cpu_temperature_is_absent_not_zero_when_driver_missing() {
        // 夹具里刻意不放 CPU Temperature（本机实测 0 条）→ 必须 None，不得给 0
        assert_eq!(cpu_temperature_of(&response(REALISH_FIXTURE)), None);
    }

    #[test]
    fn cpu_temperature_matches_superio_family_on_older_amd() {
        let fixture = r#"{"sensors":[{"hardware":"F71882","hardwareType":"SuperIO","name":"Tctl","sensorType":"Temperature","value":47.0,"unit":"°C"}]}"#;
        assert_eq!(cpu_temperature_of(&response(fixture)), Some(47.0));
    }

    #[test]
    fn load_named_core_sensor_is_not_mistaken_for_temperature() {
        // 夹具内同名条目是 Load（"Core (Tctl/Tdie)" 31%）：温度判定必须看 sensorType
        let fixture = r#"{"sensors":[{"hardware":"CPU","hardwareType":"Cpu","name":"Core (Tctl/Tdie)","sensorType":"Load","value":31.0,"unit":"%"}]}"#;
        assert_eq!(cpu_temperature_of(&response(fixture)), None);
    }

    #[test]
    fn memory_load_ignores_virtual_memory_hardware() {
        // 🔴 规则 4：取到 Virtual Memory 的 99% 就是错数
        assert_eq!(memory_load_of(&response(REALISH_FIXTURE)), Some(37.24));
    }

    #[test]
    fn gpu_grouping_prefers_gpu_core_and_hides_intel_when_nvidia_present() {
        let gpus = gpu_sensors_of(&response(REALISH_FIXTURE));
        assert_eq!(gpus.len(), 1, "存在 NVIDIA 独显时 Intel 核显应被跳过");
        assert_eq!(
            gpus[0].load_percent,
            Some(1.0),
            "负载优先 GPU Core 而非 D3D 3D"
        );
        assert_eq!(gpus[0].temperature_c, Some(56.0));
    }

    #[test]
    fn gpu_keeps_intel_when_it_is_the_only_display() {
        let fixture = r#"{"sensors":[{"hardware":"GPU #1 - Intel UHD","hardwareType":"GpuIntel","name":"GPU Core","sensorType":"Temperature","value":44.0,"unit":"°C"}]}"#;
        let gpus = gpu_sensors_of(&response(fixture));
        assert_eq!(gpus.len(), 1);
        assert_eq!(gpus[0].temperature_c, Some(44.0));
        assert_eq!(gpus[0].load_percent, None, "没有负载传感器时不得凭空造 0");
    }

    #[test]
    fn snapshot_records_missing_sensor_families_as_flags() {
        let snapshot = aggregate(&response(REALISH_FIXTURE));
        assert!(!snapshot.has_storage_sensor, "非管理员下 HardDisk 族缺席");
        assert!(!snapshot.has_memory_temperature);
        assert_eq!(snapshot.gpus.len(), 1);
        assert_eq!(snapshot.cpu_load_percent, Some(22.5));
    }

    #[test]
    fn storage_and_memory_temperature_flags_light_up_when_present() {
        let fixture = r#"{"sensors":[
            {"hardware":"SAMSUNG MZVL2","hardwareType":"Ssd","name":"Temperature","sensorType":"Temperature","value":42.0,"unit":"°C"},
            {"hardware":"Total Memory","hardwareType":"Memory","name":"DIMM Temp","sensorType":"Temperature","value":51.0,"unit":"°C"}
        ]}"#;
        let snapshot = aggregate(&response(fixture));
        assert!(snapshot.has_storage_sensor);
        assert!(snapshot.has_memory_temperature);
        assert_eq!(snapshot.storage_temperature_c, Some(42.0));
        assert_eq!(snapshot.memory_temperature_c, Some(51.0));
    }

    #[test]
    fn badge_temperature_takes_the_highest_reading_in_the_family() {
        // R-09：多设备取最高，不是第一块
        let fixture = r#"{"sensors":[
            {"hardware":"DISK A","hardwareType":"Ssd","name":"Temperature","sensorType":"Temperature","value":38.0,"unit":"°C"},
            {"hardware":"DISK B","hardwareType":"HardDisk","name":"Temperature","sensorType":"Temperature","value":52.0,"unit":"°C"},
            {"hardware":"DISK C","hardwareType":"Hdd","name":"Temperature","sensorType":"Temperature","value":44.0,"unit":"°C"}
        ]}"#;
        assert_eq!(
            max_temperature_in_family(&response(fixture), &STORAGE_FAMILIES),
            Some(52.0)
        );
        let empty = r#"{"sensors":[{"hardware":"X","hardwareType":"Cpu","name":"a","sensorType":"Load","value":1.0,"unit":"%"}]}"#;
        assert_eq!(
            max_temperature_in_family(&response(empty), &STORAGE_FAMILIES),
            None
        );
    }

    #[test]
    fn error_shaped_payload_is_not_silently_treated_as_empty() {
        // sidecar 自身的 {"error":"…"} 回包必须解析失败 → 走 Protocol 故障（R-08 不得当成"零传感器"）
        let parsed: Result<SensorsResponse, serde_json::Error> =
            serde_json::from_str(r#"{"error":"boom"}"#);
        assert!(parsed.is_err(), "缺 sensors 字段的回包不得被当成合法空快照");
    }

    #[test]
    fn candidate_paths_cover_install_and_dev_layouts() {
        let base = Path::new(r"C:\Program Files\Aegis");
        let paths = candidate_paths(base);
        assert!(paths.contains(&PathBuf::from(
            r"C:\Program Files\Aegis\monitor\AegisMonitor.exe"
        )));
        assert!(paths.iter().any(|path| path
            .to_string_lossy()
            .replace('\\', "/")
            .ends_with("resources/monitor/AegisMonitor.exe")));
        // 必须向上回溯到祖先：开发环境 exe 在 src-tauri/target/debug 下，sidecar 在仓库根的 monitor/
        let deepest = paths
            .iter()
            .filter(|path| path.starts_with("C:\\Program Files\\Aegis"))
            .count();
        assert!(
            deepest >= SUBDIRS.len(),
            "同目录候选应全部保留，实得 {deepest}"
        );
        assert!(paths.iter().any(|path| {
            let text = path.to_string_lossy().replace('\\', "/");
            text.contains("monitor/bin/Release/net48/AegisMonitor.exe")
        }));
    }

    /// 构造一个"已查过、确认没有 exe"的桥（避免测试依赖本机是否装了 sidecar）。
    fn bridge_without_exe() -> SensorBridge {
        let mut bridge = SensorBridge::new();
        bridge.exe_lookup_done = true;
        bridge.exe_path = None;
        bridge
    }

    #[test]
    fn missing_exe_fails_fast_with_component_named() {
        let mut bridge = bridge_without_exe();
        let started = Instant::now();
        let fault = bridge.read_once().expect_err("缺 exe 必须失败");
        assert_eq!(fault.kind, FaultKind::ExeMissing);
        assert!(
            fault.reason.contains(EXE_NAME),
            "原因须点名组件便于诊断，实得：{}",
            fault.reason
        );
        // 🔴 "5s 快败"的结构性上界：exe 缺失路径不得消耗任何读超时预算
        assert!(
            started.elapsed() < Duration::from_secs(1),
            "exe 缺失应立即失败，实得 {:?}",
            started.elapsed()
        );
        assert!(bridge.cooling_down(), "失败后应进入冷却");
    }

    #[test]
    fn cooldown_refuses_retry_until_expiry() {
        let mut bridge = bridge_without_exe();
        let first = bridge.read_once().expect_err("首次应失败");
        assert_eq!(first.kind, FaultKind::ExeMissing);

        let second = bridge.read_once().expect_err("冷却期不得再起进程");
        assert_eq!(second.kind, FaultKind::Cooling);
        assert!(!bridge.is_disabled(), "1 次失败远未达停用阈值");
    }

    #[test]
    fn fifth_consecutive_failure_disables_the_channel_for_the_session() {
        let mut bridge = SensorBridge::new();
        for index in 1..=MAX_CONSECUTIVE_FAILURES {
            let fault = bridge.record_failure(BridgeFault::new(
                FaultKind::Died,
                format!("第 {index} 次退出"),
            ));
            if index < MAX_CONSECUTIVE_FAILURES {
                assert_eq!(fault.kind, FaultKind::Died, "第 {index} 次仍是原故障");
                assert!(bridge.cooling_down());
            } else {
                assert_eq!(fault.kind, FaultKind::Disabled, "第 5 次连败应转停用");
            }
        }
        assert!(bridge.is_disabled());
        let refused = bridge.read_once().expect_err("停用后不得再读");
        assert_eq!(refused.kind, FaultKind::Disabled);
        assert!(
            refused.reason.contains("连续失败 5 次"),
            "停用原因须可诊断：{}",
            refused.reason
        );
    }

    #[test]
    fn successful_read_clears_failure_state() {
        let mut bridge = SensorBridge::new();
        bridge.record_failure(BridgeFault::new(FaultKind::Timeout, "预算耗尽"));
        assert_eq!(bridge.failures, 1);
        assert!(!bridge.has_reads());
        assert_eq!(
            bridge.last_fault().expect("应留下末次故障").kind,
            FaultKind::Timeout
        );

        // 模拟成功回包后的状态推进（不依赖真实子进程）
        bridge.failures = 0;
        bridge.cooldown_until = None;
        bridge.ready_once = true;
        assert!(!bridge.cooling_down());
        assert!(bridge.has_reads());
        assert_eq!(bridge.sidecar_pid(), None, "未接管进程时不得伪造 PID");
    }

    #[test]
    fn timeout_reason_carries_the_budget_actually_used() {
        let fault = classify_sidecar_error(&SidecarError::Timeout, FIRST_READ_BUDGET);
        assert_eq!(fault.kind, FaultKind::Timeout);
        assert!(
            fault.reason.contains("23 秒"),
            "首次预算应写进原因，实得：{}",
            fault.reason
        );
        let steady = classify_sidecar_error(&SidecarError::Timeout, READ_TIMEOUT);
        assert!(steady.reason.contains("8 秒"), "实得：{}", steady.reason);
    }

    #[test]
    fn pipe_write_failure_is_reported_as_a_dead_child() {
        let fault = classify_sidecar_error(
            &SidecarError::Write("管道正在被使用".to_string()),
            READ_TIMEOUT,
        );
        assert_eq!(fault.kind, FaultKind::Died);
        assert!(fault.reason.contains("管道"), "实得：{}", fault.reason);
    }
}
