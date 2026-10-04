//! `commands` 的测试。
//!
//! 🔴 命令层持有**进程级单例**，故所有触碰它的用例都必须串行（[`test_lock`]）。
//! 每个用例开头调 [`begin`] 换到独立临时设置路径并清掉上一轮残留的服务。

use std::net::{SocketAddr, TcpListener};
use std::path::{Path, PathBuf};
use std::sync::{Mutex, MutexGuard, OnceLock};

use super::*;
use crate::transfer::settings::PortProblem;

/// 串行锁：命令层是进程级状态，并行跑会互相踩。
fn test_lock() -> MutexGuard<'static, ()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

fn temp_dir(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("aegis-cmd-{}-{}", std::process::id(), tag));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("建临时目录");
    dir
}

/// 停掉可能残留的服务。
fn reset() {
    let service = lock_state().service.take();
    if let Some(service) = service {
        let _ = service.stop();
    }
}

/// 每个用例的开场：清残留 + 换临时设置路径。
fn begin(tag: &str) -> PathBuf {
    reset();
    let dir = temp_dir(tag);
    let path = dir.join("settings.json");
    lock_state().settings_path = Some(path.clone());
    path
}

fn free_port() -> u16 {
    let probe = TcpListener::bind("127.0.0.1:0").expect("探端口");
    let port = probe.local_addr().expect("取端口").port();
    drop(probe);
    port
}

/// 往设置文件里写一份"能启动"的设置（收件目录指向临时目录）。
fn write_settings(path: &Path, recv_dir: &Path, transfer_port: u16, discovery_port: u16) {
    let settings = TransferSettings {
        receive_directory: Some(recv_dir.display().to_string()),
        require_known_peer: false,
        transfer_port,
        discovery_port,
        ..TransferSettings::default()
    };
    settings::save(path, &settings).expect("写设置");
}

fn envelope(payload: Option<serde_json::Value>) -> contracts::CmdEnvelope {
    contracts::CmdEnvelope {
        correlation_id: "TEST0001".to_owned(),
        payload,
    }
}

// ── 未运行态 ──────────────────────────────────────────────────────────────

#[test]
fn status_before_start_is_honest_and_shows_a_placeholder_code() {
    let _guard = test_lock();
    let path = begin("status-idle");
    write_settings(&path, &temp_dir("status-idle-recv"), 18_889, 18_888);

    let status = transfer_service_status(envelope(None));
    assert!(!status.running);
    assert_eq!(
        status.pairing_code, None,
        "未启动时配对码必须是占位（None）"
    );
    assert_eq!(status.pairing_expires_in_ms, 0);
    assert_eq!(status.device_id, None);
    assert_eq!(status.online_peer_count, 0);
    assert_eq!(status.transfer_port, 18_889, "未运行也应显示设置里的端口");
    assert_eq!(status.discovery_port, 18_888);
    reset();
}

#[test]
fn stop_when_not_running_is_reported_not_silently_ok() {
    let _guard = test_lock();
    let _ = begin("stop-idle");
    match transfer_service_stop(envelope(None)) {
        Err(error) => assert_eq!(error.code, CODE_NOT_RUNNING),
        Ok(report) => panic!("未运行不应报成功：{report:?}"),
    }
    reset();
}

#[test]
fn peers_before_start_is_an_empty_snapshot_not_an_error() {
    let _guard = test_lock();
    let _ = begin("peers-idle");
    assert_eq!(transfer_peers_list(envelope(None)), Vec::new());
    reset();
}

// ── 启动 / 停止 ───────────────────────────────────────────────────────────

#[test]
fn start_then_status_then_stop_round_trips_through_the_commands() {
    let _guard = test_lock();
    let path = begin("lifecycle");
    let port = free_port();
    let discovery = free_port();
    write_settings(&path, &temp_dir("lifecycle-recv"), port, discovery);

    let started = transfer_service_start(envelope(None)).expect("应启动成功");
    assert!(started.running);
    assert_eq!(started.transfer_port, port);
    assert_eq!(started.discovery_port, discovery);
    // 🔴 启动后必须能拿到配对码（R-1 的信任根）与设备标识（R-6）
    let code = started.pairing_code.clone().expect("应给出配对码");
    assert_eq!(code, code.to_ascii_uppercase(), "字母表全大写");
    assert_eq!(code.len(), 6);
    assert!(started.pairing_expires_in_ms > 0);
    assert!(started.device_id.is_some());

    // 状态查询与启动返回一致
    let status = transfer_service_status(envelope(None));
    assert!(status.running);
    assert_eq!(status.transfer_port, port);

    // 重复启动要明确拒绝，而不是起第二个服务
    match transfer_service_start(envelope(None)) {
        Err(error) => assert_eq!(error.code, CODE_ALREADY_RUNNING),
        Ok(_) => panic!("重复启动应被拒"),
    }

    let report = transfer_service_stop(envelope(None)).expect("应停止成功");
    assert!(report.all_stopped);
    assert!(report.message.contains("已停止"), "{}", report.message);

    let after = transfer_service_status(envelope(None));
    assert!(!after.running);
    assert_eq!(
        after.pairing_code, None,
        "停止后配对码必须清掉（F-FT-04 同口径）"
    );

    // 🔴 端口真的释放了
    TcpListener::bind(("0.0.0.0", port)).expect("停止后端口应可再绑");
    reset();
}

#[test]
fn invalid_ports_are_attributed_before_anything_starts() {
    let _guard = test_lock();
    let path = begin("badports");
    let port = free_port();
    // 传输与发现同端口 → `settings::validate_ports` 判非法
    write_settings(&path, &temp_dir("badports-recv"), port, port);

    match transfer_service_start(envelope(None)) {
        Err(error) => {
            assert_eq!(error.code, CODE_START_INVALID_PORT);
            assert!(
                error.message.contains("未启动任何监听"),
                "{}",
                error.message
            );
        }
        Ok(_) => panic!("端口非法应拒启动"),
    }
    // 第 1 步就失败 ⇒ 端口根本没被占
    TcpListener::bind(("127.0.0.1", port)).expect("端口不该被占用");
    reset();
}

#[test]
fn occupied_transfer_port_is_attributed_to_the_transfer_stage() {
    let _guard = test_lock();
    let path = begin("portbusy");
    let port = free_port();
    let discovery = free_port();
    write_settings(&path, &temp_dir("portbusy-recv"), port, discovery);
    let _blocker = TcpListener::bind(("0.0.0.0", port)).expect("占住端口");

    match transfer_service_start(envelope(None)) {
        Err(error) => {
            assert_eq!(error.code, CODE_START_TRANSFER_FAILED);
            // F-FT-02 的专属措辞
            assert!(
                error.message.contains("传输服务启动失败"),
                "{}",
                error.message
            );
            assert!(error.message.contains("可能被占用"), "{}", error.message);
        }
        Ok(_) => panic!("端口被占应启动失败"),
    }
    reset();
}

// ── 设置读写 ──────────────────────────────────────────────────────────────

#[test]
fn settings_load_reports_missing_file_instead_of_faking_defaults() {
    let _guard = test_lock();
    let _ = begin("settings-missing");
    let loaded = transfer_settings_load(envelope(None));
    assert!(
        loaded.file_missing,
        "首次运行必须如实报「没有文件」，不能假装是默认值"
    );
    assert!(!loaded.corrupt);
    assert_eq!(loaded.settings, TransferSettings::default());
    reset();
}

#[test]
fn settings_save_then_load_round_trips_and_rejects_bad_payloads() {
    let _guard = test_lock();
    let _ = begin("settings-io");
    let settings = TransferSettings {
        require_known_peer: false,
        transfer_port: 19_100,
        discovery_port: 19_101,
        ..TransferSettings::default()
    };
    transfer_settings_save(envelope(Some(serde_json::to_value(&settings).unwrap())))
        .expect("保存应成功");

    let loaded = transfer_settings_load(envelope(None));
    assert!(!loaded.file_missing && !loaded.corrupt);
    assert_eq!(loaded.settings, settings);

    // 缺 payload → 明确报 BAD_PAYLOAD，不 panic
    match transfer_settings_save(envelope(None)) {
        Err(error) => assert_eq!(error.code, CODE_BAD_PAYLOAD),
        Ok(()) => panic!("缺 payload 应被拒"),
    }
    // payload 形状不对 → 同样
    match transfer_settings_save(envelope(Some(serde_json::json!({"nope": 1})))) {
        Err(error) => assert_eq!(error.code, CODE_BAD_PAYLOAD),
        Ok(()) => panic!("形状不对应被拒"),
    }
    reset();
}

/// 🔴 配对开关不落盘：命令层存进去的 `true/false` 读回来一律是默认（要求配对）。
#[test]
fn settings_do_not_persist_the_pairing_switch() {
    let _guard = test_lock();
    let _ = begin("settings-pairing");
    let weakened = TransferSettings {
        require_pairing: false,
        ..TransferSettings::default()
    };
    transfer_settings_save(envelope(Some(serde_json::to_value(&weakened).unwrap()))).unwrap();
    assert!(
        transfer_settings_load(envelope(None))
            .settings
            .require_pairing,
        "关掉配对不该被持久化"
    );
    reset();
}

// ── 发送 ──────────────────────────────────────────────────────────────────

#[test]
fn send_rejects_a_malformed_address_and_a_missing_file() {
    let _guard = test_lock();
    let _ = begin("send-bad");
    let bad_ip = SendFilePayload {
        ip: "not-an-ip".to_owned(),
        port: 1,
        path: "x".to_owned(),
        pair_code: None,
    };
    match send_impl(&bad_ip) {
        Err(error) => assert_eq!(error.code, CODE_BAD_PAYLOAD),
        Ok(_) => panic!("坏地址应被拒"),
    }

    let missing = SendFilePayload {
        ip: "127.0.0.1".to_owned(),
        port: free_port(),
        path: "C:/definitely/not/here.bin".to_owned(),
        pair_code: None,
    };
    match send_impl(&missing) {
        // 路径不存在 ⇒ SendSource::open 先失败（不先建连接，避免无谓的网络动作）
        Err(error) => assert_eq!(error.code, CODE_BAD_PAYLOAD),
        Ok(_) => panic!("不存在的文件应被拒"),
    }
    reset();
}

#[test]
fn send_to_a_dead_port_reports_peer_disconnected() {
    let _guard = test_lock();
    let _ = begin("send-dead");
    let dir = temp_dir("send-dead-src");
    let file = dir.join("payload.bin");
    std::fs::write(&file, b"hello").unwrap();

    let payload = SendFilePayload {
        ip: "127.0.0.1".to_owned(),
        // 刚探到就释放的端口：连它必然失败（不依赖外部服务）
        port: free_port(),
        path: file.display().to_string(),
        pair_code: None,
    };
    match send_impl(&payload) {
        Err(error) => assert_eq!(error.code, reasons::PEER_DISCONNECTED),
        Ok(_) => panic!("连不上的端口应报断连"),
    }
    reset();
}

/// 🔴 端到端穿命令层：起服务 → 用命令发文件（带配对码）→ 文件落盘。
/// 这是"界面点得动"的最近似证据（只差一个 Tauri 运行时）。
#[test]
fn send_through_the_command_layer_lands_the_file() {
    let _guard = test_lock();
    let path = begin("cmd-e2e");
    let port = free_port();
    let discovery = free_port();
    let recv_dir = temp_dir("cmd-e2e-recv");
    std::fs::create_dir_all(&recv_dir).unwrap();
    write_settings(&path, &recv_dir, port, discovery);

    let started = transfer_service_start(envelope(None)).expect("起服务");
    let code = started.pairing_code.clone().expect("配对码");

    let src_dir = temp_dir("cmd-e2e-src");
    let file = src_dir.join("命令层传输.bin");
    let payload: Vec<u8> = (0..3_000_u32).map(|i| (i % 199) as u8).collect();
    std::fs::write(&file, &payload).unwrap();

    let result = transfer_send_file(envelope(Some(serde_json::json!({
        "ip": "127.0.0.1",
        "port": port,
        "path": file.display().to_string(),
        "pairCode": code,
    }))))
    .expect("带对码应发送成功");
    assert_eq!(result.outcome, "Delivered");
    assert_eq!(result.reason_code, None);

    let landed = recv_dir.join("命令层传输.bin");
    assert!(landed.exists(), "文件应落在接收目录");
    assert_eq!(std::fs::read(&landed).unwrap(), payload, "内容逐字节一致");

    // 不带码再来一次：本次运行期免码
    let second = src_dir.join("第二份.bin");
    std::fs::write(&second, b"second").unwrap();
    let again = transfer_send_file(envelope(Some(serde_json::json!({
        "ip": "127.0.0.1",
        "port": port,
        "path": second.display().to_string(),
    }))))
    .expect("同地址免码");
    assert_eq!(again.outcome, "Delivered");

    // 错码：换一个"尚未配对"的地址才看得出效果——这里直接用错误的码试第三个文件，
    // 但 127.0.0.1 已在本轮配对，故用断言确认"免码确实对它生效"
    assert!(recv_dir.join("第二份.bin").exists());

    let report = transfer_service_stop(envelope(None)).unwrap();
    assert!(report.all_stopped);
    reset();
}

#[test]
fn send_with_a_wrong_code_is_rejected_with_pairing_invalid() {
    let _guard = test_lock();
    let path = begin("cmd-pairfail");
    let port = free_port();
    let discovery = free_port();
    let recv_dir = temp_dir("cmd-pairfail-recv");
    std::fs::create_dir_all(&recv_dir).unwrap();
    write_settings(&path, &recv_dir, port, discovery);
    transfer_service_start(envelope(None)).expect("起服务");

    let src_dir = temp_dir("cmd-pairfail-src");
    let file = src_dir.join("x.bin");
    std::fs::write(&file, b"x").unwrap();

    let result = transfer_send_file(envelope(Some(serde_json::json!({
        "ip": "127.0.0.1",
        "port": port,
        "path": file.display().to_string(),
        "pairCode": "ZZZZZZ",
    }))));
    match result {
        Err(error) => assert_eq!(error.code, reasons::PAIRING_INVALID),
        Ok(value) => panic!("错码应被拒，实得 {value:?}"),
    }
    // 🔴 拒收不得在接收目录留下任何东西
    let leftovers: Vec<String> = std::fs::read_dir(&recv_dir)
        .unwrap()
        .flatten()
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .collect();
    assert_eq!(leftovers, Vec::<String>::new(), "拒收后目录必须干净");

    reset();
}

/// 设置校验的端口问题类型仍可被命令层引用（防"启动期码"与 `PortProblem` 脱节）。
#[test]
fn port_problem_type_is_still_reachable_from_the_command_layer() {
    let _ = PortProblem::TooLow { port: 80 };
    assert_eq!(
        SocketAddr::from(([127, 0, 0, 1], 1)).port(),
        1,
        "地址构造可用"
    );
}
