//! transfer 域 — 文件互传（M-05；规格唯一口径 `Docs/功能规格/05-文件互传.md`）。
//!
//! 分期（用户裁决 2026-10-04）：**片1 = 电脑↔电脑通道**（F-FT-01~03/06~36/54~59），
//! **片2 = 手机浏览器通道**（F-FT-04/05/37~53/60，前置 spike K-S02）。
//! 本批只落片1 的**纯逻辑地基**（无网络、无文件 IO），可独立测试：
//! 原因码 / 文本口径 / 冲突裁决。协议与传输引擎随下一批。
//!
//! 复用结论（DP-1，逐条登记见 `Aegis/DEPENDENCIES.md §三`）：
//! - **原因码口径** ← `Source/SystemToolkit/src/SystemToolkit.Core/FileTransfer/Models/TransferReasonCodes.cs`
//!   （③思路复用：16 个码 + 中文说明逐条对照；GPL-3.0 同源，登记见 ADR-003 §二）。
//!   母本 `Describe()` 对未知码**原样返回不吞**，本实现用 `Option` 表达同一语义。
//! - **文本上限与预览** ← `.../Models/TransferText.cs`（③思路复用：256 KiB / 120 字预览）。
//!   🟠 **一处有意偏离**：母本按 C# `string.Length` 即 **UTF-16 码元**计"字"，
//!   代理对（emoji 等）算 2；本实现按 **Unicode 标量值**（Rust `chars()`）算，emoji 算 1。
//!   理由：与前端 `[...s].length`（JS 码点迭代）**逐值一致**，避免同一串在两端口径不同；
//!   字节数（真正的上限判据）不受影响。详见 `text` 模块头。
//! - **冲突策略与唯一裁决** ← `.../Models/TransferConflictPolicy.cs` + `.../Services/TransferConflictResolver.cs`
//!   （③思路复用：四策略 + " (n)" 后缀 + "每次询问"无闸道时降级为改名）。
//!
//! 🔴 **协议不移植**：母本电脑通道用 `WatsonTcp`（.NET 专有库，Rust 无对应物、且不要求与旧版互通），
//! 帧格式须自行定义——这是片1 最大的工作量，随 `protocol` 模块落地时单列变更记录。
//!
//! 分层纪律：本域只依赖 `contracts` / `infra` 等，🔴 禁持 Tauri 宿主类型（Window / `WebviewWindow` /
//! Menu / Tray）；FFI 一律经 `infra`（02 册 KN-04）；禁 `std::thread::sleep`
//! （`clippy.toml` 禁用清单），等待一律 channel + 限时。
//!
//! 🔴 路径越界守卫的落点**不在本模块**：`02-专题设计/02-安全与特权通道.md:94` 与
//! `infra/src/lib.rs:5` 均把 `infra::safe::join_under_root` 定为「外部路径拼接的**唯一入口**」。
//! 用户 2026-10-04 裁决（选项 A）：由 dsh 认领 W0-09 的路径守卫子集并在 `infra/src/safe.rs` 落地，
//! 本域**消费**该入口，**不得另写一份**。

pub mod admission;
pub mod chunk;
pub mod conflict;
pub mod discovery;
pub mod protocol;
pub mod reasons;
pub mod settings;
pub mod sink;
pub mod source;
pub mod text;

/// 架构守卫（仅测试期编译）：禁止本域直接 `Path::join` 消费外部输入。
/// 规格 02 专题 §5.1 的"依赖审计断言"路线——见该文件头的取舍说明。
#[cfg(test)]
mod guard;
