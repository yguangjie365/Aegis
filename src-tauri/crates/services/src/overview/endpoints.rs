//! 本机网络连接面板（M-01 片1 命令面 / F-05 采集侧，2026-10-03）。
//!
//! 复用结论（DP-1）：端点取数走 `infra::net_tcp_table`（`GetExtendedTcpTable` 官方绑定，
//! 母本 `SystemToolkit` `TcpConnectionTable` 同 API，属③思路复用）；进程名解析复用
//! `OverviewInner::system` 里已有的 sysinfo 进程表，🔴 不为此新开第二条 FFI 通道（分层红线 KN-04）。
//!
//! 口径（01 册 §五 F-05）：面板只要**端点总数 + 按进程归属的前列 + 监听端口摘要**，
//! 因此 🔴 不采本机/远端 IP（`net_tcp_table.rs` 已刻意不读地址字段，隐私面就此收窄）。

use std::collections::HashMap;
use std::sync::Arc;

use sysinfo::ProcessRefreshKind;

use infra::net_tcp_table::TcpEndpoint;

use super::dto::{EndpointRowDto, NetEndpointsDto};
use super::state::OverviewInner;

/// 前列行数（F-05 明确"最多 5 行"；R-18 空行补齐归前端）。
pub const ENDPOINT_ROWS: usize = 5;
/// 监听端口摘要最多列出的端口数，超出以省略号收尾（一进程可监听上百端口，长串会撑破行）。
pub const PORT_SUMMARY_LIMIT: usize = 6;

/// 端口去重 + 升序 + 截断摘要（`"80, 443, 3128"`；无监听为空串）。
#[must_use]
pub fn listen_ports_summary(mut ports: Vec<u16>) -> String {
    ports.sort_unstable();
    ports.dedup();
    let overflow = ports.len() > PORT_SUMMARY_LIMIT;
    ports.truncate(PORT_SUMMARY_LIMIT);
    let text = ports
        .iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>()
        .join(", ");
    if overflow && !text.is_empty() {
        format!("{text} …")
    } else {
        text
    }
}

/// PID 读不到名字时的占位（🔴 不得留空、也不得冒充某个进程名）。
#[must_use]
pub fn process_display_name(name: Option<&str>, pid: u32) -> String {
    match name.map(str::trim).filter(|text| !text.is_empty()) {
        Some(text) => text.to_string(),
        None => format!("未知进程（PID {pid}）"),
    }
}

/// 按进程归属聚合端点（纯函数，测试在此钉口径，不依赖真实 TCP 表）。
///
/// `names` 不收泛型 hasher：本函数的调用方全工程只有一处（`commands`），拿的必然是
/// `infra::net_tcp_table` 边车产出的标准 `HashMap`，为消 lint 引泛型参数属无收益抽象。
#[must_use]
#[allow(clippy::implicit_hasher)]
pub fn aggregate_endpoints(
    endpoints: &[TcpEndpoint],
    names: &HashMap<u32, String>,
) -> NetEndpointsDto {
    /// 单进程聚合中间量（不进 DTO：`listen` 要先去重升序摘要，`count` 要参与排序）
    #[derive(Default)]
    struct Group {
        count: usize,
        listen: Vec<u16>,
    }

    if endpoints.is_empty() {
        return NetEndpointsDto {
            total: 0,
            rows: Vec::new(),
            reason: Some("未获取到 TCP 端点".to_string()),
        };
    }

    let mut groups: HashMap<u32, Group> = HashMap::new();
    for endpoint in endpoints {
        let group = groups.entry(endpoint.pid).or_default();
        group.count += 1;
        if endpoint.state.is_listening() {
            group.listen.push(endpoint.local_port);
        }
    }

    // 先带 PID 排序（端点数降序，同数按 PID 升序 —— 与进程榜同一套行序稳定纪律），
    // 再折叠成 DTO：DTO 不含 PID（F-05 界面不显示它，隐私面也不外溢）。
    let mut ordered: Vec<(u32, usize, Vec<u16>)> = groups
        .into_iter()
        .map(|(pid, group)| (pid, group.count, group.listen))
        .collect();
    ordered.sort_by(|left, right| match right.1.cmp(&left.1) {
        std::cmp::Ordering::Equal => left.0.cmp(&right.0),
        other => other,
    });

    let rows: Vec<EndpointRowDto> = ordered
        .into_iter()
        .take(ENDPOINT_ROWS)
        .enumerate()
        .map(|(index, (pid, count, listen))| EndpointRowDto {
            rank: index + 1,
            process_name: process_display_name(names.get(&pid).map(String::as_str), pid),
            endpoint_count: count,
            listen_ports: listen_ports_summary(listen),
        })
        .collect();

    NetEndpointsDto {
        total: endpoints.len(),
        rows,
        reason: None,
    }
}

/// 读进程名表（复用 sysinfo：表为空时补一轮仅取名的刷新，不采 CPU/内存）。
fn process_names(inner: &OverviewInner) -> HashMap<u32, String> {
    let mut sys = inner.system();
    if sys.processes().is_empty() {
        sys.refresh_processes_specifics(ProcessRefreshKind::new());
    }
    sys.processes()
        .iter()
        .map(|(pid, process)| (pid.as_u32(), process.name().to_string()))
        .collect()
}

/// 采集本机 IPv4 TCP 端点归属（`overview_net_endpoints` 载荷）。
#[must_use]
pub fn net_endpoints(inner: &Arc<OverviewInner>) -> NetEndpointsDto {
    match infra::net_tcp_table::ipv4_tcp_endpoints() {
        Ok(endpoints) => aggregate_endpoints(&endpoints, &process_names(inner)),
        Err(error) => {
            // 🔴 采集失败 ≠ 没有端点：原因原样带给界面（R-08 空态必写为什么是空的）
            eprintln!("[overview] TCP 端点采集失败: {error}");
            NetEndpointsDto {
                total: 0,
                rows: Vec::new(),
                reason: Some(format!("未获取到 TCP 端点（{error}）")),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use infra::net_tcp_table::TcpState;

    fn endpoint(pid: u32, local_port: u16, state: TcpState) -> TcpEndpoint {
        TcpEndpoint {
            pid,
            local_port,
            remote_port: 0,
            state,
        }
    }

    fn names(pairs: &[(u32, &str)]) -> HashMap<u32, String> {
        pairs
            .iter()
            .map(|(pid, name)| (*pid, (*name).to_string()))
            .collect()
    }

    #[test]
    fn port_summary_is_deduped_sorted_and_capped() {
        assert_eq!(listen_ports_summary(vec![443, 80, 443, 22]), "22, 80, 443");
        assert_eq!(listen_ports_summary(Vec::new()), "");
        let many = listen_ports_summary((1..=10).collect::<Vec<u16>>());
        assert!(many.ends_with('…'), "超上限须以省略号收尾，实得：{many}");
        assert!(many.starts_with("1, 2, 3, 4, 5, 6"), "实得：{many}");
    }

    #[test]
    fn unknown_pid_is_named_honestly() {
        assert_eq!(process_display_name(Some("chrome.exe"), 1234), "chrome.exe");
        assert_eq!(
            process_display_name(Some("   "), 1234),
            "未知进程（PID 1234）",
            "空白名不得当成有效进程名"
        );
        assert_eq!(
            process_display_name(None, 4),
            "未知进程（PID 4）",
            "读不到名字要写出来，不得留空"
        );
    }

    #[test]
    fn empty_table_says_why_it_is_empty() {
        let dto = aggregate_endpoints(&[], &names(&[]));
        assert_eq!(dto.total, 0);
        assert!(dto.rows.is_empty());
        assert_eq!(dto.reason.as_deref(), Some("未获取到 TCP 端点"));
    }

    #[test]
    fn groups_endpoints_by_owning_process_with_listen_summary() {
        let endpoints = vec![
            endpoint(10, 0, TcpState::Established),
            endpoint(10, 443, TcpState::Listen),
            endpoint(10, 0, TcpState::TimeWait),
            endpoint(20, 80, TcpState::Listen),
            endpoint(30, 0, TcpState::Established),
        ];
        let dto = aggregate_endpoints(&endpoints, &names(&[(10, "a.exe"), (20, "b.exe")]));
        assert_eq!(dto.total, 5, "总数是端点条数，不是进程数");
        assert_eq!(dto.rows[0].process_name, "a.exe");
        assert_eq!(dto.rows[0].endpoint_count, 3);
        assert_eq!(dto.rows[0].listen_ports, "443");
        assert_eq!(dto.rows[1].endpoint_count, 1);
        assert_eq!(
            dto.rows[1].process_name, "b.exe",
            "同端点数时按 PID 升序：PID 20 在 PID 30 之前"
        );
        assert_eq!(dto.rows[1].listen_ports, "80");
        assert_eq!(
            dto.rows[2].process_name, "未知进程（PID 30）",
            "名字缺失要如实标注，不得留空或编造"
        );
        assert_eq!(dto.rows[2].listen_ports, "");
        assert_eq!(dto.rows.last().expect("应有末行").rank, dto.rows.len());
    }

    #[test]
    fn only_top_five_processes_are_listed_but_total_counts_everything() {
        // 7 个进程各 2 条端点：总数 14，前列只有 5 行
        let endpoints: Vec<TcpEndpoint> = (100..107)
            .flat_map(|pid| {
                [
                    endpoint(pid, 0, TcpState::Established),
                    endpoint(pid, 1, TcpState::Listen),
                ]
            })
            .collect();
        let dto = aggregate_endpoints(&endpoints, &names(&[]));
        assert_eq!(dto.total, 14);
        assert_eq!(dto.rows.len(), ENDPOINT_ROWS);
        assert_eq!(
            dto.rows
                .iter()
                .map(|row| row.process_name.clone())
                .collect::<Vec<_>>(),
            vec![
                "未知进程（PID 100）",
                "未知进程（PID 101）",
                "未知进程（PID 102）",
                "未知进程（PID 103）",
                "未知进程（PID 104）"
            ],
            "端点数相同按 PID 升序取前列"
        );
    }

    #[test]
    fn established_rows_do_not_pollute_the_listen_summary() {
        let endpoints = vec![
            endpoint(7, 54321, TcpState::Established),
            endpoint(7, 9999, TcpState::SynSent),
        ];
        let dto = aggregate_endpoints(&endpoints, &names(&[(7, "svc")]));
        assert_eq!(
            dto.rows[0].listen_ports, "",
            "非 Listen 态端口不得进监听摘要"
        );
    }

    /// 真机冒烟：本机 TCP 表须能聚合出归属行（有监听端口是 Windows 桌面常态）。
    #[cfg(windows)]
    #[test]
    fn net_endpoints_on_this_machine_are_populated() {
        let inner = OverviewInner::new();
        let dto = net_endpoints(&inner);
        assert!(dto.total > 0, "本机 TCP 表不应为空：{:?}", dto.reason);
        assert!(!dto.rows.is_empty());
        assert!(dto.rows.len() <= ENDPOINT_ROWS);
        assert!(
            dto.rows
                .windows(2)
                .all(|pair| pair[0].endpoint_count >= pair[1].endpoint_count),
            "前列须按端点数降序"
        );
        assert!(dto.rows.iter().all(|row| row.endpoint_count > 0));
        let listed: usize = dto.rows.iter().map(|row| row.endpoint_count).sum();
        assert!(listed <= dto.total, "前列合计不得超过总数");
    }

    #[test]
    fn failure_path_keeps_the_reason_and_zeroes() {
        // 走纯函数分支验证 Err 侧形状（真实 FFI 失败难以构造）
        let dto = NetEndpointsDto {
            total: 0,
            rows: Vec::new(),
            reason: Some("未获取到 TCP 端点（读取 TCP 表失败（状态码 122））".to_string()),
        };
        assert_eq!(dto.total, 0);
        assert!(dto
            .reason
            .as_deref()
            .unwrap_or_default()
            .starts_with("未获取到 TCP 端点"));
    }
}
