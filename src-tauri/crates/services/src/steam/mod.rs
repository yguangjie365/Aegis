//! steam 域 — Steam 游戏管理后端（NexBox 游戏模块换皮版 · 阶段一 100% 复刻基线）。
//!
//! 🔴 复刻基线（`Docs/开发文档/02-专题设计/09-游戏管理模块移植调研.md` §〇，
//! 2026-10-03 用户裁决）：游戏模块**基线是 NexBox**，仅**游戏卡片视觉**参考
//! SystemToolkit（`GameManagerView.xaml`）。故本域后端 100% 取自
//! `Source/NexBox/src-tauri/src/steam.rs`（2188 行，GPL-3.0 同源衍生），
//! **原样搬入、逐字未改**。
//!
//! ## 依赖面（G1-0 实测）
//! - `use crate::` 命中 **0 处** —— `steam.rs` 完全自包含，无跨模块拖链（与音乐域
//!   `hotkey.rs` 拖出整个游戏域的情况相反）。
//! - Tauri 宿主类型（`Window` / `WebviewWindow` / `AppHandle` / `State<..>`）命中
//!   **0 处** —— 符合 services crate「只准用 tauri 非 UI 部分」的约束，无需豁免。
//! - 外部 crate 仅 3 个：`serde`（✅ 已有）、`reqwest`（✅ 已有 0.12，同 features）、
//!   `winreg`（❌ 本次新增 0.52，读 Windows 注册表取 Steam 安装路径与账号）。
//!
//! ## 范围裁决（2026-10-03 用户裁决）
//! - ✅ Steam 主线（`steam.rs`）—— 本文件。
//! - ❌ **游戏辅助不移植**：自瞄（`crosshair.rs` / `crosshair_hold.rs`）、自动点击
//!   （`autoclicker.rs`）、三角洲（`delta_force.rs` / `game_mode.rs` / `df_stats.rs` /
//!   `game_ping.rs` / `game_win_key.rs` / `game_launcher.rs`）、游戏进程优化
//!   （`game_process_optimize.rs` / `game_filter.rs` / `game_fps.rs`）均**不搬入**。

// ── 模块可见性 ──
// NexBox 中 steam.rs 为 src/ 顶层私有 mod（其 lib.rs:74 `mod steam;`），Aegis 为
// Cargo workspace（根 crate 需跨 crate 引用），故改为 `pub mod`。
//
// 🔴 **不要在此处 `pub use` 再导出命令**（阶段一实测踩坑）：tauri-macros 为 `pub` command
// 生成的两个宏（`__cmd__<名>` / `__tauri_command_name_<名>`）是 **module-private**，
// 跨模块再导出后根 crate 的 `generate_handler!` 解析不到，报
// `E0433: cannot find __cmd__get_steam_install_info in steam`（19 个命令全炸）。
// 与音乐模块 `services::music::music_api::<名>` 必须写全路径是同一条教训。
// 故 `src-tauri/src/lib.rs` 一律写 `services::steam::steam::<命令名>`，直接指向定义模块。
pub mod inventory_online;
pub mod steam;

// ── Aegis 新增：Steam 在线库存（不在 NexBox 基线内，用户 2026-10-03 裁决要求补全）──
// 4 个命令：steam_api_key_status / steam_save_api_key / get_steam_inventory_online /
//          steam_installed_appids
// 🔴 注册时同样**必须写全路径** `services::steam::inventory_online::<名>`（理由见下）。
//
// 17 个 NexBox 命令清单（供对账用；实际注册见 src-tauri/src/lib.rs，注册顺序与
// NexBox src-tauri/src/lib.rs:1209-1227 逐行一致）：
//   get_steam_install_info / get_steam_users / get_steam_libraries / get_steam_games /
//   get_steam_all_data / launch_steam_client / launch_steam_game / open_steam_store_page /
//   open_game_folder / switch_steam_account / delete_steam_account /
//   format_file_size / get_steam_stats / get_library_disk_info /
//   steam_debug / get_steam_user_avatars / get_steam_inventory
// 🔴 `uninstall_steam_game` 与 `install_steam_game` 均已按用户裁决（2026-10-03）删除 ——
//    本项目只做**展示 + 启动**，安装与卸载一律交由 Steam 客户端管理。**现为 17 个**。
// 前端 `SteamPage.tsx` 实际调用其中 9 个，其余为分步查询与工具/调试。
