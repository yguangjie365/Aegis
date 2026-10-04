//! 传感器通道状态映射（R-08 诚实口径）：`live` 的子模块。

use super::KERNEL_DRIVER_UNINSTALL_KEY;
use crate::overview::dto::{MonitorState, MonitorStatusDto};
use crate::overview::sensor_bridge::{FaultKind, SensorBridge};
use crate::overview::state::OverviewInner;
use infra::registry;

/// 桥状态 → 用户可见的通道状态 + 原因（N-06：每次降级都要说清为什么）。
pub(super) fn map_bridge_state(bridge: &SensorBridge) -> (MonitorState, Option<String>) {
    let last_reason = bridge.last_fault().map(|fault| fault.reason.clone());
    // "组件根本没随包提供"比"冷却中/已停用"更能解释界面缺了什么（N-06）：
    // exe 路径查找每次只扫一遍，失败原因不随重试次数变化，故两处都优先报它。
    let exe_missing = bridge
        .last_fault()
        .is_some_and(|fault| fault.kind == FaultKind::ExeMissing);
    if bridge.is_disabled() {
        return (
            if exe_missing {
                MonitorState::ExeMissing
            } else {
                MonitorState::Disabled
            },
            Some(last_reason.unwrap_or_else(|| "传感器通道已停用".to_string())),
        );
    }
    if bridge.cooling_down() {
        return (
            if exe_missing {
                MonitorState::ExeMissing
            } else {
                MonitorState::CoolingDown
            },
            Some(last_reason.unwrap_or_else(|| "传感器通道冷却中".to_string())),
        );
    }
    if bridge.has_reads() {
        return (MonitorState::Ready, None);
    }
    if let Some(reason) = last_reason {
        // 未就绪且留过故障：区分"组件根本没随包提供"与"接不上"，两者文案不同
        return if exe_missing {
            (MonitorState::ExeMissing, Some(reason))
        } else {
            (MonitorState::CoolingDown, Some(reason))
        };
    }
    (
        MonitorState::Starting,
        Some("传感器通道首次接入中（硬件初始化上限 15 秒）".to_string()),
    )
}

#[must_use]
pub fn monitor_status(inner: &OverviewInner) -> MonitorStatusDto {
    let (state, reason, sidecar_pid) = {
        let bridge = inner.bridge();
        let (state, reason) = map_bridge_state(&bridge);
        (state, reason, bridge.sidecar_pid())
    };
    // 只读探测：装了就读到版本，没装就如实 false；本工程永不写注册表、永不发起安装。
    let driver = registry::detect_driver(KERNEL_DRIVER_UNINSTALL_KEY);
    MonitorStatusDto {
        state,
        reason,
        sidecar_pid,
        driver_installed: driver.installed,
        driver_version: driver.version,
    }
}
