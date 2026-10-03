//! 子进程桥原语（M-01 / Q-12=B，2026-10-03）。
//!
//! 只负责"起一个 stdio 行协议的子进程 + 有超时地收发一行"，**不含任何业务判断**：
//! 就绪判定、重启冷却、连败禁用等状态机一律在 `services::overview::sensor_bridge`，
//! 本模块不得反向依赖域层（02 架构分册 AD-*：infra → contracts 单向）。
//!
//! 🔴 门禁对齐（两处有意区别于 `NexBox` 母本 `src-tauri/src/sensor.rs`，登记见 `DEPENDENCIES.md §三`）：
//! - 母本用 `WaitForSingleObject` 等待管道可读（Windows FFI）+ `std::thread::sleep` 轮询退出；
//!   本实现改走 **读线程 + `mpsc::Receiver::recv_timeout`**，零 FFI、零同步睡眠
//!   （02 册 KN-04 FFI 收敛、clippy.toml 禁 `std::thread::sleep`）。
//! - 母本的子进程查找/生命周期与硬件采集混在同一文件；本实现只留传输原语。

use std::io::{BufRead, BufReader, Write};
use std::path::Path;
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError};
use std::time::{Duration, Instant};

/// 单次收发的等待切片：`recv_timeout` 按此粒度轮询，仅为让 `deadline` 判定能推进，
/// 不构成忙等（每次都在内核对象上阻塞）。
const POLL_SLICE: Duration = Duration::from_millis(200);

#[derive(Debug)]
pub enum SidecarError {
    /// 子进程起不来（路径不存在、无执行权限等）
    Spawn(String),
    /// 写入 stdin 失败（多为子进程已退出/管道已关）
    Write(String),
    /// 在给定超时内没等到回行
    Timeout,
    /// 读端已断开（子进程退出或 stdout 关闭）
    Closed,
}

impl std::fmt::Display for SidecarError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Spawn(reason) => write!(f, "子进程启动失败: {reason}"),
            Self::Write(reason) => write!(f, "写入命令管道失败: {reason}"),
            Self::Timeout => write!(f, "读取子进程响应超时"),
            Self::Closed => write!(f, "子进程输出管道已关闭"),
        }
    }
}

impl std::error::Error for SidecarError {}

/// 一个已启动的 stdio 行协议子进程。
pub struct Sidecar {
    child: Child,
    writer: ChildStdin,
    /// stdout 按行队列；由读线程填充
    lines: Receiver<String>,
}

impl Sidecar {
    /// 启动子进程并接管三条管道。
    ///
    /// `stderr_sink` 在专用线程上被逐行回调（🔴 必须排空，否则子进程写满管道缓冲区后会阻塞在
    /// stderr 上再也不回 stdout——母本 `sensor.rs:583` 同此处理），域层用它转日志。
    ///
    /// # Errors
    /// 管道未能建立或进程启动失败时返回 [`SidecarError::Spawn`]。
    pub fn spawn<F>(exe: &Path, mut stderr_sink: F) -> Result<Self, SidecarError>
    where
        F: FnMut(&str) + Send + 'static,
    {
        let mut cmd = Command::new(exe);
        cmd.stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        // Windows 下不弹控制台窗口
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            const CREATE_NO_WINDOW: u32 = 0x0800_0000;
            cmd.creation_flags(CREATE_NO_WINDOW);
        }
        let mut child = cmd
            .spawn()
            .map_err(|e| SidecarError::Spawn(format!("{}: {e}", exe.display())))?;

        let writer = child
            .stdin
            .take()
            .ok_or_else(|| SidecarError::Spawn("无法获取子进程 stdin".to_string()))?;
        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| SidecarError::Spawn("无法获取子进程 stdout".to_string()))?;

        let (tx, lines) = mpsc::channel();
        std::thread::spawn(move || {
            let mut reader = BufReader::new(stdout);
            let mut buf = String::new();
            loop {
                buf.clear();
                match reader.read_line(&mut buf) {
                    // EOF（子进程退出）或读错：统一结束读线程
                    Ok(0) | Err(_) => break,
                    Ok(_) => {
                        let line = buf.trim_end_matches(['\r', '\n']);
                        if line.is_empty() {
                            continue;
                        }
                        // 接收端已释放（Sidecar 被丢弃）→ 停止读线程
                        if tx.send(line.to_string()).is_err() {
                            break;
                        }
                    }
                }
            }
        });

        if let Some(stderr) = child.stderr.take() {
            std::thread::spawn(move || {
                let mut reader = BufReader::new(stderr);
                let mut buf: Vec<u8> = Vec::new();
                loop {
                    buf.clear();
                    // 🔴 按字节读 + lossy 转：母本用 UTF-8 读，而 sidecar 的中文日志在
                    // 非 UTF-8 控制台代码页下会成非法字节序列，`read_line::<String>` 直接报错
                    // → 诊断信息静默丢失（违 R-08）。lossy 至少保留可读部分。
                    match reader.read_until(b'\n', &mut buf) {
                        Ok(0) | Err(_) => break,
                        Ok(_) => stderr_sink(String::from_utf8_lossy(&buf).trim_end()),
                    }
                }
            });
        }

        Ok(Self {
            child,
            writer,
            lines,
        })
    }

    #[must_use]
    pub fn pid(&self) -> u32 {
        self.child.id()
    }

    /// 写入一行命令（自动补换行），并在 `timeout` 内等回行。
    ///
    /// 🔴 超时后**残留回行不可用**：下一次 `request` 可能读到上一次的迟到响应，
    /// 故调用方必须在 `Timeout`/`Closed` 后终止并重启子进程（`services` 侧状态机承担），
    /// 本原语不做"丢弃陈旧行"的猜测式对齐。
    ///
    /// # Errors
    /// 写入失败 [`SidecarError::Write`]；超时 [`SidecarError::Timeout`]；管道关闭 [`SidecarError::Closed`]。
    pub fn request(
        &mut self,
        command_json: &str,
        timeout: Duration,
    ) -> Result<String, SidecarError> {
        writeln!(self.writer, "{command_json}").map_err(|e| SidecarError::Write(e.to_string()))?;
        self.writer
            .flush()
            .map_err(|e| SidecarError::Write(e.to_string()))?;

        let deadline = Instant::now() + timeout;
        loop {
            let remaining = deadline.saturating_duration_since(Instant::now());
            if remaining.is_zero() {
                return Err(SidecarError::Timeout);
            }
            match self.lines.recv_timeout(remaining.min(POLL_SLICE)) {
                Ok(line) => return Ok(line),
                Err(RecvTimeoutError::Disconnected) => return Err(SidecarError::Closed),
                // 切片到期：回到循环顶部重算 remaining
                Err(RecvTimeoutError::Timeout) => {}
            }
        }
    }

    /// 子进程是否仍在运行（已退出返回 false，不阻塞）。
    #[must_use]
    pub fn is_alive(&mut self) -> bool {
        matches!(self.child.try_wait(), Ok(None))
    }

    /// 强制终止并回收（幂等：已退出时直接返回）。
    pub fn kill(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }

    /// 尽力优雅退出：发 `{"cmd":"exit"}` 后最多等 `grace`，超时则强杀。
    ///
    /// 等待走 `try_wait` + `recv_timeout` 上的哑通道切片，不用 `thread::sleep`。
    pub fn shutdown(mut self, grace: Duration) {
        let _ = writeln!(self.writer, r#"{{"cmd":"exit"}}"#);
        let _ = self.writer.flush();
        let deadline = Instant::now() + grace;
        loop {
            match self.child.try_wait() {
                Ok(Some(_)) => return,
                Ok(None) => {
                    if Instant::now() >= deadline {
                        self.kill();
                        return;
                    }
                    // 借用一个必然超时的接收来切片等待（无同步睡眠 API）
                    let _ = self.lines.recv_timeout(POLL_SLICE);
                }
                Err(_) => {
                    self.kill();
                    return;
                }
            }
        }
    }
}

impl Drop for Sidecar {
    fn drop(&mut self) {
        // 域层未显式 shutdown 时兜底，绝不留孤儿进程
        self.kill();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn spawn_reports_diagnosable_reason() {
        let missing = PathBuf::from("一定不存在的-sidecar-可执行文件.exe");
        let Err(err) = Sidecar::spawn(&missing, |_| {}) else {
            panic!("不存在的 exe 必须启动失败");
        };
        assert!(
            matches!(err, SidecarError::Spawn(_)),
            "错误形态应为 Spawn，实得 {err:?}"
        );
        assert!(
            err.to_string().contains("子进程启动失败"),
            "Display 应带原因前缀，实得 {err}"
        );
    }

    #[test]
    fn error_variants_have_distinct_messages() {
        let mut texts = [
            SidecarError::Spawn("x".into()).to_string(),
            SidecarError::Write("y".into()).to_string(),
            SidecarError::Timeout.to_string(),
            SidecarError::Closed.to_string(),
        ]
        .to_vec();
        texts.sort();
        let dedup_len = texts.len();
        texts.dedup();
        assert_eq!(dedup_len, texts.len(), "错误文案须可区分：{texts:?}");
    }
}
