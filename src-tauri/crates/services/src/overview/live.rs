//! 实时采样与卡片装配（M-01 片1：F-01 / F-08 / F-10 的采集侧，2026-10-03）。
//!
//! 复用结论（DP-1）：
//! - 占用/容量/身份项走 `sysinfo`（`NexBox` `hardware.rs` 同库同版本线，0.30.13 实测钉版）；
//! - 温度/传感器负载走本域 `sensor_bridge`（`NexBox` `sensor.rs` 的②改造移植）；
//! - 🔴 **不移植 `NexBox` `hardware.rs::long_os_version`**：母本用 `libloading` + 手写
//!   `RtlGetVersion` FFI 取内核版本，本工程禁手写 Win32 绑定（01 总览 §二），
//!   改采 `sysinfo` 的注册表实现（同读 `HKLM\...\CurrentVersion`，语义等价），
//!   已登记为对母本的有意偏离（`DEPENDENCIES.md §三`）；
//! - 存储卡口径沿用 `SystemToolkit` 2026-09-02 起的**卷（DriveInfo）单一来源**：
//!   已用率取多卷最大值，副标题按介质聚合（R-06）。
//!
//! 🔴 非阻塞纪律：本模块任何路径都不得等待 sidecar。传感器读取只经 `kick_sensor_read`
//! 投递到专用线程，节拍消费缓存——首屏不被最长 23 秒的通道接入拖住（N-03/N-04）。

/// 内核驱动卸载项名（Q-12=B：只读探测，🔴 不安装、不分发，见 DEPENDENCIES.md §四）。
pub(super) const KERNEL_DRIVER_UNINSTALL_KEY: &str = "PawnIO";

mod cards;
mod sampling;
mod status;
mod storage;

pub use self::cards::{build_cards, CardInputs};
pub use self::sampling::sample;
pub use self::status::monitor_status;
pub use self::storage::{
    collect_disk_health, collect_storage, drive_letter, max_used_percent, media_of, media_text,
    used_percent_of, Media,
};

#[cfg(test)]
mod tests;
