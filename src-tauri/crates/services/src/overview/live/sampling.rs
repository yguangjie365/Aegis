//! 单拍采样主路径（F-08 节拍的采集侧）：`live` 的子模块。

use super::cards::{build_cards, CardInputs};
use super::status::monitor_status;
use super::storage::{collect_disk_health, collect_storage, used_percent_of};
use crate::overview::dto::{HeaderDto, LiveSampleDto};
use crate::overview::state::{OverviewInner, SystemIdentity};
use std::sync::Arc;
use sysinfo::{CpuRefreshKind, System};

/// 把一次 sidecar 读取投递到专用线程（同一时刻至多一个在途，R-13）。
fn kick_sensor_read(inner: &Arc<OverviewInner>) {
    {
        let bridge = inner.bridge();
        if bridge.is_disabled() || bridge.cooling_down() {
            return;
        }
    }
    if !inner.try_begin_read() {
        return;
    }
    let shared = Arc::clone(inner);
    let spawned = std::thread::Builder::new()
        .name("overview-sensor".to_string())
        .spawn(move || {
            let outcome = {
                let mut bridge = shared.bridge();
                bridge.read_once()
            };
            match outcome {
                Ok(snapshot) => shared.store_sensors(snapshot),
                // 🔴 失败不清缓存：保留上一轮可用值（R-08 不给错数，也不突然空白）
                Err(fault) => eprintln!("[overview] 传感器读取失败：{}", fault.reason),
            }
            shared.end_read();
        });
    if let Err(error) = spawned {
        inner.end_read();
        eprintln!("[overview] 传感器读取线程未能启动: {error}");
    }
}

/// 首次调用时确定系统身份项（会话内不变，之后直接复用缓存）。
fn ensure_identity(inner: &OverviewInner) -> SystemIdentity {
    if let Some(existing) = inner.identity() {
        return existing;
    }
    let sys = inner.system();
    let identity = SystemIdentity {
        computer_name: System::host_name().unwrap_or_default(),
        os_name: System::long_os_version().unwrap_or_default(),
        cpu_brand: sys
            .cpus()
            .first()
            .map(|cpu| cpu.brand().to_string())
            .unwrap_or_default(),
        physical_cores: sys.physical_core_count(),
        logical_cores: sys.cpus().len(),
    };
    drop(sys);
    inner.store_identity(identity.clone());
    identity
}

/// 一次节拍采样（F-01/F-08/F-10 采集侧）。
#[must_use]
pub fn sample(inner: &Arc<OverviewInner>) -> LiveSampleDto {
    kick_sensor_read(inner);

    let (cpu_usage_percent, memory_usage_percent, memory_total_bytes) = {
        let mut sys = inner.system();
        sys.refresh_cpu_specifics(CpuRefreshKind::everything());
        // 首拍 PDH 计数器还没有第二个采样点，cpu_usage() 是 0 填充值（L-02）→ 判为无基线（R-08）
        let reads = inner.note_cpu_refresh();
        let cpu = if reads >= 2 {
            Some(f64::from(sys.global_cpu_info().cpu_usage()))
        } else {
            None
        };
        sys.refresh_memory();
        let total = sys.total_memory();
        // `used_percent_of` 收的是「空闲量」，内存侧 sysinfo 只给「已用量」→ 先取补，
        // 保证内存卡与存储卷走同一个已用率口径（R-07/R-08 单源）。
        let memory = used_percent_of(total, total.saturating_sub(sys.used_memory()));
        (cpu, memory, total)
    };

    let identity = ensure_identity(inner);
    // R-12：存储只在慢变量缺失时确定一次，不随节拍变化
    if inner.storage().is_none() {
        inner.store_storage(collect_storage());
    }

    // F-06：磁盘健康同样是慢变量，只在首次/全量采集时更新
    let disk_health = if inner.disk_health().is_empty() {
        collect_disk_health()
    } else {
        Vec::new() // 已有数据，返回空列表（前端用旧值）
    };

    let sensors = inner.sensors();
    let storage = inner.storage();
    let monitor = monitor_status(inner);
    let inputs = CardInputs {
        identity: &identity,
        sensors: sensors.as_ref(),
        storage: storage.as_ref(),
        cpu_usage_percent,
        memory_usage_percent,
        memory_total_bytes,
        monitor_reason: monitor.reason.as_deref(),
    };

    // 先算卡片再搬 `identity`：`inputs` 借用它，两榜与页头共用同一份身份缓存（R-21）
    let cards = build_cards(&inputs);

    LiveSampleDto {
        header: HeaderDto {
            computer_name: identity.computer_name,
            os_name: identity.os_name,
            uptime_secs: System::uptime(),
        },
        cards,
        monitor,
        in_flight: inner.read_in_flight(),
        disk_health: if disk_health.is_empty() {
            inner.disk_health().clone()
        } else {
            inner.store_disk_health(disk_health.clone());
            disk_health
        },
    }
}
