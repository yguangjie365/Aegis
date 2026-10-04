//! 传感器名→展示值的匹配与聚合表：`sensor_bridge` 的子模块。

use super::types::{GpuSensor, SensorSnapshot, SensorsResponse};
use std::time::Instant;

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
pub(super) const STORAGE_FAMILIES: [&str; 3] = ["HardDisk", "Hdd", "Ssd"];

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
