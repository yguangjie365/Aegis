//! 「用户 / 区域」面板（F-03 后 8 字段）。

use super::readers::{
    display_language, in_administrators_group, keyboard_layout_name, system_locale, timezone_name,
};
use crate::overview::dto::{kv, DetailCardDto, DetailKind, KvRowDto};

/// 「用户 / 区域」面板（8 字段）。
#[must_use]
pub(super) fn user_panel() -> DetailCardDto {
    let mut rows: Vec<KvRowDto> = Vec::new();
    let mut reasons: Vec<String> = Vec::new();

    // 1. 当前用户 ← USERNAME 环境变量
    let username = std::env::var("USERNAME").ok().filter(|s| !s.is_empty());
    rows.push(kv("detail.userRegion.username", username));

    // 2. 用户类型 ← 账户是否在 Administrators 组（WMI 组成员关系）
    if let Some(is_member) = in_administrators_group() {
        let label = if is_member {
            "管理员"
        } else {
            "标准用户"
        };
        rows.push(kv("detail.userRegion.userType", Some(label.to_string())));
    } else {
        rows.push(kv("detail.userRegion.userType", None));
        reasons.push("无法读取账户组归属".to_string());
    }

    // 3. 用户目录 ← USERPROFILE
    let user_profile = std::env::var("USERPROFILE").ok().filter(|s| !s.is_empty());
    rows.push(kv("detail.userRegion.userProfile", user_profile));

    // 4. 上次登录时间：🔴 本机无免提权来源，如实留空（R-08 禁错值冒充，R-11 单项降级）。
    //    取证（2026-10-03，普通用户令牌）：LogonUI 键只有 LastLoggedOnUser / LastLoggedOnSAMUser /
    //    LastLoggedOnUserSID / IdleTime，**没有任何时间戳值**（旧实现把用户名 `.\BlueSky` 当时间显示）；
    //    `Win32_NetworkLoginProfile.LastLogon` 实测非管理员返回空。
    rows.push(kv("detail.userRegion.lastLogon", None));
    reasons.push("上次登录时间需管理员权限或安全日志，本机未采".to_string());

    // 5. 显示语言 ← MUI\UILanguages 首个语言标识
    rows.push(kv("detail.userRegion.displayLanguage", display_language()));

    // 6. 系统区域 ← Control Panel\International\LocaleName
    rows.push(kv("detail.userRegion.systemLocale", system_locale()));

    // 7. 时区 ← TimeZoneInformation\TimeZoneKeyName
    rows.push(kv("detail.userRegion.timezone", timezone_name()));

    // 8. 键盘布局 ← Keyboard Layout\Preload\1（能取到 Layout Text 则显示名称）
    rows.push(kv(
        "detail.userRegion.keyboardLayout",
        keyboard_layout_name(),
    ));

    let reason = if reasons.is_empty() {
        None
    } else {
        Some(reasons.join("; "))
    };

    DetailCardDto {
        kind: DetailKind::UserRegion,
        index: 0,
        rows,
        reason,
    }
}
