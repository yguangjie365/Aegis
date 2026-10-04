//! 桥侧线协议类型（serde 形态）：`sensor_bridge` 的子模块。

use serde::Deserialize;
use std::time::Instant;

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
    /// sidecar 自带的测量单位（`Program.cs GetUnit`）；F-11 全量表透传给前端，避免本仓重抄单位表。
    #[serde(default)]
    pub unit: String,
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
