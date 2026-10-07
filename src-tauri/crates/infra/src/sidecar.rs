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

/// 🔴 单行回包字节上限（2026-10-07）。
///
/// **为什么需要**：`read_until` 会把一整行**无界**读进内存。若子进程因故输出一行
/// 异常巨大的内容（未截断的传感器转储、错误时把整个文件内容吐出来、编码错乱导致
/// 换行失效），这里会先把它完整读进来，**然后**才交给上层判协议故障 —— 内存峰值
/// 已经发生了，「解析失败不致崩溃」也就无从保证。
///
/// 取 4 MiB：实测真实回包约 1.6 万字符（`serde` 报错的 column 16210 印证），
/// 即正常形态只用掉千分之四，余量足够宽松；而 4 MiB 远小于任何进程可接受的
/// 单次分配上限，超限即立刻停止累积。
const MAX_LINE_BYTES: u64 = 4 * 1024 * 1024;

/// 读线程 → `request` 的超限哨兵行前缀。
///
/// 🔴 用 `\0` 开头（JSON 文本以 `{` 起，故不可能撞上真实回包），
///   后接字节上限数值，便于上层把具体上限写进故障原因。
const LINE_TOO_LARGE_PREFIX: &str = "\u{0}AEGIS_LINE_TOO_LARGE:";

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
    /// 🔴 2026-10-07：单行回包超过 `MAX_LINE_BYTES`，已被读线程丢弃。
    ///   单独成错误类型而非让上层看到截断的 JSON —— 后者会给出
    ///   一个指向**截断点**而非故障点的解析错误位置（同「片段取错位置」类错误）。
    LineTooLarge { limit: u64 },
}

impl std::fmt::Display for SidecarError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Spawn(reason) => write!(f, "子进程启动失败: {reason}"),
            Self::Write(reason) => write!(f, "写入命令管道失败: {reason}"),
            Self::Timeout => write!(f, "读取子进程响应超时"),
            Self::Closed => write!(f, "子进程输出管道已关闭"),
            Self::LineTooLarge { limit } => {
                write!(f, "子进程单行输出超过 {limit} 字节，已丢弃")
            }
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
                // 🔴 `.take(MAX_LINE_BYTES)`：单行**最多**读这么多字节就停，不再无界累积
                //   （见 `MAX_LINE_BYTES` 的理由）。`Take` 按值持有被借的
                //   `reader`，故每次迭代重建 —— 它只在本次 `read_line` 内有效。
                //   超限行会因缺尾随换行而让下一次读取接着读，那种行本身就是
                //   协议异常，故按「丢弃超限行 + 通知上层」处理，不做猜测式重对齐。
                match std::io::Read::take(&mut reader, MAX_LINE_BYTES).read_line(&mut buf) {
                    // EOF（子进程退出）或读错：统一结束读线程
                    Ok(0) | Err(_) => break,
                    Ok(_) => {
                        // 🔴 读满了上限且**没有**读到换行 ⇒ 本行被截断。
                        //   判据是「`Take` 已耗尽」而非「缺换行」——最后一行
                        //   正常以 EOF 结束时同样没有换行，不能被误判成超限。
                        let truncated = buf.len() as u64 >= MAX_LINE_BYTES && !buf.ends_with('\n');
                        if truncated {
                            // 丢弃并通知上层：截断的 JSON 交给上层只会得到一个
                            // 指向**截断点**而非故障点的错误位置（与 2026-10-07
                            // 修的「诊断片段取开头」是同一类错误）。
                            let _ = tx.send(format!("{LINE_TOO_LARGE_PREFIX}{MAX_LINE_BYTES}"));
                            break;
                        }
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
                    //
                    // 🔴 与 stdout 同款限幅（审查 R-4，2026-10-07）：`read_until` 会把一整行
                    //   **无界**读进内存，stdout 侧已用 `take(MAX_LINE_BYTES)` 堵住，stderr
                    //   不能只装一扇门。差异：stdout 超限即断（协议已坏），stderr 超限
                    //   **必须继续排空**——子进程若阻塞在写满的管道上，会反向卡死 stdout
                    //   读取（见 spawn 处注释），故丢弃超限内容、吞掉该行剩余字节后继续。
                    let n = match std::io::Read::take(&mut reader, MAX_LINE_BYTES)
                        .read_until(b'\n', &mut buf)
                    {
                        Ok(n) => n,
                        Err(_) => break,
                    };
                    if n == 0 {
                        break; // EOF（子进程退出）
                    }
                    if n as u64 >= MAX_LINE_BYTES && !buf.ends_with(b"\n") {
                        // 超限：丢内容、只留一条痕，然后排空到行尾（防管道反压）
                        stderr_sink(&format!(
                            "[sidecar] stderr 单行超过 {MAX_LINE_BYTES} 字节，该行已丢弃"
                        ));
                        let mut sink = [0u8; 8192];
                        loop {
                            // `read` 在 `Read` trait 上（本模块只 use 了 `BufRead`），故全限定调用
                            match std::io::Read::read(&mut reader, &mut sink) {
                                Ok(0) | Err(_) => return, // EOF：读线程结束
                                Ok(m) => {
                                    if sink[..m].contains(&b'\n') {
                                        break; // 本行已排空，回到外层读下一行
                                    }
                                }
                            }
                        }
                        continue;
                    }
                    stderr_sink(String::from_utf8_lossy(&buf).trim_end());
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
                Ok(line) => {
                    // 🔴 读线程只在一种情况主动发哨兵行：单行超上限。
                    //   真实回包以 `{` 开头，故该前缀不会与正常回包混淆；
                    //   单独识别为错误，避免把它当 JSON 解析出一个假的位置。
                    if let Some(limit) = line.strip_prefix(LINE_TOO_LARGE_PREFIX) {
                        return Err(SidecarError::LineTooLarge {
                            limit: limit.parse().unwrap_or(MAX_LINE_BYTES),
                        });
                    }
                    return Ok(line);
                }
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
            SidecarError::LineTooLarge { limit: 10 }.to_string(),
        ]
        .to_vec();
        texts.sort();
        let dedup_len = texts.len();
        texts.dedup();
        assert_eq!(dedup_len, texts.len(), "错误文案须可区分：{texts:?}");
    }

    #[test]
    fn line_too_large_sentinel_is_never_confused_with_a_real_reply() {
        // 🔴 核心不变量：哨兵行绝不能被当成正常回包流给上层。
        //   真实回包以 `{` 开头，而哨兵以 `\0` 起 ——两者不可能混淆。
        //   若这条失守，超长回包会被当 JSON 解析，得出一个指向
        //   **截断点**而非故障点的错误位置（同「片段取错位置」类错误）。
        assert!(
            !r#"{"sensors":[]}"#.starts_with(LINE_TOO_LARGE_PREFIX),
            "真实回包不该以哨兵前缀开头（否则两者会混淆）"
        );
        let sentinel = format!("{LINE_TOO_LARGE_PREFIX}{MAX_LINE_BYTES}");
        let parsed = sentinel
            .strip_prefix(LINE_TOO_LARGE_PREFIX)
            .and_then(|n| n.parse::<u64>().ok());
        assert_eq!(parsed, Some(MAX_LINE_BYTES), "哨兵应能还原出上限值");
    }

    #[test]
    fn line_limit_is_far_above_the_real_reply_size() {
        // 🔴 上限取值须留足余量：实测真实回包约 1.6 万字符
        //   （serde 报错 column 16210 印证），4 MiB 是它的两千余倍。
        //   若这条失守，说明上限被调得过紧，会把正常回包当异常丢弃。
        assert!(
            MAX_LINE_BYTES > 1_000_000,
            "上限须显著高于实测回包尺寸，当前 {MAX_LINE_BYTES}"
        );
    }
}
