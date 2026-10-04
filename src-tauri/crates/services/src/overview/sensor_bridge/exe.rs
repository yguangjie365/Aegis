//! sidecar 可执行文件定位（含改名降级搜索面）：`sensor_bridge` 的子模块。

use super::bridge::EXE_NAME;
use std::path::{Path, PathBuf};

/// 相对可执行文件目录的候选子路径（安装版 / 资源目录 / 开发者构建产物）。
pub(super) const SUBDIRS: [&str; 4] = [
    "monitor",
    "resources/monitor",
    "monitor/bin/Release/net48",
    "monitor/bin/Debug/net48",
];
/// 向上回溯祖先目录的层数上限（覆盖从 `src-tauri/target/debug` 运行的开发场景）。
const ANCESTOR_DEPTH: usize = 6;
/// 回溯时逐层尝试的子路径。
const ANCESTOR_SUBDIRS: [&str; 3] = [
    "monitor/bin/Release/net48",
    "monitor/bin/Debug/net48",
    "monitor",
];

/// 探测 sidecar 可执行文件：先按同目录候选，再向上回溯祖先目录（开发环境）。
#[must_use]
pub fn find_monitor_exe() -> Option<PathBuf> {
    let base = std::env::current_exe().ok()?.parent()?.to_path_buf();
    candidate_paths(&base)
        .into_iter()
        .find(|path| path.exists())
}

/// 候选路径生成（与文件系统无关，单独拆出便于测试）。
#[must_use]
pub fn candidate_paths(base: &Path) -> Vec<PathBuf> {
    let mut paths: Vec<PathBuf> = SUBDIRS
        .iter()
        .map(|sub| base.join(sub).join(EXE_NAME))
        .collect();
    let mut probe = base.to_path_buf();
    for _ in 0..ANCESTOR_DEPTH {
        for sub in ANCESTOR_SUBDIRS {
            let candidate = probe.join(sub).join(EXE_NAME);
            if !paths.contains(&candidate) {
                paths.push(candidate);
            }
        }
        if !probe.pop() {
            break;
        }
    }
    paths
}

/// 故障形态（同时决定 DTO 里的 `MonitorState` 与用户可读原因）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FaultKind {
    /// 未找到 AegisMonitor.exe
    ExeMissing,
    /// 子进程起不来
    Spawn,
    /// 子进程已退出 / 管道失效
    Died,
    /// 在预算内没等到回行
    Timeout,
    /// 回了行但不是可解析的传感器 JSON
    Protocol,
    /// 连败达上限
    Disabled,
    /// 冷却期内被再次调用
    Cooling,
}
