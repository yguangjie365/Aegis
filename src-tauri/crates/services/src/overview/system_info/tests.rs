//! `system_info` 域单测：🔴 不真查 WMI 的断言只用纯函数；三条本机取证断言显式标出。

use super::os::{kernel_version, os_family_name, os_panel, strip_version_token};
use super::readers::{display_language, system_locale, timezone_name};
use super::user::user_panel;
use super::{panels, readers};
use crate::overview::dto::{DetailCardDto, DetailKind, KvRowDto};

/// 按 key 取一行（None = 该行未出现）。
fn row<'a>(card: &'a DetailCardDto, key: &str) -> Option<&'a KvRowDto> {
    card.rows.iter().find(|row| row.key == key)
}

#[test]
fn os_panel_returns_os_kind() {
    let panel = os_panel();
    assert_eq!(panel.kind, DetailKind::Os);
    assert_eq!(panel.index, 0);
    // 至少应有 osName 行
    assert!(panel.rows.iter().any(|r| r.key == "detail.os.osName"));
}

#[test]
fn user_panel_returns_user_region_kind() {
    let panel = user_panel();
    assert_eq!(panel.kind, DetailKind::UserRegion);
    assert_eq!(panel.index, 0);
    // 至少应有 username 和 userType 行
    assert!(panel
        .rows
        .iter()
        .any(|r| r.key == "detail.userRegion.username"));
    assert!(panel
        .rows
        .iter()
        .any(|r| r.key == "detail.userRegion.userType"));
}

#[test]
fn panels_returns_two_cards() {
    let cards = panels();
    assert_eq!(cards.len(), 2);
    assert_eq!(cards[0].kind, DetailKind::Os);
    assert_eq!(cards[1].kind, DetailKind::UserRegion);
}

#[test]
fn kernel_version_returns_valid_tuple_on_windows() {
    let ver = kernel_version();
    if cfg!(windows) {
        assert!(
            ver.is_some(),
            "注册表 CurrentMajorVersionNumber 取不到 = 系统类型判定失效"
        );
        if let Some((major, _minor, _build)) = ver {
            // Windows 10/11 内核主版本应为 10
            assert!(major >= 10);
        }
    }
}

#[test]
fn os_family_name_maps_known_versions() {
    assert_eq!(os_family_name(10, 0, 22000), "Windows 11");
    assert_eq!(os_family_name(10, 0, 19045), "Windows 10");
    assert_eq!(os_family_name(6, 3, 9600), "Windows 8.1");
}

#[test]
fn strip_version_token_removes_leading_digits() {
    assert_eq!(strip_version_token(" 7 旗舰版"), "旗舰版");
    assert_eq!(strip_version_token(" 10 Pro"), "Pro");
    assert_eq!(strip_version_token("专业版"), "专业版");
}

/// 🔴 R-08 回归闸（收口批补正）：「上次登录时间」取不到时**必须留空并写明原因**，
/// 严禁再退回 `LastLoggedOnUser` 那种"用户名冒充时间"的错值。
#[test]
fn last_logon_is_missing_with_reason_never_a_username() {
    let panel = user_panel();
    let last_logon = row(&panel, "detail.userRegion.lastLogon")
        .expect("lastLogon 行必须出现（R-11 单项降级不删行）");
    assert!(
        last_logon.value.is_none(),
        "lastLogon 取不到值时必须为 null，实际 = {:?}",
        last_logon.value
    );
    let reason = panel.reason.unwrap_or_default();
    assert!(
        reason.contains("上次登录时间"),
        "空态必须写原因（R-08），实际 reason = {reason:?}"
    );
}

/// 🔴 R-11 回归闸：语言/区域/时区三项改走注册表后，Windows 上必须**真的取得值**
/// （旧实现读 `LANG`/`LC_ALL`/`TZ` 环境变量，Windows 桌面常态为空 → 恒"不可用"）。
#[test]
fn locale_and_timezone_come_from_registry_on_windows() {
    if !cfg!(windows) {
        return;
    }
    assert!(display_language().is_some(), "MUI\\UILanguages 读不到");
    assert!(system_locale().is_some(), "LocaleName 读不到");
    assert!(timezone_name().is_some(), "TimeZoneKeyName 读不到");
}

/// 🟢 分层纪律回归闸（KN-04）：本域只准走注册表 / `infra::wmi` / sysinfo 三条通道。
/// 静态判定用源码文本，🔴 不依赖运行期——防后来人再手写 FFI 回来。
#[test]
fn system_info_domain_has_no_hand_written_ffi() {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src/overview/system_info");
    // 门面与子模块一起扫；守卫所在的测试文件除外（它的字面量就是被检对象）
    let mut files: Vec<std::path::PathBuf> = vec![dir
        .parent()
        .expect("门面必与 system_info 目录同级")
        .join("system_info.rs")];
    files.extend(
        std::fs::read_dir(&dir)
            .expect("system_info 子模块目录缺失")
            .filter_map(|entry| entry.ok().map(|e| e.path()))
            .filter(|path| path.file_name().is_some_and(|name| name != "tests.rs")),
    );
    let mut offenders: Vec<String> = Vec::new();
    for path in files {
        let text = std::fs::read_to_string(&path).expect("读源文件失败");
        for (no, line) in text.lines().enumerate() {
            if line.trim_start().starts_with("//") {
                continue;
            }
            if line.contains("extern \"system\"") || line.contains("libloading") {
                offenders.push(format!(
                    "{}:{}",
                    path.file_name().unwrap().to_string_lossy(),
                    no + 1
                ));
            }
        }
    }
    assert!(
        offenders.is_empty(),
        "services 层出现手写 FFI（触 KN-04，FFI 只准在 infra / elevated-helper）：{offenders:?}"
    );
}

/// 🟡 本机取证：读数助手在真机上不该整片失灵（失败要看得见，而不是静默返回空表）。
#[test]
fn registry_readers_return_values_on_this_machine() {
    if !cfg!(windows) {
        return;
    }
    println!("本机 19 字段取值：{}", dump_panels());
    assert!(readers::read_reg_string(readers::CURRENT_VERSION_KEY, "ProductName").is_some());
    // uptime / boot_mode 在新口径下不会失败（最坏返回"状态未知"文案），故断言其**形态**而非非空。
    let uptime = readers::uptime_text();
    assert!(
        uptime.contains("分钟"),
        "运行时长必须落到分钟粒度，实际 = {uptime:?}"
    );
    let boot_mode = readers::boot_mode_info();
    assert!(
        boot_mode.contains(','),
        "启动模式必须是「固件, 快速启动」两段，实际 = {boot_mode:?}"
    );
    assert!(readers::runtime_environment().is_some());
}

/// 真机取证明细（`-- --nocapture` 才可见）：F-03 十九字段的实际形态，供验收单与变更记录取证。
fn dump_panels() -> String {
    use std::fmt::Write as _;
    let mut out = String::new();
    for card in super::panels() {
        for row in &card.rows {
            let _ = writeln!(&mut out, "{:?}.{}={:?}", card.kind, row.key, row.value);
        }
        let _ = writeln!(&mut out, "| {:?}.reason={:?}", card.kind, card.reason);
    }
    out
}
