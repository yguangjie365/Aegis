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

use super::dto::{LiveSampleDto, MonitorStatusDto, NetEndpointsDto, TopProcessesDto};
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
    Ok(LiveSampleResult {
        correlation_id: envelope.correlation_id,
        sample: super::live::sample(&inner),
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
