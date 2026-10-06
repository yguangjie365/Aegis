//! overview 域命令（命名 `<域>_<动词>_<对象>`，01 编码规范；🔴 pub 命令禁放 crate 根，E0255）。
//!
//! 四条命令都是**只读采集**，无危险操作，故不走预览→确认→执行→记录四步（02 专题 §一），
//! 但每次都带 `CorrelationId` 出参，供前端日志串联（O-* 纪律）。
//!
//! 🔴 命令面纪律：
//! - `overview_live_sample` 永不等 sidecar（非阻塞设计见 `live.rs` 头注），故可安全地每 2 秒被调；
//! - 返回值一律 `Result<T, String>`，`Err` 只装用户可读原因（禁静默失败）；
//! - 采集失败**不返回 `Err`** 而返回带 `reason` 的降级 DTO：概览页要求分区独立容错（R-10/R-11），
//!   整块面板消失比数值缺失更糟。只有入参级错误（缺 `CorrelationId`）才走 `Err`。

#![allow(clippy::unused_async)] // Tauri command 约定：async 派发到异步池，命令面统一为将来含 await 留位

use serde::Serialize;
use tauri::State;

use contracts::CmdEnvelope;

use super::dto::{
    HardwareDetailDto, LiveSampleDto, MonitorStatusDto, NetEndpointsDto, SensorRowDto,
    SensorTableDto, StatCardDto, TopProcessesDto,
};
use super::hardware;
use super::sensor_bridge::metric_id_of;
use super::snapshot;
use super::state::OverviewState;

/// 实时概览上行（F-01/F-08/F-10）。
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LiveSampleResult {
    pub correlation_id: String,
    #[serde(flatten)]
    pub sample: LiveSampleDto,
}

/// 进程两榜上行（F-04）。
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TopProcessesResult {
    pub correlation_id: String,
    #[serde(flatten)]
    pub ranking: TopProcessesDto,
}

/// TCP 端点归属上行（F-05）。
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NetEndpointsResult {
    pub correlation_id: String,
    #[serde(flatten)]
    pub endpoints: NetEndpointsDto,
}

/// 传感器通道状态上行（F-12；空面板原因的唯一来源）。
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MonitorStatusResult {
    pub correlation_id: String,
    #[serde(flatten)]
    pub status: MonitorStatusDto,
}

/// 入参守卫：`CorrelationId` 是日志串联的唯一线索，缺失即拒绝（禁"顺手宽容"成无痕调用）。
///
/// # Errors
///
/// 关联 ID 为空/全空白时返回用户可读原因。
fn check_envelope(envelope: &CmdEnvelope) -> Result<(), String> {
    if envelope.correlation_id.trim().is_empty() {
        return Err("缺少 correlationId：概览命令必须带关联 ID 以供日志串联".to_string());
    }
    Ok(())
}

/// 一次节拍采样：页头 + 四卡 + 通道状态。
///
/// # Errors
/// 仅入参级错误：`correlationId` 缺失或全空白。采集本身失败走 DTO 的 `reason` 降级位。
#[tauri::command]
pub async fn overview_live_sample(
    state: State<'_, OverviewState>,
    envelope: CmdEnvelope,
) -> Result<LiveSampleResult, String> {
    check_envelope(&envelope)?;
    let inner = state.shared();
    let sample = super::live::sample(&inner);

    // 🔴 R-12：本命令是 2 秒节拍，**只做快变量**（页头/四卡/通道状态）。
    // 这里曾为了 F-09 快照秒显顺手调了一次 `hardware::collect()` + `write_snapshot()`，
    // 违反 `hardware/collect.rs:11` 的明文纪律（"不得在 2 秒节拍里调用"）——
    // WMI 硬件枚举 + SMART 读取以秒计，每 2 秒同步跑一次会把 Tauri 异步运行时压住。
    // 快照回写已改由前端 `useStaticInfo` 在**静态采集成功时**触发
    // （`writeSnapshot(payload, cards)`，见 `use-overview-live.ts`），语义等价且不在节拍内。

    Ok(LiveSampleResult {
        correlation_id: envelope.correlation_id,
        sample,
    })
}

/// CPU 榜与内存榜（同一轮采样产出，保证两榜是同一时刻的口径）。
///
/// # Errors
///
/// 仅入参级错误：`correlationId` 缺失或全空白。
#[tauri::command]
pub async fn overview_top_processes(
    state: State<'_, OverviewState>,
    envelope: CmdEnvelope,
) -> Result<TopProcessesResult, String> {
    check_envelope(&envelope)?;
    let inner = state.shared();
    Ok(TopProcessesResult {
        correlation_id: envelope.correlation_id,
        ranking: super::processes::top_processes(&inner),
    })
}

/// 本机 IPv4 TCP 端点总数与按进程归属前列。
///
/// # Errors
///
/// 仅入参级错误：`correlationId` 缺失或全空白。表读不到时走 DTO 的 `reason`。
#[tauri::command]
pub async fn overview_net_endpoints(
    state: State<'_, OverviewState>,
    envelope: CmdEnvelope,
) -> Result<NetEndpointsResult, String> {
    check_envelope(&envelope)?;
    let inner = state.shared();
    Ok(NetEndpointsResult {
        correlation_id: envelope.correlation_id,
        endpoints: super::endpoints::net_endpoints(&inner),
    })
}

/// 传感器通道状态（含内核驱动只读探测结果；🔴 不含任何安装/提权动作）。
///
/// # Errors
///
/// 仅入参级错误：`correlationId` 缺失或全空白。通道不可用是正常状态，走 DTO 的 `state`+`reason`。
#[tauri::command]
pub async fn overview_monitor_status(
    state: State<'_, OverviewState>,
    envelope: CmdEnvelope,
) -> Result<MonitorStatusResult, String> {
    check_envelope(&envelope)?;
    let inner = state.shared();
    Ok(MonitorStatusResult {
        correlation_id: envelope.correlation_id,
        status: super::live::monitor_status(&inner),
    })
}

/// 静态上行：硬件详情 + 系统信息两面板（F-02/F-03；片2-e 注册命令时经 `spawn_blocking` 投递到阻塞线程池）。
///
/// 🔴 两类数据同装一个 [`HardwareDetailDto`]：F-09 快照的载体就是这个 DTO（规格 §F-09 明写快照含"系统信息"），
/// F-11 报告的"硬件详情/系统概览"两节也取同一趟上行——拆两条命令会让快照与报告各自少一半。
/// 前端按 `kind`（`os` / `userRegion` 为系统信息，其余为硬件）分流渲染。
///
/// # Errors
///
/// 仅入参级错误：`correlationId` 缺失或全空白。采集失败返回带降级卡的 DTO（R-10），不整体报错。
#[tauri::command]
pub async fn overview_static_info(envelope: CmdEnvelope) -> Result<HardwareDetailDto, String> {
    check_envelope(&envelope)?;
    // 🔴 线程纪律（`hardware/collect.rs:10-13`）：本函数是**同步阻塞**的
    // （每条 WQL 走一次 COM 连接，NVML 首调要加载 dll），async 上下文里直接调会阻塞
    // Tauri 的异步运行时。必须经 `spawn_blocking` 投递到阻塞线程池。
    // ⚠️ 此前这里只写了注释、实际是同步直调 —— `grep spawn_blocking` 在本域只命中注释。
    tauri::async_runtime::spawn_blocking(|| {
        let mut detail = hardware::collect();
        detail.cards.extend(super::system_info::panels());
        Ok(detail)
    })
    .await
    .map_err(|err| format!("静态信息采集任务异常终止: {err}"))?
}

/// 读取快照（F-09 快照秒显）。
///
/// 缺失/损坏/超龄均返回 `None`，调用方走冷启动全量采集。
///
/// # Errors
///
/// 本命令不做入参校验、也不上报读取失败（R-14：快照问题一律退化为冷启动，不得让页面报错），
/// 故恒返回 `Ok`。
#[tauri::command]
pub async fn overview_snapshot_read(
    _envelope: CmdEnvelope,
) -> Result<Option<snapshot::Snapshot>, String> {
    Ok(snapshot::read_snapshot())
}

/// 写入快照（F-09 快照秒显）。
///
/// # Errors
///
/// 序列化或原子写失败时返回原因。
///
/// 🔴 `cards` 可缺省（前端首次回写时若四卡尚无数据）；缺省即写空数组，
/// 下次冷启动四卡自然缺席而不是读到空壳。
#[tauri::command]
pub async fn overview_snapshot_write(
    envelope: CmdEnvelope,
    payload: HardwareDetailDto,
    cards: Option<Vec<StatCardDto>>,
) -> Result<(), String> {
    check_envelope(&envelope)?;
    snapshot::write_snapshot(&payload, cards.as_deref().unwrap_or(&[]))
}

/// F-07 已安装软件清单（慢变量，不进 2 秒节拍）。
///
/// # Errors
///
/// 仅入参级错误：`correlationId` 缺失或全空白。采集失败返回带降级说明的 DTO（R-10），不整体报错。
#[tauri::command]
pub async fn overview_software_list(
    envelope: CmdEnvelope,
) -> Result<super::dto::SoftwareListDto, String> {
    check_envelope(&envelope)?;
    // 🔴 同 `overview_static_info`：注册表枚举是同步阻塞 I/O，须投递到阻塞线程池
    tauri::async_runtime::spawn_blocking(|| {
        let apps = super::software::list_installed_apps();
        let total = apps.len();
        let reason = if apps.is_empty() {
            Some("未检测到已安装的桌面应用（需管理员权限读取注册表）".to_string())
        } else {
            None
        };
        Ok(super::dto::SoftwareListDto {
            apps: apps
                .into_iter()
                .map(|app| super::dto::SoftwareRowDto {
                    name: app.name,
                    version: app.display_version,
                    publisher: app.publisher,
                    install_date: app.install_date,
                    size_bytes: app.estimated_size_bytes,
                })
                .collect(),
            total,
            reason,
        })
    })
    .await
    .map_err(|err| format!("软件清单采集任务异常终止: {err}"))?
}

/// F-11 全量传感器逐条读数（报告的"传感器快照"节取数）。
///
/// # Errors
///
/// 仅入参级错误：`correlationId` 缺失或全空白。本路未读到就返回空数组，
/// 由前端据此**整节省略**该章节（母本 `OverviewReportBuilder.cs` 同口径），不报错。
#[tauri::command]
pub async fn overview_sensor_table(
    state: State<'_, OverviewState>,
    envelope: CmdEnvelope,
) -> Result<SensorTableDto, String> {
    check_envelope(&envelope)?;
    let rows = state
        .shared()
        .bridge()
        .last_raw()
        .map(|readings| {
            readings
                .iter()
                .map(|reading| SensorRowDto {
                    hardware: reading.hardware.clone(),
                    hardware_type: reading.hardware_type.clone(),
                    name: reading.name.clone(),
                    // 🔴 归一化在**此处**算而不是预先缓存：`metric_id` 是纯函数
                    //   （只依赖本条读数的 hardwareType/name/sensorType），
                    //   缓存它就得为 `last_raw` 再存一份派生数据，两份真相会漂移。
                    metric_id: metric_id_of(reading),
                    sensor_type: reading.sensor_type.clone(),
                    value: reading.value,
                    unit: reading.unit.clone(),
                })
                .collect()
        })
        .unwrap_or_default();
    Ok(SensorTableDto { rows })
}

/// F-11 写出报告文件（文本由前端组装，本侧只管落盘）。
///
/// 路径来自前端另存为对话框，用户取消时前端不会调到本命令（零副作用，F-11 预期结果）。
///
/// # Errors
///
/// 入参级错误（`correlationId` 空白、路径为空）或落盘失败原因。
#[tauri::command]
pub async fn overview_report_write(
    envelope: CmdEnvelope,
    path: String,
    content: String,
) -> Result<(), String> {
    check_envelope(&envelope)?;
    if path.trim().is_empty() {
        return Err("报告保存路径为空".to_string());
    }
    super::report::write_report(std::path::Path::new(&path), &content)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn envelope(id: &str) -> CmdEnvelope {
        CmdEnvelope {
            correlation_id: id.to_string(),
            payload: None,
        }
    }

    #[test]
    fn envelope_requires_a_correlation_id() {
        assert!(check_envelope(&envelope("a1b2c3d4")).is_ok());
        assert!(check_envelope(&envelope("  ")).is_err());
        assert!(check_envelope(&envelope("")).is_err());
    }

    #[test]
    fn results_flatten_the_dto_under_one_camel_case_object() {
        // 前端类型按扁平形状写（`src/types/overview.ts`），故上行 JSON 不得多包一层
        let result = MonitorStatusResult {
            correlation_id: "a1b2c3d4".to_string(),
            status: MonitorStatusDto {
                state: super::super::dto::MonitorState::ExeMissing,
                reason: Some("未找到 AegisMonitor.exe".to_string()),
                sidecar_pid: None,
                driver_installed: false,
                driver_version: None,
            },
        };
        let json = serde_json::to_value(&result).expect("命令上行应可序列化");
        assert_eq!(json["correlationId"], "a1b2c3d4");
        assert_eq!(json["state"], "exeMissing");
        assert_eq!(json["driverInstalled"], false);
        assert!(
            json.get("status").is_none(),
            "flatten 后不得再嵌套 status 对象"
        );
    }

    #[test]
    fn ranking_and_endpoints_flatten_too() {
        let ranking = TopProcessesResult {
            correlation_id: "deadbeef".to_string(),
            ranking: TopProcessesDto {
                has_baseline: false,
                cpu: Vec::new(),
                memory: Vec::new(),
                reason: Some("首次采样中".to_string()),
            },
        };
        let json = serde_json::to_value(&ranking).expect("榜单上行应可序列化");
        assert_eq!(json["hasBaseline"], false);
        assert!(json.get("ranking").is_none());

        let endpoints = NetEndpointsResult {
            correlation_id: "deadbeef".to_string(),
            endpoints: NetEndpointsDto {
                total: 0,
                rows: Vec::new(),
                reason: Some("未获取到 TCP 端点".to_string()),
            },
        };
        let json = serde_json::to_value(&endpoints).expect("端点上行走应可序列化");
        assert_eq!(json["total"], 0);
        assert_eq!(json["reason"], "未获取到 TCP 端点");
    }

    /// 真机冒烟：首拍（无任何缓存）就能出可序列化上行，且卡序与通道状态齐备。
    #[cfg(windows)]
    #[test]
    fn live_sample_serializes_on_the_first_tick() {
        let state = OverviewState::new();
        let inner = state.shared();
        // 🔴 命令面冒烟也不得起真子进程：卡数会随传感器到货时机跳动
        inner.bridge().force_exe_missing_for_tests();
        let dto: LiveSampleDto = super::super::live::sample(&inner);
        let result = LiveSampleResult {
            correlation_id: "a1b2c3d4".to_string(),
            sample: dto,
        };
        let json = serde_json::to_value(&result).expect("首拍上行应可序列化");
        assert!(json["cards"].is_array(), "四卡数组必须在场");
        assert!(json["cards"].as_array().expect("cards 应为数组").len() >= 3);
        assert!(json["header"]["computerName"].is_string());
        assert!(json["monitor"]["state"].is_string());
        assert!(json["inFlight"].is_boolean(), "在途标志必须随上行给出");
    }
}
