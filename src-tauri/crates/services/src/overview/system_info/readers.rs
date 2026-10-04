//! 系统信息面板的读数助手（`system_info` 的子模块）：注册表 / WMI / sysinfo 三条合规通道。
//!
//! 🔴 本文件零 FFI（理由见门面头注的分层纪律条）。每个读数**独立容错、失败即 None**，
//! 不做跨字段推断，也不用 0 或占位字符串冒充（R-08）。

use winreg::enums::{HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE};
use winreg::RegKey;

/// 内核版本与产品名的共同出处。
pub(super) const CURRENT_VERSION_KEY: &str = r"SOFTWARE\Microsoft\Windows NT\CurrentVersion";

/// 读取 HKLM 注册表字符串值。
pub(super) fn read_reg_string(path: &str, value_name: &str) -> Option<String> {
    let hklm = RegKey::predef(HKEY_LOCAL_MACHINE);
    hklm.open_subkey(path)
        .and_then(|key| key.get_value::<String, _>(value_name))
        .ok()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
}

/// 读取 HKCU 注册表字符串值。
pub(super) fn read_hkcu_string(path: &str, value_name: &str) -> Option<String> {
    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    hkcu.open_subkey(path)
        .and_then(|key| key.get_value::<String, _>(value_name))
        .ok()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
}

/// 读取 HKLM 注册表 DWORD 值。
pub(super) fn read_reg_u32(path: &str, value_name: &str) -> Option<u32> {
    let hklm = RegKey::predef(HKEY_LOCAL_MACHINE);
    hklm.open_subkey(path)
        .and_then(|key| key.get_value::<u32, _>(value_name))
        .ok()
}

/// `Win32_OperatingSystem.InstallDate` → YYYY-MM-DD。
pub(super) fn wmi_os_install_date() -> Option<String> {
    use infra::wmi;

    let rows = wmi::query("SELECT InstallDate FROM Win32_OperatingSystem").ok()?;

    let cim_dt = rows.first()?.text("InstallDate")?;
    // CIM_DATETIME 格式：YYYYMMDDHHMMSS.mmmmmm+UUU
    let digits: String = cim_dt.chars().filter(char::is_ascii_digit).collect();
    if digits.len() < 8 {
        return None;
    }
    Some(format!(
        "{}-{}-{}",
        &digits[0..4],
        &digits[4..6],
        &digits[6..8]
    ))
}

/// `Win32_ComputerSystem.Name` → 设备名称。
pub(super) fn wmi_computer_system_name() -> Option<String> {
    use infra::wmi;

    let rows = wmi::query("SELECT Name FROM Win32_ComputerSystem").ok()?;

    rows.first().and_then(|row| row.text("Name"))
}

/// `Win32_ComputerSystem.Domain` + `PartOfDomain` → 工作组/域。
pub(super) fn wmi_domain_info() -> Option<String> {
    use infra::wmi;

    let rows = wmi::query("SELECT Domain, PartOfDomain FROM Win32_ComputerSystem").ok()?;
    let row = rows.first()?;
    let domain = row.text("Domain").unwrap_or_default();
    let part_of_domain = row
        .text("PartOfDomain")
        .and_then(|v| v.parse::<bool>().ok())
        .unwrap_or(false);

    Some(if part_of_domain {
        format!("域: {domain}")
    } else {
        format!("工作组: {domain}")
    })
}

/// `Win32_OperatingSystem.WindowsDirectory` → 系统目录。
pub(super) fn wmi_windows_directory() -> Option<String> {
    use infra::wmi;

    let rows = wmi::query("SELECT WindowsDirectory FROM Win32_OperatingSystem").ok()?;

    rows.first().and_then(|row| row.text("WindowsDirectory"))
}

/// 启动模式：固件类型（UEFI/Legacy BIOS）+ 快速启动状态。
///
/// 两个分支都是**有证据的描述**：`SecureBoot` 键不存在就写 Legacy BIOS，`HiberbootEnabled`
/// 取不到就写"状态未知"——本函数无失败形态，故返回 `String` 而非 `Option`。
#[must_use]
pub(super) fn boot_mode_info() -> String {
    // 固件类型：HKLM\SYSTEM\CurrentControlSet\Control\SecureBoot\State\UEFISecureBootEnabled
    // 更可靠的方式：检查是否存在 HKLM\SYSTEM\CurrentControlSet\Control\SecureBoot
    let is_uefi = RegKey::predef(HKEY_LOCAL_MACHINE)
        .open_subkey(r"SYSTEM\CurrentControlSet\Control\SecureBoot")
        .is_ok();

    // 快速启动：HKLM\SYSTEM\CurrentControlSet\Control\Session Manager\Power\HiberbootEnabled
    let hiberboot = read_reg_u32(
        r"SYSTEM\CurrentControlSet\Control\Session Manager\Power",
        "HiberbootEnabled",
    );

    let firmware = if is_uefi { "UEFI" } else { "Legacy BIOS" };
    let fast_boot = match hiberboot {
        Some(1) => "快速启动已启用",
        Some(0) => "快速启动已禁用",
        _ => "快速启动状态未知",
    };

    format!("{firmware}, {fast_boot}")
}

/// 运行时环境描述（.NET Framework 最高版本）。
pub(super) fn runtime_environment() -> Option<String> {
    // HKLM\SOFTWARE\Microsoft\NET Framework Setup\NDP\v4\Full\Release
    let release = read_reg_u32(
        r"SOFTWARE\Microsoft\NET Framework Setup\NDP\v4\Full",
        "Release",
    );

    release.map(|r| {
        // Release 值映射到 .NET 版本（微软官方对照表）
        let version = if r >= 533_320 {
            "4.8.1"
        } else if r >= 528_040 {
            "4.8"
        } else if r >= 461_808 {
            "4.7.2"
        } else if r >= 461_308 {
            "4.7.1"
        } else if r >= 460_798 {
            "4.7"
        } else if r >= 394_802 {
            "4.6.2"
        } else if r >= 394_254 {
            "4.6.1"
        } else if r >= 393_295 {
            "4.6"
        } else {
            "4.x（早期版本）"
        };
        format!(".NET Framework {version}")
    })
}

/// 系统运行时长（天/小时/分钟）← `sysinfo::System::uptime()`。
///
/// 🔴 与 F-12 页头条的 `uptime_secs` 同一 API、同一口径（`live/sampling.rs`），🟢 不另起炉灶：
/// 收口批前的初稿在此手写 `extern "system" { fn GetTickCount64() }`，触 KN-04 的 FFI 落点表。
#[must_use]
pub(super) fn uptime_text() -> String {
    let total_seconds = sysinfo::System::uptime();
    let days = total_seconds / 86_400;
    let hours = (total_seconds % 86_400) / 3_600;
    let minutes = (total_seconds % 3_600) / 60;

    if days > 0 {
        format!("{days} 天 {hours} 小时 {minutes} 分钟")
    } else if hours > 0 {
        format!("{hours} 小时 {minutes} 分钟")
    } else {
        format!("{minutes} 分钟")
    }
}

/// 当前账户是否属于 `Administrators` 组（F-03「用户类型」口径）。
///
/// WQL 形态实建于 2026-10-03 本机（普通用户令牌）：`Win32_Group.Domain` 的实测值是**计算机名**
/// （`BLUESKY`）而非 `BUILTIN`，且组名值需以 `\"` 转义嵌进 `GroupComponent` 字面量；两种引号写法
/// 实测都返回 2 名成员（`Administrator` / `BlueSky`）。
/// 🔴 不用 `IsUserAnAdmin`：一是手写绑定触 KN-04，二是它的语义是"令牌已提权"而非"账户是管理员"。
#[must_use]
pub(super) fn in_administrators_group() -> Option<bool> {
    use infra::wmi;

    let username = std::env::var("USERNAME").ok().filter(|s| !s.is_empty())?;
    let domain = std::env::var("COMPUTERNAME")
        .ok()
        .filter(|s| !s.is_empty())?;
    // 名称含引号或反斜杠会让 WQL 字面量失真——宁可不取，也不发一条语义变形的查询
    if [username.as_str(), domain.as_str()]
        .iter()
        .any(|s| s.contains('"') || s.contains('\\'))
    {
        return None;
    }

    let group_path = format!("Win32_Group.Domain=\\\"{domain}\\\",Name=\\\"Administrators\\\"");
    let wql =
        format!("SELECT PartComponent FROM Win32_GroupUser WHERE GroupComponent=\"{group_path}\"");
    let rows = wmi::query(&wql).ok()?;
    let needle = format!("name=\"{username}\"").to_lowercase();

    Some(rows.iter().any(|row| {
        row.text("PartComponent")
            .is_some_and(|part| part.to_lowercase().contains(&needle))
    }))
}

/// 显示语言 ← `HKLM\SYSTEM\CurrentControlSet\Control\MUI\UILanguages` 下首个语言标识子键。
/// （本机实测 `zh-CN`。收口批前的初稿读 `LANG` 环境变量，Windows 上通常为空 → 恒"不可用"。）
#[must_use]
pub(super) fn display_language() -> Option<String> {
    let key = RegKey::predef(HKEY_LOCAL_MACHINE)
        .open_subkey(r"SYSTEM\CurrentControlSet\Control\MUI\UILanguages")
        .ok()?;
    let mut names: Vec<String> = key.enum_keys().filter_map(Result::ok).collect();
    names.sort();
    names.into_iter().next()
}

/// 系统区域 ← `HKCU\Control Panel\International\LocaleName`（本机实测 `zh-CN`）。
#[must_use]
pub(super) fn system_locale() -> Option<String> {
    read_hkcu_string(r"Control Panel\International", "LocaleName")
}

/// 时区 ← `HKLM\SYSTEM\CurrentControlSet\Control\TimeZoneInformation\TimeZoneKeyName`
/// （本机实测 `China Standard Time`；这是系统的规范时区名，不做中文译名表）。
#[must_use]
pub(super) fn timezone_name() -> Option<String> {
    read_reg_string(
        r"SYSTEM\CurrentControlSet\Control\TimeZoneInformation",
        "TimeZoneKeyName",
    )
}

/// 键盘布局 ← `HKCU\Keyboard Layout\Preload\1` 的布局标识符；
/// 若 `HKLM\SYSTEM\CurrentControlSet\Control\Keyboard Layouts\<标识符>\Layout Text` 存在则优先显示名称。
///
/// 本机实测（2026-10-03 收口批）：标识符 `00000804` 在 `Keyboard Layouts` 下有项，
/// `Layout Text` = "Chinese (Simplified) - US Keyboard" → 走名称分支。取不到名称时才退化为
/// 显示标识符本身（真实值，不是猜测）。
#[must_use]
pub(super) fn keyboard_layout_name() -> Option<String> {
    let id = read_hkcu_string(r"Keyboard Layout\Preload", "1")?;
    let layout_text = read_reg_string(
        &format!(r"SYSTEM\CurrentControlSet\Control\Keyboard Layouts\{id}"),
        "Layout Text",
    );
    Some(layout_text.unwrap_or(id))
}
