//! `sys` 域：跨模块系统级命令（域词汇见 04 分册 §三）。

use serde::Serialize;

/// `sys_runtime_ping` 的返回载荷（骨架接线验证用；正式 DTO 随 W0-07 由 contracts 生成）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimePing {
    /// 产品代号（Q-01 定名前 = "Aegis"）
    pub product: &'static str,
    /// crate 版本（应用版本单源 = package.json，K-S05 接线前此处仅镜像）
    pub version: &'static str,
    /// 回显 `CorrelationId`，证明前端包络贯通
    pub correlation_id: String,
}

/// 骨架接线命令：域前缀 `sys`（04 分册 §三 域词汇表）。
///
/// 无副作用只读命令，不触发提权（按需提权红线见 02 专题）。
/// `#[must_use]` 必须放在 `#[tauri::command]` **之前**（宏会原样重放函数与其外层属性）。
#[must_use]
#[tauri::command]
pub fn sys_runtime_ping(envelope: contracts::CmdEnvelope) -> RuntimePing {
    RuntimePing {
        product: "Aegis",
        version: env!("CARGO_PKG_VERSION"),
        correlation_id: envelope.correlation_id,
    }
}
