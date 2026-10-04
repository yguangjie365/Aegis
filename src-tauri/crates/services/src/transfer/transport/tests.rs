//! `transport` 的测试。
//!
//! 全部走**真实 TCP 环回**（`127.0.0.1:0`，端口由系统分配）：本模块的正确性有一半在
//! "两个阻塞线程真的能按协议对上话"上，用内存流替不掉。每条用例都设了读超时，不会挂死。

use std::fs;
use std::io::Write;
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::path::PathBuf;
use std::thread::JoinHandle;

use super::*;
use crate::transfer::admission::{Admission, Gate};

/// 测试用短超时：正常路径远用不到，异常路径上保证用例不会挂死。
const TEST_TIMEOUT: Duration = Duration::from_secs(10);

fn temp_dir(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("aegis-transport-{}-{}", std::process::id(), tag));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).expect("建临时目录");
    dir
}

/// 起一个环回监听，接受 `accepts` 次连接后把每个连接交给 `f`；返回地址与线程句柄。
fn spawn_server<T, F>(accepts: usize, f: F) -> (SocketAddr, JoinHandle<Vec<T>>)
where
    T: Send + 'static,
    F: Fn(TcpStream) -> T + Send + Sync + 'static,
{
    let listener = TcpListener::bind("127.0.0.1:0").expect("绑定环回");
    let addr = listener.local_addr().expect("取地址");
    let handle = std::thread::spawn(move || {
        let mut results = Vec::with_capacity(accepts);
        for _ in 0..accepts {
            let (stream, _) = listener.accept().expect("应收到连接");
            configure_timeout(&stream, TEST_TIMEOUT).expect("设超时");
            results.push(f(stream));
        }
        results
    });
    (addr, handle)
}

fn connect(addr: SocketAddr) -> TcpStream {
    let stream = TcpStream::connect(addr).expect("连接环回");
    configure_timeout(&stream, TEST_TIMEOUT).expect("设超时");
    stream
}

/// 一律放行（第二个参数是本次冲突策略，本辅助不改它）。
fn always_admit(_facts: &HandshakeFacts<'_>, _policy: &mut ConflictPolicy) -> Admission {
    Admission::Admitted {
        disk_unknown: false,
    }
}

/// 🔴 回归闸：等回执的上限**必须严格大于**接收端的"接收前询问"窗口。
///
/// 这条不变式是真机端到端跑出来的教训：两者曾同为 30 秒，
/// 于是开启"接收前确认"（Q-15 默认开）时发送方必然先读超时，
/// 而接收端还在等用户点按钮——两端给出**互相矛盾**的结论。
/// 它靠肉眼很难发现（两个常量在不同文件里），故用测试钉住。
#[test]
fn handshake_wait_outlasts_the_receive_confirmation_window() {
    assert!(
        HANDSHAKE_WAIT > crate::transfer::confirm::CONFIRM_TIMEOUT,
        "HANDSHAKE_WAIT({HANDSHAKE_WAIT:?}) 必须严格大于 CONFIRM_TIMEOUT({:?})，否则开启接收确认时发送方会先超时",
        crate::transfer::confirm::CONFIRM_TIMEOUT
    );
    // 也不能只多一点点：要留出「问完用户之后真正收发」的余地
    assert!(
        HANDSHAKE_WAIT >= crate::transfer::confirm::CONFIRM_TIMEOUT + IO_TIMEOUT,
        "握手宽限应覆盖「询问窗口 + 一次常规 IO 超时」"
    );
}

/// 备好"源文件 + 摘要 + SendSource"三件套。
fn prepare(dir: &Path, payload: &[u8], chunk_size: u64) -> (SendSource, String, PathBuf) {
    let path = dir.join("payload.bin");
    fs::write(&path, payload).expect("写源文件");
    let hash = crate::transfer::source::hash_file(&path).expect("算摘要");
    let source = SendSource::open(&path, chunk_size).expect("打开源");
    (source, hash, path)
}

// ── 帧编解码 ──────────────────────────────────────────────────────────────

#[test]
fn frame_codec_round_trips_over_an_in_memory_buffer() {
    let mut buffer: Vec<u8> = Vec::new();
    let message = TransferMessage::Chunk {
        task_id: "t1".to_owned(),
        index: 3,
        offset: 6,
    };
    write_frame(&mut buffer, &message, b"abcdef").unwrap();

    let mut reader = FrameReader::new(std::io::Cursor::new(buffer));
    let (decoded, body) = reader.read_frame().unwrap();
    assert_eq!(decoded, message);
    assert_eq!(body, b"abcdef");
}

#[test]
fn frame_reader_handles_a_frame_split_across_reads() {
    // 把一帧拆成"每次只给 1 字节"的流：读取器必须攒够再返回
    struct OneByte(Vec<u8>, usize);
    impl Read for OneByte {
        fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
            if self.1 >= self.0.len() || buf.is_empty() {
                return Ok(0);
            }
            buf[0] = self.0[self.1];
            self.1 += 1;
            Ok(1)
        }
    }

    let mut buffer: Vec<u8> = Vec::new();
    write_frame(
        &mut buffer,
        &TransferMessage::Text {
            task_id: "t".to_owned(),
            chars: 2,
            bytes: 4,
            from_device: None,
        },
        b"text",
    )
    .unwrap();
    let total = buffer.len();

    let mut reader = FrameReader::new(OneByte(buffer, 0));
    let (message, body) = reader.read_frame().unwrap();
    assert!(matches!(message, TransferMessage::Text { .. }));
    assert_eq!(body, b"text");
    assert!(
        total > 20,
        "本用例前提：帧确实被拆成多次读（共 {total} 字节）"
    );
}

#[test]
fn frame_errors_propagate_and_never_panic() {
    // 🔴 首帧必须先长到定长前缀（13 字节）才谈得上比对魔数；
    // 短于前缀只会被判"没读满"而继续等数据（见 peer_closing_mid_frame 用例）。
    let mut bad_magic = b"XXXX\x01".to_vec();
    bad_magic.extend_from_slice(&[0_u8; 9]); // 总长 14 ≥ PREFIX_LEN
    let mut reader = FrameReader::new(std::io::Cursor::new(bad_magic));
    assert!(matches!(
        reader.read_frame(),
        Err(TransportError::Frame(protocol::FrameError::BadMagic))
    ));

    // 版本不符（魔数对、版本 9）
    let mut bad_version = b"AGFT\x09".to_vec();
    bad_version.extend_from_slice(&[0_u8; 9]);
    let mut reader = FrameReader::new(std::io::Cursor::new(bad_version));
    assert!(matches!(
        reader.read_frame(),
        Err(TransportError::Frame(
            protocol::FrameError::UnsupportedVersion(9)
        ))
    ));
}

#[test]
fn peer_closing_mid_frame_is_reported_as_peer_closed() {
    let mut buffer: Vec<u8> = Vec::new();
    write_frame(
        &mut buffer,
        &TransferMessage::Pause {
            task_id: "t".into(),
        },
        &[],
    )
    .unwrap();
    buffer.truncate(buffer.len() / 2); // 半帧后没人再写 → EOF
    let mut reader = FrameReader::new(std::io::Cursor::new(buffer));
    assert!(matches!(
        reader.read_frame(),
        Err(TransportError::PeerClosed)
    ));
}

// ── 完整会话（真 socket） ─────────────────────────────────────────────────

/// 🔴 本批最有价值的一条：两个**真实线程**各持一端 TCP，跑完整会话。
#[test]
fn send_and_receive_round_trip_over_tcp() {
    let dir = temp_dir("roundtrip");
    let send_dir = dir.join("send");
    let recv_dir = dir.join("recv");
    fs::create_dir_all(&send_dir).unwrap();
    fs::create_dir_all(&recv_dir).unwrap();
    let payload: Vec<u8> = (0..1000_u32).map(|i| (i % 251) as u8).collect();

    let (addr, server) = spawn_server(1, move |stream| {
        let cancel = CancellationToken::new();
        receive_file(
            &stream,
            &recv_dir,
            "peer1",
            &mut ConflictPolicy::Rename,
            always_admit,
            &cancel,
            |_| {},
        )
    });

    let (mut source, hash, _) = prepare(&send_dir, &payload, 256);
    let stream = connect(addr);
    let cancel = CancellationToken::new();
    let sent = send_file(
        &stream,
        &mut source,
        &hash,
        "task-1",
        None,
        None,
        &cancel,
        |_| {},
    );

    let received = server.join().expect("接收线程");
    assert_eq!(sent.unwrap(), SendOutcome::Delivered);
    let outcome = received.into_iter().next().unwrap().unwrap();
    match outcome {
        ReceiveOutcome::Committed { path, renamed } => {
            assert!(!renamed);
            assert_eq!(fs::read(&path).unwrap(), payload, "落盘内容必须逐字节一致");
        }
        other => panic!("应入位，实得 {other:?}"),
    }
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn rejected_handshake_creates_no_file_and_tells_the_sender_why() {
    let dir = temp_dir("rejected");
    let send_dir = dir.join("send");
    let recv_dir = dir.join("recv");
    fs::create_dir_all(&send_dir).unwrap();
    fs::create_dir_all(&recv_dir).unwrap();
    let payload = b"do not want this".to_vec();

    let (addr, server) = spawn_server(1, move |stream| {
        let cancel = CancellationToken::new();
        receive_file(
            &stream,
            &recv_dir,
            "peer1",
            &mut ConflictPolicy::Rename,
            |_, _| Admission::Rejected {
                gate: Gate::Whitelist,
                reason: crate::transfer::reasons::PEER_NOT_DISCOVERED,
            },
            &cancel,
            |_| {},
        )
    });

    let (mut source, hash, _) = prepare(&send_dir, &payload, 256);
    let stream = connect(addr);
    let cancel = CancellationToken::new();
    let sent = send_file(
        &stream,
        &mut source,
        &hash,
        "task-2",
        None,
        None,
        &cancel,
        |_| {},
    );

    // 发送方拿到明确拒收与原因码
    match sent {
        Err(TransportError::Rejected { reason_code }) => {
            assert_eq!(reason_code, crate::transfer::reasons::PEER_NOT_DISCOVERED);
        }
        other => panic!("应被拒收，实得 {other:?}"),
    }
    // 接收方结论带闸门名
    let outcome = server.join().unwrap().into_iter().next().unwrap().unwrap();
    match outcome {
        ReceiveOutcome::Rejected { reason_code } => {
            assert!(reason_code.starts_with("whitelist:"), "实得 {reason_code}");
        }
        other => panic!("应拒收，实得 {other:?}"),
    }
    // 🔴 拒收即不建任何文件（R-13 顺序：闸门在建未完成文件之前）
    let leftovers: Vec<_> = fs::read_dir(dir.join("recv")).unwrap().flatten().collect();
    assert!(leftovers.is_empty(), "拒收后目录必须干净");
    let _ = fs::remove_dir_all(&dir);
}

/// 断连后续传：第一条连接传一半就断，第二条连接从真实进度接上，最终仍校验通过。
// 🔴 允许超长：这是一条**单一场景的端到端**（断连 → 续传 → 校验），
// 拆成多个辅助函数会让"这个场景到底做了什么"散落四处，反而更难读。
// 本批仅因给 send_file 多传一个进度回调参数而从 100 行越到 102 行。
#[allow(clippy::too_many_lines)]
#[test]
fn interrupted_transfer_resumes_on_a_second_connection() {
    let dir = temp_dir("resume");
    let send_dir = dir.join("send");
    let recv_dir = dir.join("recv");
    fs::create_dir_all(&send_dir).unwrap();
    fs::create_dir_all(&recv_dir).unwrap();
    let payload: Vec<u8> = (0..600_u32).map(|i| (i % 97) as u8).collect();

    let recv_dir_second = recv_dir.clone();
    // join 之后还要检查目录，故留一份自己用（闭包会拿走另一个）
    let recv_dir_check = recv_dir.clone();
    let (addr, server) = spawn_server(2, move |stream| -> Result<ReceiveOutcome, TransportError> {
        // 第一条连接：只收前两块就主动断开（模拟网络中断）
        static CALL: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
        let nth = CALL.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        if nth == 0 {
            // 🔴 第一条连接必须**真的落两块盘**再断——只读帧不留 .part，
            // 那就没有"未完成文件"可供第二条连接续传（首版就是这么写错的）。
            let mut reader = FrameReader::new(&stream);
            let (handshake, _) = reader.read_frame()?;
            let TransferMessage::Handshake {
                file_name,
                file_size,
                ..
            } = handshake
            else {
                return Err(TransportError::Unexpected("首帧必须是 Handshake"));
            };
            let mut sink = ReceiveSink::open(&recv_dir_second, &file_name, "peer1", file_size)?;
            let mut writer = &stream;
            write_frame(
                &mut writer,
                &TransferMessage::HandshakeAck {
                    task_id: "t".into(),
                    accepted: true,
                    reason_code: None,
                    resume_from: Some(sink.received()),
                    chunk_size: None,
                },
                &[],
            )?;
            // 收两块写盘后断连（**不** finish、**不** discard —— 模拟网络中断）
            for _ in 0..2 {
                let (message, body) = reader.read_frame()?;
                if let TransferMessage::Chunk { offset, .. } = message {
                    sink.append(offset, &body)?;
                }
            }
            drop(reader);
            drop(stream);
            return Err(TransportError::PeerClosed);
        }
        let cancel = CancellationToken::new();
        receive_file(
            &stream,
            &recv_dir_second,
            "peer1",
            &mut ConflictPolicy::Rename,
            always_admit,
            &cancel,
            |_| {},
        )
    });

    // 第一段：发到一半会遇到断连
    let (mut source, hash, _) = prepare(&send_dir, &payload, 128);
    let stream = connect(addr);
    let cancel = CancellationToken::new();
    let first = send_file(
        &stream,
        &mut source,
        &hash,
        "task-3",
        None,
        None,
        &cancel,
        |_| {},
    );
    assert!(first.is_err(), "断连必须报错，实得 {first:?}");

    // 未完成文件应留在盘上（R-21 的前提）
    let parts: Vec<String> = fs::read_dir(&recv_dir_check)
        .unwrap()
        .flatten()
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .filter(|n| {
            std::path::Path::new(n)
                .extension()
                .is_some_and(|e| e == "part")
        })
        .collect();
    assert_eq!(parts.len(), 1, "断连后应留下一个未完成文件，实得 {parts:?}");

    // 第二段：重新连接，接收方报告真实进度，发送方从那里接上
    let stream = connect(addr);
    let mut source = SendSource::open(&send_dir.join("payload.bin"), 128).unwrap();
    let second = send_file(
        &stream,
        &mut source,
        &hash,
        "task-4",
        None,
        None,
        &cancel,
        |_| {},
    );

    let results = server.join().unwrap();
    assert_eq!(second.unwrap(), SendOutcome::Delivered, "续传应成功");
    match results.into_iter().nth(1).unwrap().unwrap() {
        ReceiveOutcome::Committed { path, .. } => {
            assert_eq!(fs::read(&path).unwrap(), payload, "续传后必须逐字节完整");
        }
        other => panic!("续传应入位，实得 {other:?}"),
    }
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn hash_mismatch_is_reported_to_both_sides_and_leaves_nothing() {
    let dir = temp_dir("mismatch");
    let send_dir = dir.join("send");
    let recv_dir = dir.join("recv");
    fs::create_dir_all(&send_dir).unwrap();
    fs::create_dir_all(&recv_dir).unwrap();
    let payload = b"content".to_vec();

    let (addr, server) = spawn_server(1, move |stream| {
        let cancel = CancellationToken::new();
        receive_file(
            &stream,
            &recv_dir,
            "peer1",
            &mut ConflictPolicy::Rename,
            always_admit,
            &cancel,
            |_| {},
        )
    });

    let (mut source, _real_hash, _) = prepare(&send_dir, &payload, 256);
    let stream = connect(addr);
    let cancel = CancellationToken::new();
    // 故意给一个错的摘要（模拟"对端文件与声明不符"）
    let wrong = "0".repeat(64);
    let sent = send_file(
        &stream,
        &mut source,
        &wrong,
        "task-5",
        None,
        None,
        &cancel,
        |_| {},
    );

    let outcome = server.join().unwrap().into_iter().next().unwrap().unwrap();
    assert_eq!(outcome, ReceiveOutcome::HashMismatch);
    assert_eq!(
        sent.unwrap(),
        SendOutcome::HashMismatch,
        "发送方应得知摘要不符"
    );
    // F-FT-20：损坏的未完成文件必须被删掉，目录里什么也不该剩
    let leftovers: Vec<_> = fs::read_dir(dir.join("recv")).unwrap().flatten().collect();
    assert!(leftovers.is_empty(), "摘要不符后目录必须干净");
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn conflict_skip_is_reported_as_skipped_not_delivered() {
    let dir = temp_dir("skip");
    let send_dir = dir.join("send");
    let recv_dir = dir.join("recv");
    fs::create_dir_all(&send_dir).unwrap();
    fs::create_dir_all(&recv_dir).unwrap();
    let payload = b"incoming".to_vec();
    // 预置同名文件
    fs::write(recv_dir.join("payload.bin"), b"original").unwrap();

    let (addr, server) = spawn_server(1, move |stream| {
        let cancel = CancellationToken::new();
        receive_file(
            &stream,
            &recv_dir,
            "peer1",
            &mut ConflictPolicy::Skip,
            always_admit,
            &cancel,
            |_| {},
        )
    });

    let (mut source, hash, _) = prepare(&send_dir, &payload, 256);
    let stream = connect(addr);
    let cancel = CancellationToken::new();
    let sent = send_file(
        &stream,
        &mut source,
        &hash,
        "task-6",
        None,
        None,
        &cancel,
        |_| {},
    );

    let outcome = server.join().unwrap().into_iter().next().unwrap().unwrap();
    assert_eq!(outcome, ReceiveOutcome::Skipped);
    assert_eq!(
        sent.unwrap(),
        SendOutcome::Skipped,
        "终态是「已跳过」而非「已完成」"
    );
    // 原文件一字未动
    assert_eq!(
        fs::read(dir.join("recv").join("payload.bin")).unwrap(),
        b"original"
    );
    let _ = fs::remove_dir_all(&dir);
}

/// 🔴 取消：本机令牌触发 → **主动告知对端**，且未完成文件保留（可续传）。
#[test]
fn cancel_notifies_the_peer_and_keeps_the_partial_file() {
    let dir = temp_dir("cancel");
    let send_dir = dir.join("send");
    let recv_dir = dir.join("recv");
    fs::create_dir_all(&send_dir).unwrap();
    fs::create_dir_all(&recv_dir).unwrap();
    // 文件大到足以在取消前只发一部分
    let payload: Vec<u8> = vec![7_u8; 200_000];

    let recv_dir_clone = recv_dir.clone();
    let (addr, server) = spawn_server(1, move |stream| {
        let cancel = CancellationToken::new();
        receive_file(
            &stream,
            &recv_dir_clone,
            "peer1",
            &mut ConflictPolicy::Rename,
            always_admit,
            &cancel,
            |_| {},
        )
    });

    let (mut source, hash, _) = prepare(&send_dir, &payload, 8 * 1024);
    let stream = connect(addr);
    let cancel = CancellationToken::new();
    cancel.cancel(); // 会话开始前就取消：第一轮循环即中止
    let sent = send_file(
        &stream,
        &mut source,
        &hash,
        "task-7",
        None,
        None,
        &cancel,
        |_| {},
    );

    assert!(
        matches!(sent, Err(TransportError::Aborted)),
        "实得 {sent:?}"
    );
    // 接收方应收到 Cancel（而不是一直等到自己的读超时）
    let outcome = server.join().unwrap().into_iter().next().unwrap();
    match outcome {
        Err(TransportError::Cancelled { reason_code }) => {
            assert_eq!(reason_code, crate::transfer::reasons::USER_CANCEL);
        }
        other => panic!("对端应收到取消，实得 {other:?}"),
    }
    // 🔴 未完成文件保留（中断后 .part 正是续传的前提）
    let parts: Vec<String> = fs::read_dir(dir.join("recv"))
        .unwrap()
        .flatten()
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .filter(|n| {
            std::path::Path::new(n)
                .extension()
                .is_some_and(|e| e == "part")
        })
        .collect();
    assert_eq!(parts.len(), 1, "取消后未完成文件应保留，实得 {parts:?}");
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn garbage_on_the_wire_is_rejected_without_panicking() {
    let dir = temp_dir("garbage");
    let recv_dir = dir.join("recv");
    fs::create_dir_all(&recv_dir).unwrap();

    let (addr, server) = spawn_server(1, move |stream| {
        let cancel = CancellationToken::new();
        receive_file(
            &stream,
            &recv_dir,
            "peer1",
            &mut ConflictPolicy::Rename,
            always_admit,
            &cancel,
            |_| {},
        )
    });

    let mut stream = connect(addr);
    stream
        .write_all(b"GET / HTTP/1.1\r\nHost: x\r\n\r\n")
        .unwrap();
    stream.flush().unwrap();

    match server.join().unwrap().into_iter().next().unwrap() {
        Err(TransportError::Frame(protocol::FrameError::BadMagic)) => {}
        other => panic!("垃圾首帧应报魔数不符，实得 {other:?}"),
    }
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn peer_that_never_speaks_is_cut_off_by_the_read_timeout() {
    let dir = temp_dir("silent");
    let recv_dir = dir.join("recv");
    fs::create_dir_all(&recv_dir).unwrap();

    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();
    let handle = std::thread::spawn(move || {
        let (stream, _) = listener.accept().unwrap();
        // 极短超时：模拟"连上就不发数据的对端"
        configure_timeout(&stream, Duration::from_millis(150)).unwrap();
        let cancel = CancellationToken::new();
        receive_file(
            &stream,
            &recv_dir,
            "peer1",
            &mut ConflictPolicy::Rename,
            always_admit,
            &cancel,
            |_| {},
        )
    });

    let _quiet = connect(addr); // 连上后什么都不发
    match handle.join().unwrap() {
        Err(TransportError::TimedOut) => {}
        other => panic!("静默对端应被读超时切断，实得 {other:?}"),
    }
    let _ = fs::remove_dir_all(&dir);
}

// ── 文本事务（F-FT-33/34） ────────────────────────────────────────────────
//
// 🔴 这些用例**一律注入 `commit`**，绝不碰真实剪贴板：
//    第 45 轮那次 `STATUS_HEAP_CORRUPTION` 就是并发操作真实剪贴板造成的
//    （详见 `infra::clipboard` 模块头）。

/// 一律准入（文本侧的白名单/配对另有专测，这里只测传输流程）。
fn admit_text(_facts: &TextFacts<'_>) -> Admission {
    Admission::Admitted {
        disk_unknown: false,
    }
}

#[test]
fn text_round_trip_reports_delivered_when_commit_succeeds() {
    let (addr, server) = spawn_server(1, |stream| {
        let cancel = CancellationToken::new();
        receive_text(&stream, admit_text, |_text| Ok(()), &cancel)
    });
    let stream = connect(addr);
    let cancel = CancellationToken::new();
    let outcome = send_text(
        &stream,
        "你好，互传 🚀",
        "t-text-1",
        Some("测试机"),
        &cancel,
    )
    .expect("发送文本");
    assert_eq!(outcome, TextOutcome::Delivered);
    assert_eq!(
        server.join().unwrap()[0].as_ref().unwrap(),
        &TextOutcome::Delivered
    );
}

/// 🔴 F-FT-34 的红线：**交付失败必须回 `CLIPBOARD_WRITE_FAILED`，不许回"已送达"**。
///
/// 这条不守，用户会看到"发送成功"而对方剪贴板里什么都没有——
/// 一个**两头都无从察觉**的谎言。
#[test]
fn text_commit_failure_surfaces_clipboard_write_failed_not_delivered() {
    let (addr, server) = spawn_server(1, |stream| {
        let cancel = CancellationToken::new();
        // 模拟"剪贴板被占用/写不进去"
        receive_text(
            &stream,
            admit_text,
            |_text| Err("剪贴板被其它程序占用".to_owned()),
            &cancel,
        )
    });
    let stream = connect(addr);
    let cancel = CancellationToken::new();
    let outcome = send_text(&stream, "写不进去的文本", "t-text-2", None, &cancel)
        .expect("发送文本（业务性拒绝不算传输错误）");
    let expected = TextOutcome::Rejected {
        reason_code: crate::transfer::reasons::CLIPBOARD_WRITE_FAILED.to_owned(),
    };
    assert_eq!(
        outcome, expected,
        "交付失败必须如实回传 CLIPBOARD_WRITE_FAILED"
    );
    assert_ne!(outcome, TextOutcome::Delivered, "绝不能回已送达");
    assert_eq!(server.join().unwrap()[0].as_ref().unwrap(), &expected);
}

#[test]
fn text_admission_rejection_returns_the_gate_reason() {
    let (addr, server) = spawn_server(1, |stream| {
        let cancel = CancellationToken::new();
        receive_text(
            &stream,
            |_facts| Admission::Rejected {
                gate: Gate::Whitelist,
                reason: crate::transfer::reasons::PEER_NOT_DISCOVERED,
            },
            |_text| Ok(()),
            &cancel,
        )
    });
    let stream = connect(addr);
    let cancel = CancellationToken::new();
    let outcome = send_text(&stream, "应被白名单拒", "t-text-3", None, &cancel).expect("发送");
    assert_eq!(
        outcome,
        TextOutcome::Rejected {
            reason_code: crate::transfer::reasons::PEER_NOT_DISCOVERED.to_owned()
        }
    );
    let _ = server.join();
}

/// 声明字节数与实际 body 不符 ⇒ 拒收并回 `TEXT_TOO_LONG`，
/// 🔴 **不按声明值裁剪**（宁可拒绝，也不交付半个文本）。
#[test]
fn text_with_a_lying_byte_count_is_rejected_not_truncated() {
    let (addr, server) = spawn_server(1, |stream| {
        let cancel = CancellationToken::new();
        // 服务端走**真实**接收流程；客户端手写一条畸形帧
        receive_text(&stream, admit_text, |_text| Ok(()), &cancel)
    });
    let stream = connect(addr);
    // 畸形：声明 999 字节，实际只写 3 字节
    let mut writer = &stream;
    write_frame(
        &mut writer,
        &TransferMessage::Text {
            task_id: "t-lying".to_owned(),
            chars: 3,
            bytes: 999,
            from_device: None,
        },
        b"abc",
    )
    .expect("写畸形帧");

    // 读回执：必须是拒收且码为 TEXT_TOO_LONG
    let mut reader = FrameReader::new(&stream);
    let (message, _) = reader.read_frame().expect("读回执");
    match message {
        TransferMessage::TextAck {
            ok, reason_code, ..
        } => {
            assert!(!ok, "畸形帧不得被当成已交付");
            assert_eq!(
                reason_code.as_deref(),
                Some(crate::transfer::reasons::TEXT_TOO_LONG)
            );
        }
        other => panic!("应收到 TextAck，实得 {other:?}"),
    }
    let received = server.join().unwrap()[0].as_ref().unwrap().clone();
    assert_eq!(
        received,
        TextOutcome::Rejected {
            reason_code: crate::transfer::reasons::TEXT_TOO_LONG.to_owned()
        }
    );
}

#[test]
fn receive_text_refuses_a_non_text_first_frame() {
    let (addr, server) = spawn_server(1, |stream| {
        let cancel = CancellationToken::new();
        receive_text(&stream, admit_text, |_text| Ok(()), &cancel)
    });
    let stream = connect(addr);
    let mut writer = &stream;
    write_frame(
        &mut writer,
        &TransferMessage::Handshake {
            task_id: "x".to_owned(),
            file_name: "a.bin".to_owned(),
            file_size: 1,
            file_hash: "h".to_owned(),
            chunk_size: 2 * 1024 * 1024,
            file_modified_at_ms: None,
            pair_code: None,
        },
        &[],
    )
    .expect("写帧");
    let result = server.join().unwrap();
    assert!(
        result[0].is_err(),
        "第一帧不是 Text 时应报错，而不是当成文本交付"
    );
}

#[test]
fn text_round_trip_preserves_multibyte_and_emoji() {
    let (addr, server) = spawn_server(1, |stream| {
        let cancel = CancellationToken::new();
        receive_text(
            &stream,
            admit_text,
            |text| {
                // 交付侧看到的必须与发送侧一模一样（多字节/表情/制表/换行都不许变）
                assert_eq!(text, "中文🚀\ttab\n换行");
                Ok(())
            },
            &cancel,
        )
    });
    let stream = connect(addr);
    let cancel = CancellationToken::new();
    let outcome = send_text(&stream, "中文🚀\ttab\n换行", "t-mb", None, &cancel).expect("发送");
    assert_eq!(outcome, TextOutcome::Delivered, "发送方应看到已交付");
    // 服务端也必须报已交付（交付侧那条 assert_eq 已经验过正文逐字相同）
    assert_eq!(
        server.join().unwrap()[0].as_ref().unwrap(),
        &TextOutcome::Delivered
    );
}

#[test]
fn empty_text_is_a_valid_transaction() {
    let (addr, server) = spawn_server(1, |stream| {
        let cancel = CancellationToken::new();
        receive_text(&stream, admit_text, |_text| Ok(()), &cancel)
    });
    let stream = connect(addr);
    let cancel = CancellationToken::new();
    let outcome = send_text(&stream, "", "t-empty", None, &cancel).expect("发送空文本");
    assert_eq!(outcome, TextOutcome::Delivered);
    let _ = server.join();
}
