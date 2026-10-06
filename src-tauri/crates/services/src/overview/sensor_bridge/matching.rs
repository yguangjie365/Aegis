//! 传感器名→展示值的匹配与聚合表：`sensor_bridge` 的子模块。

use super::availability::{classify_dimm, DimmEvidence};
use super::metrics::{metric_id_of, temp_slot};
use super::types::{GpuSensor, SensorSnapshot, SensorsResponse};
use std::time::Instant;

/// 🔴 大小写无关比较：`hardwareType` / `sensorType` 的取值大小写在不同 LHM 版本
///   间并不一致（实测 `SuperIO` / `superio` 都出现过），故所有族/类型判据都走它，
///   避免同一族在某些机器上突然「消失」。
pub(crate) fn eq_ignore_case(left: &str, right: &str) -> bool {
    left.eq_ignore_ascii_case(right)
}

/// 🔴 2026-10-07：判据从**严格等值**改为**小写包含**。
///
///起因：实测发现 LHM 给的 `hardwareType` 是 `Storage`，而等值清单里写的是
///   `Ssd` / `Hdd` / `HardDisk` —— 一个都不命中。等值匹配要求清单穷举 LHM
///   的**全部**可能取值，而那份清单是照抄母本的、并不完整。
///   改成包含后，`Storage` / `NvmExpress` / `SataSd` 之类命名都能命中，
///   代价是可能命中 `StorageSomethingElse`（本仓无此硬件，可接受）。
fn is_storage_family(hardware_type: &str) -> bool {
    let lower = hardware_type.to_ascii_lowercase();
    STORAGE_FAMILIES
        .iter()
        .any(|kind| lower.contains(&kind.to_ascii_lowercase()))
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

/// 🔴 温度选值：**三档优先级**（采 `SystemToolkit` 基线 `SelectTemperature`
///   `HardwareSensorProbe.cs:468` 的口径）。
///
/// 为什么不沿用 `NexBox` 母本的「首个命中」（`Source/NexBox/src-tauri/src/sensor.rs:292`）：
/// 母本是 `s.name == "…"` 严格等值 + `if temp.is_none()` **取数组里第一个命中**。
/// ① 名单里没有 `Core Average`，也不区分单核与整片 ⇒ LHM 返回顺序一变，
///    就可能取到 `Core #1`（单核读数，通常**低于**整片封装温度）；
/// ② 严格等值对不同厂商的命名差异无余量（`Package Temperature` / `CPU Package`
///    / `Tctl/Tdie` 各家写法不一）。
/// 基线的三档 + 末档取最大值**与顺序无关**，且能兜住「只有逐核读数」的情况。
///
/// 优先级（🔴 2026-10-07 起按 **`metric_id` 语义位**判，不再读 `name`）：
/// 1. `core_average` —— 整片平均，最接近「CPU 温度」的用户直觉；
/// 2. `package` / `tctl` / `gpu_core` —— 整片/最高封装温度；
/// 3. `core` / `other` —— **取最大值**（逐核时首个可能是最低那核）。
///
/// 🔴 2026-10-07：**判据从「读 `name`」切到「读 `metric_id`」**。
///   此前归一表（`metrics.rs`）与本函数是**两处真相** —— 上游改名时归一表要改、
///   这里也要改，漏一处就静默取到错值。现在三档只按 `metric_id` 的语义位判，
///   上游改名的影响面被限制在 `metrics.rs` **一张表**里。
///   （`Core Max` 曾单列在第 2 档，切metricId 后它归入 `core` 落第 3 档 ——
///     真机有 `Core Average` 时第 1 档先生效，故取值不变。）
///
/// 🔴 调用方**必须**先排除阈值型条目（见 [`is_threshold_sensor_name`]）：
///   `Critical Temperature` / `High Limit` 会命中下面第 2、3 档的语义。
fn select_temperature(candidates: &[(String, String, f64)]) -> Option<f64> {
    let mut average: Option<f64> = None;
    let mut package: Option<f64> = None;
    let mut max_core: Option<f64> = None;
    for (metric_id, _name, value) in candidates {
        match metric_id.split('.').nth(1).unwrap_or_default() {
            temp_slot::CORE_AVERAGE => average = average.or(Some(*value)),
            temp_slot::PACKAGE | temp_slot::TCTL | temp_slot::GPU_CORE => {
                package = package.or(Some(*value));
            }
            temp_slot::CORE | temp_slot::OTHER => {
                max_core = Some(max_core.map_or(*value, |current: f64| current.max(*value)));
            }
            // 🔴 `misc.*`（未识别族）与拼写异常一律**不参与**选值：
            //   语义未知，纳入取最大只会污染结果。
            _ => {}
        }
    }
    average.or(package).or(max_core)
}

/// K-Q12-2 §二 规则 2：CPU 温度 —— 硬件族白名单沿用 `NexBox` 母本（`sensor.rs:289`），
/// 选值改用基线的三档优先级（见 [`select_temperature`] 的取舍说明）。
/// 温度可能来自 CPU 之外的板载芯片族（老 AMD A 系列经 `SuperIO` 上报，母本同口径）。
const CPU_TEMP_HARDWARE: [&str; 3] = ["Cpu", "SuperIO", "Motherboard"];

#[must_use]
pub fn cpu_temperature_of(response: &SensorsResponse) -> Option<f64> {
    let candidates: Vec<(String, String, f64)> = response
        .sensors
        .iter()
        .filter(|s| {
            CPU_TEMP_HARDWARE
                .iter()
                .any(|kind| eq_ignore_case(&s.hardware_type, kind))
        })
        .filter(|s| eq_ignore_case(&s.sensor_type, "Temperature"))
        .filter(|s| !is_threshold_sensor_name(&s.name))
        // 🔴 三元组带上 `metric_id`：`select_temperature` 已改为按语义位判，
        //   归一表（`metrics.rs`）是唯一需要随上游改名更新的地方。
        .map(|s| (metric_id_of(s), s.name.clone(), s.value))
        .collect();
    select_temperature(&candidates)
}

/// K-Q12-2 §二 规则 4：RAM 占用只认 `hardware == "Total Memory"`（🔴 不得取 `Virtual Memory`）。
const TOTAL_MEMORY_HARDWARE: &str = "Total Memory";
/// 磁盘族硬件类型名。
///
/// 🔴 2026-10-07 **实测修正**：本清单原先是 `["HardDisk", "Hdd", "Ssd"]`，
///   全部**不匹配** LHM 0.9.6 在本机实际给出的 `hardwareType` ——
///   真机回包（管理员，205 条传感器）里两块盘的 `hardwareType` 都是 **`Storage`**。
///
///   ⇒ 后果：`is_storage_family` 恒false ⇒ `has_storage_sensor` 恒 false、
///     `storage_temperature_of` 恒 `None` ⇒ **存储卡温度徽章永远「未检测」**，
///     即便 LHM 已把`Composite Temperature` 45/44°C 摆在回包里。
///   这就是「SystemToolkit 能读到盘温而本仓读不到」的**唯一**原因
///   （基线 `HardwareSensorProbe.cs:174` 用的是 LHM `HardwareType.Storage` 枚举）。
///
/// 🔴 **教训**：这份白名单是照抄 `NexBox` 母本的，而母本同样没覆盖 `Storage` ——
///   复刻基线不等于基线正确。判据必须以**真机回包**为准，而非上游源码。
///   取值用「小写包含」而非严格等值，兼顾未来 LHM 细分类型（如 `Nvme` / `Sata`）。
pub(super) const STORAGE_FAMILIES: [&str; 5] = ["Storage", "Ssd", "Hdd", "HardDisk", "Nvme"];

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

/// 🔴 阈值型传感器名 —— **必须排除**，否则会把「温度上限」当成「当前温度」。
///
/// 基线实现同名判据（`Source/SystemToolkit/.../HardwareSensorProbe.cs:557`
/// `IsThresholdSensorName`），那边的注释写得很直白：
/// 「同节点混有 High/Low/Critical Limit、Warning/Critical Temperature」。
///
/// 2026-10-07 实测后果（本条缺失导致）：内存与 SSD **同时**显示 85°C，
/// 而对照工具读数是 `DIMM` 53°C / SSD 46°C —— 85 正是 `NVMe`
/// `Critical Comp. Temperature` 的典型阈值。两个不同硬件报同一个数值，
/// 就是「取到了阈值」的指纹。
///
/// 🔴 2026-10-07 **补`distance to tjmax`**（真机 205 条回包里发现）：
///   LHM 给出 `P-Core #1 Distance to TjMax` = 46°C 这样的条目，含义是
///   「当前温度距该核上限还差多少度」，**方向与温度相反**，不是读数。
///   而它**不含** `limit` / `warning` / `critical` 任何一个词 ⇒ 此前会被
///   当成「单核温度」参与取最大（`cpu.core.temperature`），污染 CPU 温度。
#[must_use]
pub fn is_threshold_sensor_name(name: &str) -> bool {
    let lower = name.to_ascii_lowercase();
    [
        "limit",
        "warning",
        "critical",
        "resolution",
        "distance to tjmax",
    ]
    .iter()
    .any(|marker| lower.contains(marker))
}

/// 基线 `IsDimmReading`（`HardwareSensorProbe.cs:528`）：内存温度**只认 `DIMM` 开头**
/// 的传感器名 —— 内存节点下除 TSOD 读数外还混有阈值与其它读数。
#[must_use]
pub fn is_dimm_reading(name: &str) -> bool {
    name.to_ascii_lowercase().starts_with("dimm")
}

/// 基线 `SelectMaxDriveTemperature`（见基线文件 538 行）：
/// 每盘优先 `Composite Temperature`（`NVMe` 标准复合温度），
/// 它才是「这盘此刻的温度」；缺失时才退回该盘其它温度读数的最大值。
///
/// 🔴 2026-10-07：判据由 `name == "Composite Temperature"` 改为
///   **`metric_id == "storage.composite.temperature"`** —— 至此 `matching.rs`
///   **不再有任何一处读 `name` 做选值**（`is_dimm_reading` /
///   `is_threshold_sensor_name` 仍在，但它们只用于「排除」，
///   属`metrics.rs` 归一表的输入，不是独立的第二套语义）。
#[must_use]
#[allow(clippy::type_complexity)] // 三元组是内部过渡形态，不值得为它引入具名结构
fn pick_drive_temperature(entries: &[(String, String, f64)]) -> Option<f64> {
    let composite = entries
        .iter()
        .find(|(metric_id, _, _)| metric_id == "storage.composite.temperature")
        .map(|(_, _, v)| *v);
    composite.or_else(|| {
        entries
            .iter()
            .map(|(_, _, v)| *v)
            .fold(None, |highest: Option<f64>, v| {
                Some(highest.map_or(v, |c| c.max(v)))
            })
    })
}

/// 某一硬件族里的**最高**温度（R-09 徽章口径：取各设备中的最大值）。
///
/// 🔴 2026-10-07 补上「排除阈值型条目」：原实现只按 `hardware_type` 过滤，
///   把同节点下的 `Critical Temperature` / `High Limit` 也算了进去，
///   于是内存与 SSD 同时显示 85°C（= `NVMe` 临界温度阈值）而与对照工具差 30+ 度。
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
        .filter(|sensor| !is_threshold_sensor_name(&sensor.name))
        .map(|sensor| sensor.value)
        .fold(None, |highest: Option<f64>, value| {
            Some(highest.map_or(value, |current| current.max(value)))
        })
}

/// 内存温度：🔴 **只认 `DIMM` 开头的读数**（基线 `IsDimmReading` 口径），
/// 且**必须再排除阈值型条目** —— 阈值名恰好也以 `DIMM` 开头
/// （`DIMM1 High Limit` / `DIMM1 Critical Limit`），只判前缀会把 85°C 的上限当读数。
/// 这两个判据是**与**关系，缺一个都会重现 2026-10-07 的 85°C 缺陷。
#[must_use]
pub fn memory_temperature_of(response: &SensorsResponse) -> Option<f64> {
    response
        .sensors
        .iter()
        .filter(|s| eq_ignore_case(&s.hardware_type, "Memory"))
        .filter(|s| eq_ignore_case(&s.sensor_type, "Temperature"))
        .filter(|s| is_dimm_reading(&s.name))
        .filter(|s| !is_threshold_sensor_name(&s.name))
        .map(|s| s.value)
        .fold(None, |highest: Option<f64>, value| {
            Some(highest.map_or(value, |current| current.max(value)))
        })
}

/// 存储温度：按盘分组，每盘优先 `Composite Temperature`，再取各盘最高。
#[must_use]
pub fn storage_temperature_of(response: &SensorsResponse) -> Option<f64> {
    let mut best: Option<f64> = None;
    for device in response
        .sensors
        .iter()
        .filter(|s| is_storage_family(&s.hardware_type))
        .map(|s| s.hardware.clone())
        .collect::<std::collections::BTreeSet<_>>()
    {
        let entries: Vec<(String, String, f64)> = response
            .sensors
            .iter()
            .filter(|s| s.hardware == device)
            .filter(|s| is_storage_family(&s.hardware_type))
            .filter(|s| eq_ignore_case(&s.sensor_type, "Temperature"))
            .filter(|s| !is_threshold_sensor_name(&s.name))
            // 🔴 第一元素改传 `metric_id`（`pick_drive_temperature` 已改按语义位判）。
            //   `hardware` 在此函数内已无用途（分组靠外层的 `device` 变量），
            //   但保留位置以维持三元组形状。
            .map(|s| (metric_id_of(s), s.name.clone(), s.value))
            .collect();
        if let Some(drive_best) = pick_drive_temperature(&entries) {
            best = Some(best.map_or(drive_best, |c: f64| c.max(drive_best)));
        }
    }
    best
}

/// K-Q12-2 §二 规则 3：显卡按设备名分组；**负载**按优先名取首个命中。
///
/// 🔴 温度**不走**优先名表（2026-10-07）：原先的 `GPU_TEMP_NAMES`
///   （`["GPU Core","GPU","Core","GPU Temperature"]` 严格等值 + 首个命中）
///   是 `NexBox` 母本的写法，已随 CPU 一起改为 `select_temperature` 三档优先级
///   并删除该常量 —— 保留它只会让人以为温度还在用它。
const GPU_LOAD_NAMES: [&str; 5] = ["GPU Core", "D3D 3D", "GPU", "D3D Usage", "Core"];

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
        // 🔴 温度与负载走**不同**判据（2026-10-07）：
        //   温度 → 三档优先级 `select_temperature`（同 CPU 口径，与顺序无关，
        //          并排除阈值型条目）；`GPU_TEMP_NAMES` 那套严格等值+首个命中是
        //          NexBox 母本的写法，LHM 返回顺序一变就可能取到别的读数。
        //   负载 → 严格等值 + 首个命中即可（负载无「取错对象」的风险）。
        let temperature_c = select_temperature(
            &response
                .sensors
                .iter()
                .filter(|s| s.hardware == device)
                .filter(|s| eq_ignore_case(&s.sensor_type, "Temperature"))
                .filter(|s| !is_threshold_sensor_name(&s.name))
                .map(|s| (metric_id_of(s), s.name.clone(), s.value))
                .collect::<Vec<_>>(),
        );
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
///
/// 🔴 2026-10-07 曾有一个 `DIMM_STALE_AFTER`（读数超龄即降级为 `Stale`），
///   连同 `apply_staleness` 与 `Stale` 态**已整体删除** —— 阈值按「3 倍2 秒节拍」
///   拍成 6秒，但传感器实际更新周期由 **sidecar 读耗时**支配
///   （`FIRST_READ_BUDGET = 15s + 8s = 23s`），必然误报。理由见
///   `availability.rs` 里`apply_staleness` 原位的删除说明。
///
/// `permission_evidence` = sidecar 诊断里是否出现过明确的权限失败字样；
/// `None` 表示本次没有取到任何证据（首次采集、或缓冲已清空）。
#[must_use]
pub fn aggregate(response: &SensorsResponse, permission_evidence: Option<bool>) -> SensorSnapshot {
    let has_dimm_reading = response.sensors.iter().any(|s| {
        eq_ignore_case(&s.hardware_type, "Memory")
            && eq_ignore_case(&s.sensor_type, "Temperature")
            && is_dimm_reading(&s.name)
            && !is_threshold_sensor_name(&s.name)
    });
    let has_memory_node = response
        .sensors
        .iter()
        .any(|s| eq_ignore_case(&s.hardware_type, "Memory"));

    SensorSnapshot {
        cpu_load_percent: cpu_load_of(response),
        cpu_temperature_c: cpu_temperature_of(response),
        memory_load_percent: memory_load_of(response),
        memory_temperature_c: memory_temperature_of(response),
        storage_temperature_c: storage_temperature_of(response),
        gpus: gpu_sensors_of(response),
        has_storage_sensor: response
            .sensors
            .iter()
            .any(|s| is_storage_family(&s.hardware_type)),
        // 🔴 2026-10-07：判据与 `memory_temperature_of` **完全对齐**（DIMM 前缀
        //   **且**非阈值）—— 此前只看 hardwareType+类型，会把「内存可见但 DIMM
        //   读数不可用」误报成「有内存温度」，界面给出虚假的确定性；
        //   而阈值名（`DIMM1 Critical Limit`）也以 DIMM 开头，只判前缀同样会漏。
        //   基线 `HardwareSensorProbe.cs:440` 正是为区分这两者准备的提示文案。
        has_memory_temperature: has_dimm_reading,
        memory_availability: classify_dimm(DimmEvidence {
            permission_denied_hint: permission_evidence.unwrap_or(false),
            has_memory_node,
            has_dimm_reading,
        }),
        taken_at: Instant::now(),
    }
}
