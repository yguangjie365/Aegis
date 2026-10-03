//! infra — 平台与通用能力层（04 分册 §一）。
//!
//! 计划落点（不在本骨架内实现）：
//! - ✅ 原子写最小版已落 `fs::atomic_write`（W0-09 提前子集，随 MU-1 落地）
//! - `safe::join_under_root` 路径越界守卫——外部路径拼接唯一入口（W0-09，02 专题 §五）
//! - DPAPI / Windows 凭据管理器封装（凭据永不明文落盘）
//! - SQLite 访问 + `migrations/`（W0-19；🔴 业务代码禁建表）
//! - tracing JSONL 日志总线落盘（W0-10）
//! - 命名管道 IPC 原语（helper 通道，W0-08）
//!
//! 🔴 依赖方向：infra → contracts 单向；禁止依赖 services、tauri、任何前端资源（02 架构分册 AD-*）。
//! FFI 只允许出现在本 crate 与 elevated-helper（02 册 KN-04）；新代码禁手写 Win32 绑定，
//! 一律经 `windows` / `windows-sys` crate 官方生成绑定（01 总览 §二）。

pub mod fs;
pub mod net_tcp_table;
pub mod registry;
pub mod sidecar;
