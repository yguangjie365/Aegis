//! 存储慢变量与介质判定（R-05/R-06/R-12）：`live` 的子模块。

use crate::overview::dto::{clamp_percent, gib_text, DiskHealthRowDto};
use crate::overview::state::StorageSummary;
use infra::smart::{read_disk_smart, SmartInfo};
use infra::wmi::{self, Row};
use sysinfo::{DiskKind, Disks};

/// 盘符/介质分类（R-05 信号链在本批可得的部分，见 `media_of`）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Media {
    Ssd,
    Hdd,
    Unknown,
}

impl Media {
    #[must_use]
    pub fn label(self) -> &'static str {
        match self {
            Self::Ssd => "SSD",
            Self::Hdd => "HDD",
            Self::Unknown => "未知介质",
        }
    }
}

/// R-05 介质判定信号链在本批的**可得片段**：sysinfo 在 Windows 上经
/// `IOCTL_STORAGE_QUERY_PROPERTY / StorageDeviceSeekPenaltyProperty`（寻道惩罚）给出介质，
/// 拿不到时是 `Unknown`。母本链的另外两级（转速、型号关键词启发式）需要物理盘对象，
/// 随片2 的 `Win32_DiskDrive` 采集补齐；🔴 全部未知时不臆断（走文案兜底）。
#[must_use]
pub fn media_of(kind: DiskKind) -> Media {
    match kind {
        DiskKind::SSD => Media::Ssd,
        DiskKind::HDD => Media::Hdd,
        DiskKind::Unknown(_) => Media::Unknown,
    }
}

/// R-06 副标题：按介质聚合容量，固定 SSD→HDD 顺序；零容量类目不出现。
#[must_use]
pub fn media_text(groups: &[(Media, u64)]) -> String {
    let mut parts: Vec<String> = Vec::new();
    for media in [Media::Ssd, Media::Hdd] {
        let total = groups
            .iter()
            .filter(|(kind, _)| *kind == media)
            .map(|(_, bytes)| *bytes)
            .sum::<u64>();
        if total > 0 {
            parts.push(format!("{} {}", media.label(), gib_text(total)));
        }
    }
    parts.join(" + ")
}

/// 多卷已用率取最大（SystemToolkit 口径：统计卡与副文案同源；聚合分母会掩盖单卷爆满）。
#[must_use]
pub fn max_used_percent(volumes: &[(u64, u64)]) -> Option<f64> {
    volumes
        .iter()
        .filter_map(|(total, free)| used_percent_of(*total, *free))
        .reduce(f64::max)
}

/// 单卷已用率；总量为 0 时返回 `None`（🔴 不得当成 0%，R-08）。
#[must_use]
#[allow(clippy::cast_precision_loss)] // 容量超 2^53 B（9 PB）才会失真，本机位宽恒安全
pub fn used_percent_of(total: u64, free: u64) -> Option<f64> {
    if total == 0 {
        return None;
    }
    let used = total.saturating_sub(free) as f64;
    Some(clamp_percent(used / total as f64 * 100.0))
}

/// 挂载点 → 盘符（`C:\` 或 `\\?\D:\` → `C:`/`D:`）；形态不认识时原样返回，便于诊断。
#[must_use]
pub fn drive_letter(mount: &str) -> String {
    let normalized = mount.trim_start_matches(r"\\?\");
    let bytes = normalized.as_bytes();
    if bytes.len() >= 2 && bytes[1] == b':' && bytes[0].is_ascii_alphabetic() {
        normalized[..2].to_string()
    } else {
        normalized.to_string()
    }
}

/// 采集存储慢变量（R-12：只在首次/全量采集时确定，不入 2 秒节拍）。
///
/// 可移动卷（U 盘/光驱）不进汇总：插拔会让存储卡容量在节拍间跳变，与"慢变量"语义冲突；
/// 物理盘逐台列出属片2 详情卡（R-01）。
#[must_use]
pub fn collect_storage() -> StorageSummary {
    let mut groups: Vec<(Media, u64)> = Vec::new();
    let mut volumes: Vec<(u64, u64)> = Vec::new();
    let mut total_bytes = 0_u64;
    let mut free_bytes = 0_u64;
    let mut largest: Option<(u64, String)> = None;
    let mut volume_count = 0_usize;

    for disk in Disks::new_with_refreshed_list().list() {
        if disk.is_removable() {
            continue;
        }
        let total = disk.total_space();
        if total == 0 {
            continue;
        }
        let free = disk.available_space().min(total);
        volume_count += 1;
        total_bytes = total_bytes.saturating_add(total);
        free_bytes = free_bytes.saturating_add(free);
        volumes.push((total, free));
        groups.push((media_of(disk.kind()), total));
        let mount = drive_letter(&disk.mount_point().to_string_lossy());
        let replace = match &largest {
            Some((best, _)) => total > *best,
            None => true,
        };
        if replace {
            largest = Some((total, mount));
        }
    }

    if volume_count == 0 {
        return StorageSummary {
            media_text: String::new(),
            used_percent: 0.0,
            total_bytes: 0,
            free_bytes: 0,
            largest_volume: String::new(),
            volume_count: 0,
            unavailable_reason: Some("未获取到本机卷信息（存储枚举无返回）".to_string()),
        };
    }

    let (total_of_largest, largest_volume) = largest.unwrap_or((0, String::new()));
    let aggregated = media_text(&groups);
    // R-06 回退支：介质全部解析失败 → 最大卷形态（片2 拿到物理盘型号后替换）
    let media_text = if aggregated.is_empty() {
        format!("{} {}", largest_volume, gib_text(total_of_largest))
    } else {
        aggregated
    };

    StorageSummary {
        media_text,
        used_percent: max_used_percent(&volumes).unwrap_or(0.0),
        total_bytes,
        free_bytes,
        largest_volume,
        volume_count,
        unavailable_reason: None,
    }
}

/// F-06 磁盘健康面板采集（慢变量，只在首次/全量采集时更新）。
///
/// # 实现路径
/// 1. WMI `Win32_DiskDrive` 枚举物理盘（型号、MediaType、PNPDeviceID）；
/// 2. 按 `Index` 映射到 `PhysicalDriveN`；
/// 3. 调用 `infra::smart::read_disk_smart` 读取 SMART 数据；
/// 4. 组装为 [`DiskHealthRowDto`] 列表（最多 5 行，不足补空行）。
///
/// 🔴 非管理员运行：SMART 读取可能失败，此时返回空态文案而非静默失败（R-08）。
#[must_use]
pub fn collect_disk_health() -> Vec<DiskHealthRowDto> {
    // 1. WMI 查询物理盘信息
    let wql = "SELECT Index, Model, MediaType, PNPDeviceID FROM Win32_DiskDrive";
    let drives = match wmi::query(wql) {
        Ok(rows) => rows,
        Err(e) => {
            log::warn!("[DISK_HEALTH] WMI 查询失败: {}", e);
            return vec![empty_row(Some("WMI 查询失败（需管理员权限）".to_string()))];
        }
    };

    if drives.is_empty() {
        return vec![empty_row(Some("未检测到存储设备（需管理员权限）".to_string()))];
    }

    let mut rows: Vec<DiskHealthRowDto> = Vec::new();
    for drive in &drives {
        let index = drive.u32("Index").unwrap_or(0);
        let model = drive.text("Model").unwrap_or_else(|| format!("PhysicalDrive{}", index));
        let media_type = drive.text("MediaType").unwrap_or_default();
        let pnp_id = drive.text("PNPDeviceID").unwrap_or_default();

        // 判定是否为 NVMe / SSD
        let is_nvme = is_nvme_drive(&media_type, &model);
        let is_ssd = is_ssd_drive(&media_type, &model);

        // 读取 SMART
        let smart = read_disk_smart(index, is_nvme, is_ssd, &model, &pnp_id);
        rows.push(to_disk_health_row(&model, &smart));
    }

    // 最多 5 行，不足补空行（F-06）
    while rows.len() < 5 && !rows.is_empty() {
        rows.push(empty_row(None));
    }
    rows.truncate(5);

    rows
}

/// 将 SMART 结果转为磁盘健康行
fn to_disk_health_row(model: &str, smart: &SmartInfo) -> DiskHealthRowDto {
    // 统计 SMART 属性数量（用于判断是否有完整数据）
    let smart_attribute_count = if smart.has_smart {
        // 简化：有 SMART 即认为有完整属性（实际数量需解析原始缓冲，此处不暴露）
        1
    } else {
        0
    };

    DiskHealthRowDto {
        device_name: model.to_string(),
        temperature_c: smart.temperature_c,
        smart_attribute_count,
        health_status: smart.status.as_str().to_string(),
        health_percent: smart.life_percent,
        reason: smart.error.clone(),
    }
}

/// 构造空态行（用于无盘或错误场景）
fn empty_row(reason: Option<String>) -> DiskHealthRowDto {
    DiskHealthRowDto {
        device_name: String::new(),
        temperature_c: None,
        smart_attribute_count: 0,
        health_status: "unknown".to_string(),
        health_percent: None,
        reason,
    }
}

/// 判定是否为 NVMe 盘（简化启发式： MediaType 含 "NVMe" 或型号关键词匹配）
fn is_nvme_drive(media_type: &str, model: &str) -> bool {
    media_type.contains("NVMe")
        || model.contains("NVMe")
        || model.contains("Samsung SSD 9") // Samsung NVMe 常见型号
        || model.contains("WD Black SN") // WD NVMe
        || model.contains("Intel SSD 6") // Intel NVMe
}

/// 判定是否为 SSD（简化启发式： MediaType 含 "SSD" 或排除 HDD 关键词）
fn is_ssd_drive(media_type: &str, model: &str) -> bool {
    if media_type.contains("SSD") || media_type.contains("Solid State") {
        return true;
    }
    // 排除机械盘关键词
    if media_type.contains("HDD") || media_type.contains("Hard Disk") {
        return false;
    }
    // 型号关键词（常见 SSD 厂商）
    model.contains("SSD") || model.contains("Samsung SSD") || model.contains("Kingston")
}

