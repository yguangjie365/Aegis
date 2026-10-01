//! contracts — Rust↔TS 契约唯一事实源（04 分册 §一）。
//!
//! 纪律：本 crate 零向上依赖，只依赖 serde——不引用任何业务 crate、不含实现逻辑。
//! TS 侧类型由本 crate 生成（工具链 `ts-rs`，Q-17，ADR 待立于 W0-07 开工前），
//! 🔴 禁止在 `src/types/` 手写镜像类型。
//!
//! W0-01 只落 `CmdEnvelope`（前端 invoke 包装器已在发送该形状）；
//! 命令注册表 / 事件注册表 / `AegisErrorPayload` / 错误码表随 W0-07 落地。

use serde::{Deserialize, Serialize};

/// 统一 command 参数包络：前端 `src/lib/tauri/invoke.ts` 以 `{ envelope: ... }` 传递。
/// `CorrelationId` 一次用户操作一个 ID，贯穿前端 → command → 日志总线 → helper（01 总览 §5.2）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CmdEnvelope {
    /// `CorrelationId`（前端生成的短 ID；日志与事件按此串联）
    pub correlation_id: String,
    /// 具体命令的载荷；无参命令收到 `null`
    #[serde(default)]
    pub payload: Option<serde_json::Value>,
}
