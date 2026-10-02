//! src-tauri 主 crate —— 薄壳（04 分册 §一 准入规则）。
//!
//! 本 crate 只做：builder 组装 / 插件注册 / `generate_handler!` 命令挂载 / 托盘 / 单实例 / panic 兜底。
//! 🔴 业务逻辑一律在 `crates/services`；此处出现域逻辑即越界。
//!
//! 窗口口径（01 总览 §5.3 / UI 规范 §四）：1280×800 居中、min 1240×640、禁最大化/全屏、
//! `decorations:false`（32px 自绘标题条）、`dragDropEnabled:false`（HTML5 拖放接管，W0-02 窗口配置核对项）、
//! `visible:false` —— 首帧就绪后由前端 Shell 调 `show()` 显形，避免白屏闪烁。

use std::io::Write as _;

use tauri::{
    menu::{Menu, MenuItem},
    tray::TrayIconBuilder,
    AppHandle, Manager, RunEvent, WebviewWindow,
};

mod music_login;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    install_panic_hook();

    let app = tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            focus_main(app);
        }))
        .plugin(tauri_plugin_dialog::init())
        .invoke_handler(tauri::generate_handler![
            // 🔴 注册点：此处只允许 `services::<域>::<command>` 薄转发，禁在本 crate 写 command 实体
            services::sys::sys_runtime_ping,
            services::music::commands::music_scan_import_root,
            services::music::commands::music_scan_start,
            services::music::commands::music_scan_cancel,
            services::music::commands::music_library_get,
            services::music::commands::music_lyric_read,
            services::music::proxy::music_get_proxy_port,
            services::music::online::music_search,
            services::music::online::music_song_url,
            services::music::online::music_user_playlist,
            services::music::online::music_playlist_tracks,
            services::music::online::music_recommend_songs,
            services::music::online::music_recommend_resource,
            services::music::online::music_rank_list,
            services::music::online::music_rank_songs,
            services::music::online::music_login_status,
            services::music::online::music_login_cookie,
            services::music::online::music_logout,
            services::music::online::music_lyric_online,
            services::music::online::music_like_song,
            services::music::online::music_liked_songs,
            services::music::online::music_song_comments,
            services::music::online::music_send_comment,
            services::music::online::music_artist_detail,
            services::music::online::music_album_detail,
            services::music::online::music_mv_url,
            // 🔴 窗口命令例外（music_login.rs 头注）：WebviewWindow 属宿主类型无法下沉 services
            music_login::music_open_login_window
        ])
        .setup(|app| {
            // MU-1：music 状态托管 + 曲库缓存加载（R-7 首启静默 / 损坏降级警告）
            {
                use services::music::library::MusicState;
                let state = MusicState::new();
                match app.path().app_data_dir() {
                    Ok(dir) => {
                        let outcome = state.load_from_disk(&dir);
                        if outcome.cache_warning {
                            eprintln!("[music] 曲库缓存损坏，已重置（R-7）");
                        }
                        if outcome.pruned > 0 {
                            eprintln!("[music] 启动自愈剔除 {} 条已消失曲目", outcome.pruned);
                        }
                    }
                    Err(e) => eprintln!("[music] app_data_dir 获取失败，本次会话从空曲库开始: {e}"),
                }
                // MU-2（Q-16）：启动时把已登记曲库根重新授予 asset 协议（授权不跨进程持久）
                let handle = app.handle().clone();
                for root in state.snapshot().roots {
                    services::music::commands::allow_asset_root(&handle, &root);
                }
                app.manage(state);
            }
            build_tray(app)?;
            Ok(())
        })
        .on_menu_event(|app, event| match event.id().as_ref() {
            "toggle" => toggle_main(app),
            "quit" => app.exit(0),
            _ => {}
        })
        .build(tauri::generate_context!())
        .expect(
            "Tauri 应用构建失败（panic hook 已尽力落盘 %LOCALAPPDATA%\\Aegis\\logs\\panic.log）",
        );

    app.run(|_app, event| {
        // 资源残留守卫挂点（W5：手机通道端口/监听器关闭核对）
        if let RunEvent::Exit = event {}
    });
}

fn main_window(app: &AppHandle) -> Option<WebviewWindow> {
    app.get_webview_window("main")
}

/// 二次启动：恢复并聚焦既有主窗（不新建进程）。
fn focus_main(app: &AppHandle) {
    let Some(win) = main_window(app) else { return };
    let _ = win.unminimize();
    let _ = win.show();
    let _ = win.set_focus();
}

/// 托盘"显示/隐藏"：可见则收进托盘，否则恢复聚焦。
fn toggle_main(app: &AppHandle) {
    let Some(win) = main_window(app) else { return };
    if win.is_visible().unwrap_or(false) {
        let _ = win.hide();
    } else {
        let _ = win.unminimize();
        let _ = win.show();
        let _ = win.set_focus();
    }
}

fn build_tray(app: &mut tauri::App) -> Result<(), Box<dyn std::error::Error>> {
    let toggle = MenuItem::with_id(app, "toggle", "显示 / 隐藏 Aegis", true, None::<&str>)?;
    let quit = MenuItem::with_id(app, "quit", "退出 Aegis", true, None::<&str>)?;
    let menu = Menu::with_items(app, &[&toggle, &quit])?;
    let icon = app
        .default_window_icon()
        .ok_or("bundle.icon 缺失：先运行 `npx tauri icon <1024px 源图>` 生成 src-tauri/icons/")?;
    TrayIconBuilder::with_id("aegis-tray")
        .icon(icon.clone())
        .menu(&menu)
        .tooltip("Aegis")
        .build(app)?;
    Ok(())
}

/// 全局异常兜底：panic 先落盘再走默认行为。W0-10 日志总线（八字段 JSONL）就绪后迁过去。
fn install_panic_hook() {
    let previous = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        if let Some(local) = std::env::var_os("LOCALAPPDATA") {
            let dir = std::path::Path::new(&local).join("Aegis").join("logs");
            if std::fs::create_dir_all(&dir).is_ok() {
                let line = format!("panic pid={} {info}\n", std::process::id());
                let file = std::fs::OpenOptions::new()
                    .create(true)
                    .append(true)
                    .open(dir.join("panic.log"));
                if let Ok(mut f) = file {
                    let _ = f.write_all(line.as_bytes());
                }
            }
        }
        previous(info);
    }));
}
