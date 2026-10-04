//! `service` 的测试。
//!
//! 除生命周期本身，这里有一条**真正的端到端**：起一个真实服务 → 用真实 TCP 连上去 →
//! 配对 → 传文件 → 校验落盘。前九批的零件在这里第一次全部串起来跑。

use std::collections::HashMap;

use std::fs;
use std::net::{SocketAddr, TcpListener, UdpSocket};
use std::path::PathBuf;

use super::*;
use crate::transfer::settings::KnownPeer;

/// 探一个空闲端口（绑 `:0` 读回端口后立刻释放）。
///
/// 🟠 与"之后真的绑它"之间有极小的竞态；对本机测试可接受，且失败时会明确报端口被占用。
fn free_port() -> u16 {
    let probe = TcpListener::bind("127.0.0.1:0").expect("探端口");
    let port = probe.local_addr().expect("取端口").port();
    drop(probe);
    port
}

fn free_udp_port() -> u16 {
    let probe = UdpSocket::bind("127.0.0.1:0").expect("探 UDP 端口");
    let port = probe.local_addr().expect("取端口").port();
    drop(probe);
    port
}

fn temp_dir(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("aegis-service-{}-{}", std::process::id(), tag));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).expect("建临时目录");
    // 🔴 把历史数据目录也指到临时区：`TransferService::start` 会往
    //    `default_history_path()` 写历史，不隔离就会**写脏用户的真实历史**。
    //    放在这里是因为本文件的用例都先建临时目录（幂等设置，多线程下安全）。
    std::env::set_var("AEGIS_TRANSFER_DATA_DIR", &dir);
    dir
}

/// 造一份"能收文件"的设置：指定端口与接收目录，关掉白名单（回环测试里没有发现对端）。
fn settings_for(recv_dir: &Path, transfer_port: u16, discovery_port: u16) -> TransferSettings {
    TransferSettings {
        receive_directory: Some(recv_dir.display().to_string()),
        // 回环测试里不会有真设备宣告进来，故关掉白名单这一关
        require_known_peer: false,
        // 🔴 关掉「接收前询问」：本文件专测传输机制，确认门另有专测
        //    （与母本测试同一处置——不关的话每次接收都会等满 30 秒超时再被拒）
        require_receive_confirmation: false,
        transfer_port,
        discovery_port,
        ..TransferSettings::default()
    }
}

// ── 生命周期与 R-28 的阶段化归因 ────────────────────────────────────────

/// 🔴 回归闸（2026-10-04 修的漏洞）：启动服务**必须**把接收目录建出来。
///
/// 此前从不创建它，而落盘侧的 `infra::safe::join_under_root` 要求根真实存在
/// （要 `canonicalize`）⇒ 首次运行时**每一次接收都以 `RootUnusable` 失败**，功能整个不可用。
#[test]
fn start_creates_the_receive_directory() {
    let dir = temp_dir("mkdir");
    // 刻意用两层不存在的路径：只建一层不算修好
    let nested = dir.join("Downloaded").join("Received");
    assert!(!nested.exists());
    let port = free_port();
    let discovery = free_udp_port();

    let service = TransferService::start(settings_for(&nested, port, discovery), "测试机")
        .expect("应启动成功");
    assert!(nested.is_dir(), "接收目录必须被创建出来");
    assert_eq!(service.receive_root(), nested);
    let _ = service.stop();
    let _ = fs::remove_dir_all(&dir);
}

/// 🔴 **回归闸（2026-10-04 复核查出）**：孤儿清理**必须真被调用**。
///
/// `sink::cleanup_orphans` 早就写好、也有单测，但**服务从没调用它** ✗ ⇒
/// 中断的传输留下的 `.part` 会永远堆在接收目录里（违背 F-FT-19"服务启动……时清理"）。
/// 这条测试验的是**接线**，不是那个函数本身——否则它随时可能再变回死代码。
#[test]
fn start_sweeps_expired_orphan_part_files() {
    let dir = temp_dir("orphan");
    let root = dir.join("Received");
    fs::create_dir_all(&root).unwrap();
    // 造一个"别的对端、且已过期"的遗留件（名字必须带 PART_MARKER，否则清理会跳过它）
    let stale = root.join(format!(
        "old.bin.{}othertok.part",
        crate::transfer::sink::PART_MARKER
    ));
    fs::write(&stale, b"stale").unwrap();
    let long_ago = crate::transfer::sink::now_unix_ms() - 30 * 24 * 60 * 60 * 1000; // 30 天前
    infra::fs::set_modified_time(&stale, long_ago).unwrap();
    // 同时造一个**新鲜**的：不该被清
    let fresh = root.join(format!(
        "new.bin.{}othertok.part",
        crate::transfer::sink::PART_MARKER
    ));
    fs::write(&fresh, b"fresh").unwrap();

    let service =
        TransferService::start(settings_for(&root, free_port(), free_udp_port()), "测试机")
            .expect("应启动成功");
    assert!(
        !stale.exists(),
        "启动时必须清掉属于其它对端且超过 7 天的遗留未完成文件（F-FT-19）"
    );
    assert!(fresh.exists(), "没过期的遗留件不能清（人家可能还要续传）");
    let _ = service.stop();
    let _ = fs::remove_dir_all(&dir);
}

/// 接收目录建不出来时，归因必须指向**目录**而不是端口（否则用户去查错的地方）。
#[test]
fn unbuildable_receive_directory_is_attributed_to_the_directory_not_the_port() {
    let dir = temp_dir("mkfail");
    // 让"目录"落在**一个文件**下面：create_dir_all 必然失败
    let blocker = dir.join("blocker");
    fs::write(&blocker, b"x").unwrap();
    let impossible = blocker.join("Received");
    let port = free_port();
    let discovery = free_udp_port();

    match TransferService::start(settings_for(&impossible, port, discovery), "测试机") {
        Err(StartFailure::ReceiveDirectory { path, .. }) => assert_eq!(path, impossible),
        other => panic!("应归因到接收目录，实得 {other:?}"),
    }
    // 专属措辞里要有"接收目录"，不能是端口语
    let text = StartFailure::ReceiveDirectory {
        path: impossible.clone(),
        cause: io::Error::new(io::ErrorKind::NotADirectory, "x"),
    }
    .to_string();
    assert!(text.contains("接收目录不可用"), "实得 {text}");
    assert!(!text.contains("端口"), "不得提到端口：{text}");
    // 🔴 目录建不出来时**不该占端口**（第 2 步就失败，第 3 步还没走）
    TcpListener::bind(("127.0.0.1", port)).expect("端口不该被占用");
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn invalid_ports_fail_before_any_listener_is_opened() {
    let dir = temp_dir("badports");
    let port = free_port();
    let mut settings = settings_for(&dir, port, port); // 传输与发现同端口 → 非法
    settings.discovery_port = port;

    match TransferService::start(settings, "测试机") {
        Err(StartFailure::InvalidPort { problems }) => {
            assert!(!problems.is_empty(), "应给出具体问题");
        }
        other => panic!("应报端口非法，实得 {other:?}"),
    }
    // 🔴 F-FT-02 第 1 步的意义：端口非法就**没起任何监听**，端口立刻还能绑
    TcpListener::bind(("127.0.0.1", port)).expect("传输端口不该被占用");
    UdpSocket::bind(("127.0.0.1", port)).expect("发现端口不该被占用");
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn occupying_the_tcp_port_attributes_the_failure_to_the_transfer_stage() {
    let dir = temp_dir("tcpport");
    let port = free_port();
    let discovery = free_udp_port();
    // 先把 TCP 端口占住
    let _blocker = TcpListener::bind(("0.0.0.0", port)).expect("占住 TCP 端口");

    match TransferService::start(settings_for(&dir, port, discovery), "测试机") {
        Err(StartFailure::Transfer { port: p, .. }) => assert_eq!(p, port),
        other => panic!("应归因到传输段，实得 {other:?}"),
    }
    // 专属措辞（F-FT-02）
    let text = StartFailure::Transfer {
        port,
        cause: io::Error::new(io::ErrorKind::AddrInUse, "x"),
    }
    .to_string();
    assert!(text.contains("传输服务启动失败"), "实得 {text}");
    assert!(text.contains("可能被占用"), "实得 {text}");
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn occupying_the_udp_port_attributes_to_discovery_and_rolls_back_transfer() {
    let dir = temp_dir("udpport");
    let port = free_port();
    let discovery = free_udp_port();
    let _blocker = UdpSocket::bind(("0.0.0.0", discovery)).expect("占住 UDP 端口");

    match TransferService::start(settings_for(&dir, port, discovery), "测试机") {
        Err(StartFailure::Discovery {
            port: p,
            rollback_ok,
            ..
        }) => {
            assert_eq!(p, discovery);
            // 🔴 R-28：任一段失败要回滚已起来的部分
            assert!(rollback_ok, "传输服务必须被回滚（TCP {port} 应已释放）");
        }
        other => panic!("应归因到发现段，实得 {other:?}"),
    }
    // 回滚后 TCP 端口真的能再绑
    TcpListener::bind(("0.0.0.0", port)).expect("回滚后传输端口应可再绑");
    // 专属措辞
    let text = StartFailure::Discovery {
        port: discovery,
        cause: io::Error::new(io::ErrorKind::AddrInUse, "x"),
        rollback_ok: true,
    }
    .to_string();
    assert!(text.contains("设备发现服务启动失败"), "实得 {text}");
    assert!(text.contains("已回滚"), "实得 {text}");
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn start_then_stop_releases_ports_and_reports_honestly() {
    let dir = temp_dir("lifecycle");
    let port = free_port();
    let discovery = free_udp_port();
    let service =
        TransferService::start(settings_for(&dir, port, discovery), "测试机").expect("应启动成功");
    assert_eq!(service.local_addr().port(), port);
    assert_eq!(service.discovery_port(), discovery);
    assert_eq!(service.local_addr().ip().to_string(), "0.0.0.0");
    assert_eq!(service.live_connections(), 0);
    assert_eq!(service.receive_root(), dir);

    let report = service.stop();
    assert!(report.all_stopped, "正常停止应报「全部停住」：{report:?}");
    assert_eq!(report.threads_joined, 2, "接受循环 + 发现循环");
    assert!(
        report.describe().contains("已停止"),
        "措辞：{}",
        report.describe()
    );

    // 🔴 端口真的释放了（"停止"不是嘴上说说）
    TcpListener::bind(("0.0.0.0", port)).expect("停止后传输端口应可再绑");
    UdpSocket::bind(("0.0.0.0", discovery)).expect("停止后发现端口应可再绑");
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn stop_wording_admits_an_incomplete_stop() {
    // R-28 原文：停止失败不得宣称已停止
    let report = StopReport {
        threads_joined: 1,
        all_stopped: false,
    };
    let text = report.describe();
    assert!(text.contains("停止不完整"), "实得 {text}");
    assert!(text.contains("可能仍在运行"), "实得 {text}");
    assert!(!text.contains("✅"), "不得给成功标记");
}

// ── 端到端：真服务 + 真 socket + 真配对 ──────────────────────────────────

/// 🔴 本批的核心用例：前九批的零件在这里**第一次全部串起来**。
#[test]
fn end_to_end_transfer_with_pairing_over_a_real_service() {
    let dir = temp_dir("e2e");
    let recv_dir = dir.join("recv");
    fs::create_dir_all(&recv_dir).unwrap();
    let send_dir = dir.join("send");
    fs::create_dir_all(&send_dir).unwrap();

    let port = free_port();
    let discovery = free_udp_port();
    let service =
        TransferService::start(settings_for(&recv_dir, port, discovery), "测试机").expect("起服务");

    // 配对码由接收端生成并展示
    let code = service.current_pairing_code(now_unix_ms());
    assert_eq!(code.len(), 6, "R-2：6 位");
    assert!(service.pairing_expires_in_ms(now_unix_ms()) > 0);

    let payload: Vec<u8> = (0..5_000_u32).map(|i| (i % 233) as u8).collect();
    let file = send_dir.join("示例 文件.bin");
    fs::write(&file, &payload).unwrap();
    let peer: SocketAddr = format!("127.0.0.1:{port}").parse().unwrap();
    let cancel = CancellationToken::new();

    // ① 正确的码 → 成功
    let outcome = send_to(
        peer,
        &file,
        crate::transfer::chunk::DEFAULT_CHUNK_SIZE,
        Some(&code),
        "task-e2e-1",
        &cancel,
        |_| {},
    )
    .expect("带正确配对码应成功");
    assert_eq!(outcome, SendOutcome::Delivered);
    let landed = recv_dir.join("示例 文件.bin");
    assert!(landed.exists(), "文件应落在接收目录");
    assert_eq!(fs::read(&landed).unwrap(), payload, "内容必须逐字节一致");

    // ② 同一地址再来一次，**不带码** → 本次运行期免码（F-FT-13）
    let second = send_dir.join("第二份.bin");
    fs::write(&second, b"second payload").unwrap();
    let outcome = send_to(
        peer,
        &second,
        crate::transfer::chunk::DEFAULT_CHUNK_SIZE,
        None,
        "task-e2e-2",
        &cancel,
        |_| {},
    )
    .expect("同一地址本次运行期免码");
    assert_eq!(outcome, SendOutcome::Delivered);
    assert_eq!(
        fs::read(recv_dir.join("第二份.bin")).unwrap(),
        b"second payload"
    );

    let report = service.stop();
    assert!(report.all_stopped);
    let _ = fs::remove_dir_all(&dir);
}

/// 🔴 配对是**唯一信任根**：没有码 / 错码都必须被拒，且原因码可读（R-1 / F-FT-13）。
#[test]
fn wrong_or_missing_pair_code_is_rejected_with_a_readable_reason() {
    let dir = temp_dir("pairfail");
    let recv_dir = dir.join("recv");
    fs::create_dir_all(&recv_dir).unwrap();
    let send_dir = dir.join("send");
    fs::create_dir_all(&send_dir).unwrap();

    let port = free_port();
    let discovery = free_udp_port();
    let service =
        TransferService::start(settings_for(&recv_dir, port, discovery), "测试机").expect("起服务");
    let peer: SocketAddr = format!("127.0.0.1:{port}").parse().unwrap();
    let cancel = CancellationToken::new();
    let file = send_dir.join("拒绝我.bin");
    fs::write(&file, b"nope").unwrap();

    // 错码 → 拒，且回传 PAIRING_INVALID
    match send_to(
        peer,
        &file,
        crate::transfer::chunk::DEFAULT_CHUNK_SIZE,
        Some("ZZZZZZ"),
        "task-bad",
        &cancel,
        |_| {},
    ) {
        Err(SendFailure::Transport(TransportError::Rejected { reason_code })) => {
            assert_eq!(reason_code, reasons::PAIRING_INVALID);
        }
        other => panic!("错码应被拒，实得 {other:?}"),
    }
    // 不带码 → 同样拒
    match send_to(
        peer,
        &file,
        crate::transfer::chunk::DEFAULT_CHUNK_SIZE,
        None,
        "task-nocode",
        &cancel,
        |_| {},
    ) {
        Err(SendFailure::Transport(TransportError::Rejected { reason_code })) => {
            assert_eq!(reason_code, reasons::PAIRING_INVALID);
        }
        other => panic!("缺码应被拒，实得 {other:?}"),
    }

    // 🔴 拒收期间**不得**在接收目录里留下任何东西（R-13：闸门在"建未完成文件"之前）
    let leftovers: Vec<String> = fs::read_dir(&recv_dir)
        .unwrap()
        .flatten()
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .collect();
    assert_eq!(
        leftovers,
        Vec::<String>::new(),
        "拒收不得留下文件：{leftovers:?}"
    );

    // 正确码仍应能配上（说明前面的失败没把接收端搞坏）
    let code = service.current_pairing_code(now_unix_ms());
    assert_eq!(
        send_to(
            peer,
            &file,
            crate::transfer::chunk::DEFAULT_CHUNK_SIZE,
            Some(&code),
            "task-good",
            &cancel,
            |_| {}
        )
        .expect("正确码应成功"),
        SendOutcome::Delivered
    );
    assert!(recv_dir.join("拒绝我.bin").exists());

    let _ = service.stop();
    let _ = fs::remove_dir_all(&dir);
}

/// 白名单开着时，未发现的地址必须被拒（R-1：白名单是便利过滤，但仍按 F-FT-09 生效）。
#[test]
fn unknown_peer_is_rejected_when_the_whitelist_is_on() {
    let dir = temp_dir("whitelist");
    let recv_dir = dir.join("recv");
    fs::create_dir_all(&recv_dir).unwrap();
    let send_dir = dir.join("send");
    fs::create_dir_all(&send_dir).unwrap();

    let port = free_port();
    let discovery = free_udp_port();
    let mut settings = settings_for(&recv_dir, port, discovery);
    settings.require_known_peer = true; // 开白名单
    let service = TransferService::start(settings, "测试机").expect("起服务");
    let code = service.current_pairing_code(now_unix_ms());

    let peer: SocketAddr = format!("127.0.0.1:{port}").parse().unwrap();
    let file = send_dir.join("不该进来.bin");
    fs::write(&file, b"x").unwrap();
    let cancel = CancellationToken::new();

    // 即便带了正确的码，白名单这一关先拦（R-13 的顺序：白名单在配对之前）
    match send_to(
        peer,
        &file,
        crate::transfer::chunk::DEFAULT_CHUNK_SIZE,
        Some(&code),
        "task-wl",
        &cancel,
        |_| {},
    ) {
        Err(SendFailure::Transport(TransportError::Rejected { reason_code })) => {
            assert_eq!(reason_code, reasons::PEER_NOT_DISCOVERED, "应卡在白名单关");
        }
        other => panic!("未发现设备应被拒，实得 {other:?}"),
    }
    // 已知清单里的地址则放行（F-FT-09：已发现**或**已列入清单）
    // —— 这里直接构造"清单命中"要比造一次真发现便宜得多
    let _ = service.stop();
    let _ = fs::remove_dir_all(&dir);
}

// ── 零碎但会被别处依赖的口径 ────────────────────────────────────────────

#[test]
fn peer_token_is_stable_per_ip_and_ignores_the_ephemeral_port() {
    let a: IpAddr = "192.168.1.24".parse().unwrap();
    let b: IpAddr = "192.168.1.25".parse().unwrap();
    assert_eq!(peer_token_for(a), peer_token_for(a), "同一地址必须稳定");
    assert_ne!(peer_token_for(a), peer_token_for(b), "不同地址必须可区分");
    assert_eq!(peer_token_for(a), "v4c0a80118", "可读的十六进制形式");
    // IPv6 也要能给出稳定值（母本用 LastIndexOf(':') 会切错，本工程不会）
    let v6: IpAddr = "fe80::1".parse().unwrap();
    assert_eq!(peer_token_for(v6), peer_token_for(v6));
    assert!(peer_token_for(v6).starts_with("v6"));
}

/// 🔴 R-1：配对开关**默认必须开**，且**不落盘**（F-FT-59 的持久化清单没列它）。
#[test]
fn pairing_switch_defaults_on_and_is_not_persisted() {
    let settings = TransferSettings::default();
    assert!(settings.require_pairing, "R-1：配对默认要求");

    // 不落盘：序列化里不得出现该字段
    let value = serde_json::to_value(&settings).unwrap();
    let obj = value.as_object().unwrap();
    assert!(
        !obj.contains_key("requirePairing"),
        "配对开关不得落盘（服务重启后应回到安全默认）"
    );
    // 反序列化时也不认它（写进文件也没用）
    let restored: TransferSettings = serde_json::from_str(r#"{"requirePairing":false}"#).unwrap();
    assert!(restored.require_pairing, "文件里的 false 不该生效");
}

#[test]
fn max_receive_concurrency_comes_from_the_single_source() {
    assert_eq!(
        TransferService::max_receive_concurrency(),
        usize::try_from(MAX_RECEIVE_CONCURRENCY).unwrap()
    );
}

#[test]
fn known_peers_hit_makes_a_peer_known_without_discovery() {
    // 直接验证 ctx 的白名单判定口径（F-FT-09："已发现**或**已列入清单"）
    let dir = temp_dir("known");
    let settings = TransferSettings {
        known_peers: vec![KnownPeer {
            name: "书房台式机".to_owned(),
            ip: "127.0.0.1".to_owned(),
            port: 18_889,
        }],
        ..TransferSettings::default()
    };
    let identity = SelfIdentity::new("测试机", "tok");
    let ctx = ServiceContext {
        settings,
        identity,
        receive_root: dir.clone(),
        pairing: Mutex::new(PairingGuard::new(true)),
        confirm: ConfirmGate::default(),
        tasks: Mutex::new(TaskRegistry::default()),
        history: Mutex::new(crate::transfer::history::History::new()),
        history_path: std::env::temp_dir().join("aegis-hist-fixture.json"),
        cancels: Mutex::new(HashMap::new()),
        pauses: Mutex::new(HashMap::new()),
        task_seq: AtomicU64::new(1),
        peers: Mutex::new(PeerTable::new("测试机#tok", OFFLINE_AFTER)),
        last_refresh_ms: AtomicI64::new(0),
        active_receives: AtomicUsize::new(0),
    };
    assert!(ctx.is_known_peer("127.0.0.1".parse().unwrap()), "清单命中");
    assert!(!ctx.is_known_peer("10.0.0.1".parse().unwrap()), "清单外");
    let _ = fs::remove_dir_all(&dir);
}
