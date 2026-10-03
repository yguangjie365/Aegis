//! 概览域上行 DTO（M-01 片1，2026-10-03）。
//!
//! 字段命名一律 `camelCase`（与前端 `src/types/overview.ts` 对齐；Q-17 `ts-rs` 生成器接入后
//! 由 contracts 单源生成，届时本文件只保留实现侧构造逻辑，🔴 手写镜像类型的禁令见 contracts 头注）。
//!
//! 🔴 设计纪律：**凡"要不要显示"的判定都在 Rust 侧做完**（R-07 回退链、R-08 状态诚实、R-09 徽章分级），
//! 前端只按 `Option`/枚举渲染，不再二次猜测——否则同一口径会在两端各长一遍并漂移。

use serde::Serialize;

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
}
