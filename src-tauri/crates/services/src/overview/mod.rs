//! overview 域 — 本机概览（M-01；规格唯一口径 `Docs/功能规格/01-本机概览.md`）。
//!
//! 片1（2026-10-03）交付的采集地基 + 实时面：F-12 传感器通道 / F-08 页头 / F-10 实时节拍 / F-01 四卡，
//! 另含 F-04 进程榜与 F-05 端点面板的**后端命令面**（UI 分别随片3/片4 上屏）。
//!
//! 复用结论（DP-1，逐条登记见 `Aegis/DEPENDENCIES.md §三`）：
//! - `sensor_bridge` ← `NexBox/src-tauri/src/sensor.rs`（②改造移植，🔴 零分发零安装 `PawnIO`）；
//! - 聚合规则 ← `Aegis/spikes/K-Q12-2-sensor-coverage.md §二` 五条（本机实测固化）；
//! - `live` 的存储口径 ← `SystemToolkit` 卷单一来源 + 介质聚合（③思路复用）；
//! - `processes` 差分采样 ← `SystemToolkit` `TopProcessSampler`（③思路复用，🔴 拒 WMI `PerfProc`）；
//! - `endpoints` 归属聚合 ← `SystemToolkit` `TcpConnectionTable`（③思路复用，FFI 落 `infra`）；
//! - 网卡可见性判定单源在 [`crate::network::visibility`]（R-02/R-20），本域不得另写一份。
//!
//! 分层纪律：本域只依赖 `contracts` / `infra` / `sysinfo` 等，🔴 禁持 Tauri 宿主类型
//! （Window / `WebviewWindow` / Menu / Tray）；FFI 一律经 `infra`（02 册 KN-04）；
//! 禁 `std::thread::sleep`（`clippy.toml` 禁用清单），等待一律 channel + 限时。

pub mod badges;
pub mod commands;
pub mod dto;
pub mod endpoints;
pub mod hardware;
pub mod live;
pub mod processes;
pub mod sensor_bridge;
pub mod snapshot;
pub mod software;
pub mod state;
pub mod system_info;
