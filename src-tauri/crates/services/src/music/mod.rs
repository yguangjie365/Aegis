//! music 域 — 音乐管理后端（NexBox 音乐模块换皮版 · 阶段一 100% 复刻基线）。
//!
//! 🔴 复刻基线（`Docs/功能规格/07-音乐管理.md` §〇，2026-10-03 用户裁决）：本模块是
//! **NexBox 音乐模块的换皮版本**。阶段一按 100% 复刻执行——目录组织、命令定义、
//! 凭据管理、平台 API 全部自 `Source/NexBox/src-tauri/src/` 原样搬入（GPL-3.0 同源
//! 衍生），**不做等价改写**；阶段二再按 UI 设计规范换皮。
//!
//! B3 裁决（用户 2026-10-03）：**只调整音乐域内部目录组织，不改全项目 Cargo workspace
//! 形态**（全项目单 crate 化会波及另外 7 个模块，不在复刻范围）。本目录已与 NexBox 同构：
//!
//! ```text
//! Aegis services/src/          NexBox src/
//!   music/                       music_api/          ← 平台 API 层
//!     mod.rs        ← 本文件       mod.rs   (83 个 Tauri 命令 + 凭据管理)
//!     music_api/                 kugou.rs qqmusic.rs qishui.rs migu.rs netease.rs
//!       mod.rs                    audio_proxy.rs cookie.rs crypto.rs models.rs
//!       kugou.rs
//!       qqmusic.rs               music.rs            ← 本地曲库 / 系统集成
//!       qishui.rs
//!       migu.rs                  audio_eq.rs         ← 10 段 EQ + 虚拟声卡
//!       netease.rs
//!       audio_proxy.rs           audio_engine.rs     ← WASAPI 播放内核 + 频谱分析
//!       cookie.rs
//!       crypto.rs                netease_lyrics.rs   ← 网易云歌词特殊管线
//!       models.rs
//!     music.rs                   smtc.rs             ← 系统媒体会话
//!     audio_eq.rs
//!     audio_engine.rs            dattorro.rs / spectrum.rs / media_keys.rs /
//!     netease_lyrics.rs          external_player.rs  ← 同在 crate 根，本仓已对齐
//!     smtc.rs
//! ```
//!
//! ## 🔴 Aegis 自造层已整体删除（2026-10-03，用户裁决）
//!
//! 阶段一收口时曾保留 7 个 Aegis 自造文件（`auth` / `login_window` / `meta` / `library` /
//! `scan` / `commands` / `proxy` / `legacy_bridge`，共 2638 行），用于承载 MU-1 本地曲库。
//! NexBox 阶段一 100% 复刻完成后实测确认：
//! - 其定义的 13 个 Tauri 命令**全部零注册或零前端调用**（前端走 `music-store.ts` 直调
//!   NexBox 原名命令，如 `cmd_get_proxy_port`）；
//! - `src/lib/tauri/music.ts`（封装这批命令的前端门面）**整个文件零 import**；
//! - 该簇自闭合（仅 `legacy_bridge` 单向读 `auth` / `music_api`），NexBox 层**不反向依赖**它
//!   → 删除无连带风险。
//!
//! 故按用户裁决「全删 Aegis 自造层」整体移除，本目录现与 NexBox `src/` 同构。
//! 保留 `login_window.rs`：它是 NexBox `music_open_login_window` 的 Aegis 宿主类型例外
//! （WebviewWindow 属宿主类型无法下沉 services），**不是自造层**。

pub mod audio_engine;
pub mod audio_eq;
pub mod login_window;
pub mod music;
pub mod music_api;
pub mod netease_lyrics;
pub mod smtc;


// ── 命令再导出（保持 lib.rs 中 services::music::<命令名> 的注册路径不变）──
// NexBox 的 83 个 Tauri 命令定义在 music_api/mod.rs 内；此处按其原名逐个再导出，
// 使 crate::music::<命令名> 与 NexBox 的 music_api::<命令名> 等价可用。
// 这是**纯路径适配**：函数体、参数、返回值一律未改。
// 凭据与 profile 缓存辅助（NexBox 定义在 music_api/mod.rs，供同层平台文件以
// crate::music::xxx 路径调用；Aegis 保持同一调用形态，不改 qishui.rs 等文件内的写法）
pub use music_api::{
    music_search,
    music_song_url,
    music_login_qr_key,
    music_login_qr_create,
    music_login_qr_check,
    music_login_status,
    music_login_cookie,
    music_logout,
    music_user_playlist,
    music_playlist_tracks,
    music_playlist_tracks_range,
    music_playlist_info_with_track_ids,
    music_playlist_detail,
    music_likelist,
    music_like,
    music_playlist_subscribe,
    music_lyric,
    music_song_comments,
    music_send_comment,
    music_personalized,
    music_recommend_songs,
    music_recommend_resource,
    music_simi_song,
    music_artist_search,
    music_playlist_search,
    music_artist_songs,
    music_artist_detail,
    music_artist_albums,
    music_artist_mvs,
    music_album_detail,
    music_mv_url,
    kugou_search,
    kugou_artist_search,
    kugou_playlist_search,
    kugou_artist_songs,
    kugou_song_url,
    kugou_lyric,
    kugou_login_status,
    kugou_login_cookie,
    kugou_logout,
    kugou_user_playlists,
    kugou_playlist_tracks,
    kugou_playlist_tracks_range,
    kugou_guess_like,
    kugou_rank_list,
    kugou_rank_songs,
    kugou_like_toggle,
    kugou_liked_hashes,
    qq_search,
    qq_song_url,
    qq_lyric,
    qq_login_status,
    qq_login_cookie,
    qq_logout,
    qq_user_playlists,
    qq_playlist_tracks,
    qq_playlist_tracks_range,
    qq_artist_search,
    qq_artist_songs,
    qq_playlist_search,
    qq_rank_list,
    qq_rank_songs,
    music_qq_recommend_playlists,
    qq_liked_hashes,
    qq_like_toggle,
    migu_search,
    migu_playlist_search,
    migu_artist_search,
    migu_artist_songs,
    migu_song_url,
    migu_lyric,
    migu_login_status,
    migu_logout,
    migu_user_playlists,
    migu_playlist_tracks,
    migu_playlist_tracks_range,
    migu_rank_list,
    migu_rank_songs,
    migu_recommend_playlists,
    music_get_login_statuses,
    music_switch_provider,
    music_get_playback_source,
    music_open_login_window,
};

// ── 外部播放器接管命令再导出 ──
// NexBox 的 external_player 为 src/ 顶层私有 mod（其 lib.rs:18 声明 mod external_player;），
// 本仓同样保持私有；因 Aegis 为 workspace（根 crate 需跨 crate 引用），故在此再导出命令，
// 使 services::music::external_control 与 NexBox 的 external_player::external_control 等价。
pub use crate::external_player::external_control;
