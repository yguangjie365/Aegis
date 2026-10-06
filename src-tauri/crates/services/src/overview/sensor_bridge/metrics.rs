//! 传感器**稳定标识**（`metric_id`）的归一化层：`sensor_bridge` 的子模块。
//!
//! 🔴 **为什么需要这一层（2026-10-07）**：本模块存在的全部理由是，
//!   `matching.rs` 里每一条判据都建立在 **第三方库返回的字符串**上——
//!   `starts_with("dimm")`、`== "Composite Temperature"`、`contains("core average")`。
//!   这些字符串是 **LHM 的输出，不是本工程的契约**：厂商改一次命名、换一个 LHM 版本、
//!   同一硬件在不同驱动下命名不同，判据就失效 —— 而**失效方式是静默取到错值**
//!   （把 `Critical Comp. Temperature` 当成当前温度），不是报错。
//!
//!   2026-10-07 的 85°C 缺陷就是它的第一个症状：内存与 SSD **同时**显示 85°C，
//!   而对照工具读数是 DIMM 53°C / SSD 46°C。85 正是 `NVMe`
//!   临界温度的典型阈值。
//!
//! 三条设计规则：
//!
//! 1. **归一表按「硬件族 × 语义位」建，不按具体传感器名建。** 先判
//!    `hardware_type`（稳定的类枚举），再在族内映射语义位。这样厂商在名字上
//!    增删修饰词时，最多让某个语义位落空，而不是整条判据失效。
//! 2. **阈值型条目映射成 `*.critical_temperature` 等独立 id，而不是被过滤掉。**
//!    过滤会让人以为它不存在，而它其实是**阈值判定的天然来源** —— 设备自己报的
//!    临界值比规格默认值准（见 `badges.rs` 的 `critical_override`）。
//! 3. **未识别的传感器归入 `misc.*` 并原样上报**（F-11 全量表），不静默丢弃。
//!    这样上游新增传感器时我们**看得见**，而不是悄悄失效。
//!
//! ⚠️ **本层不改业务语义**：它只负责「给每条读数起一个稳定名字」。
//!   选值逻辑仍在 `matching.rs`，只是判据从「字符串」改成「读 `metric_id`」。
//!   这样上游改名的影响面被限制在**本文件一张表**里。

use super::matching::{eq_ignore_case, is_dimm_reading, is_threshold_sensor_name};
use super::types::SensorReading;

/// 稳定指标标识。
///
/// 🔴 命名规则：`族.语义[.槽位]`（全小写下划线分���，点号分层）。
///   **不要**用厂商原始名做 id —— 那等于把上游契约直接抄成本仓契约。
pub type MetricId = String;

/// 温度读数的语义位（`metric_id` 的第二段）。
pub mod temp_slot {
    /// 封装 / 整体温度（`CPU Package`）。
    pub const PACKAGE: &str = "package";
    /// Tctl/Tdie（AMD 的等效封装温度口径）。
    pub const TCTL: &str = "tctl";
    /// 整片平均（`Core Average`）。
    pub const CORE_AVERAGE: &str = "core_average";
    /// 单核读数。
    pub const CORE: &str = "core";
    /// GPU 核心。
    pub const GPU_CORE: &str = "gpu_core";
    /// `NVMe` 标准复合温度（**这才是「这盘此刻的温度」**）。
    pub const COMPOSITE: &str = "composite";
    /// 内存模槽 TSOD 读数。
    pub const DIMM: &str = "dimm";
    /// 其它温度（归一后仍保留原名，不猜语义）。
    pub const OTHER: &str = "other";
}

/// 归一化一条传感器读数。
///
/// 返回的 id **保证非空**且只含 `[a-z0-9_.]` —— 上游名字里的空格、斜杠、方括号
/// 等字符一律替换，避免拼出的 id 不可用作键。
#[must_use]
pub fn metric_id_of(reading: &SensorReading) -> MetricId {
    let family = family_of(&reading.hardware_type);
    let lower = reading.name.to_ascii_lowercase();

    // 🔴 阈值型条目单独成id，**不是**普通温度读数。
    //   「当前温度」与「温度上限」是两个语义，混在一起就是 85°C 缺陷的根因；
    //   而上限本身是有用信息（可作为阈值判定的来源），不该被丢弃。
    if is_threshold_sensor_name(&reading.name) {
        return format!("{family}.{}.critical_temperature", threshold_slot(&lower));
    }

    if !eq_ignore_case(&reading.sensor_type, "Temperature") {
        // 非温度读数：负载 / 风扇 / 电压 / 数据 …… 统一走 misc，只保留类型与原名。
        return format!(
            "misc.{}.{}",
            sanitize(&reading.sensor_type),
            sanitize(&lower)
        );
    }

    match family {
        "cpu" => cpu_temperature_id(&lower),
        "memory" => memory_temperature_id(&lower),
        "storage" => storage_temperature_id(&lower),
        "gpu" => gpu_temperature_id(&lower),
        _ => format!("{family}.{}.{}", temp_slot::OTHER, sanitize(&lower)),
    }
}

/// 硬件族归一（`hardware_type` 是本工程与 LHM 之间**最稳定**的一层契约）。
fn family_of(hardware_type: &str) -> &'static str {
    let ht = hardware_type.to_ascii_lowercase();
    if eq_ignore_case(hardware_type, "Cpu")
        || eq_ignore_case(hardware_type, "SuperIO")
        || eq_ignore_case(hardware_type, "Motherboard")
    {
        // 板载芯片（`SuperIO` / `Motherboard`）常同时报 CPU 与主板温度，
        // 归入 cpu 族后仍由名称关键词区分语义（见 `cpu_temperature_id`）。
        "cpu"
    } else if ht.contains("memory") || ht.contains("ram") {
        "memory"
    } else if ht.contains("ssd")
        || ht.contains("hdd")
        || ht.contains("harddisk")
        || ht.contains("nvme")
        || ht.contains("disk")
        // 🔴 2026-10-07：真机回包里存储族的 `hardwareType` 就是 **`Storage`**
        //   （此前漏了它 ⇒ 盘温会被归进 `misc.*`，归一化对它完全失效）。
        || ht.contains("storage")
    {
        "storage"
    } else if ht.contains("gpu")
        || ht.contains("nvidia")
        || ht.contains("amd")
        || ht.contains("intel")
        || ht.contains("display")
    {
        "gpu"
    } else {
        "misc"
    }
}

/// 阈值条目的语义位：优先 `high_limit` / `low_limit`，否则归 `critical`。
fn threshold_slot(lower_name: &str) -> &'static str {
    if lower_name.contains("low") {
        "low_limit"
    } else if lower_name.contains("high") {
        "high_limit"
    } else {
        "critical"
    }
}

fn cpu_temperature_id(lower_name: &str) -> MetricId {
    let slot = if lower_name.contains("core average") {
        temp_slot::CORE_AVERAGE
    } else if lower_name.contains("package") {
        temp_slot::PACKAGE
    } else if lower_name.contains("tctl") || lower_name.contains("tdie") {
        temp_slot::TCTL
    } else if lower_name.contains("gpu core") || lower_name.contains("gpu") {
        temp_slot::GPU_CORE
    } else if lower_name.contains("core") {
        temp_slot::CORE
    } else {
        temp_slot::OTHER
    };
    format!("cpu.{slot}.temperature")
}

fn gpu_temperature_id(lower_name: &str) -> MetricId {
    let slot = if lower_name.contains("hot spot")
        || lower_name.contains("hotspot")
        || lower_name.contains("edge")
    {
        "hotspot"
    } else if lower_name.contains("memory") {
        "memory"
    } else if lower_name.contains("core") {
        temp_slot::GPU_CORE
    } else {
        temp_slot::OTHER
    };
    format!("gpu.{slot}.temperature")
}

fn memory_temperature_id(lower_name: &str) -> MetricId {
    // `is_dimm_reading` 内部本就会 `to_ascii_lowercase`，传小写串是安全的。
    if is_dimm_reading(lower_name) {
        format!("memory.{}.temperature", temp_slot::DIMM)
    } else {
        format!("memory.{}.temperature", temp_slot::OTHER)
    }
}

fn storage_temperature_id(lower_name: &str) -> MetricId {
    let slot = if lower_name.contains("composite") {
        temp_slot::COMPOSITE
    } else {
        temp_slot::OTHER
    };
    format!("storage.{slot}.temperature")
}

/// 上游名字 → id 片段：只保留 `[a-z0-9_.]`，其余一律 `_`。
fn sanitize(raw: &str) -> String {
    let lowered = raw.to_ascii_lowercase();
    let mapped: String = lowered
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '.' || c == '_' {
                c
            } else {
                '_'
            }
        })
        .collect();
    let trimmed = mapped.trim_matches('_').to_owned();
    if trimmed.is_empty() {
        "unknown".to_owned()
    } else {
        trimmed
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn reading(hardware_type: &str, name: &str, sensor_type: &str) -> SensorReading {
        SensorReading {
            hardware: "hw".to_owned(),
            hardware_type: hardware_type.to_owned(),
            name: name.to_owned(),
            sensor_type: sensor_type.to_owned(),
            value: 1.0,
            unit: String::new(),
        }
    }

    #[test]
    fn dimm_readings_map_to_one_stable_slot() {
        // 厂商名各异的 DIMM 读数必须收敛到同一个 id —— 这才是「稳定」的含义。
        for name in ["DIMM1", "DIMM3", "dimm 1", "DIMM_A1"] {
            let id = metric_id_of(&reading("Memory", name, "Temperature"));
            assert_eq!(
                id, "memory.dimm.temperature",
                "「{name}」应映射到统一的内存温度 id"
            );
        }
    }

    #[test]
    fn threshold_entries_get_their_own_id_instead_of_being_dropped() {
        // 🔴 核心不变量：阈值**不与读数混同**，但也**不丢弃** ——
        //   它是设备自报的临界值，是阈值判定的天然来源。
        let crit = metric_id_of(&reading("Ssd", "Critical Comp. Temperature", "Temperature"));
        assert_eq!(crit, "storage.critical.critical_temperature");

        let dimm_high = metric_id_of(&reading("Memory", "DIMM1 High Limit", "Temperature"));
        assert_eq!(
            dimm_high, "memory.high_limit.critical_temperature",
            "阈值条目不得与 DIMM 读数共用 id"
        );
    }

    #[test]
    fn nvme_composite_is_distinguishable_from_other_disk_temperatures() {
        let composite = metric_id_of(&reading("Ssd", "Composite Temperature", "Temperature"));
        assert_eq!(composite, "storage.composite.temperature");
        let other = metric_id_of(&reading("Ssd", "Temperature", "Temperature"));
        assert_eq!(other, "storage.other.temperature");
        assert_ne!(composite, other, "复合温度与其它盘温必须可区分");
    }

    #[test]
    fn cpu_temperature_semantics_are_kept_apart() {
        let avg = metric_id_of(&reading("Cpu", "Core Average", "Temperature"));
        let pkg = metric_id_of(&reading("Cpu", "CPU Package", "Temperature"));
        let tctl = metric_id_of(&reading("Cpu", "Core (Tctl/Tdie)", "Temperature"));
        let core = metric_id_of(&reading("Cpu", "Core #7", "Temperature"));
        for (got, want) in [
            (&avg, "cpu.core_average.temperature"),
            (&pkg, "cpu.package.temperature"),
            (&tctl, "cpu.tctl.temperature"),
            (&core, "cpu.core.temperature"),
        ] {
            assert_eq!(got, want);
        }
        // 四个语义互不相同 —— 否则阈值型缺陷会换个名字复发
        assert_eq!(
            [avg, pkg, tctl, core]
                .iter()
                .collect::<std::collections::HashSet<_>>()
                .len(),
            4
        );
    }

    #[test]
    fn unknown_sensors_fall_into_misc_and_keep_their_name() {
        // 🔴 未识别 ≠ 丢弃。归入 misc 且保留原名片段，上游新增传感器时
        //   我们能在 F-11 全量表里**看见**，而不是悄悄失效。
        let id = metric_id_of(&reading("AcmeSensor", "Widget Core", "Temperature"));
        assert!(id.starts_with("misc."), "未识别应归 misc：{id}");

        let load = metric_id_of(&reading("GpuIntel", "D3D 3D", "Load"));
        assert!(load.starts_with("misc."), "非温度读数走 misc：{load}");
    }

    #[test]
    fn ids_contain_only_portable_characters() {
        // 上游名字里的空格/斜杠/方括号不得漏进 id，否则它不能当键用。
        let id = metric_id_of(&reading("Ssd", "Temperature (Max)", "Temperature"));
        assert!(
            id.chars()
                .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_')),
            "id 含非法字符：{id}"
        );
        assert!(!id.contains(' '), "id 不得含空格：{id}");
    }

    #[test]
    fn renaming_upstream_does_not_change_the_id() {
        // 🔴 本层的存在意义：上游改名 ⇒ id 不变 ⇒ 判据不失效。
        //   （改前判据直接 `starts_with("dimm")`，改名即失效。）
        let before = metric_id_of(&reading("Memory", "DIMM1", "Temperature"));
        let after = metric_id_of(&reading("Memory", "DIMM1 TSOD", "Temperature"));
        assert_eq!(before, after, "上游在名字后加修饰词不应改变 id");
    }
}
