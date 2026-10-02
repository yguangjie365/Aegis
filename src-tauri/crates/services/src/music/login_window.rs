//! 登录窗口域逻辑（移植自 NexBox `music_api/mod.rs`「登录窗口 - 多平台」段，GPL-3.0 同源衍生）。
//!
//! 只承接纯逻辑（宿主窗口编排在 app crate `music_login.rs`）：
//! - 平台 cookie 优先级表与域名判定（决定 webview cookie 的取舍与拼接顺序）
//! - 从 webview Cookie 列表构建 cookie 字符串
//! - 第三方登录弹窗完成判定（OAuth 回跳到平台域名 = 授权走完）
//!
//! `tauri::webview::Cookie` 在此仅作数据载体（name/value/domain 读取），不涉宿主窗口类型。

use std::collections::HashMap;

use tauri::Url;

/// 网易云登录 cookie 优先级（参考 Mineradio）
pub const NETEASE_COOKIE_PRIORITY: &[&str] = &[
    "MUSIC_U",
    "__csrf",
    "NMTID",
    "MUSIC_A",
    "__remember_me",
    "_ntes_nuid",
    "_ntes_nnid",
    "WEVNSM",
    "WNMCID",
    "JSESSIONID-WYYY",
];

/// QQ 音乐登录 cookie 优先级（参考 Mineradio QQ_LOGIN_COOKIE_PRIORITY）
pub const QQ_COOKIE_PRIORITY: &[&str] = &[
    "uin",
    "qqmusic_uin",
    "wxuin",
    "login_type",
    "qm_keyst",
    "qqmusic_key",
    "p_skey",
    "skey",
    "psrf_qqopenid",
    "psrf_qqunionid",
    "psrf_qqaccess_token",
    "psrf_qqrefresh_token",
    "wxopenid",
    "wxunionid",
    "wxrefresh_token",
    "wxskey",
    "p_uin",
    "ptcz",
    "RK",
];

/// 检查域名是否属于网易云
pub fn is_netease_domain(domain: &str) -> bool {
    let d = domain.trim_start_matches('.').to_lowercase();
    d == "163.com"
        || d.ends_with(".163.com")
        || d == "music.163.com"
        || d.ends_with(".music.163.com")
        || d == "netease.com"
        || d.ends_with(".netease.com")
}

/// 检查域名是否属于 QQ 音乐
pub fn is_qq_domain(domain: &str) -> bool {
    let d = domain.trim_start_matches('.').to_lowercase();
    d == "qq.com" || d.ends_with(".qq.com")
}

/// 从 webview cookies 构建指定平台的 cookie 字符串
///
/// 先按 `priority` 顺序挑出目标平台的 cookie（`domain_check` 过滤），
/// 剩余的本平台 cookie 追加在后。
pub fn build_cookie_from_webview(
    cookies: &[tauri::webview::Cookie<'_>],
    priority: &[&str],
    domain_check: fn(&str) -> bool,
) -> String {
    let mut picked: HashMap<String, String> = HashMap::new();
    for c in cookies {
        if let Some(domain) = c.domain() {
            if domain_check(domain) {
                let name = c.name().to_string();
                let value = c.value().to_string();
                if !name.is_empty() && !value.is_empty() {
                    picked.insert(name, value);
                }
            }
        }
    }
    let mut ordered: Vec<(String, String)> = Vec::new();
    for name in priority {
        if let Some(value) = picked.remove(*name) {
            ordered.push((name.to_string(), value));
        }
    }
    for (name, value) in picked {
        ordered.push((name, value));
    }
    ordered
        .iter()
        .map(|(k, v)| format!("{k}={v}"))
        .collect::<Vec<_>>()
        .join("; ")
}

/// 判断登录子窗口当前 URL 是否表示「第三方授权已走完、可以关窗了」
///
/// 实际回跳链路（以 QQ 为例）：
/// ```text
/// graph.qq.com/oauth2.0/show?...     ← 第三方授权页
///   → {平台域名}/...                  ← 平台收 code / 换票据
///   → {平台域名}/...                  ← 回主站，cookie 已落地
/// ```
/// 所以判据是「已经回到平台域名」。同时必须排除第三方授权域，
/// 否则 `graph.qq.com` 若含平台串就会误关。
pub fn login_popup_should_close(url: &Url) -> bool {
    let Some(host) = url.host_str().map(str::to_lowercase) else {
        return false;
    };

    // 第三方授权域：出现这些说明还在授权流程中，绝不能关
    const THIRD_PARTY: &[&str] = &[
        "graph.qq.com",
        "open.weixin.qq.com",
        "api.weibo.com",
        "ptlogin2.qq.com",
        "xui.ptlogin2.qq.com",
        "openapi.weibo.com",
        "passport.",
    ];
    if THIRD_PARTY
        .iter()
        .any(|d| host == *d || host.ends_with(&format!(".{d}")))
    {
        return false;
    }

    // 回到平台域名 = 授权完成
    const PLATFORM: &[&str] = &[
        "163.com", // 网易云
        "qq.com",  // QQ 音乐（y.qq.com 等）
    ];
    PLATFORM
        .iter()
        .any(|d| host == *d || host.ends_with(&format!(".{d}")))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn u(s: &str) -> Url {
        s.parse().expect("test url should parse")
    }

    fn c(name: &str, value: &str, domain: &str) -> tauri::webview::Cookie<'static> {
        tauri::webview::cookie::Cookie::build((name.to_string(), value.to_string()))
            .domain(domain.to_string())
            .build()
    }

    /// 真实的 QQ 登录回跳链：授权页不能提前关，回到平台域名后必须关
    #[test]
    fn qq_redirect_chain_closes_only_at_end() {
        assert!(
            !login_popup_should_close(&u(
                "https://graph.qq.com/oauth2.0/show?which=ConfirmPage&display=pc&client_id=100243533"
            )),
            "QQ 授权页不应被关闭"
        );
        assert!(
            login_popup_should_close(&u("https://music.163.com/back/qq?code=xxx")),
            "回到网易云域名后应关闭"
        );
        assert!(
            login_popup_should_close(&u("https://y.qq.com/portal/profile.html")),
            "回到 QQ 音乐域名后应关闭"
        );
    }

    /// 其它平台的第三方域不能误判为「已完成」
    #[test]
    fn other_third_party_domains_stay_open() {
        for c in [
            "https://open.weixin.qq.com/connect/qrconnect?appid=wx41c1275bb3e28427",
            "https://api.weibo.com/oauth2/authorize?client_id=2972927130",
            "https://xui.ptlogin2.qq.com/cgi-bin/xlogin",
        ] {
            assert!(
                !login_popup_should_close(&u(c)),
                "第三方授权域不应被关闭: {c}"
            );
        }
    }

    /// 空 host / about:blank 不能触发关闭
    #[test]
    fn blank_and_hostless_urls_stay_open() {
        assert!(!login_popup_should_close(&u("about:blank")));
    }

    /// 伪造相似域名不能被当作平台域（防后缀混淆）
    #[test]
    fn lookalike_domains_are_not_treated_as_platform() {
        for c in [
            "https://163.com.evil.com/callback",
            "https://fakeqq.com.evil.com/callback",
        ] {
            assert!(!login_popup_should_close(&u(c)), "相似域名不应被误判: {c}");
        }
    }

    #[test]
    fn domain_checks() {
        assert!(is_netease_domain(".music.163.com"));
        assert!(is_netease_domain("163.com"));
        assert!(!is_netease_domain("163.com.evil.com"));
        assert!(is_qq_domain(".qq.com"));
        assert!(is_qq_domain("y.qq.com"));
        assert!(!is_qq_domain("fakeqq.com"));
    }

    /// 优先级生效：MUSIC_U/__csrf 按表排前，非同域 cookie 被过滤
    #[test]
    fn build_cookie_prioritizes_and_filters_by_domain() {
        let cookies = vec![
            c("MUSIC_U", "abc", "music.163.com"),
            c("__csrf", "xyz", ".music.163.com"),
            c("SOME_OTHER", "1", "example.com"),
            c("WID", "w", ".163.com"),
        ];
        let out = build_cookie_from_webview(&cookies, NETEASE_COOKIE_PRIORITY, is_netease_domain);
        assert!(out.starts_with("MUSIC_U=abc; __csrf=xyz"));
        assert!(out.contains("WID=w"));
        assert!(!out.contains("SOME_OTHER"));
    }
}
