//! backup 域 — 文件备份（M-06；规格唯一口径 `Docs/功能规格/06-文件备份.md`）。
//!
//! 复用结论（DP-1，完整逐条登记见 `Aegis/.worknotes/changes/2026-10-05-dsh-M06-file-backup-kickoff.md`）：
//! - **③ 思路复用** ← `Source/SystemToolkit/src/SystemToolkit.Core/Backup/`（Contracts/Models/Services 三层，
//!   GPL-3.0 同源）。C#/WPF 代码不可移植（02 册 DP-1 §4），故只取**分层职责与判据**，
//!   按本工程 Rust 约定重写。
//! - **① 直接消费本工程既有**（不另写，🔴 硬纪律）：
//!   路径越界守卫 → `infra::safe`（`join_under_root` / `validate_relative` / `is_lexically_under`），
//!   母本 `Core/Utilities/PathUtil.cs` 是**同一判据的更弱版本**（无 `\\?\` 前缀处理、无符号链接复验）；
//!   原子写 → `infra::fs::atomic_write`；磁盘空间取数 → `infra::fs::available_bytes`（FFI 只能在 infra，02 册 KN-04）；
//!   设文件时间 → `infra::fs::set_modified_time`。
//!
//! 🔴 **本批不做**（K-002 / K-003 未拍板，用户 2026-10-05 裁决「留接口不实现」）：
//! 卷影快照（VSS）、每日定时注册与补做、未变文件复用（依赖前两者）。
//! 见 [`vss`] 与 [`schedule`] 的 trait 边界。
//!
//! 分层纪律：本域🔴 禁持 Tauri 宿主类型（`Window` / `WebviewWindow` / `Menu` / `Tray`，02 册 AD-4）；
//! 🔴 禁 `std::thread::sleep`（`clippy.toml` 禁用清单），等待一律 channel + 限时；
//! 🔴 禁 `unwrap()` / `expect()` 出现在 command 路径（01 册 §1.2）。
//!
//! **并发**：跨域零互引（AD-2）。文件互传域的`transfer::guard` 架构守卫模式（禁止本域直接
//! `Path::join` 消费外部输入）在 [`guard`] 复刻一份。

pub mod admission;
pub mod commands;
pub mod engine;
pub mod hash;
pub mod ids;
pub mod manifest;
pub mod models;
pub mod portability;
pub mod reasons;
pub mod restore;
pub mod retention;
pub mod scan;
pub mod schedule;
pub mod store;
pub mod verify;
pub mod vss;

mod dto;

/// 架构守卫（仅测试期编译）：禁止本域直接 `Path::join` 消费外部输入。
/// 与 `transfer::guard` 同款（02专题 §5.1「依赖审计断言」路线）。
#[cfg(test)]
mod guard;

pub use dto::*;
pub use models::*;
