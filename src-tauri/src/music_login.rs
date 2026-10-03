//! 音乐登录窗口（MU-4.15，移植自 NexBox `music_api/mod.rs`「登录窗口 - 多平台」段）。
//!
//! 用户点击平台登录后弹出该平台官方登录页（页面默认即扫码登录），宿主层轮询 webview
//! cookie，捕获登录态后写入 Credential Manager 并 emit 事件（`music-login-success` /
//! `music-login-failed`）。
//!
//! 🔴 本文件是对「app crate 只作 `services::<域>::<command>` 薄转发」的显式例外：
//! `WebviewWindowBuilder` / `cookies()` 均为宿主类型，无法下沉 services；
//! 域逻辑（cookie 取舍/拼接/回跳判定）在 `services::music::login_window`，
//! 凭据保存与登录态校验全部走 `services::music`。新增窗口命令须在此登记。

use std::time::Duration;

use contracts::CmdEnvelope;
use services::music::login_window::{
    build_cookie_from_webview, is_netease_domain, is_qq_domain, login_popup_should_close,
    NETEASE_COOKIE_PRIORITY, QQ_COOKIE_PRIORITY,
};
use services::music::music_api::cookie as music_cookie;
use tauri::{AppHandle, Emitter, Manager, Url, WebviewUrl, WebviewWindow, WebviewWindowBuilder};

/// WebView2 参数：与主窗口口径一致（禁 Chromium 自动媒体会话，避免与 SMTC 会话重复）
const LOGIN_WINDOW_BROWSER_ARGS: &str = "--disable-features=MediaSessionService,HardwareMediaKeyHandling,msWebOOUI,msPdfOOUI,msSmartScreenProtection,msEdgeAutofill,msEdgeShopping,msEdgeWallet --autoplay-policy=no-user-gesture-required --disable-background-networking --disable-client-side-phishing-detection --disable-component-update --disable-default-apps --disable-extensions --disable-sync";

/// 单平台登录流参数（两平台差异的单一归集点）
struct LoginFlow {
    label: &'static str,
    title: &'static str,
    login_url: &'static str,
    /// 登录态不完整时跳转的预热 URL（QQ：触发更完整的 cookie 写入）
    warmup_url: Option<&'static str>,
    provider: &'static str,
    priority: &'static [&'static str],
    domain_check: fn(&str) -> bool,
    /// 登录完成判据（cookie 已含播放凭据）
    done: fn(&str) -> bool,
    /// 已有登录态但缺播放凭据（触发 warmup）
    has_login: fn(&str) -> bool,
    poll_interval: Duration,
    /// 轮询超时后的宽松放行判据（QQ：抓到部分登录态也收）
    timeout_accept: Option<fn(&str) -> bool>,
    inner_size: (f64, f64),
    min_size: (f64, f64),
}

static NETEASE_FLOW: LoginFlow = LoginFlow {
    label: "netease-login",
    title: "网易云音乐登录",
    login_url: "https://music.163.com/#/login",
    warmup_url: None,
    provider: "netease",
    priority: NETEASE_COOKIE_PRIORITY,
    domain_check: is_netease_domain,
    done: services::music::music_api::cookie::netease_cookie_has_login,
    has_login: services::music::music_api::cookie::netease_cookie_has_login,
    poll_interval: Duration::from_secs(2),
    timeout_accept: None,
    inner_size: (940.0, 760.0),
    min_size: (780.0, 580.0),
};

static QQ_FLOW: LoginFlow = LoginFlow {
    label: "qq-login",
    title: "QQ 音乐登录",
    login_url: "https://y.qq.com/n/ryqq/profile",
    warmup_url: Some("https://y.qq.com/n/ryqq/player"),
    provider: "qq",
    priority: QQ_COOKIE_PRIORITY,
    domain_check: is_qq_domain,
    done: services::music::music_api::cookie::qq_cookie_has_playback,
    has_login: services::music::music_api::cookie::qq_cookie_has_login,
    poll_interval: Duration::from_millis(1200),
    timeout_accept: Some(services::music::music_api::cookie::qq_cookie_has_login),
    inner_size: (900.0, 720.0),
    min_size: (760.0, 560.0),
};

/// 打开平台官方登录窗口（页面默认即扫码登录）
///
/// 同平台重复点击：清空浏览数据并回登录页（NexBox 语义）。
///
/// 🔴 契约适配记录（2026-10-03，修复登录窗口调用失败）：
/// 本命令初版签名为 `(app, envelope: CmdEnvelope, platform: String)`，但前端
/// `MusicManager/store/music-store.ts` 是 **NexBox 原件复刻**，其调用形状为
/// `invoke("music_open_login_window", { provider })`——直连 `@tauri-apps/api/core`、
/// 不经 `@/lib/tauri/invoke`、不带 envelope。两处不一致导致 Tauri v2 参数校验失败
/// （缺必填 `envelope` + 无 `platform` 键），登录流程在运行时直接报错。
/// 核对 `Source/NexBox/src-tauri/src/music_api/mod.rs:938`，原件签名本就是
/// `(app, provider: String)`，**原件是自洽的**，契约错配源自 Aegis 自建时单方面
/// 改签名未同步复刻前端。
/// 故按「铁律冲突优先保持复用」修正为：
///   1. 参数名回到原件口径 `provider`（不动复刻前端）；
///   2. `envelope` 放宽为 `Option`——经 `@/lib/tauri/invoke` 调用时仍能取到
///      `correlationId` 串日志，复刻前端直连时降级为 `unknown`，不再报错。
#[tauri::command]
pub async fn music_open_login_window(
    app: AppHandle,
    provider: String,
    envelope: Option<CmdEnvelope>,
) -> Result<String, String> {
    let correlation_id = envelope
        .map(|e| e.correlation_id)
        .unwrap_or_else(|| "unknown".to_string());
    let flow = match provider.as_str() {
        "netease" => &NETEASE_FLOW,
        "qq" => &QQ_FLOW,
        _ => {
            tracing::warn!(
                correlation_id = correlation_id,
                command = "music_open_login_window",
                platform = %provider,
                action = "open_validate",
                result = "failure",
                duration_ms = 0,
                error_code = "UNSUPPORTED_PLATFORM",
                user_id = "",
                "不支持的音乐平台"
            );
            return Err(format!("不支持的音乐平台: {provider}"));
        }
    };

    tracing::debug!(
        correlation_id = correlation_id,
        command = "music_open_login_window",
        platform = flow.provider,
        action = "open_start",
        result = "pending",
        duration_ms = 0,
        error_code = "",
        user_id = "",
        "打开登录窗口开始"
    );

    open_platform_login_window(&app, flow, &correlation_id).await
}

async fn open_platform_login_window(
    app: &AppHandle,
    flow: &'static LoginFlow,
    correlation_id: &str,
) -> Result<String, String> {
    // 已存在同 label 窗口：清浏览数据 + 回登录页 + 聚焦（重新扫码）
    if let Some(existing) = app.get_webview_window(flow.label) {
        let _ = existing.clear_all_browsing_data();
        if let Ok(login_url) = flow.login_url.parse::<Url>() {
            let _ = existing.navigate(login_url);
        }
        let _ = existing.set_focus();
        tracing::info!(
            correlation_id = correlation_id,
            command = "music_open_login_window",
            platform = flow.provider,
            action = "window_refreshed",
            result = "success",
            duration_ms = 0,
            error_code = "",
            user_id = "",
            "登录窗口已存在，清浏览数据后回到登录页"
        );
        return Ok("window_refreshed".into());
    }

    let blank = "about:blank".parse::<Url>().map_err(|e| e.to_string())?;
    let login_window = attach_login_popup_handler(
        WebviewWindowBuilder::new(app, flow.label, WebviewUrl::External(blank)),
        app,
        correlation_id,
    )
    .title(flow.title)
    .additional_browser_args(LOGIN_WINDOW_BROWSER_ARGS)
    .inner_size(flow.inner_size.0, flow.inner_size.1)
    .min_inner_size(flow.min_size.0, flow.min_size.1)
    .build()
    .map_err(|e| format!("创建登录窗口失败: {e}"))?;

    // 清空旧浏览数据：保证每次登录都是干净会话（避免读到历史 cookie 直接判「已登录」）
    let _ = login_window.clear_all_browsing_data();
    let login_url = flow.login_url.parse::<Url>().map_err(|e| e.to_string())?;
    let _ = login_window.navigate(login_url);

    spawn_login_poll(login_window, app.clone(), flow, correlation_id.to_string());

    tracing::info!(
        correlation_id = correlation_id,
        command = "music_open_login_window",
        platform = flow.provider,
        action = "window_created",
        result = "success",
        duration_ms = 0,
        error_code = "",
        user_id = "",
        "登录窗口已创建，开始轮询登录态"
    );
    Ok("window_created".into())
}

/// 为登录窗口安装第三方登录弹窗处理（`window.open` 拦截）
///
/// 各平台登录页的第三方登录按钮走 `window.open`，Tauri 默认 Deny 会静默丢弃，
/// 表现为「点了没反应」。这里真正创建子窗口装载第三方登录页：
/// 与父窗口共用同一 WebView2 用户数据目录，第三方回跳写入的 cookie 能被父窗口轮询读到。
/// 子窗口在回跳到平台域名后自动关闭（见 `login_popup_should_close`）。
fn attach_login_popup_handler<'a, R: tauri::Runtime, M: tauri::Manager<R>>(
    builder: WebviewWindowBuilder<'a, R, M>,
    app: &AppHandle<R>,
    correlation_id: &str,
) -> WebviewWindowBuilder<'a, R, M> {
    use std::sync::atomic::{AtomicU32, Ordering};
    static POPUP_SEQ: AtomicU32 = AtomicU32::new(0);

    let app_handle = app.clone();
    let correlation_id = correlation_id.to_string();
    builder.on_new_window(move |url, _features| {
        let label = format!(
            "music-login-popup-{}",
            POPUP_SEQ.fetch_add(1, Ordering::Relaxed)
        );
        tracing::info!(
            correlation_id = correlation_id,
            command = "music_open_login_window",
            platform = "",
            action = "popup_open",
            result = "pending",
            duration_ms = 0,
            error_code = "",
            user_id = "",
            "window.open 拦截，创建登录子窗口: {url}"
        );

        let builder =
            tauri::WebviewWindowBuilder::new(&app_handle, label.clone(), WebviewUrl::External(url))
                .title("账号登录")
                .additional_browser_args(LOGIN_WINDOW_BROWSER_ARGS)
                .on_page_load(|window, payload| {
                    if payload.event() != tauri::webview::PageLoadEvent::Finished {
                        return;
                    }
                    let loaded = payload.url().clone();
                    // OAuth 回跳到平台域名 = 授权完成，子窗口自己关掉，避免残留孤儿窗口
                    if login_popup_should_close(&loaded) {
                        let _ = window.close();
                    }
                });

        match builder.build() {
            Ok(window) => tauri::webview::NewWindowResponse::Create { window },
            Err(e) => {
                tracing::warn!(
                    correlation_id = correlation_id,
                    command = "music_open_login_window",
                    platform = "",
                    action = "popup_error",
                    result = "failure",
                    duration_ms = 0,
                    error_code = "POPUP_CREATE_FAILED",
                    user_id = "",
                    "创建登录子窗口失败: {e}"
                );
                tauri::webview::NewWindowResponse::Deny
            }
        }
    })
}

/// 关闭登录窗口，并顺带关掉它弹出的所有第三方登录子窗口
///
/// 子窗口 label 前缀 `music-login-popup-`；正常情况下子窗口会在 OAuth 回跳时自己关闭，
/// 但用户中途取消授权、或第三方页面停在未知域名上时会残留，登录成功后统一清一次。
fn close_login_window_with_popups<R: tauri::Runtime>(app: &AppHandle<R>, login_label: &str) {
    for (label, win) in app.webview_windows() {
        if label.starts_with("music-login-popup-") {
            let _ = win.close();
        }
    }
    if let Some(win) = app.get_webview_window(login_label) {
        let _ = win.close();
    }
}

/// 轮询登录窗口 webview cookie，捕获到登录态即保存凭据并 emit 事件
///
/// 3s 初始等待 + 150 次轮询（NexBox 同参）；窗口被用户关闭即停止。
fn spawn_login_poll(
    win: WebviewWindow,
    app: AppHandle,
    flow: &'static LoginFlow,
    correlation_id: String,
) {
    tauri::async_runtime::spawn(async move {
        tokio::time::sleep(Duration::from_secs(3)).await;
        let mut warmup_started = false;

        for _ in 0..150 {
            match win.cookies() {
                Ok(cookies) => {
                    let cookie_str =
                        build_cookie_from_webview(&cookies, flow.priority, flow.domain_check);
                    if (flow.done)(&cookie_str) {
                        finish_login(&app, flow, cookie_str, &correlation_id).await;
                        return;
                    }
                    if let Some(warmup_url) = flow.warmup_url {
                        if !warmup_started && (flow.has_login)(&cookie_str) {
                            warmup_started = true;
                            tracing::info!(
                                correlation_id = correlation_id,
                                command = "music_open_login_window",
                                platform = flow.provider,
                                action = "warmup_navigate",
                                result = "pending",
                                duration_ms = 0,
                                error_code = "",
                                user_id = "",
                                "已有登录态但缺播放凭据，跳转预热页触发 cookie 写入"
                            );
                            if let Ok(w) = warmup_url.parse::<Url>() {
                                let _ = win.navigate(w);
                            }
                        }
                    }
                }
                Err(e) => {
                    tracing::warn!(
                        correlation_id = correlation_id,
                        command = "music_open_login_window",
                        platform = flow.provider,
                        action = "cookie_read",
                        result = "failure",
                        duration_ms = 0,
                        error_code = "COOKIE_READ_FAILED",
                        user_id = "",
                        "读取登录窗口 cookie 失败: {e}"
                    );
                    // 窗口可能已被用户关闭，检测到后停止轮询
                    if !win.is_visible().unwrap_or(false) {
                        break;
                    }
                }
            }
            tokio::time::sleep(flow.poll_interval).await;
        }

        // 超时兜底：宽松判据放行（QQ：抓到部分登录态也收）
        if let Some(accept) = flow.timeout_accept {
            if let Ok(cookies) = win.cookies() {
                let cookie_str =
                    build_cookie_from_webview(&cookies, flow.priority, flow.domain_check);
                if accept(&cookie_str) {
                    finish_login(&app, flow, cookie_str, &correlation_id).await;
                    return;
                }
            }
        }

        tracing::warn!(
            correlation_id = correlation_id,
            command = "music_open_login_window",
            platform = flow.provider,
            action = "poll_timeout",
            result = "not_logged_in",
            duration_ms = 0,
            error_code = "LOGIN_TIMEOUT",
            user_id = "",
            "登录窗口轮询超时（约 5 分钟），未捕获到登录态"
        );
    });
}

/// 捕获登录态后的收口：保存凭据（Credential Manager）→ 关窗 → 校验登录态 → emit 事件
async fn finish_login(
    app: &AppHandle,
    flow: &'static LoginFlow,
    cookie_str: String,
    correlation_id: &str,
) {
    // 凭据保存改走 **NexBox 原路径** `music_api::cookie::save_cookie`（tauri-plugin-store
    // 的 settings.json，键 `cookie_<provider>`）。
    //
    // 🔴 2026-10-03 变更：原实现调 Aegis 自造的 `auth::save_credential`（Windows
    // Credential Manager），该文件随「全删 Aegis 自造层」整体删除。改接 NexBox 原路径后：
    //   - 存储位置从 Credential Manager 变为 settings.json（与 NexBox 一致，复刻保真）；
    //   - 键名沿用 `cookie_<provider>`，NexBox 侧 `load_cookie` / `clear_cookie` 可直接读；
    //   - `LoginFlow.platform` 字段随之删除（`provider` 字符串即 `save_cookie` 所需键）。
    if let Err(e) = music_cookie::save_cookie(app, flow.provider, &cookie_str) {
        tracing::error!(
            correlation_id = correlation_id,
            command = "music_open_login_window",
            platform = flow.provider,
            action = "credential_save",
            result = "failure",
            duration_ms = 0,
            error_code = "CREDENTIAL_SAVE_FAILED",
            user_id = "",
            "凭据保存失败: {e}"
        );
        close_login_window_with_popups(app, flow.label);
        let _ = app.emit("music-login-failed", format!("凭据保存失败: {e}"));
        return;
    }
    close_login_window_with_popups(app, flow.label);

    let info = match flow.provider {
        "netease" => services::music::music_api::netease::login_status(&cookie_str).await,
        _ => services::music::music_api::qqmusic::login_info(&cookie_str).await,
    };
    match info {
        Ok(info) if info.logged_in => {
            tracing::info!(
                correlation_id = correlation_id,
                command = "music_open_login_window",
                platform = flow.provider,
                action = "login_complete",
                result = "success",
                duration_ms = 0,
                error_code = "",
                user_id = &info.user_id,
                "登录成功: user_id={}, nickname={}",
                info.user_id,
                info.nickname
            );
            let _ = app.emit("music-login-success", &info);
        }
        Ok(_) => {
            tracing::warn!(
                correlation_id = correlation_id,
                command = "music_open_login_window",
                platform = flow.provider,
                action = "login_validate",
                result = "failure",
                duration_ms = 0,
                error_code = "INVALID_SESSION",
                user_id = "",
                "cookie 已保存但登录态校验未通过"
            );
            let _ = app.emit("music-login-failed", "Cookie 无效或已过期".to_string());
        }
        Err(e) => {
            tracing::warn!(
                correlation_id = correlation_id,
                command = "music_open_login_window",
                platform = flow.provider,
                action = "login_validate",
                result = "failure",
                duration_ms = 0,
                error_code = "API_ERROR",
                user_id = "",
                "登录态校验失败: {e}"
            );
            let _ = app.emit("music-login-failed", e);
        }
    }
}
