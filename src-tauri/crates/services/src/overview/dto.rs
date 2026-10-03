//! 概览域上行 DTO（M-01 片1，2026-10-03）。
//!
//! 字段命名一律 `camelCase`（与前端 `src/types/overview.ts` 对齐；Q-17 `ts-rs` 生成器接入后
//! 由 contracts 单源生成，届时本文件只保留实现侧构造逻辑，🔴 手写镜像类型的禁令见 contracts 头注）。
//!
//! 🔴 设计纪律：**凡"要不要显示"的判定都在 Rust 侧做完**（R-07 回退链、R-08 状态诚实、R-09 徽章分级），
//! 前端只按 `Option`/枚举渲染，不再二次猜测——否则同一口径会在两端各长一遍并漂移。

use serde::{Deserialize, Serialize};

/// 一次实时节拍的上行包（`overview_live_sample` 载荷）。
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LiveSampleDto {
    pub header: HeaderDto,
    /// 实时概览卡（R-01：多显卡/多盘一律全列；R-07：无意义的卡不创建）
    pub cards: Vec<StatCardDto>,
    pub monitor: MonitorStatusDto,
    /// R-13 重入闸门：本拍是否有采集在途（true 时前端跳过并给出原因）
    pub in_flight: bool,
    /// F-06 磁盘健康面板（慢变量，只在首次/全量采集时更新）
    pub disk_health: Vec<DiskHealthRowDto>,
}

/// F-07 已安装软件清单一行
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SoftwareRowDto {
    pub name: String,
    pub version: Option<String>,
    pub publisher: Option<String>,
    pub install_date: Option<String>,
    /// 估计占用字节数（前端格式化为 KB/MB/GB）
    pub size_bytes: Option<u64>,
}

/// F-07 软件清单上行
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SoftwareListDto {
    pub apps: Vec<SoftwareRowDto>,
    /// 过滤后的应用总数
    pub total: usize,
    /// 空态原因（R-08：无数据必写为什么是空的）
    pub reason: Option<String>,
}

/// 页头条信息项（规格 §五 页头；快照状态随片2 追加字段）。
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HeaderDto {
    pub computer_name: String,
    pub os_name: String,
    /// 运行时长秒数（格式化与单位文案归前端 i18n，避免后端长出一份中文串）
    pub uptime_secs: u64,
}

/// 卡片类别（前端据此选图标与标题）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum CardKind {
    Cpu,
    Gpu,
    Memory,
    Storage,
}

/// 占用大数字的来源（R-07 回退链的哪一级，前端据此出占用说明）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum ValueSource {
    /// 一级：系统占用率（sysinfo）
    Usage,
    /// 二级：传感器负载值（sidecar）
    SensorLoad,
    /// 三级：温度顶位（保证卡片不空白，仍属诚实降级：徽章同步显示同一温度）
    Temperature,
}

/// 温度徽章状态（R-09）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum BadgeState {
    /// 有读数
    Measured,
    /// 该硬件有温度语义但本次无证据 → 前端出"未检测"中性样式（R-08）
    Undetected,
    /// 该硬件在本次环境无温度来源 → 🔴 隐藏徽章而非显示占位文字（R-09）
    NotApplicable,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum BadgeLevel {
    Normal,
    Warning,
    Critical,
}

#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TempBadgeDto {
    pub state: BadgeState,
    pub temperature_c: Option<f64>,
    pub level: Option<BadgeLevel>,
}

/// 实时概览卡（F-01）。
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StatCardDto {
    pub kind: CardKind,
    /// 同类多设备的序号（R-01；单值卡恒 0）
    pub index: usize,
    /// 精简型号（R-21）
    pub label: String,
    /// 副标题（内存=容量汇总，存储=按介质聚合容量，R-06）
    pub detail: String,
    /// 0–100 整数占用值；`None` = 本项无数据（R-08，前端出破折号）
    pub big_value: Option<u8>,
    pub value_source: Option<ValueSource>,
    /// 进度条百分比（与 `big_value` 同源，允许小数以免 0.4% 显示为 0）
    pub bar_percent: Option<f64>,
    pub badge: TempBadgeDto,
    /// L-08 卡内一行降级原因（整卡降级时给出，卡不消失、卡高不变）
    pub degrade_reason: Option<String>,
}

/// 传感器通道（sidecar）状态（N-06 降级留痕的用户可见面）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum MonitorState {
    /// 已就绪，读数可用
    Ready,
    /// 首次接入中（含硬件通道 15 秒兜底等待）
    Starting,
    /// 失败后的冷却期
    CoolingDown,
    /// 连败达上限，本次会话不再重试
    Disabled,
    /// 未找到 sidecar 可执行文件
    ExeMissing,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MonitorStatusDto {
    pub state: MonitorState,
    /// 用户可读原因（🔴 禁止空泛的"不可用"，须带具体故障形态）
    pub reason: Option<String>,
    pub sidecar_pid: Option<u32>,
    /// 内核驱动只读探测结果（Q-12=B 零分发路线：仅检测，不安装）
    pub driver_installed: bool,
    pub driver_version: Option<String>,
}

/// 进程榜一行（F-04；`None` 值 = 宁缺毋滥，R-17）。
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProcessRowDto {
    pub rank: usize,
    pub name: String,
    pub pid: u32,
    /// 占整机百分比（已按逻辑核数归一，与任务管理器一致，R-17）
    pub cpu_percent: Option<f64>,
    pub memory_mb: Option<f64>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TopProcessesDto {
    /// false = 首拍无差分基线（R-08：前端出破折号 + "首次采样中…"，🔴 不得显示 0%）
    pub has_baseline: bool,
    pub cpu: Vec<ProcessRowDto>,
    pub memory: Vec<ProcessRowDto>,
    pub reason: Option<String>,
}

/// 端点归属一行（F-05：按进程归属的端点数前列）。
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EndpointRowDto {
    pub rank: usize,
    pub process_name: String,
    pub endpoint_count: usize,
    /// 监听端口摘要（去重升序，逗号分隔；无监听则为空串）
    pub listen_ports: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NetEndpointsDto {
    pub total: usize,
    pub rows: Vec<EndpointRowDto>,
    /// 空面板原因（R-08：空态必写为什么是空的）
    pub reason: Option<String>,
}

/// 详情卡类目（规格 §五「详细信息」的逐卡键值行；F-02，片2-b）。
///
/// 🔴 类目名与前端 i18n 的键一一对应：新增类目必须同时改 `src/locales/zh-CN/overview.ts`，
/// 否则前端只会退化成裸键名。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum DetailKind {
    Cpu,
    Gpu,
    Storage,
    Memory,
    Board,
    Bios,
    Network,
    Audio,
    /// 随片2-c（`infra::display` + EDID）上屏，本批不产出
    Display,
    /// 系统信息：操作系统面板（片2-d）
    Os,
    /// 系统信息：用户/区域面板（片2-d）
    UserRegion,
}

/// 详情卡的一行键值（F-02）。
///
/// `key` 是 i18n 键（🔴 不是中文文案，文案归前端，与 `HeaderDto` 同一口径）；
/// `value = None` 一律渲染成"未检测"（R-08），🔴 不得用型号或容量冒充缺失值。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct KvRowDto {
    pub key: String,
    pub value: Option<String>,
}

/// 一张详情卡（F-02）。同类多设备按 `index` 全列（R-01），单值卡恒 0。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DetailCardDto {
    pub kind: DetailKind,
    pub index: usize,
    pub rows: Vec<KvRowDto>,
    /// 整卡降级原因（R-08/L-08：类目采集失败时卡仍在、行仍空，🔴 不得让面板整体消失）
    pub reason: Option<String>,
}

/// 静态硬件详情上行（F-02；`overview_static_info` 载荷，命令面随片2-e 注册）。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HardwareDetailDto {
    pub cards: Vec<DetailCardDto>,
    /// 采集时刻（Unix 毫秒）；供片2-e 快照的新鲜度与指纹比对
    pub collected_at_ms: u64,
}

/// 磁盘健康一行（F-06：设备名、最高温度、SMART 属性数）。
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DiskHealthRowDto {
    pub device_name: String,
    /// 温度（摄氏度）；无 SMART 时也可能有盘温
    pub temperature_c: Option<i32>,
    /// SMART 属性数量（用于判断是否有完整 SMART 数据）
    pub smart_attribute_count: usize,
    /// 健康状态（healthy/warning/unhealthy/unknown）
    pub health_status: String,
    /// 健康度百分比 0-100（None = 无法判定）
    pub health_percent: Option<u8>,
    /// 失败原因（R-08：空态必写为什么是空的）
    pub reason: Option<String>,
}

/// 把任意浮点占用率钳制到 0–100（R-07 一律钳制）。
#[must_use]
pub fn clamp_percent(value: f64) -> f64 {
    if value.is_nan() {
        return 0.0;
    }
    value.clamp(0.0, 100.0)
}

/// 占用率转大整数（四舍五入后仍受 0–100 约束）。
#[must_use]
#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)] // 入参已钳到 0–100，取整必不溢出
pub fn to_big_value(value: f64) -> u8 {
    clamp_percent(value).round() as u8
}

/// 字节 → GiB 一位小数（容量展示单源；规格 §五 的 "476.9 GB" 形态即此）。
#[must_use]
#[allow(clippy::cast_precision_loss)] // 容量只展示一位小数，字节的低位精度对结果无影响
pub fn gib_text(bytes: u64) -> String {
    const GIB: f64 = 1024.0 * 1024.0 * 1024.0;
    let gib = bytes as f64 / GIB;
    if gib >= 1024.0 {
        let tb = gib / 1024.0;
        format!("{tb:.1} TB")
    } else {
        format!("{gib:.1} GB")
    }
}

/// 字节 → MB 一位小数（进程工作集）。
#[must_use]
#[allow(clippy::cast_precision_loss)] // 同 `gib_text`
pub fn mb_value(bytes: u64) -> f64 {
    bytes as f64 / (1024.0 * 1024.0)
}

/// 一行键值（`value` 为 `None` 时前端出"未检测"，R-08）。
#[must_use]
pub fn kv(key: &str, value: Option<String>) -> KvRowDto {
    KvRowDto {
        key: key.to_string(),
        value,
    }
}

/// 一行键值（直接给字符串，空串当没取到）。
#[must_use]
pub fn kv_text(key: &str, value: Option<&str>) -> KvRowDto {
    kv(
        key,
        value
            .map(str::trim)
            .filter(|text| !text.is_empty())
            .map(ToString::to_string),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn percent_is_clamped_to_zero_hundred() {
        assert_eq!(clamp_percent(-3.0), 0.0);
        assert_eq!(clamp_percent(37.5), 37.5);
        assert_eq!(clamp_percent(250.0), 100.0);
        assert!(clamp_percent(f64::NAN).is_finite());
    }

    #[test]
    fn big_value_rounds_then_clamps() {
        assert_eq!(to_big_value(43.6), 44);
        assert_eq!(to_big_value(-1.0), 0);
        assert_eq!(to_big_value(999.0), 100);
    }

    #[test]
    fn gib_text_switches_to_tb_above_one_tebibyte() {
        assert_eq!(gib_text(512 * 1024 * 1024 * 1024), "512.0 GB");
        assert_eq!(gib_text(2 * 1024_u64.pow(4)), "2.0 TB");
        assert_eq!(gib_text(0), "0.0 GB");
    }

    #[test]
    fn mb_value_converts_bytes() {
        assert!((mb_value(1024 * 1024) - 1.0).abs() < 1e-9);
    }

    #[test]
    fn dto_serializes_in_camel_case() {
        let header = HeaderDto {
            computer_name: "DESKTOP-AEGIS".to_string(),
            os_name: "Windows 11".to_string(),
            uptime_secs: 12,
        };
        let json = serde_json::to_value(&header).expect("HeaderDto 应可序列化");
        assert_eq!(json["computerName"], "DESKTOP-AEGIS");
        assert_eq!(json["osName"], "Windows 11");
        assert_eq!(json["uptimeSecs"], 12);
    }

    #[test]
    fn enums_serialize_as_camel_case_tags() {
        assert_eq!(
            serde_json::to_value(CardKind::Cpu).expect("card kind 应可序列化"),
            "cpu"
        );
        assert_eq!(
            serde_json::to_value(BadgeState::NotApplicable).expect("badge state 应可序列化"),
            "notApplicable"
        );
        assert_eq!(
            serde_json::to_value(ValueSource::SensorLoad).expect("value source 应可序列化"),
            "sensorLoad"
        );
    }

    #[test]
    fn detail_dto_serializes_in_camel_case() {
        let card = DetailCardDto {
            kind: DetailKind::Gpu,
            index: 0,
            reason: None,
            rows: vec![KvRowDto {
                key: "detail.gpu.model".to_string(),
                value: Some("NVIDIA GeForce RTX 4070".to_string()),
            }],
        };
        let json = serde_json::to_value(&card).expect("DetailCardDto 应可序列化");
        assert_eq!(json["kind"], "gpu");
        assert_eq!(json["index"], 0);
        assert_eq!(json["reason"], serde_json::Value::Null);
        let first_row = &json["rows"][0];
        assert_eq!(first_row["key"], "detail.gpu.model");
        assert_eq!(first_row["value"], "NVIDIA GeForce RTX 4070");
    }

    #[test]
    fn hardware_detail_dto_has_collected_at_ms() {
        let detail = HardwareDetailDto {
            cards: Vec::new(),
            collected_at_ms: 1_728_000_000_000,
        };
        let json = serde_json::to_value(&detail).expect("HardwareDetailDto 应可序列化");
        assert_eq!(json["collectedAtMs"], serde_json::Value::Number(serde_json::Number::from(1_728_000_000_000_i64)));
        assert_eq!(json["cards"], serde_json::Value::Array(Vec::new()));
    }

    #[test]
    fn detail_kind_variants_are_camel_case() {
        for (variant, expected) in [
            (DetailKind::Cpu, "cpu"),
            (DetailKind::Gpu, "gpu"),
            (DetailKind::Storage, "storage"),
            (DetailKind::Memory, "memory"),
            (DetailKind::Board, "board"),
            (DetailKind::Bios, "bios"),
            (DetailKind::Network, "network"),
            (DetailKind::Audio, "audio"),
            (DetailKind::Display, "display"),
        ] {
            assert_eq!(
                serde_json::to_value(variant).expect("detail kind 应可序列化"),
                expected
            );
        }
    }
}
