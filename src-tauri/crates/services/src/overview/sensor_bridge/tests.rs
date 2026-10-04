use super::bridge::{
    classify_sidecar_error, BridgeFault, SensorBridge, EXE_NAME, FIRST_READ_BUDGET,
    MAX_CONSECUTIVE_FAILURES, READ_TIMEOUT,
};
use super::exe::{candidate_paths, FaultKind, SUBDIRS};
use super::matching::{
    aggregate, cpu_load_of, cpu_temperature_of, gpu_sensors_of, max_temperature_in_family,
    memory_load_of, STORAGE_FAMILIES,
};
use super::types::SensorsResponse;
use infra::sidecar::SidecarError;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

fn response(json: &str) -> SensorsResponse {
    serde_json::from_str(json).expect("测试夹具应为合法传感器 JSON")
}

/// 本机实测形态（K-Q12-2：无内核驱动 + 非管理员 → CPU 无温度、无磁盘族传感器）。
const REALISH_FIXTURE: &str = r#"{
    "updatedAt":"2026-10-03T08:00:00Z",
    "sensors":[
        {"hardware":"CPU #1 - 12th Gen Intel Core i7-12700H","hardwareType":"Cpu","name":"CPU Total","sensorType":"Load","value":22.5,"unit":"%"},
        {"hardware":"CPU #1 - 12th Gen Intel Core i7-12700H","hardwareType":"Cpu","name":"Core (Tctl/Tdie)","sensorType":"Load","value":31.0,"unit":"%"},
        {"hardware":"Total Memory","hardwareType":"Memory","name":"Memory Used","sensorType":"Data","value":11.84,"unit":"GB"},
        {"hardware":"Total Memory","hardwareType":"Memory","name":"Memory","sensorType":"Load","value":37.24,"unit":"%"},
        {"hardware":"Virtual Memory","hardwareType":"Memory","name":"Memory","sensorType":"Load","value":99.0,"unit":"%"},
        {"hardware":"GPU #1 - NVIDIA GeForce RTX 3060 Laptop","hardwareType":"GpuNvidia","name":"GPU Core","sensorType":"Temperature","value":56.0,"unit":"°C"},
        {"hardware":"GPU #1 - NVIDIA GeForce RTX 3060 Laptop","hardwareType":"GpuNvidia","name":"D3D 3D","sensorType":"Load","value":3.0,"unit":"%"},
        {"hardware":"GPU #1 - NVIDIA GeForce RTX 3060 Laptop","hardwareType":"GpuNvidia","name":"GPU Core","sensorType":"Load","value":1.0,"unit":"%"},
        {"hardware":"GPU #2 - Intel UHD","hardwareType":"GpuIntel","name":"GPU Core","sensorType":"Load","value":7.0,"unit":"%"}
    ]
}"#;

#[test]
fn cpu_load_prefers_cpu_total() {
    assert_eq!(cpu_load_of(&response(REALISH_FIXTURE)), Some(22.5));
}

#[test]
fn cpu_temperature_is_absent_not_zero_when_driver_missing() {
    // 夹具里刻意不放 CPU Temperature（本机实测 0 条）→ 必须 None，不得给 0
    assert_eq!(cpu_temperature_of(&response(REALISH_FIXTURE)), None);
}

#[test]
fn cpu_temperature_matches_superio_family_on_older_amd() {
    let fixture = r#"{"sensors":[{"hardware":"F71882","hardwareType":"SuperIO","name":"Tctl","sensorType":"Temperature","value":47.0,"unit":"°C"}]}"#;
    assert_eq!(cpu_temperature_of(&response(fixture)), Some(47.0));
}

#[test]
fn load_named_core_sensor_is_not_mistaken_for_temperature() {
    // 夹具内同名条目是 Load（"Core (Tctl/Tdie)" 31%）：温度判定必须看 sensorType
    let fixture = r#"{"sensors":[{"hardware":"CPU","hardwareType":"Cpu","name":"Core (Tctl/Tdie)","sensorType":"Load","value":31.0,"unit":"%"}]}"#;
    assert_eq!(cpu_temperature_of(&response(fixture)), None);
}

#[test]
fn memory_load_ignores_virtual_memory_hardware() {
    // 🔴 规则 4：取到 Virtual Memory 的 99% 就是错数
    assert_eq!(memory_load_of(&response(REALISH_FIXTURE)), Some(37.24));
}

#[test]
fn gpu_grouping_prefers_gpu_core_and_hides_intel_when_nvidia_present() {
    let gpus = gpu_sensors_of(&response(REALISH_FIXTURE));
    assert_eq!(gpus.len(), 1, "存在 NVIDIA 独显时 Intel 核显应被跳过");
    assert_eq!(
        gpus[0].load_percent,
        Some(1.0),
        "负载优先 GPU Core 而非 D3D 3D"
    );
    assert_eq!(gpus[0].temperature_c, Some(56.0));
}

#[test]
fn gpu_keeps_intel_when_it_is_the_only_display() {
    let fixture = r#"{"sensors":[{"hardware":"GPU #1 - Intel UHD","hardwareType":"GpuIntel","name":"GPU Core","sensorType":"Temperature","value":44.0,"unit":"°C"}]}"#;
    let gpus = gpu_sensors_of(&response(fixture));
    assert_eq!(gpus.len(), 1);
    assert_eq!(gpus[0].temperature_c, Some(44.0));
    assert_eq!(gpus[0].load_percent, None, "没有负载传感器时不得凭空造 0");
}

#[test]
fn snapshot_records_missing_sensor_families_as_flags() {
    let snapshot = aggregate(&response(REALISH_FIXTURE));
    assert!(!snapshot.has_storage_sensor, "非管理员下 HardDisk 族缺席");
    assert!(!snapshot.has_memory_temperature);
    assert_eq!(snapshot.gpus.len(), 1);
    assert_eq!(snapshot.cpu_load_percent, Some(22.5));
}

#[test]
fn storage_and_memory_temperature_flags_light_up_when_present() {
    let fixture = r#"{"sensors":[
        {"hardware":"SAMSUNG MZVL2","hardwareType":"Ssd","name":"Temperature","sensorType":"Temperature","value":42.0,"unit":"°C"},
        {"hardware":"Total Memory","hardwareType":"Memory","name":"DIMM Temp","sensorType":"Temperature","value":51.0,"unit":"°C"}
    ]}"#;
    let snapshot = aggregate(&response(fixture));
    assert!(snapshot.has_storage_sensor);
    assert!(snapshot.has_memory_temperature);
    assert_eq!(snapshot.storage_temperature_c, Some(42.0));
    assert_eq!(snapshot.memory_temperature_c, Some(51.0));
}

#[test]
fn badge_temperature_takes_the_highest_reading_in_the_family() {
    // R-09：多设备取最高，不是第一块
    let fixture = r#"{"sensors":[
        {"hardware":"DISK A","hardwareType":"Ssd","name":"Temperature","sensorType":"Temperature","value":38.0,"unit":"°C"},
        {"hardware":"DISK B","hardwareType":"HardDisk","name":"Temperature","sensorType":"Temperature","value":52.0,"unit":"°C"},
        {"hardware":"DISK C","hardwareType":"Hdd","name":"Temperature","sensorType":"Temperature","value":44.0,"unit":"°C"}
    ]}"#;
    assert_eq!(
        max_temperature_in_family(&response(fixture), &STORAGE_FAMILIES),
        Some(52.0)
    );
    let empty = r#"{"sensors":[{"hardware":"X","hardwareType":"Cpu","name":"a","sensorType":"Load","value":1.0,"unit":"%"}]}"#;
    assert_eq!(
        max_temperature_in_family(&response(empty), &STORAGE_FAMILIES),
        None
    );
}

#[test]
fn error_shaped_payload_is_not_silently_treated_as_empty() {
    // sidecar 自身的 {"error":"…"} 回包必须解析失败 → 走 Protocol 故障（R-08 不得当成"零传感器"）
    let parsed: Result<SensorsResponse, serde_json::Error> =
        serde_json::from_str(r#"{"error":"boom"}"#);
    assert!(parsed.is_err(), "缺 sensors 字段的回包不得被当成合法空快照");
}

#[test]
fn candidate_paths_cover_install_and_dev_layouts() {
    let base = Path::new(r"C:\Program Files\Aegis");
    let paths = candidate_paths(base);
    assert!(paths.contains(&PathBuf::from(
        r"C:\Program Files\Aegis\monitor\AegisMonitor.exe"
    )));
    assert!(paths.iter().any(|path| path
        .to_string_lossy()
        .replace('\\', "/")
        .ends_with("resources/monitor/AegisMonitor.exe")));
    // 必须向上回溯到祖先：开发环境 exe 在 src-tauri/target/debug 下，sidecar 在仓库根的 monitor/
    let deepest = paths
        .iter()
        .filter(|path| path.starts_with("C:\\Program Files\\Aegis"))
        .count();
    assert!(
        deepest >= SUBDIRS.len(),
        "同目录候选应全部保留，实得 {deepest}"
    );
    assert!(paths.iter().any(|path| {
        let text = path.to_string_lossy().replace('\\', "/");
        text.contains("monitor/bin/Release/net48/AegisMonitor.exe")
    }));
}

/// 构造一个"已查过、确认没有 exe"的桥（避免测试依赖本机是否装了 sidecar）。
fn bridge_without_exe() -> SensorBridge {
    let mut bridge = SensorBridge::new();
    bridge.exe_lookup_done = true;
    bridge.exe_path = None;
    bridge
}

#[test]
fn missing_exe_fails_fast_with_component_named() {
    let mut bridge = bridge_without_exe();
    let started = Instant::now();
    let fault = bridge.read_once().expect_err("缺 exe 必须失败");
    assert_eq!(fault.kind, FaultKind::ExeMissing);
    assert!(
        fault.reason.contains(EXE_NAME),
        "原因须点名组件便于诊断，实得：{}",
        fault.reason
    );
    // 🔴 "5s 快败"的结构性上界：exe 缺失路径不得消耗任何读超时预算
    assert!(
        started.elapsed() < Duration::from_secs(1),
        "exe 缺失应立即失败，实得 {:?}",
        started.elapsed()
    );
    assert!(bridge.cooling_down(), "失败后应进入冷却");
}

#[test]
fn cooldown_refuses_retry_until_expiry() {
    let mut bridge = bridge_without_exe();
    let first = bridge.read_once().expect_err("首次应失败");
    assert_eq!(first.kind, FaultKind::ExeMissing);

    let second = bridge.read_once().expect_err("冷却期不得再起进程");
    assert_eq!(second.kind, FaultKind::Cooling);
    assert!(!bridge.is_disabled(), "1 次失败远未达停用阈值");
}

#[test]
fn fifth_consecutive_failure_disables_the_channel_for_the_session() {
    let mut bridge = SensorBridge::new();
    for index in 1..=MAX_CONSECUTIVE_FAILURES {
        let fault = bridge.record_failure(BridgeFault::new(
            FaultKind::Died,
            format!("第 {index} 次退出"),
        ));
        if index < MAX_CONSECUTIVE_FAILURES {
            assert_eq!(fault.kind, FaultKind::Died, "第 {index} 次仍是原故障");
            assert!(bridge.cooling_down());
        } else {
            assert_eq!(fault.kind, FaultKind::Disabled, "第 5 次连败应转停用");
        }
    }
    assert!(bridge.is_disabled());
    let refused = bridge.read_once().expect_err("停用后不得再读");
    assert_eq!(refused.kind, FaultKind::Disabled);
    assert!(
        refused.reason.contains("连续失败 5 次"),
        "停用原因须可诊断：{}",
        refused.reason
    );
}

#[test]
fn successful_read_clears_failure_state() {
    let mut bridge = SensorBridge::new();
    bridge.record_failure(BridgeFault::new(FaultKind::Timeout, "预算耗尽"));
    assert_eq!(bridge.failures, 1);
    assert!(!bridge.has_reads());
    assert_eq!(
        bridge.last_fault().expect("应留下末次故障").kind,
        FaultKind::Timeout
    );

    // 模拟成功回包后的状态推进（不依赖真实子进程）
    bridge.failures = 0;
    bridge.cooldown_until = None;
    bridge.ready_once = true;
    assert!(!bridge.cooling_down());
    assert!(bridge.has_reads());
    assert_eq!(bridge.sidecar_pid(), None, "未接管进程时不得伪造 PID");
}

#[test]
fn timeout_reason_carries_the_budget_actually_used() {
    let fault = classify_sidecar_error(&SidecarError::Timeout, FIRST_READ_BUDGET);
    assert_eq!(fault.kind, FaultKind::Timeout);
    assert!(
        fault.reason.contains("23 秒"),
        "首次预算应写进原因，实得：{}",
        fault.reason
    );
    let steady = classify_sidecar_error(&SidecarError::Timeout, READ_TIMEOUT);
    assert!(steady.reason.contains("8 秒"), "实得：{}", steady.reason);
}

#[test]
fn pipe_write_failure_is_reported_as_a_dead_child() {
    let fault = classify_sidecar_error(
        &SidecarError::Write("管道正在被使用".to_string()),
        READ_TIMEOUT,
    );
    assert_eq!(fault.kind, FaultKind::Died);
    assert!(fault.reason.contains("管道"), "实得：{}", fault.reason);
}
