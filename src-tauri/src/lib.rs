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

// 🔴 2026-10-03 清理：原 `mod music_login;` 及其实现文件 `src/music_login.rs`（470 行）
// **整体删除**。该文件是 Aegis 在复刻期自建的宿主层登录窗（自造 `LoginFlow` 两平台流 +
// `spawn_login_poll` + 八字段 `tracing` 日志），功能被 services 版取代，改动后全文件
// dead_code（7 项警告）。🔴 同名命令不可并存：Tauri 解析不确定（见下方注册表注释）。
// 详见下方「音乐登录窗口」注册段的口径说明。

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    install_panic_hook();

    let app = tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            focus_main(app);
        }))
        .plugin(tauri_plugin_dialog::init())
        // 阶段一 100% 复刻（2026-10-03）：NexBox lib.rs:312 同款注册，供音乐域凭据/设置持久化
        .plugin(tauri_plugin_store::Builder::default().build())
        .invoke_handler(tauri::generate_handler![
            // 🔴 注册点：此处只允许 `services::<域>::<command>` 薄转发，禁在本 crate 写 command 实体
            services::sys::sys_runtime_ping,
            // 🔴 Aegis 自造层的 6 个命令注册已删除（2026-10-03 用户裁决「全删自造层」）：
            //    music_scan_import_root / music_scan_start / music_scan_cancel /
            //    music_library_get / music_lyric_read / music_get_proxy_port。
            //    实测依据：① 13 个自造命令全部零前端调用（前端走 NexBox 原名）；
            //    ② 其前端门面 src/lib/tauri/music.ts 整个文件零 import。
            //    保留下方 music_api::* 全部 NexBox 原命令 + cmd_get_proxy_port（前端在用）。

            // ── B4 阶段一复刻（2026-10-03）：music-store.ts（NexBox 原样搬入）调用的命令 ──
            // 按 NexBox src-tauri/src/lib.rs 的注册口径分组：music_api::（平台 API）/ music::（本地曲库）
            // / smtc::（系统媒体会话）/ external_player::（外部播放器接管）。
            // 命令实现均为 B1/B2 已搬入的 NexBox 原件，此处仅补注册。
            services::music::music_api::audio_proxy::cmd_get_proxy_port,
            services::music::music_api::kugou_liked_hashes,
            services::music::music_api::kugou_lyric,
            services::music::music_api::kugou_rank_list,
            services::music::music_api::kugou_song_url,
            services::music::music_api::migu_artist_search,
            services::music::music_api::migu_artist_songs,
            services::music::music_api::migu_lyric,
            services::music::music_api::migu_rank_list,
            services::music::music_api::migu_rank_songs,
            services::music::music_api::migu_recommend_playlists,
            services::music::music_api::migu_song_url,
            services::music::music_api::music_get_playback_source,
            services::music::music_api::music_like,
            services::music::music_api::music_likelist,
            services::music::music_api::music_lyric,
            services::music::music_api::music_playlist_detail,
            services::music::music_api::music_switch_provider,
            services::music::music::get_music_files,
            // ── 终检补注册（2026-10-03）：对齐 NexBox lib.rs 音乐侧全量口径 ──
            // 上列命令前端当前零引用（NexBox 侧亦仅供壳层/其它页面使用），
            // 但按「100% 复刻 = 能力不缺」口径一并注册，避免阶段二接入时再缺。
            //
            // 🔴 例外说明：`music_api::music_open_login_window`（mod.rs:1128）是**唯一注册版本**，
            //     另有历史上一份宿主层同名实现（原 `src/music_login.rs`）已整体删除（2026-10-03）：
            //     Tauri 对同名 command 的解析不确定 → 可能命中只开窗、不轮询 cookie、不写凭据的旧版，
            //     表现为「点了登录、窗口开了、但登录态永远不落库」的功能性失效。
            //     更长的覆盖不全原因（宿主版只有 netease/qq 两流、事件名非各平台专属
            //     `kugou-login-success` 等、provider 键 `qq` ≠ 前端 `qqmusic`）记在该文件 git 历史中。
            services::external_player::external_player_state,
            services::music::music_api::kugou_guess_like,
            services::music::music_api::kugou_login_cookie,
            services::music::music_api::kugou_rank_songs,
            services::music::music_api::music_login_qr_check,
            services::music::music_api::music_login_qr_create,
            services::music::music_api::music_login_qr_key,
            services::music::music_api::music_personalized,
            services::music::music_api::music_playlist_info_with_track_ids,
            services::music::music_api::music_qq_recommend_playlists,
            services::music::music_api::qq_login_cookie,
            services::media_keys::set_media_keys_enabled,
            // ── EQ 调音命令（2026-10-03 补注册）──
            // audio_eq.rs 随 NexBox 原件搬入（20 个 tauri 命令），但 lib.rs 未注册；
            // 对照 NexBox src-tauri/src/lib.rs:938-957 的注册段逐条补齐（含虚拟声卡
            // install/uninstall_virtual_audio_driver —— 涉及系统驱动变更，
            // AGENTS.md「特权操作一律经提权通道」对本模块暂不豁免，实机验证前不得执行）。
            services::music::audio_eq::check_virtual_audio_driver,
            services::music::audio_eq::install_virtual_audio_driver,
            services::music::audio_eq::uninstall_virtual_audio_driver,
            services::music::audio_eq::start_eq_engine,
            services::music::audio_eq::stop_eq_engine,
            services::music::audio_eq::get_eq_engine_status,
            services::music::audio_eq::get_eq_presets,
            services::music::audio_eq::apply_eq_preset,
            services::music::audio_eq::import_eq_preset,
            services::music::audio_eq::delete_eq_preset,
            services::music::audio_eq::save_eq_preset,
            services::music::audio_eq::export_fac_file,
            services::music::audio_eq::get_audio_levels,
            services::music::audio_eq::get_spectrum,
            services::music::audio_eq::get_default_audio_device,
            services::music::audio_eq::list_audio_devices,
            services::music::audio_eq::update_eq_bands,
            services::music::audio_eq::update_eq_preamp,
            services::music::audio_eq::update_eq_effects,
            services::music::audio_eq::get_eq_effects,
            // ── B5 补注册（2026-10-03）：按平台分派的 34 个命令 ──
            // 起因：B4 补注册时按 `invoke("xxx")` 直接字面量扫描，漏了 loadUserPlaylistsFor /
            // loginStatusFor 等函数内的**三元分派**串（如 `kugou ? "kugou_user_playlists" : ...`），
            // 导致这些命令后端已实现但未注册 → 运行时 command not found →
            // 界面表现：登录态显示正常（走 music_login_status 等已注册命令），
            // 但歌单/歌手/搜索/分页区空白（走 qq_user_playlists 等未注册命令）。
            // 全部命令实现均在 B1 搬入的 NexBox `music_api/mod.rs`（汽水 3 个在 `qishui.rs`）内，此处仅补注册。
            services::music::music_api::kugou_artist_search,
            services::music::music_api::kugou_artist_songs,
            services::music::music_api::kugou_like_toggle,
            services::music::music_api::kugou_login_status,
            services::music::music_api::kugou_logout,
            services::music::music_api::kugou_playlist_search,
            services::music::music_api::kugou_playlist_tracks,
            services::music::music_api::kugou_playlist_tracks_range,
            services::music::music_api::kugou_search,
            services::music::music_api::kugou_user_playlists,
            services::music::music_api::migu_login_status,
            services::music::music_api::migu_logout,
            services::music::music_api::migu_playlist_search,
            services::music::music_api::migu_playlist_tracks,
            services::music::music_api::migu_playlist_tracks_range,
            services::music::music_api::migu_search,
            services::music::music_api::migu_user_playlists,
            services::music::music_api::music_get_login_statuses,
            services::music::music_api::qishui::qishui_logout,
            services::music::music_api::qishui::qishui_playlist_tracks,
            services::music::music_api::qishui::qishui_feed_modes,
            services::music::music_api::qishui::qishui_user_playlists,
            services::music::music_api::qq_artist_search,
            services::music::music_api::qq_artist_songs,
            services::music::music_api::qq_like_toggle,
            services::music::music_api::qq_login_status,
            services::music::music_api::qq_logout,
            services::music::music_api::qq_playlist_search,
            services::music::music_api::qq_playlist_tracks,
            services::music::music_api::qq_playlist_tracks_range,
            services::music::music_api::qq_search,
            services::music::music_api::qq_user_playlists,
            services::music::music_api::qishui::qishui_deps_download,
            services::music::music_api::qishui::qishui_deps_status,
            services::music::music_api::qishui::qishui_feed,
            services::music::music_api::qishui::qishui_lyric,
            services::music::music_api::qishui::qishui_qr_check,
            services::music::music_api::qishui::qishui_qr_get,
            services::music::music_api::qishui::qishui_search,
            services::music::music_api::qishui::qishui_search_artists,
            services::music::music_api::qishui::qishui_search_playlists,
            services::music::music_api::qishui::qishui_song_url,
            services::music::music_api::qishui::qishui_status,
            services::music::music_api::qq_liked_hashes,
            services::music::music_api::qq_lyric,
            services::music::music_api::qq_rank_list,
            services::music::music_api::qq_rank_songs,
            services::music::music_api::qq_song_url,
            services::music::music::delete_cover_cache_files,
            services::music::music::get_local_lyric,
            services::music::music::import_local_music,
            services::music::music::import_local_music_folder,
            services::music::music::verify_local_covers,
            services::music::smtc::smtc_clear,
            services::music::smtc::smtc_update_state,
            services::external_player::external_control,
            services::music::music_api::music_search,
            services::music::music_api::music_song_url,
            services::music::music_api::music_user_playlist,
            services::music::music_api::music_playlist_tracks,
            services::music::music_api::music_recommend_songs,
            services::music::music_api::music_recommend_resource,
            services::music::music_api::music_playlist_subscribe,
            services::music::music_api::music_playlist_search,
            services::music::music_api::music_artist_search,
            services::music::music_api::music_artist_songs,
            services::music::music_api::music_artist_albums,
            services::music::music_api::music_artist_mvs,
            services::music::music_api::music_simi_song,
            services::music::music_api::music_playlist_tracks_range,
            services::music::music_api::music_login_status,
            services::music::music_api::music_login_cookie,
            services::music::music_api::music_logout,
            services::music::music_api::music_song_comments,
            services::music::music_api::music_send_comment,
            services::music::music_api::music_artist_detail,
            services::music::music_api::music_album_detail,
            services::music::music_api::music_mv_url,
            // M-01 片1：本机概览采集面（只读命令；传感器通道生命周期在 services 内部）
            services::overview::commands::overview_live_sample,
            services::overview::commands::overview_top_processes,
            services::overview::commands::overview_net_endpoints,
            services::overview::commands::overview_monitor_status,
            services::overview::commands::overview_static_info,
            services::overview::commands::overview_snapshot_read,
            services::overview::commands::overview_snapshot_write,
            // F-07 已安装软件清单（慢变量，不进 2 秒节拍）
            services::overview::commands::overview_software_list,
            // F-11 导出硬件信息报告（传感器全量表上行 + 前端拼好的文本落盘）
            services::overview::commands::overview_sensor_table,
            services::overview::commands::overview_report_write,
            // ── B-G1 Steam 主线（2026-10-03）：NexBox steam.rs（2188 行）原样搬入 ──
            // 注册顺序与 NexBox src-tauri/src/lib.rs:1209-1227 逐行一致；19 个命令全注册
            // 口径原为「能力不缺」，但 uninstall_steam_game、install_steam_game 均经用户裁决
            // 删除（安装/卸载都归 Steam 客户端），实为 17 个。
            services::steam::steam::get_steam_install_info,
            services::steam::steam::get_steam_users,
            services::steam::steam::get_steam_libraries,
            services::steam::steam::get_steam_games,
            services::steam::steam::get_steam_all_data,
            services::steam::steam::launch_steam_client,
            services::steam::steam::launch_steam_game,
            services::steam::steam::open_steam_store_page,
            services::steam::steam::open_game_folder,
            services::steam::steam::switch_steam_account,
            services::steam::steam::delete_steam_account,
            // 🔴 uninstall_steam_game 已按用户裁决（2026-10-03）删除 —— 卸载由 Steam 客户端管理
            // 🔴 install_steam_game  已按用户裁决（2026-10-03）删除 —— 安装同样交由 Steam 客户端；
            //    至此本模块对游戏只提供「展示 + 启动」，安装/卸载均不提供命令（实为 17 个）。
            services::steam::steam::format_file_size,
            services::steam::steam::get_steam_stats,
            services::steam::steam::get_library_disk_info,
            services::steam::steam::steam_debug,
            services::steam::steam::get_steam_user_avatars,
            services::steam::steam::get_steam_inventory,
            // ── 游戏封面候选链（2026-10-04 修「获取不到封面」）──
            // 前端按返回顺序逐个 <img> 尝试、onError 前进；头一条取 appinfo
            // `common.header_image`（新式资源带 hash 子目录，最准）。
            services::steam::steam::steam_cover_urls,
            // ── Steam 在线库存（Aegis 新增 · 2026-10-03 用户裁决）──
            // NexBox 的 get_steam_inventory 纯本地、无 API Key 路径（source 只有
            // cache/none），故按 SystemToolkit 的 GetOwnedGames 补全在线能力。
            // API Key 的**录入 UI 放设置页**（用户明确要求不放游戏管理页）。
            // 同样必须写全路径（inventory_online 不是 steam::steam）。
            services::steam::inventory_online::steam_api_key_status,
            services::steam::inventory_online::steam_save_api_key,
            services::steam::inventory_online::get_steam_inventory_online,
            services::steam::inventory_online::steam_installed_appids,
            // ── 音乐登录窗口 ──
            // 🔴 2026-10-03 修正（回归修复）：此处原注册宿主层 `music_login::music_open_login_window`，
            //    但它**只覆盖 netease / qq 两个平台**，而前端 `MusicLoginSection.tsx:51-83` 有
            //    **5 个平台**（netease / kugou / qqmusic / migu / qishui），且事件名
            //    `kugou-login-success` / `qqmusic-login-success` / `migu-login-success`
            //    与 `music-store.ts` 的 listen 一一对应 —— 宿主层 emit 不出这些事件，
            //    表现为**除网易云与汽水外三个平台点登录无反应**。
            //    另：宿主层的 provider 键是 `qq`，前端传的是 `qqmusic`，连 QQ 音乐都不匹配。
            // 故改回注册 services 版（NexBox 原样搬入）：它 4 个平台开窗 + 轮询 cookie +
            // `save_cookie` + emit 各平台事件，功能完整且与前端契约天然吻合（签名就是
            // `(app, provider: String)`，不需要 CmdEnvelope）。
            // ⚠️ 该文件**不再保留**宿主层同名的第二份实现：同名命令会致 Tauri 解析不确定，
            //    且它是 Aegis 自造层（非 NexBox 原样），留着违反「复刻域不得自造」口径。
            services::music::music_api::music_open_login_window,
            // ── M-05 文件互传（2026-10-04，片1 电脑通道）：命令实体在
            //    `crates/services/src/transfer/commands.rs`，此处只做薄转发 ──
            // 口径：入参统一 `contracts::CmdEnvelope`；每个命令无论成败都落一条日志
            // （字段按 01 专题 §2.1 八字段命名；JSONL 落盘由 W0-10 的 sink 负责）。
            services::transfer::commands::transfer_service_status,
            services::transfer::commands::transfer_service_start,
            services::transfer::commands::transfer_service_stop,
            services::transfer::commands::transfer_settings_load,
            services::transfer::commands::transfer_settings_save,
            services::transfer::commands::transfer_peers_list,
            services::transfer::commands::transfer_send_file,
            services::transfer::commands::transfer_pending_confirms,
            services::transfer::commands::transfer_confirm_receive,
            services::transfer::commands::transfer_tasks_list
        ])
        .setup(|app| {
            // 🔴 MU-1 曲库状态托管段已删除（2026-10-03 用户裁决「全删 Aegis 自造层」）：
            //     原先在此 new MusicState + 从 app_data_dir 加载曲库缓存 + 对各曲库根
            //     重授 asset 协议（services::music::{library, commands}），随自造层整体移除。
            //     现音乐域为纯 NexBox 复刻形态：曲库读取走 music_api 的本地扫描命令，
            //     登录凭据走 `services::music::music_api::cookie::save_cookie`
            //     （settings.json，键 `cookie_<provider>`；2026-10-03 起即此路径，
            //      原宿主层 `src/music_login.rs` 自造实现已整体删除）。
            //     如后续要恢复 Aegis 自建本地曲库（目录树 + 元数据标签 + 封面代理），
            //     应作为**新功能**重新设计，不再挂回 MU-1 的过渡形态。
            // M-01 片1：概览域状态托管（传感器桥 + 缓存 + sysinfo 实例；随应用生命周期释放，
            // 桥的 `Drop` 负责收掉 sidecar 子进程，🔴 本壳不含任何域逻辑）
            {
                use services::overview::state::OverviewState;
                app.manage(OverviewState::new());
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
