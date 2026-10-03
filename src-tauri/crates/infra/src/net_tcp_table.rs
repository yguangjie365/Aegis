//! IPv4 TCP 端点表（M-01 F-05 / R-02，2026-10-03）。
//!
//! 🔴 分层位置：本模块是 FFI 边界，只允许出现在 `infra`（02 架构分册 KN-04）。
//! 绑定一律走 `windows-sys` 官方生成的绑定，**禁手写 Win32 声明**（01 总览 §二）。
//!
//! 采集口径（与规格对齐，宁缺毋滥）：
//! - 仅 IPv4 TCP（规格 §七 L-04 明确 IPv6/UDP 属后续扩展项，本批不做）；
//! - 🔴 **不采集本机/远端 IP 地址**——F-05 的数据视图只要"端点总数 / 进程归属 / 监听端口摘要"
//!   （01 册 §五），少采一个隐私面就少一个脱敏风险；
//! - 母本参照 `SystemToolkit` `TcpConnectionTable`（C#，跨栈仅③思路复用：同样走
//!   `GetExtendedTcpTable` + `MIB_TCPTABLE_OWNER_PID`），非代码移植。
//!
//! # Errors
//! 一律返回用户可读原因，禁静默失败；调用方（`services::overview`）据此走 R-08 空态文案。

/// 单条 IPv4 TCP 端点（已按 F-05 口径裁剪到最小字段集）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TcpEndpoint {
    pub pid: u32,
    pub local_port: u16,
    pub remote_port: u16,
    pub state: TcpState,
}

/// Windows `MIB_TCP_STATE` 的已登记取值。
///
/// 🔴 系统原值只在 `from_raw`/`as_raw` 两处出现（本 enum 不带 `#[repr]` 判别式：
/// `Other(u32)` 是非 unit 变体，与显式判别式共存会强制 repr 整数，反而禁掉 `as u32` 转换）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TcpState {
    Closed,
    Listen,
    SynSent,
    SynReceived,
    Established,
    FinWait1,
    FinWait2,
    CloseWait,
    Closing,
    LastAck,
    TimeWait,
    /// 系统返回了未登记的取值：如实保留原值，不臆断成某个已知态
    Other(u32),
}

impl TcpState {
    #[must_use]
    pub fn from_raw(raw: u32) -> Self {
        match raw {
            1 => Self::Closed,
            2 => Self::Listen,
            3 => Self::SynSent,
            4 => Self::SynReceived,
            5 => Self::Established,
            6 => Self::FinWait1,
            7 => Self::FinWait2,
            8 => Self::CloseWait,
            9 => Self::Closing,
            10 => Self::LastAck,
            11 => Self::TimeWait,
            other => Self::Other(other),
        }
    }

    /// 是否"监听中"（F-05 的"监听端口摘要"判定唯一依据）。
    #[must_use]
    pub fn is_listening(self) -> bool {
        matches!(self, Self::Listen)
    }

    /// 还原系统原值（`from_raw` 的逆；两侧同处维护，供跨层传输与测试断言用）。
    #[must_use]
    pub const fn as_raw(self) -> u32 {
        match self {
            Self::Closed => 1,
            Self::Listen => 2,
            Self::SynSent => 3,
            Self::SynReceived => 4,
            Self::Established => 5,
            Self::FinWait1 => 6,
            Self::FinWait2 => 7,
            Self::CloseWait => 8,
            Self::Closing => 9,
            Self::LastAck => 10,
            Self::TimeWait => 11,
            Self::Other(value) => value,
        }
    }
}

/// `GetExtendedTcpTable` 的唯一调用点（本文件唯一 `unsafe` 块）。
///
/// # Safety
/// 见块内 `// SAFETY:` 注释；调用方只需保证 `buffer` 是可写的、长度与 `size_bytes` 语义一致。
#[cfg(windows)]
#[allow(unsafe_code)]
fn call_tcp_table(buffer: &mut [u8], size_bytes: &mut u32) -> u32 {
    use windows_sys::Win32::NetworkManagement::IpHelper::{
        GetExtendedTcpTable, TCP_TABLE_OWNER_PID_ALL,
    };
    use windows_sys::Win32::Networking::WinSock::AF_INET;

    // SAFETY: 缓冲区由调用方持有且长度不小于 `*size_bytes`（第二段传精确长度，第一段传 4 字节
    // 占位只用于探长度，系统在此情形下只写 size_bytes 与返回码、不写缓冲区）。
    // `size_bytes` 为 `&mut u32`，指向调用方栈上的活对象。指针类型经 `cast()` 转成
    // `*mut c_void`，与 API 声明的无类型输出指针一致。
    unsafe {
        GetExtendedTcpTable(
            buffer.as_mut_ptr().cast(),
            size_bytes,
            0, // bOrder = FALSE：排序与聚合归域层
            u32::from(AF_INET),
            TCP_TABLE_OWNER_PID_ALL,
            0,
        )
    }
}

/// 读取本机 IPv4 TCP 端点全表（含 PID 归属）。
///
/// # Errors
/// 系统调用失败、返回长度与声明行数不符时返回用户可读原因（调用方不得当成"没有端点"）。
#[cfg(windows)]
pub fn ipv4_tcp_endpoints() -> Result<Vec<TcpEndpoint>, String> {
    /// `MIB_TCPROW_OWNER_PID` 定长 6 个 u32；表头是 1 个 u32 行数。
    const ROW_WORDS: usize = 6;
    const HEADER_WORDS: usize = 1;

    let mut size_bytes: u32 = 0;
    // 第一次调用只为探长度：返回 ERROR_INSUFFICIENT_BUFFER 并把所需字节数写进 size_bytes。
    // 用 4 字节占位而非 null，避开任何"是否接受空指针"的实现差异。
    let mut probe = [0u8; 4];
    let status = call_tcp_table(&mut probe, &mut size_bytes);
    if size_bytes == 0 {
        return Err(format!("探测 TCP 表长度失败（状态码 {status}）"));
    }

    let mut buffer = vec![0u8; size_bytes as usize];
    let mut filled = size_bytes;
    let status = call_tcp_table(&mut buffer, &mut filled);
    // ERROR_SUCCESS = 0
    if status != 0 {
        return Err(format!("读取 TCP 表失败（状态码 {status}）"));
    }
    if buffer.len() < HEADER_WORDS * 4 {
        return Err("TCP 表返回长度不足表头".to_string());
    }

    let row_count = u32::from_le_bytes([buffer[0], buffer[1], buffer[2], buffer[3]]) as usize;
    let need = HEADER_WORDS * 4 + row_count * ROW_WORDS * 4;
    if buffer.len() < need {
        return Err(format!(
            "TCP 表声明 {row_count} 行但缓冲区只有 {} 字节",
            buffer.len()
        ));
    }

    let mut rows = Vec::with_capacity(row_count);
    for index in 0..row_count {
        let base = HEADER_WORDS * 4 + index * ROW_WORDS * 4;
        let word_at = |offset_words: usize| {
            let start = base + offset_words * 4;
            u32::from_le_bytes([
                buffer[start],
                buffer[start + 1],
                buffer[start + 2],
                buffer[start + 3],
            ])
        };
        // 行内字段顺序：dwState, dwLocalAddr, dwLocalPort, dwRemoteAddr, dwRemotePort, dwOwningPid
        // 🔴 两个地址字段（word 1 / word 3）刻意不读——F-05 不需要 IP，少采一个隐私面
        let state = word_at(0);
        let local_port = normalise_port(word_at(2));
        let remote_port = normalise_port(word_at(4));
        let pid = word_at(5);
        rows.push(TcpEndpoint {
            pid,
            local_port,
            remote_port,
            state: TcpState::from_raw(state),
        });
    }
    Ok(rows)
}

/// 把表里的端口字段还原成主机序端口号。
///
/// 系统结构体声明为 `u32`，有效值在低 16 位且**按网络字节序存放**；
/// 小端机器读出的 u32 低 16 位是字节互换后的结果，故先回字节再 `from_be`。
#[cfg(windows)]
fn normalise_port(raw_word: u32) -> u16 {
    let low = (raw_word & 0xFFFF) as u16;
    u16::from_be_bytes(low.to_le_bytes())
}

#[cfg(not(windows))]
pub fn ipv4_tcp_endpoints() -> Result<Vec<TcpEndpoint>, String> {
    Err("本平台不支持读取 IPv4 TCP 表（本工程交付目标为 Windows）".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tcp_state_roundtrips_known_values() {
        for raw in 1..=11u32 {
            assert_eq!(TcpState::from_raw(raw).as_raw(), raw);
        }
        assert_eq!(TcpState::from_raw(99), TcpState::Other(99));
        assert_eq!(TcpState::from_raw(99).as_raw(), 99);
    }

    #[test]
    fn only_listen_counts_as_listening() {
        assert!(TcpState::Listen.is_listening());
        assert!(!TcpState::Established.is_listening());
        assert!(!TcpState::Other(2).is_listening());
        assert!(!TcpState::Closed.is_listening());
    }

    #[test]
    fn port_normalisation_matches_network_order() {
        // 443 = 0x01BB；网络字节序存进 u32 低 16 位后，小端机器读到的是 0xBB01
        let as_stored: u32 = 0x01BB_u16.to_be().into();
        assert_eq!(normalise_port(as_stored), 443);
        assert_eq!(normalise_port(0), 0);
    }

    /// 真机冒烟：本机的 TCP 表必须读得出来且非空（有监听端口是 Windows 桌面的常态）。
    ///
    /// 🔴 这条测的是"FFI 接线是否真的通"，不是系统状态断言；若换到无网络的受限环境失败，
    /// 应改为 `#[ignore]` 并在 spikes 里记原因，不得删掉。
    #[cfg(windows)]
    #[test]
    fn ipv4_tcp_table_is_readable_on_this_machine() {
        let rows = ipv4_tcp_endpoints().expect("本机应能读取 IPv4 TCP 表");
        assert!(!rows.is_empty(), "本机 TCP 表为空不合常理");
        assert!(
            rows.iter().any(|row| row.pid != 0),
            "TCP 表应至少有一条带 PID 归属的条目"
        );
    }
}
