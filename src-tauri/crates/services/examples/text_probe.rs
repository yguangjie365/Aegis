//! 独立进程的**文本**发送探针（F-FT-33/34 端到端验证用）。
//!
//! 与 `send_probe` 同理：应用（另一个进程）跑着接收服务时，从本进程发一条文本过去。
//!
//! 🔴 这个探针做三件事，缺一不可：
//!   1. 先把剪贴板设成**哨兵值**（证明"最终内容来自接收方"而不是本来就那样）；
//!   2. 发文本、等回执；
//!   3. 回执之后**读回剪贴板并比对**——这才是"接收侧真的写了剪贴板"的证据。
//!
//! 为什么能跨进程比对剪贴板：Windows 剪贴板是**系统级**资源，
//! 本进程写、应用进程写，看到的是同一份。
//!
//! 用法：
//!   cargo run -p services --example text_probe -- <ip:port> <文本>
//!
//! 退出码：0 = 已交付且剪贴板内容相符；1 = 被拒/失败/内容不符。

use std::net::SocketAddr;

use services::transfer::transport::{self, TextOutcome};
use tokio_util::sync::CancellationToken;

/// 哨兵：一个正常流程绝不会产生的内容，用来证明剪贴板**被改写过**。
const SENTINEL: &str = "AEGIS-TEXT-PROBE-SENTINEL-请勿保留";

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 3 {
        eprintln!("用法: text_probe <ip:port> <文本>");
        std::process::exit(2);
    }
    let peer: SocketAddr = match args[1].parse() {
        Ok(value) => value,
        Err(error) => {
            eprintln!("地址不合法：{}（{error}）", args[1]);
            std::process::exit(2);
        }
    };
    let text = args[2].clone();

    // ── 1) 哨兵 ──
    if let Err(error) = infra::clipboard::set_text(SENTINEL) {
        eprintln!("[probe] 🔴 无法设置哨兵（剪贴板不可用）：{error}");
        std::process::exit(1);
    }
    println!("[probe] 哨兵已写入剪贴板（{} 字节）", SENTINEL.len());

    // ── 2) 发送 ──
    println!("[probe] 发送文本 → {peer}（{} 字）", text.chars().count());
    let stream =
        match std::net::TcpStream::connect_timeout(&peer, std::time::Duration::from_secs(5)) {
            Ok(stream) => stream,
            Err(error) => {
                eprintln!("[probe] 🔴 连不上对端：{error}");
                std::process::exit(1);
            }
        };
    if let Err(error) = transport::configure_timeout(&stream, transport::IO_TIMEOUT) {
        eprintln!("[probe] 🔴 设置超时失败：{error}");
        std::process::exit(1);
    }
    let cancel = CancellationToken::new();
    // 🔴 本进程是另一个进程，"接收前询问"要等用户在应用里点——上限由 `send_text`
    //    内部的 HANDSHAKE_WAIT（60s）兜住，这里不再叠一层。
    let outcome = transport::send_text(&stream, &text, "probe-text-1", Some("探针"), &cancel);
    drop(stream);

    let delivered = match &outcome {
        Ok(TextOutcome::Delivered) => {
            println!("[probe] ✅ 对端回执：已交付（= 对方剪贴板写入成功）");
            true
        }
        Ok(TextOutcome::Rejected { reason_code }) => {
            println!("[probe] ⛔ 对端拒收，原因码：{reason_code}");
            false
        }
        Err(error) => {
            println!("[probe] 🔴 会话失败：{error}");
            false
        }
    };

    // ── 3) 读回剪贴板并比对 ──
    let read_back = infra::clipboard::get_text();
    match &read_back {
        Ok(value) if value == &text => {
            println!(
                "[probe] ✅ 剪贴板内容与发送文本**逐字相符**（{} 字节）",
                value.len()
            );
        }
        Ok(value) if value == SENTINEL => {
            println!("[probe] 🔴 剪贴板仍是哨兵 ⇒ 接收侧**没有**写入剪贴板");
            std::process::exit(1);
        }
        Ok(value) => {
            println!(
                "[probe] 🔴 剪贴板内容不符，实得 {} 字节：{value:?}",
                value.len()
            );
            std::process::exit(1);
        }
        Err(error) => {
            println!("[probe] 🔴 读不回剪贴板：{error}");
            std::process::exit(1);
        }
    }

    if delivered {
        println!("[probe] 结论：文本通道端到端通过");
        std::process::exit(0);
    }
    std::process::exit(1);
}
