//! contracts — Rust↔TS 契约唯一事实源（04 分册 §一）。
//!
//! 纪律：本 crate 零向上依赖，只依赖 serde——不引用任何业务 crate、不含实现逻辑。
//! TS 侧类型由本 crate 生成（工具链 `ts-rs`，Q-17，ADR 待立于 W0-07 开工前），
//! 🔴 禁止在 `src/types/` 手写镜像类型。
//!
//! W0-01 只落 `CmdEnvelope`（前端 invoke 包装器已在发送该形状）；
//! 命令注册表 / 事件注册表 / `AegisErrorPayload` / 错误码表随 W0-07 落地。

use serde::{Deserialize, Serialize};

/// 文件互传「接收前询问」的**出厂默认值**（Q-15，用户 2026-10-04 裁决）。
///
/// # 🔴 这是全链路唯一的默认值来源
///
/// Q-15 的原始问题（规格 05 册 §七 已知限制 15 原文）**就是两处默认值相反**：
/// *"契约层默认关闭询问，应用界面默认开启询问；重写时需明确一个默认值并在全链路统一"*。
/// 母本实测确实如此——`TransferSettings.cs` 里 `RequireReceiveConfirmation` **没有初始化器**
/// （C# `bool` 默认 `false`），而界面层 `FileTransferDesktopViewModel.cs` 显式写 `= true`。
/// 照抄契约层就会得到与旧版界面**相反**的行为（"新机器装完第一台就能被静默接收"）。
///
/// 故本工程把它收成**一个常量**：后端 `TransferSettings::default()` 取它；
/// 前端设置初值也必须来自后端（🔴 不得自己写 `useState(true)` 之类的字面量）。
/// 改动本值即改动全链路，不会出现"两处各说各话"。
pub const TRANSFER_REQUIRE_RECEIVE_CONFIRMATION_DEFAULT: bool = true;

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

#[cfg(test)]
mod tests {
    use super::TRANSFER_REQUIRE_RECEIVE_CONFIRMATION_DEFAULT;

    /// 🔴 Q-15 的用户裁决（2026-10-04）：**默认开启询问**。改动此值即改动全链路行为，
    /// 且会推翻用户裁决——故用用例把它钉死，任何改动都必须先经过一次显式决策。
    #[test]
    // 常量断言是**刻意**的：本用例就是要把这个出厂值钉成 golden value，
    // 任何改动都必须先经过一次显式决策（见断言消息）。
    #[allow(clippy::assertions_on_constants)]
    fn receive_confirmation_defaults_to_on_per_q15_user_ruling() {
        assert!(
            TRANSFER_REQUIRE_RECEIVE_CONFIRMATION_DEFAULT,
            "Q-15 裁决是「默认开启询问」（安全侧：避免新机器装完就被静默接收）；\
             若确需改为 false，请先回看 TASKS Q-15 行与规格 05 册 §七 限制 15"
        );
    }
}
