//! 系统信息采集器（M-01 片2-d，2026-10-03；本文件为门面，实物在 `system_info/` 子模块）。
//!
//! **复用结论（DP-1）**：
//! - 操作系统名称 ← `Source/NexBox/src-tauri/src/hardware.rs:2240 long_os_version()`
//!   （🔴 **只取"内核版本优先于注册表 `ProductName`"这一口径**，不取其 `libloading` + 手写
//!   `RtlGetVersion` 的实现路径——见下方分层纪律）；
//! - 其余字段 ← WMI `Win32_OperatingSystem` / `Win32_ComputerSystem` / `Win32_GroupUser` / 注册表。
//!
//! 🔴 R-11 字段级回退：每字段独立容错，单项失败显示 None（前端渲染"不可用"），
//!    不整面板消失；失败项同时折进卡级 reason，面板不得"空着但不解释"。
//!
//! 🔴 分层纪律（02 册 KN-04 / 04 分册 §一 准入表"FFI 只出现在 infra 与 elevated-helper"）：
//!    本域**零 FFI**——不用 `libloading`、不写 `extern "system"`。系统事实的三条合规通道是
//!    注册表（`winreg`）、WMI（经 `infra::wmi`）、sysinfo。片2-d 初稿曾在 `kernel_version`
//!    用 libloading 取 `RtlGetVersion`、在运行时长/管理员判定上手写 `GetTickCount64` /
//!    `IsUserAnAdmin`，🔴 收口批（2026-10-03 片4）全部改回这三条通道，`services` 侧
//!    `libloading` 依赖同批摘除。
//!
//! 🟡 与母本的一处**语义**偏离：F-03 的"用户类型（管理员/标准）"按规格原文取**账户是否在
//!    `Administrators` 组**（WMI 组成员关系），不是母本 `IElevationProvider` / `NexBox`
//!    `optimization::is_admin()` 的"当前令牌是否已提权"——后者答的是"程序有没有以管理员运行"，
//!    与"用户类型"不是同一个问题。

mod os;
mod readers;
mod user;

use crate::overview::dto::DetailCardDto;

/// 两块系统信息面板：操作系统 + 用户/区域。
#[must_use]
pub fn panels() -> Vec<DetailCardDto> {
    vec![os::os_panel(), user::user_panel()]
}

#[cfg(test)]
mod tests;
