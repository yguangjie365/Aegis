//! 「操作系统」面板（F-03 前 11 字段）+ 操作系统名称的家族判定。

use super::readers::{
    boot_mode_info, read_reg_string, read_reg_u32, runtime_environment, uptime_text,
    wmi_computer_system_name, wmi_domain_info, wmi_os_install_date, wmi_windows_directory,
    CURRENT_VERSION_KEY,
};
use crate::overview::dto::{kv, DetailCardDto, DetailKind, KvRowDto};

/// 「操作系统」面板（11 字段）。
#[must_use]
pub(super) fn os_panel() -> DetailCardDto {
    let mut rows: Vec<KvRowDto> = Vec::new();
    let mut reasons: Vec<String> = Vec::new();

    // 1. 操作系统名称（内核版本优先于注册表 ProductName）
    rows.push(kv("detail.os.osName", long_os_version()));

    // 2. 版本（含内部构建号）← CurrentVersion\CurrentBuildNumber
    let build_number = read_reg_string(CURRENT_VERSION_KEY, "CurrentBuildNumber");
    if let Some(build) = &build_number {
        rows.push(kv("detail.os.version", Some(format!("内部版本 {build}"))));
    } else {
        reasons.push("无法读取系统构建号".to_string());
    }

    // 3. 安装日期 ← Win32_OperatingSystem.InstallDate（CIM_DATETIME → YYYY-MM-DD）
    let install_date = wmi_os_install_date();
    if let Some(date_str) = &install_date {
        rows.push(kv("detail.os.installDate", Some(date_str.clone())));
    } else {
        reasons.push("无法读取系统安装日期".to_string());
    }

    // 4. 系统类型（位数 + 处理器架构）← 环境变量 PROCESSOR_ARCHITECTURE
    let bits = if cfg!(target_pointer_width = "64") {
        "64 位"
    } else {
        "32 位"
    };
    let sys_type = match std::env::var("PROCESSOR_ARCHITECTURE") {
        Ok(arch) => format!("{bits}，{arch}"),
        Err(_) => bits.to_string(),
    };
    rows.push(kv("detail.os.systemType", Some(sys_type)));

    // 5. 体验扩展包 ← CurrentVersion\CSDVersion（Service Pack 字符串）
    let sp = read_reg_string(CURRENT_VERSION_KEY, "CSDVersion");
    if let Some(sp_val) = &sp {
        if !sp_val.is_empty() && sp_val != "Service Pack 0" {
            rows.push(kv("detail.os.servicePack", Some(sp_val.clone())));
        }
    }

    // 6. 设备名称 ← Win32_ComputerSystem.Name
    let computer_name = wmi_computer_system_name();
    rows.push(kv("detail.os.deviceName", computer_name));

    // 7. 工作组/域 ← Win32_ComputerSystem.Domain / PartOfDomain
    let domain_info = wmi_domain_info();
    if let Some(domain_str) = &domain_info {
        rows.push(kv("detail.os.workgroupOrDomain", Some(domain_str.clone())));
    }

    // 8. 系统目录 ← Win32_OperatingSystem.WindowsDirectory
    let windows_dir = wmi_windows_directory();
    if let Some(dir) = &windows_dir {
        rows.push(kv("detail.os.systemDirectory", Some(dir.clone())));
    }

    // 9. 启动模式（固件类型 + 快速启动开关状态）
    // 🔴 `boot_mode_info()` 返回 `String` 而非 `Option<String>`（readers.rs:102）：
    //    它最坏返回 "Legacy BIOS, 快速启动状态未知"，**不会失败**，故此行不存在
    //    "无法读取"分支（拆分 `system_info.rs` 时残留的 `if let Some` 会报 E0308）。
    let boot_mode = boot_mode_info();
    rows.push(kv("detail.os.bootMode", Some(boot_mode)));

    // 10. 运行时环境描述（.NET Framework 版本摘要）
    let runtime_desc = runtime_environment();
    if let Some(rt) = &runtime_desc {
        rows.push(kv("detail.os.runtimeEnvironment", Some(rt.clone())));
    }

    // 11. 运行时长（与 F-12 页头条同口径的 sysinfo 秒数，格式化在此完成）
    // `kv` 的第二参是 `Option<String>`（dto.rs:330），`uptime_text()` 返回 `String` → 需包 Some
    rows.push(kv("detail.os.uptime", Some(uptime_text())));

    let reason = if reasons.is_empty() {
        None
    } else {
        Some(reasons.join("; "))
    };

    DetailCardDto {
        kind: DetailKind::Os,
        index: 0,
        rows,
        reason,
    }
}

/// 操作系统名称（内核版本优先于注册表 ProductName；母本口径的**语义**继承、实现路径不继承）。
fn long_os_version() -> Option<String> {
    let product_name = read_reg_string(CURRENT_VERSION_KEY, "ProductName");

    let Some((major, minor, build)) = kernel_version() else {
        return product_name;
    };
    let real_family = os_family_name(major, minor, build);

    let Some(pn) = product_name else {
        return Some(real_family);
    };

    // 服务器系统直接用原名
    if pn.starts_with("Windows Server") {
        return Some(pn);
    }

    // 注册表家族与真实内核一致：直接沿用
    if pn.contains(&real_family) {
        return Some(pn);
    }

    // Win11 的 ProductName 仍是 "Windows 10 xx"，按真实内核替换
    if real_family == "Windows 11" && pn.contains("Windows 10") {
        return Some(pn.replacen("Windows 10", "Windows 11", 1));
    }

    // 家族不一致：保留后缀，替换家族名
    if let Some(idx) = pn.find("Windows") {
        let suffix = strip_version_token(&pn[idx + "Windows".len()..]);
        if suffix.is_empty() {
            return Some(real_family);
        }
        return Some(format!("{real_family} {suffix}"));
    }

    Some(real_family)
}

/// 内核版本 (major, minor, build) ← 注册表 `CurrentVersion` 的三个值（Win8+ 均存在，本机实测
/// major=10 / minor=0 / build=26300）。
///
/// 🔴 收口批前的初稿经 `libloading` 调 ntdll `RtlGetVersion` 取值，触 KN-04 的 FFI 落点表，
/// 也与片1 已登记的"不移植母本 `long_os_version` 实现路径"（`live.rs` 头注）自相矛盾；
/// 注册表这两项本就是 `RtlGetVersion` 所读同源数据的落盘形态，语义等价。
pub(super) fn kernel_version() -> Option<(u32, u32, u32)> {
    let major = read_reg_u32(CURRENT_VERSION_KEY, "CurrentMajorVersionNumber")?;
    let minor = read_reg_u32(CURRENT_VERSION_KEY, "CurrentMinorVersionNumber").unwrap_or(0);
    let build = read_reg_string(CURRENT_VERSION_KEY, "CurrentBuildNumber")?
        .parse::<u32>()
        .ok()?;

    Some((major, minor, build))
}

/// 根据内核版本判定系统家族名。
pub(super) fn os_family_name(major: u32, minor: u32, build: u32) -> String {
    match (major, minor) {
        (10, _) if build >= 22000 => "Windows 11".to_string(),
        (10, _) => "Windows 10".to_string(),
        (6, 3) => "Windows 8.1".to_string(),
        (6, 2) => "Windows 8".to_string(),
        (6, 1) => "Windows 7".to_string(),
        _ => format!("Windows {major}.{minor}"),
    }
}

/// 去除 `ProductName` 中紧随 "Windows" 之后的旧版本号。
pub(super) fn strip_version_token(rest: &str) -> String {
    let rest = rest.trim_start();
    let bytes = rest.as_bytes();
    let mut i = 0;
    while i < bytes.len() && (bytes[i].is_ascii_digit() || bytes[i] == b'.') {
        i += 1;
    }
    if i > 0 {
        rest[i..].trim_start().to_string()
    } else {
        rest.to_string()
    }
}
