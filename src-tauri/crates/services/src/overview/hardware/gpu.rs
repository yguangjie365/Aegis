//! 显卡详情卡（F-02 · 规格 §五「显卡（×n）」行集：型号/厂商/温度/负载/显存/分辨率/驱动版本，片2-b）。
//!
//! **复用结论（DP-1）**：WQL 与虚拟显卡过滤链取自 `Source/NexBox/src-tauri/src/hardware.rs:959-1053`
//! `get_gpus_static_from_wmi()`，名称匹配走 `:1188 normalize_gpu_name()`（②档改造移植，登记见
//! `DEPENDENCIES.md §三`）；显存总量与驱动版本走 [`infra::nvml`]（母本 `:741` 的 NVML 路径）。
//!
//! 🔴 **K-004 边界（写这个文件前必读）**：本文件只补**静态规格项**。`温度`、`负载` 两行恒 `None`——
//! 二者唯一来源是 C# sidecar（`crate::overview::sensor_bridge`，经 NVAPI/NVML 但不进本路径），由片2-e 的
//! 快照装配注入；母本在同一个 NVML 句柄上顺手读 `temperature`（`:769`）与 `utilization_rates`（`:773`）
//! 的写法🔴 **不移植**，`catch_unwind`（`:1073`/`:1228`/`:1571`）也不移植，改按 `Result` 走 R-08。
//!
//! 🔴 与母本的另外三处偏离：
//! 1. **显存回退链**：NVML 命中优先；未命中才用 WMI `AdapterRAM`，且顶格值 `0xFFFFFFFF`
//!    （>4 GiB 的设备 WMI 一律截断到这里）判 `None`。母本直接 `÷1024³`，会把 8 GB 卡报成 "4.0 GB"
//!    ——R-08 不许给错数，宁缺毋滥；
//! 2. 母本一次 `SELECT` 16 列，其中 `VideoArchitecture`/`VideoMemoryType`/`InstalledDisplayDrivers`/
//!    `VideoModeDescription`/`CurrentRefreshRate`/`DeviceID` 在规格 §五 无展示位，本域不收（少采一列
//!    就少一处指纹面）；
//! 3. 缺失值一律 `None`，不是母本的 `"未知"` 串（理由见 [`super::rows`] 头注）。

use infra::wmi::Row;

use super::rows::kv;
use crate::overview::dto::{gib_text, DetailCardDto, DetailKind};

/// 规格 §五 用得上的列（见头注偏离 2）。
const WQL: &str = "SELECT Name, AdapterCompatibility, AdapterRAM, DriverVersion, \
                   CurrentHorizontalResolution, CurrentVerticalResolution, \
                   PNPDeviceID, Status FROM Win32_VideoController";

/// 虚拟显卡厂商 PCI ID 黑名单（VMware / `VirtualBox` / Hyper-V / QEMU 等，母本 `:704` 照搬）。
const VIRTUAL_GPU_VENDORS: [&str; 6] = [
    "VEN_15AD", "VEN_80EE", "VEN_1414", "VEN_1234", "VEN_1AF4", "VEN_1B36",
];

/// 已知虚拟显示 / 远程串流产品名（母本 `:716` 13 条照搬，作结构化过滤之外的兜底）。
const VIRTUAL_GPU_NAME_KEYWORDS: [&str; 13] = [
    "mu mu virtual display",
    "mumu virtual display",
    "gameviewer virtual display",
    "parsec",
    "sunflower",
    "oray",
    "todesk",
    "rustdesk",
    "virtual display",
    "remote display",
    "iddcx",
    "usb mobile monitor",
    "虚拟机",
];

/// `AdapterRAM` 的截断顶格值：`u32::MAX` 字节。WMI 该列是 uint32，>4 GiB 一律报到这里。
const ADAPTER_RAM_CEILING: u64 = u32::MAX as u64;

/// 显卡卡（每物理 PCI 显卡一张，R-01 全列；过滤后为空时出一张降级卡，R-10 卡位不消失）。
#[must_use]
pub fn cards() -> Vec<DetailCardDto> {
    let rows = match infra::wmi::query(WQL) {
        Ok(rows) => rows,
        Err(reason) => return vec![degraded(&reason)],
    };
    let physical: Vec<&Row> = rows.iter().filter(|row| is_visible_gpu(row)).collect();
    if physical.is_empty() {
        return vec![degraded(
            "WMI 未返回可展示的显卡（Win32_VideoController 无物理 PCI 显示适配器）",
        )];
    }

    // NVML 只在有 NVIDIA 卡时才值得加载 dll；失败原因不上屏（WMI 行仍完整），交片2-e 记诊断日志。
    let specs = if physical.iter().any(|row| is_nvidia_row(row)) {
        infra::nvml::nvidia_specs().unwrap_or_default()
    } else {
        Vec::new()
    };

    physical
        .iter()
        .enumerate()
        .map(|(index, row)| {
            let spec = specs.iter().find(|spec| names_match(&spec.name, row));
            card_of(index, row, spec)
        })
        .collect()
}

/// 该行是否 NVIDIA 卡（按名称判，与 [`vendor_label`] 同一张表）。
fn is_nvidia_row(row: &Row) -> bool {
    row.text("Name")
        .and_then(|name| vendor_label(&name).map(ToString::to_string))
        .as_deref()
        == Some("NVIDIA")
}

/// 显卡名归一（去空白 + 小写，母本 `:1188`）：WMI 与 NVML 的名字只差空格与大小时仍能对上。
fn normalize_name(name: &str) -> String {
    name.chars()
        .filter(|c| !c.is_whitespace())
        .flat_map(char::to_lowercase)
        .collect()
}

/// NVML 条目与本行的匹配键：行名优先，名字缺失时退回 NVML 列表第一条（单卡机器常见形态）。
fn names_match(spec_name: &str, row: &Row) -> bool {
    let Some(row_name) = row.text("Name") else {
        return false;
    };
    !row_name.is_empty() && normalize_name(spec_name) == normalize_name(&row_name)
}

/// 展示口径过滤链（母本 `:977-1002` 三段照搬：占位设备 → 非物理 PCI → 状态异常）。
fn is_visible_gpu(row: &Row) -> bool {
    let Some(name) = row.text("Name") else {
        return false;
    };
    if name.to_lowercase().contains("basic display") {
        return false;
    }
    let device_id = row.text("PNPDeviceID").unwrap_or_default();
    if !is_physical_pci_gpu(&device_id) {
        return false;
    }
    if is_virtual_gpu_by_name(&name) {
        return false;
    }
    if let Some(status) = row.text("Status") {
        let lowered = status.to_lowercase();
        if lowered.contains("error") || lowered.contains("degraded") {
            return false;
        }
    }
    true
}

/// 物理 PCI 显卡：实例路径必须含 `PCI\VEN_` 且厂商不在虚拟黑名单（母本 `:726`）。
///
/// 基础显示适配器、RDP 远程显示、`SWD\` 软件设备、MuMu/GameViewer 等 IDD 虚拟显示器
/// 都是 `ROOT\...` / `SWD\...` 路径，由本判定直接拦掉。
#[must_use]
pub fn is_physical_pci_gpu(device_id: &str) -> bool {
    let upper = device_id.to_uppercase();
    upper.contains(r"PCI\VEN_") && !VIRTUAL_GPU_VENDORS.iter().any(|ven| upper.contains(ven))
}

/// 名称黑名单兜底：以 PCI 路径出现的已知虚拟显示产品名同样拦掉（母本 `:736`）。
#[must_use]
pub fn is_virtual_gpu_by_name(name: &str) -> bool {
    let lowered = name.to_lowercase();
    VIRTUAL_GPU_NAME_KEYWORDS
        .iter()
        .any(|keyword| lowered.contains(keyword))
}

/// 按名称判厂商（母本 `detect_gpu_vendor` 的分支照搬；未知档返回 `None` 而非 `"Unknown"`）。
#[must_use]
pub fn vendor_label(name: &str) -> Option<&'static str> {
    let lowered = name.to_lowercase();
    if lowered.contains("nvidia")
        || lowered.contains("geforce")
        || lowered.contains("gtx")
        || lowered.contains("rtx")
    {
        return Some("NVIDIA");
    }
    if lowered.contains("amd") || lowered.contains("radeon") || lowered.contains("rx ") {
        return Some("AMD");
    }
    if lowered.contains("intel") {
        return Some("Intel");
    }
    None
}

fn card_of(index: usize, row: &Row, spec: Option<&infra::nvml::NvmlGpuSpec>) -> DetailCardDto {
    let name = row.text("Name");
    let vendor = row.text("AdapterCompatibility").or_else(|| {
        name.as_deref()
            .and_then(vendor_label)
            .map(ToString::to_string)
    });
    DetailCardDto {
        kind: DetailKind::Gpu,
        index,
        reason: None,
        rows: vec![
            kv("detail.gpu.model", name),
            kv("detail.gpu.vendor", vendor),
            // 🔴 K-004：温度/负载只来自 sidecar，本批恒空（片2-e 装配时覆盖这两行）
            kv("detail.gpu.temperature", None),
            kv("detail.gpu.utilization", None),
            kv("detail.gpu.memory", memory_text(row, spec)),
            kv("detail.gpu.resolution", resolution_text(row)),
            kv(
                "detail.gpu.driverVersion",
                spec.and_then(|spec| spec.driver_version.clone())
                    .or_else(|| row.text("DriverVersion")),
            ),
        ],
    }
}

/// 显存：NVML 总量优先，未命中退回 `AdapterRAM`；顶格截断值判 `None`（头注偏离 1）。
fn memory_text(row: &Row, spec: Option<&infra::nvml::NvmlGpuSpec>) -> Option<String> {
    if let Some(total) = spec.and_then(|spec| spec.memory_total_bytes) {
        return (total > 0).then(|| gib_text(total));
    }
    let adapter_ram = row.u64("AdapterRAM").filter(|value| *value > 0)?;
    if adapter_ram >= ADAPTER_RAM_CEILING {
        return None;
    }
    Some(gib_text(adapter_ram))
}

/// 当前分辨率：宽×高；任一维缺失或为 0 即 `None`（未接显示器时的 WMI 形态）。
fn resolution_text(row: &Row) -> Option<String> {
    let width = row
        .u32("CurrentHorizontalResolution")
        .filter(|value| *value > 0)?;
    let height = row
        .u32("CurrentVerticalResolution")
        .filter(|value| *value > 0)?;
    Some(format!("{width} × {height}"))
}

#[must_use]
fn degraded(reason: &str) -> DetailCardDto {
    DetailCardDto {
        kind: DetailKind::Gpu,
        index: 0,
        reason: Some(reason.to_string()),
        rows: Vec::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use infra::wmi::Value;

    fn row(pairs: &[(&str, Value)]) -> Row {
        Row::from_pairs(pairs)
    }

    #[test]
    fn virtual_and_root_adapters_are_filtered() {
        let basic = row(&[
            (
                "Name",
                Value::Text("Microsoft Basic Display Adapter".to_string()),
            ),
            ("PNPDeviceID", Value::Text("PCI\\VEN_8086".to_string())),
        ]);
        assert!(!is_visible_gpu(&basic));

        let hyper_v = row(&[
            ("Name", Value::Text("Hyper-V Display Adapt".to_string())),
            (
                "PNPDeviceID",
                Value::Text("PCI\\VEN_1414&DEV_0037".to_string()),
            ),
        ]);
        assert!(!is_visible_gpu(&hyper_v), "虚拟厂商 PCI ID 必须拦掉");

        let idd = row(&[
            ("Name", Value::Text("MuMu Virtual Display".to_string())),
            ("PNPDeviceID", Value::Text("ROOT\\DISPLAY\\0".to_string())),
        ]);
        assert!(!is_visible_gpu(&idd));

        let real = row(&[
            ("Name", Value::Text("NVIDIA GeForce RTX 4070".to_string())),
            (
                "PNPDeviceID",
                Value::Text("PCI\\VEN_10DE&DEV_2705".to_string()),
            ),
            ("Status", Value::Text("OK".to_string())),
        ]);
        assert!(is_visible_gpu(&real));

        let broken = row(&[
            ("Name", Value::Text("AMD Radeon RX 7800 XT".to_string())),
            (
                "PNPDeviceID",
                Value::Text("PCI\\VEN_1002&DEV_7480".to_string()),
            ),
            ("Status", Value::Text("Error".to_string())),
        ]);
        assert!(!is_visible_gpu(&broken), "状态异常设备不展示");
    }

    #[test]
    fn vendor_detection_folds_unknown_to_missing() {
        assert_eq!(vendor_label("NVIDIA GeForce RTX 4070"), Some("NVIDIA"));
        assert_eq!(vendor_label("AMD Radeon(TM) Graphics"), Some("AMD"));
        assert_eq!(vendor_label("Intel(R) UHD Graphics 770"), Some("Intel"));
        assert_eq!(
            vendor_label("Some Clone Adapter"),
            None,
            "🔴 不返回\"Unknown\"占位串"
        );
    }

    #[test]
    fn nvml_name_matches_wmi_despite_spacing_and_case() {
        let wmi_row = row(&[("Name", Value::Text("NVIDIA GeForce RTX 4070".to_string()))]);
        assert!(names_match("nvidia  geforce rtx 4070", &wmi_row));
        let unnamed = row(&[]);
        assert!(!names_match("NVIDIA GeForce RTX 4070", &unnamed));
    }

    #[test]
    fn nvml_memory_wins_and_adapter_ram_ceiling_is_refused() {
        let row_with_ram = row(&[("AdapterRAM", Value::Unsigned(4_294_967_295))]);
        assert_eq!(
            memory_text(&row_with_ram, None),
            None,
            ">4 GiB 被 WMI 截断成顶格值，不得报成 4.0 GB"
        );
        let row_with_nvml = row(&[("AdapterRAM", Value::Unsigned(4_294_967_295))]);
        let spec = infra::nvml::NvmlGpuSpec {
            name: "NVIDIA GeForce RTX 4070".to_string(),
            memory_total_bytes: Some(12_884_901_888),
            driver_version: Some("551.86".to_string()),
        };
        assert_eq!(
            memory_text(&row_with_nvml, Some(&spec)).as_deref(),
            Some("12.0 GB"),
            "显存以 NVML 为准"
        );
        let real_2g = row(&[("AdapterRAM", Value::Unsigned(2 * 1024_u64.pow(3)))]);
        assert_eq!(memory_text(&real_2g, None).as_deref(), Some("2.0 GB"));
        assert_eq!(memory_text(&row(&[]), None), None);
    }

    #[test]
    fn resolution_needs_both_dimensions() {
        let full = row(&[
            ("CurrentHorizontalResolution", Value::Unsigned(2560)),
            ("CurrentVerticalResolution", Value::Unsigned(1440)),
        ]);
        assert_eq!(resolution_text(&full).as_deref(), Some("2560 × 1440"));
        let half = row(&[("CurrentHorizontalResolution", Value::Unsigned(2560))]);
        assert_eq!(resolution_text(&half), None);
        let zero = row(&[
            ("CurrentHorizontalResolution", Value::Unsigned(0)),
            ("CurrentVerticalResolution", Value::Unsigned(0)),
        ]);
        assert_eq!(resolution_text(&zero), None, "没接显示器不是 0 × 0");
    }

    #[test]
    fn temperature_and_utilization_stay_empty_in_this_slice() {
        // 🔴 K-004 回归闸：本批若在显卡卡里塞进温度/负载值，就是越界
        let row_data = row(&[("Name", Value::Text("NVIDIA GeForce RTX 4070".to_string()))]);
        let card = card_of(0, &row_data, None);
        let value_of = |key: &str| {
            card.rows
                .iter()
                .find(|item| item.key == key)
                .and_then(|item| item.value.clone())
        };
        assert_eq!(value_of("detail.gpu.temperature"), None);
        assert_eq!(value_of("detail.gpu.utilization"), None);
        assert_eq!(card.kind, DetailKind::Gpu);
        assert_eq!(card.rows.len(), 7, "规格 §五 的显卡行集是 7 项");
    }
}
