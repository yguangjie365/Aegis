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

mod bridge;
mod exe;
mod matching;
mod types;

pub use self::bridge::{
    BridgeFault, SensorBridge, COOLDOWN, FIRST_READ_BUDGET, MAX_CONSECUTIVE_FAILURES, OPEN_BUDGET,
    READ_TIMEOUT, SHUTDOWN_GRACE,
};
pub use self::exe::{candidate_paths, find_monitor_exe, FaultKind};
pub use self::matching::{
    aggregate, cpu_load_of, cpu_temperature_of, gpu_sensors_of, max_temperature_in_family,
    memory_load_of,
};
pub use self::types::{GpuSensor, SensorReading, SensorSnapshot, SensorsResponse};

#[cfg(test)]
mod tests;
