//! 注册表**只读**半区（M-01 / Q-12=B，2026-10-03）。
//!
//! 🔴 本模块只提供"读"的能力，且刻意不认识任何具体产品名：
//! 第三方内核驱动（PawnIO）在本工程按 **零分发 / 零安装 / 仅只读检测** 处理
//! （许可证判定与义务见 `../../../DEPENDENCIES.md §四`），因此这里只需要"某个卸载键在不在"这一件事；
//! 版本与安装路径同为只读取证信息，供诊断面板展示。
//!
//! 移植来源：`Source/NexBox/src-tauri/src/pawnio_driver.rs` 的 L21-43（`check_pawnio_installed`）
//! 与 L46-60（`get_pawnio_info`）两段——🔴 同文件的 L110 起 `Start-Process -Verb RunAs` 静默安装段
//! **整体不移植**（禁档 + 特权红线），已在 `DEPENDENCIES.md §三` 登记为对母本的有意偏离。
//!
//! 后续批次的扩展点：`overview/software.rs`（片4）所需的 Uninstall 三视图枚举同样落本模块，
//! 保持"注册表访问只经 infra"的分层约束。

/// HKLM 卸载根键相对路径前缀（64/32 位两个视图都在它下面）。
#[cfg(windows)]
const UNINSTALL_ROOT: &str = r"SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall";

/// 一个内核驱动在系统中的登记状态（纯只读视图，不代表本应用安装过它）。
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DriverStatus {
    pub installed: bool,
    pub version: Option<String>,
    pub install_path: Option<String>,
}

/// 按卸载项名称只读探测某内核驱动是否已在系统中登记。
///
/// `uninstall_key` 是 `Uninstall` 根键下的子键名（如 `"PawnIO"`），由调用方给出，
/// 本模块不内置任何产品知识。
///
/// 非 Windows 平台与"读不到"一律返回 `installed = false`（等价于"无证据"，不得臆断）。
#[must_use]
pub fn detect_driver(uninstall_key: &str) -> DriverStatus {
    #[cfg(windows)]
    {
        use winreg::enums::{HKEY_LOCAL_MACHINE, KEY_READ, KEY_WOW64_32KEY, KEY_WOW64_64KEY};
        use winreg::RegKey;

        let root = RegKey::predef(HKEY_LOCAL_MACHINE);
        let path = format!(r"{UNINSTALL_ROOT}\{uninstall_key}");
        // 64 位视图优先，再试 32 位（WoW64）——与 LHM 的 IsInstalled 判定顺序一致
        for flag in [KEY_WOW64_64KEY, KEY_WOW64_32KEY] {
            if let Ok(key) = root.open_subkey_with_flags(&path, KEY_READ | flag) {
                let version: Option<String> = key.get_value("DisplayVersion").ok();
                // 无 DisplayVersion 的登记视为"未确认安装"，与母本判定口径保持一致
                if version.is_some() {
                    return DriverStatus {
                        installed: true,
                        version,
                        install_path: key.get_value("InstallLocation").ok(),
                    };
                }
            }
        }
        DriverStatus::default()
    }
    #[cfg(not(windows))]
    {
        let _ = uninstall_key;
        DriverStatus::default()
    }
}

#[cfg(all(test, windows))]
mod tests {
    use super::*;

    #[test]
    fn absent_key_reads_as_not_installed() {
        let status = detect_driver("Aegis-本機不可能存在的卸载键-0xdeadbeef");
        assert!(
            !status.installed,
            "读不到的键必须判为未安装（无证据不得臆断）：{status:?}"
        );
        assert!(status.version.is_none() && status.install_path.is_none());
    }

    #[test]
    fn root_path_matches_windows_uninstall_location() {
        // 路径写错会静默永远探测不到，故把常量钉住
        assert_eq!(
            UNINSTALL_ROOT,
            r"SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall"
        );
    }
}
