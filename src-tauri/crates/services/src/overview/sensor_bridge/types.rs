//! 桥侧线协议类型（serde 形态）：`sensor_bridge` 的子模块。

use super::availability::MetricAvailability;
use serde::{Deserialize, Deserializer};
use std::time::Instant;

/// 🔴 `null` → 该字段的默认值。
///
/// 为什么需要它：**`#[serde(default)]` 只处理字段「缺失」，不处理显式 `null`。**
/// 真实 sidecar 数据里存在 `null` 值（2026-10-07 实测回包里就有
/// `"subHardware":null`，同一族的 `hardware` 在某些设备上同样是 null），
/// 对 `String`/`f64` 直接反序列化会报 `invalid type: null, expected a string`
/// ⇒ **整份快照被判协议故障**，概览页所有温度/增强项一并降级
/// （用户截图：CPU/RAM 卡各挂一条黄色警告，显存/频率全空）。
///
/// 这里统一把「显式 null」视作「该字段没有值」⇒ 走它自己的默认值。
/// 用 `Option` 显式建模也可以，但那样每个使用处都要 `.unwrap_or_default()`，
/// 而聚合逻辑（`matching.rs`）已经把 `String` 当非空串直接用了。
fn null_to_string<'de, D>(deserializer: D) -> Result<String, D::Error>
where
    D: Deserializer<'de>,
{
    Ok(Option::<String>::deserialize(deserializer)?.unwrap_or_default())
}

fn null_to_f64<'de, D>(deserializer: D) -> Result<f64, D::Error>
where
    D: Deserializer<'de>,
{
    Ok(Option::<f64>::deserialize(deserializer)?.unwrap_or_default())
}

/// sidecar 回单条传感器（字段名与 `monitor/SensorReader.cs` 逐字节对齐）。
///
/// 🔴 字段**全部**带 `deserialize_with`：因为 `default` 对 `null` 无效（见上），
///   而 sidecar 的真实数据里 `null` 是常态而非例外。
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SensorReading {
    #[serde(default, deserialize_with = "null_to_string")]
    pub hardware: String,
    #[serde(default, deserialize_with = "null_to_string")]
    pub hardware_type: String,
    #[serde(default, deserialize_with = "null_to_string")]
    pub name: String,
    #[serde(default, deserialize_with = "null_to_string")]
    pub sensor_type: String,
    #[serde(default, deserialize_with = "null_to_f64")]
    pub value: f64,
    /// sidecar 自带的测量单位（`Program.cs GetUnit`）；F-11 全量表透传给前端，避免本仓重抄单位表。
    #[serde(default, deserialize_with = "null_to_string")]
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
    /// 🔴 2026-10-07：DIMM 采集的**五态**可得性（用户裁决：不伪造状态）。
    ///
    ///   与 `has_memory_temperature` 并存而非替换：后者是 R-09 徽章的判据
    ///   （`false` ⇒ 隐藏徽章），前者回答「为什么读不到、该做什么」。二者正交。
    ///
    ///   `permission_denied` 只在有 sidecar 诊断证据时成立（判据见
    ///   `availability.rs`）—— 「读不到」不足以推出权限不足。
    pub memory_availability: MetricAvailability,
    pub taken_at: Instant,
}

impl SensorSnapshot {
    #[must_use]
    pub fn age_secs(&self) -> u64 {
        self.taken_at.elapsed().as_secs()
    }
}
