//! 静态硬件详情采集（M-01 片2-b：F-02「详细信息」区的采集侧，2026-10-03）。
//!
//! 组织形态沿用片1 拆分后的门面约定：本文件只做**声明与出口**，🔴 新增代码写进 `hardware/` 下的
//! 对应子模块（`.rs` ≤500 行的规范闸见 03 分册），不要把采集器塞回父文件。
//!
//! 复用结论（DP-1，逐条登记见 `DEPENDENCIES.md §三`）：
//! - 八类 WQL 与解码表 ← `NexBox/src-tauri/src/hardware.rs`（②档改造移植，🔴 手写 FFI 段不移植）；
//! - 显存总量/驱动版本 ← [`infra::nvml`]（母本 `:741` 同路径，🔴 K-004 只准补静态规格项）；
//! - 网卡可见性判定 ← `crate::network::visibility`（R-02/R-20 单源，本域不另写一份）；
//! - 介质枚举 [`crate::overview::live::Media`] ← 片1 `live::storage`（R-05 同一概念单定义）；
//! - 声卡过滤基线 ← `SystemToolkit` `OverviewService.cs:362`（③思路复用，R-03 四条关键词的出处）。
//!
//! 🔴 三条贯穿本域的纪律：
//! 1. **取值诚实**（R-08）：所有"要不要显示/是不是未知"在 Rust 侧判完，缺失一律 `Option::None`；
//!    母本的 `"未知"` 占位串与 `catch_unwind`、`as` 截断三处写法都不移植；
//! 2. **序列号**（R-22）：只采物理盘与内存条，🔴 主板序列号不采不展示，值不得进任何日志正文；
//! 3. **分区独立容错**（R-10）：每类自己出降级卡，装配层不吞卡、不整体失败。

pub mod collect;

mod audio;
mod board;
mod cpu;
mod gpu;
mod memory;
mod monitor;
mod network;
mod rows;
mod storage;

pub use self::collect::{collect, now_ms};
