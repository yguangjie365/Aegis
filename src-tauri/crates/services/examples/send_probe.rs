//! 独立进程的发送探针（**跨进程**端到端验证用）。
//!
//! 用途：应用（另一个进程）跑着接收服务时，从**本进程**发一个文件过去。
//!
//! 为什么要单独起进程而不是用回环测试：同进程自测会把发送与接收塞进同一个进程，
//! 而 Tauri 的同步命令会占住命令线程（其它命令排队）——那会把"接收确认弹窗"这类
//! 依赖命令轮询的路径一起堵死，测出来的现象真假难辨。两个进程没有这层耦合。
//!
//! 用法：
//!   cargo run -p services --example send_probe -- <ip:port> <文件路径> [配对码]
//!
//! 退出码：0 = 送达；1 = 被拒或失败（原因码打到 stdout）。

use std::net::SocketAddr;
use std::path::PathBuf;

use services::transfer::chunk::DEFAULT_CHUNK_SIZE;
use services::transfer::service::send_to;
use tokio_util::sync::CancellationToken;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 3 {
        eprintln!("用法: send_probe <ip:port> <文件路径> [配对码]");
        std::process::exit(2);
    }
    let peer: SocketAddr = match args[1].parse() {
        Ok(value) => value,
        Err(error) => {
            eprintln!("地址不合法：{}（{error}）", args[1]);
            std::process::exit(2);
        }
    };
    let path = PathBuf::from(&args[2]);
    let code = args.get(3).map(String::as_str);
    let cancel = CancellationToken::new();

    println!("[probe] 发送 {} → {peer}", path.display());
    match send_to(
        peer,
        &path,
        DEFAULT_CHUNK_SIZE,
        code,
        "probe-task",
        &cancel,
        |sent| {
            // 进度回调：探针只把进度打到 stdout，便于观察是否真的在走
            println!("[probe] 已发送 {sent} 字节");
        },
    ) {
        Ok(outcome) => {
            println!("[probe] 结果：{outcome:?}");
            std::process::exit(0);
        }
        Err(failure) => {
            println!("[probe] 失败：{failure:?}");
            // 原因码单独打一行：脚本里好抓
            println!("[probe] 原因码：{failure}");
            std::process::exit(1);
        }
    }
}
