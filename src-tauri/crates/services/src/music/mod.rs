//! music 域 — 音乐管理后端（规格 07 v1.2；移植自 `NexBox` `music.rs`，GPL-3.0 同源衍生）。
//!
//! MU-1 本地曲库核心：扫描/元数据/封面缓存/曲库持久化（F-MU-01~09、R-1~8）。
//! 后续批次：歌词管线（MU-3）、在线平台与中转（MU-4/5）、系统集成（MU-7）。

pub mod commands;
pub mod library;
pub mod meta;
pub mod scan;
