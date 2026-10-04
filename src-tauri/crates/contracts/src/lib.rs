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

/// 日志 `Module` 字段的**取值登记表**（01 专题 §2.1 的 `Module` 行）。
///
/// 🔴 **逐字与前端 `src/lib/logging/index.ts` 的 `LOG_MODULES` 相同**（含顺序）——
/// 前端那份是"W0-07 前的手工投影"，本常量是它的 Rust 侧对应物。
/// 两侧同值由 `log_modules_match_the_frontend_projection` 用例钉住。
///
/// 规则原文：*"取值限定在 contracts 登记的 `LOG_MODULES` 常量集（八模块 +
/// `ElevatedHelper / Worker / Shell / Security`）…🔴 禁止散落魔法字符串；新增须登记"*。
///
/// 🟠 本常量是 M-05 批为"命令层不散落魔法字符串"而落的；W0-07 落地时应把
/// `LogEntry` / `LogLevel` / `LogResult` 一并放到本 crate（并改由 ts-rs 生成 TS 侧），
/// 届时本数组**原样保留**即可。
pub const LOG_MODULES: [&str; 12] = [
    "Overview",
    "Software",
    "Driver",
    "Network",
    "Transfer",
    "Backup",
    "Music",
    "Steam",
    "ElevatedHelper",
    "Worker",
    "Shell",
    "Security",
];

/// 文件互传域的 `Module` 取值（避免在各命令里写字面量）。
pub const LOG_MODULE_TRANSFER: &str = "Transfer";

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
    use super::{LOG_MODULES, LOG_MODULE_TRANSFER, TRANSFER_REQUIRE_RECEIVE_CONFIRMATION_DEFAULT};

    /// 🔴 `LOG_MODULES` 与**前端手工投影**必须逐字同值同序。
    ///
    /// 直接读前端文件（相对本 crate 的已知路径）而不是抄一份数组——
    /// 抄一份就变成"两份都写死的表"，那就失去了这条用例的意义。
    #[test]
    fn log_modules_match_the_frontend_projection() {
        // CARGO_MANIFEST_DIR = <repo>/Aegis/src-tauri/crates/contracts
        // ⇒ 上三级到 <repo>/Aegis/，再进 src/lib/logging/
        let path = concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../../src/lib/logging/index.ts"
        );
        let source = std::fs::read_to_string(path)
            .unwrap_or_else(|error| panic!("读不到前端日志契约 {path}：{error}"));

        // 从前端文件里把 LOG_MODULES 数组的字面量抽出来（只认形如 "Xxx", 的行）
        let start = source
            .find("export const LOG_MODULES")
            .expect("前端应有 LOG_MODULES");
        let rest = &source[start..];
        let end = rest.find("] as const").expect("数组应以 ] as const 结束");
        let mut frontend: Vec<String> = Vec::new();
        for line in rest[..end].lines() {
            let trimmed = line.trim().trim_end_matches(',');
            if trimmed.starts_with('"') && trimmed.ends_with('"') && trimmed.len() > 2 {
                frontend.push(trimmed.trim_matches('"').to_owned());
            }
        }

        assert_eq!(
            frontend,
            LOG_MODULES.to_vec(),
            "Rust 与 TS 两侧的日志模块表漂移了（含顺序）"
        );
        assert!(
            frontend.contains(&LOG_MODULE_TRANSFER.to_owned()),
            "Transfer 必须在登记表内"
        );
        assert_eq!(LOG_MODULES.len(), 12);
    }

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
