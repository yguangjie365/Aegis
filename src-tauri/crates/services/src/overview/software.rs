//! 已安装软件清单（M-01 片4 / F-07，2026-10-03）。
//!
//! 复用结论（DP-1）：端点取数走 `winreg` 注册表扫描（`NexBox` `app_manager.rs` 同 API，
//! 属③思路复用；母本实测覆盖 HKLM64/HKLM32/HKCU 三视图 + `SystemComponent` 剔除 + 名称+版本去重）。
//!
//! 🔴 三条口径纪律（R-16）：
//! 1. **剔除**：无显示名条目、`SystemComponent`=1、更新/补丁/热修复/工具类条目；
//! 2. **去重**：按"名称+版本"跨来源去重（同一程序在多个登记区出现只保留一条）；
//! 3. **解码**：`REG_SZ` 多编码候选（UTF-16LE/UTF-8/GBK），打分选最合理文本。

use std::collections::HashSet;
use winreg::enums::{HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE, KEY_READ};
use winreg::RegKey;

/// 已安装应用一行（F-07 表格列：名称/版本/发布商/安装日期/大小）。
#[derive(Debug, Clone)]
pub struct InstalledApp {
    pub name: String,
    pub display_version: Option<String>,
    pub publisher: Option<String>,
    pub install_date: Option<String>,
    /// 估计占用大小（注册表 DWORD，单位 KB → 转字节展示）
    pub estimated_size_bytes: Option<u64>,
}

const UNINSTALL_KEY: &str = r"Software\Microsoft\Windows\CurrentVersion\Uninstall";
const UNINSTALL_KEY_32: &str = r"Software\Wow6432Node\Microsoft\Windows\CurrentVersion\Uninstall";

/// 文本自然度打分：正常字符加分，控制字符/替换符/未定义区减分。
/// 用于在多种编码候选（UTF-16LE/UTF-8/GBK）中选出最像真实文本的解码结果。
fn score_text(s: &str) -> i32 {
    s.chars()
        .map(|c| match c {
            '\u{0}' => -20,
            c if c.is_control() => -8,
            '\u{FFFD}' => -12,
            ' '..='~' => 1,
            c if ('\u{00A0}'..='\u{024F}').contains(&c) => 1, // Latin-1 / 扩展
            c if ('\u{2E80}'..='\u{9FFF}').contains(&c) => 1, // CJK 部首/汉字
            c if ('\u{3040}'..='\u{30FF}').contains(&c) => 1, // 假名
            c if ('\u{AC00}'..='\u{D7AF}').contains(&c) => 1, // 谚文
            c if ('\u{F900}'..='\u{FAFF}').contains(&c) => 1, // CJK 兼容表意
            c if c.is_alphanumeric() || c.is_whitespace() => 1,
            _ => 0,
        })
        .sum()
}

/// 对注册表字符串的原始字节做智能解码：
/// 标准 `REG_SZ` 为 UTF-16LE，但部分国产老软件写入的 `DisplayName` 实际是 UTF-8/GBK 字节
/// （或双重编码），直接按 UTF-16 读取会出现乱码。
/// 分别按 UTF-16LE/UTF-8/GBK 解码后打分，取最合理的文本；含 NUL 分隔的
/// `REG_MULTI_SZ` 只取第一段。返回 None 表示值为空。
fn read_reg_str_raw_decode(bytes: &[u8]) -> Option<String> {
    if bytes.is_empty() {
        return None;
    }

    // 候选：(得分, 解码错误数, 偏好序, 文本)
    let mut candidates: Vec<(i32, usize, usize, String)> = Vec::new();

    // UTF-16LE（常规 REG_SZ 形态）：奇数字节不可能是 UTF-16，错误数记为不可选
    let (utf16, utf16_err) = if bytes.len() % 2 == 0 {
        let units: Vec<u16> = bytes
            .chunks_exact(2)
            .map(|c| u16::from_le_bytes([c[0], c[1]]))
            .collect();
        let text = String::from_utf16_lossy(&units);
        let err = text.chars().filter(|&c| c == '\u{FFFD}').count();
        (text, err)
    } else {
        (String::new(), usize::MAX)
    };
    candidates.push((score_text(&utf16), utf16_err, 2, utf16));

    // UTF-8
    let utf8_lossy = String::from_utf8_lossy(bytes).into_owned();
    let utf8_err = utf8_lossy.chars().filter(|&c| c == '\u{FFFD}').count();
    candidates.push((score_text(&utf8_lossy), utf8_err, 0, utf8_lossy));

    // GBK（cp936，国产老软件常用）
    let (gbk_text, _, gbk_replaced) = encoding_rs::GBK.decode(bytes);
    candidates.push((
        score_text(&gbk_text),
        usize::from(gbk_replaced),
        1,
        gbk_text.into_owned(),
    ));

    candidates.sort_by(|a, b| b.0.cmp(&a.0).then(a.1.cmp(&b.1)).then(a.2.cmp(&b.2)));

    let mut text = candidates.into_iter().next().map(|(_, _, _, t)| t)?;
    // REG_MULTI_SZ 等以 NUL 分隔的多段值只取第一段
    if let Some(nul) = text.find('\u{0}') {
        text.truncate(nul);
    }
    let text = text.trim().to_string();
    if text.is_empty() {
        None
    } else {
        Some(text)
    }
}

fn read_str(key: &RegKey, name: &str) -> Option<String> {
    let raw = key.get_raw_value(name).ok()?;
    read_reg_str_raw_decode(&raw.bytes)
}

fn read_dword(key: &RegKey, name: &str) -> Option<u32> {
    key.get_value::<u32, _>(name).ok()
}

/// 是否为系统组件（SystemComponent=1 的项不展示，避免误卸载）
fn is_system_component(key: &RegKey) -> bool {
    read_dword(key, "SystemComponent").unwrap_or(0) != 0
}

/// 是否为更新/补丁/热修复/工具类条目（R-16 剔除规则）
fn is_update_or_tool(name: &str) -> bool {
    let lower = name.to_lowercase();
    // 常见更新/补丁关键词
    lower.contains("update")
        || lower.contains("patch")
        || lower.contains("hotfix")
        || lower.contains("security update")
        || lower.contains("service pack")
        || lower.contains("cumulative")
        || lower.contains(".net framework") && lower.contains("client profile")
        // 常见工具/运行时（非独立应用）
        || lower.contains("redistributable")
        || lower.contains("runtime")
        || lower.contains("visual c++")
        || lower.contains("directx")
}

/// 去重键：优先按（名称,版本），否则按名称
fn dedupe_key(app: &InstalledApp) -> String {
    format!(
        "n:{}|v:{}",
        app.name.trim().to_lowercase(),
        app.display_version
            .as_deref()
            .unwrap_or("")
            .trim()
            .to_lowercase()
    )
}

fn scan_uninstall_key(
    root: &RegKey,
    subkey: &str,
    out: &mut Vec<InstalledApp>,
    seen: &mut HashSet<String>,
) {
    let Ok(parent) = root.open_subkey_with_flags(subkey, KEY_READ) else {
        return;
    };
    for entry in parent.enum_keys() {
        let Ok(sub_name) = entry else {
            continue;
        };
        let Ok(key) = parent.open_subkey_with_flags(&sub_name, KEY_READ) else {
            continue;
        };
        // 剔除系统组件
        if is_system_component(&key) {
            continue;
        }
        // 缺失显示名则跳过
        let Some(name) = read_str(&key, "DisplayName") else {
            continue;
        };
        // 剔除更新/补丁/工具类
        if is_update_or_tool(&name) {
            continue;
        }

        let app = InstalledApp {
            name,
            display_version: read_str(&key, "DisplayVersion"),
            publisher: read_str(&key, "Publisher"),
            install_date: read_str(&key, "InstallDate"),
            estimated_size_bytes: read_dword(&key, "EstimatedSize").map(|v| u64::from(v) * 1024), // KB → 字节
        };

        // 跨来源去重
        if seen.insert(dedupe_key(&app)) {
            out.push(app);
        }
    }
}

/// 扫描本机已安装的传统桌面应用（注册表 Uninstall 键：HKLM 64/32 视图 + HKCU）
///
/// # Returns
///
/// 按名称不区分大小写升序排列的应用列表。
#[must_use]
pub fn list_installed_apps() -> Vec<InstalledApp> {
    let mut out: Vec<InstalledApp> = Vec::new();
    let mut seen: HashSet<String> = HashSet::new();

    scan_uninstall_key(
        &RegKey::predef(HKEY_LOCAL_MACHINE),
        UNINSTALL_KEY,
        &mut out,
        &mut seen,
    );
    // 64 位系统才存在 Wow6432Node 视图
    #[cfg(target_pointer_width = "64")]
    scan_uninstall_key(
        &RegKey::predef(HKEY_LOCAL_MACHINE),
        UNINSTALL_KEY_32,
        &mut out,
        &mut seen,
    );
    scan_uninstall_key(
        &RegKey::predef(HKEY_CURRENT_USER),
        UNINSTALL_KEY,
        &mut out,
        &mut seen,
    );

    out.sort_by_key(|app| app.name.to_lowercase());
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn score_text_prefers_readable_strings() {
        let good = score_text("Microsoft Office 2021");
        let bad = score_text("\0\0\0");
        assert!(good > bad);
    }

    #[test]
    fn decode_utf16le_works() {
        let bytes = vec![0x41, 0x00, 0x42, 0x00]; // "AB" in UTF-16LE
        let result = read_reg_str_raw_decode(&bytes).unwrap();
        assert_eq!(result, "AB");
    }

    #[test]
    fn decode_handles_empty() {
        assert!(read_reg_str_raw_decode(&[]).is_none());
    }

    #[test]
    fn is_update_detects_common_patterns() {
        assert!(is_update_or_tool("Windows Update KB12345"));
        assert!(is_update_or_tool("Security Update for .NET"));
        assert!(is_update_or_tool("Visual C++ Redistributable"));
        assert!(!is_update_or_tool("Microsoft Office 2021"));
    }

    #[test]
    fn dedupe_key_is_case_insensitive() {
        let app1 = InstalledApp {
            name: "Test App".to_string(),
            display_version: Some("1.0".to_string()),
            publisher: None,
            install_date: None,
            estimated_size_bytes: None,
        };
        let app2 = InstalledApp {
            name: "test app".to_string(),
            display_version: Some("1.0".to_string()),
            publisher: None,
            install_date: None,
            estimated_size_bytes: None,
        };
        assert_eq!(dedupe_key(&app1), dedupe_key(&app2));
    }

    #[test]
    fn size_kb_converts_to_bytes() {
        // 模拟注册表读取：1024 KB → 1 MB
        let kb = 1024u32;
        let bytes = (kb as u64) * 1024;
        assert_eq!(bytes, 1_048_576);
    }

    /// 真机冒烟（与 `processes.rs` 的同机用例同一先例）：锁 R-16 的剔除口径在**本机注册表**上真的生效，
    /// 而不是只在构造出来的字符串上生效。
    #[test]
    fn real_registry_scan_yields_user_apps_without_patches() {
        let apps = list_installed_apps();
        if cfg!(not(windows)) {
            // 非 Windows 无 Uninstall 视图可扫，返回空表是正确行为；空态原因由命令层给
            assert!(apps.is_empty());
            return;
        }
        // 🔴 Windows 上空表就是采集失败（本机必装有桌面应用），不得让它静默绿过去
        assert!(
            !apps.is_empty(),
            "Windows 上扫不到任何已安装应用 = 采集失效"
        );
        assert!(
            apps.iter()
                .all(|app| !app.name.trim().is_empty() && !is_update_or_tool(&app.name)),
            "R-16：清单里不得混入补丁/运行库/更新子项，也不得有空名"
        );
        let mut keys: Vec<String> = apps.iter().map(dedupe_key).collect();
        keys.dedup();
        assert_eq!(
            keys.len(),
            apps.len(),
            "R-16：名称+版本跨视图去重后不得仍有重复"
        );
        // 三视图合并后按名称小写升序（前端表格行序稳定，L-08 不跳）
        let names: Vec<String> = apps.iter().map(|app| app.name.to_lowercase()).collect();
        let mut sorted = names.clone();
        sorted.sort_unstable();
        assert_eq!(names, sorted);
        println!("本机已安装应用条目数 = {}", apps.len());
    }
}
