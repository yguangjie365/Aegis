//! 在线音乐命令层（F-MU-25~43；网易云/QQ 双平台）。
//!
//! 🔴 本模块为渐进式移植：优先打通搜索 + 播放地址获取，歌单/推荐/榜单留桩待后续批次。

use super::models::*;
use crate::music::auth::{self, load_credential, MusicPlatform};
use crate::music::commands::payload_as;
use contracts::CmdEnvelope;
use serde::Deserialize;
use tracing::{debug, error, info, warn};

/// 搜索载荷
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchPayload {
    pub keywords: String,
    pub platform: String, // "netease" | "qq"
    pub limit: Option<u32>,
}

/// 搜索结果
#[derive(Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchResult {
    pub correlation_id: String,
    pub songs: Vec<Song>,
}

/// 播放地址载荷
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SongUrlPayload {
    pub song_id: String,
    pub platform: String,
    pub media_mid: Option<String>,
    pub quality: Option<String>, // "standard" | "exhigh" | "lossless" | "hires"
}

/// 播放地址结果
#[derive(Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SongUrlResultWrapper {
    pub correlation_id: String,
    pub url: Option<String>,
    pub quality: Option<String>,
    pub trial: bool,
    pub reason: Option<String>,
}

/// 🔴 provider 词表入站归一单点（Q-20）：命令层收到的平台串一律经此规范为线格式
/// （`"netease"` / `"qq"`；别名 `"qqmusic"` 见 `MusicPlatform::from_slug`）。
/// 无法识别即拒绝并留八字段日志，禁静默兜底成默认平台（P1 根因②防线）。
fn normalized_platform(
    command: &str,
    correlation_id: &str,
    raw: &str,
) -> Result<&'static str, String> {
    MusicPlatform::from_slug(raw)
        .map(MusicPlatform::slug)
        .ok_or_else(|| {
            warn!(
                correlation_id = correlation_id,
                command = command,
                platform = raw,
                action = "platform_validate",
                result = "failure",
                duration_ms = 0,
                error_code = "UNSUPPORTED_PLATFORM",
                user_id = "",
                "不支持的音乐平台: {}",
                raw
            );
            format!("不支持的音乐平台: {}", raw)
        })
}

/// 统一搜索（F-MU-32）
///
/// # 参数
/// * `envelope` - 命令信封（含 correlationId）
/// * `payload` - 搜索参数（关键字/平台/数量）
///
/// # 返回
/// 歌曲列表（最多 limit 首，默认 30）
#[tauri::command]
pub async fn music_search(envelope: CmdEnvelope) -> Result<SearchResult, String> {
    let payload: SearchPayload = payload_as(&envelope)?;
    let correlation_id = &envelope.correlation_id;
    let keywords = payload.keywords.trim();
    let platform = normalized_platform("music_search", correlation_id, &payload.platform)?;
    let limit = payload.limit.unwrap_or(30);

    debug!(
        correlation_id = correlation_id,
        command = "music_search",
        platform = platform,
        action = "search_start",
        result = "pending",
        duration_ms = 0,
        error_code = "",
        user_id = "",
        "在线搜索开始: keywords={}, limit={}",
        keywords,
        limit
    );

    if keywords.is_empty() {
        warn!(
            correlation_id = correlation_id,
            command = "music_search",
            platform = platform,
            action = "search_validate",
            result = "failure",
            duration_ms = 0,
            error_code = "EMPTY_KEYWORDS",
            user_id = "",
            "搜索关键字为空"
        );
        return Err("请输入搜索关键词".to_string());
    }

    // 加载凭据（若已登录）
    let cookie = match platform {
        "netease" => load_credential(MusicPlatform::Netease, correlation_id).unwrap_or_default(),
        "qq" => load_credential(MusicPlatform::QQ, correlation_id).unwrap_or_default(),
        _ => String::new(),
    };

    // 调用实际搜索 API
    let songs = match platform {
        "netease" => super::netease::search(keywords, limit, &cookie)
            .await
            .map_err(|e| {
                warn!(
                    correlation_id = correlation_id,
                    command = "music_search",
                    platform = platform,
                    action = "search_api_error",
                    result = "failure",
                    duration_ms = 0,
                    error_code = "API_ERROR",
                    user_id = "",
                    "网易云搜索失败: {}",
                    e
                );
                format!("搜索失败: {}", e)
            })?,
        "qq" => super::qqmusic::search(keywords, limit, &cookie)
            .await
            .map_err(|e| {
                warn!(
                    correlation_id = correlation_id,
                    command = "music_search",
                    platform = platform,
                    action = "search_api_error",
                    result = "failure",
                    duration_ms = 0,
                    error_code = "API_ERROR",
                    user_id = "",
                    "QQ 音乐搜索失败: {}",
                    e
                );
                format!("搜索失败: {}", e)
            })?,
        _ => {
            return Err(format!("不支持的音乐平台: {}", platform));
        }
    };

    debug!(
        correlation_id = correlation_id,
        command = "music_search",
        platform = platform,
        action = "search_complete",
        result = "success",
        duration_ms = 0,
        error_code = "",
        user_id = "",
        "在线搜索完成: 找到 {} 首歌曲",
        songs.len()
    );

    Ok(SearchResult {
        correlation_id: correlation_id.clone(),
        songs,
    })
}

/// 获取歌曲播放地址（F-MU-39/40）
///
/// # 参数
/// * `envelope` - 命令信封
/// * `payload` - 歌曲 ID/平台/音质档位
///
/// # 返回
/// 播放 URL + 实际音质 + 是否试听
#[tauri::command]
pub async fn music_song_url(envelope: CmdEnvelope) -> Result<SongUrlResultWrapper, String> {
    let payload: SongUrlPayload = payload_as(&envelope)?;
    let correlation_id = &envelope.correlation_id;
    let song_id = &payload.song_id;
    let platform = normalized_platform("music_song_url", correlation_id, &payload.platform)?;
    let quality = payload.quality.as_deref().unwrap_or("hires");

    debug!(
        correlation_id = correlation_id,
        command = "music_song_url",
        platform = platform,
        action = "url_fetch_start",
        result = "pending",
        duration_ms = 0,
        error_code = "",
        user_id = "",
        "获取播放地址开始: song_id={}, quality={}",
        song_id,
        quality
    );

    // 加载凭据（若已登录）
    let cookie = match platform {
        "netease" => load_credential(MusicPlatform::Netease, correlation_id).unwrap_or_default(),
        "qq" => load_credential(MusicPlatform::QQ, correlation_id).unwrap_or_default(),
        _ => String::new(),
    };

    if cookie.is_empty() {
        warn!(
            correlation_id = correlation_id,
            command = "music_song_url",
            platform = platform,
            action = "url_no_cookie",
            result = "warning",
            duration_ms = 0,
            error_code = "NOT_LOGGED_IN",
            user_id = "",
            "未登录，部分曲目可能无法播放"
        );
    }

    // 调用实际播放地址 API
    let result = match platform {
        "netease" => super::netease::song_url(song_id, quality, &cookie)
            .await
            .map_err(|e| {
                warn!(
                    correlation_id = correlation_id,
                    command = "music_song_url",
                    platform = platform,
                    action = "url_api_error",
                    result = "failure",
                    duration_ms = 0,
                    error_code = "API_ERROR",
                    user_id = "",
                    "网易云获取播放地址失败: {}",
                    e
                );
                format!("获取播放地址失败: {}", e)
            })?,
        "qq" => super::qqmusic::song_url(
            song_id,
            payload.media_mid.as_deref().unwrap_or(""),
            quality,
            &cookie,
        )
        .await
        .map_err(|e| {
            warn!(
                correlation_id = correlation_id,
                command = "music_song_url",
                platform = platform,
                action = "url_api_error",
                result = "failure",
                duration_ms = 0,
                error_code = "API_ERROR",
                user_id = "",
                "QQ 音乐获取播放地址失败: {}",
                e
            );
            format!("获取播放地址失败: {}", e)
        })?,
        _ => {
            return Err(format!("不支持的音乐平台: {}", platform));
        }
    };

    debug!(
        correlation_id = correlation_id,
        command = "music_song_url",
        platform = platform,
        action = "url_fetch_complete",
        result = if result.playable {
            "success"
        } else {
            "warning"
        },
        duration_ms = 0,
        error_code = "",
        user_id = "",
        "获取播放地址完成: playable={}, trial={}, quality={}",
        result.playable,
        result.trial,
        result.quality
    );

    Ok(SongUrlResultWrapper {
        correlation_id: correlation_id.clone(),
        url: result.url,
        quality: Some(result.quality),
        trial: result.trial,
        reason: result.message,
    })
}

/// 我的歌单列表（F-MU-34）
#[tauri::command]
pub async fn music_user_playlist(
    envelope: CmdEnvelope,
    platform: String,
) -> Result<Vec<Playlist>, String> {
    let correlation_id = &envelope.correlation_id;
    let platform = normalized_platform("music_user_playlist", correlation_id, &platform)?;

    // 加载凭据
    let cookie = match platform {
        "netease" => load_credential(MusicPlatform::Netease, correlation_id).unwrap_or_default(),
        "qq" => load_credential(MusicPlatform::QQ, correlation_id).unwrap_or_default(),
        _ => return Err(format!("不支持的音乐平台: {}", platform)),
    };

    if cookie.is_empty() {
        warn!(
            correlation_id = correlation_id,
            command = "music_user_playlist",
            platform = %platform,
            action = "no_cookie",
            result = "warning",
            duration_ms = 0,
            error_code = "NOT_LOGGED_IN",
            user_id = "",
            "未登录，无法获取歌单列表"
        );
        return Err("请先登录".to_string());
    }

    match platform {
        "netease" => {
            // 需要先获取用户 UID
            let status = super::netease::login_status(&cookie)
                .await
                .map_err(|e| format!("获取登录状态失败: {}", e))?;
            super::netease::user_playlist(&status.user_id, &cookie)
                .await
                .map_err(|e| format!("获取歌单失败: {}", e))
        }
        "qq" => super::qqmusic::user_playlists(&cookie)
            .await
            .map_err(|e| format!("获取歌单失败: {}", e)),
        _ => Err(format!("不支持的音乐平台: {}", platform)),
    }
}

/// 歌单详情（F-MU-35）
#[tauri::command]
pub async fn music_playlist_tracks(
    envelope: CmdEnvelope,
    playlist_id: String,
    platform: String,
) -> Result<(Playlist, Vec<Song>), String> {
    let correlation_id = &envelope.correlation_id;
    let platform = normalized_platform("music_playlist_tracks", correlation_id, &platform)?;

    // 加载凭据
    let cookie = match platform {
        "netease" => load_credential(MusicPlatform::Netease, correlation_id).unwrap_or_default(),
        "qq" => load_credential(MusicPlatform::QQ, correlation_id).unwrap_or_default(),
        _ => return Err(format!("不支持的音乐平台: {}", platform)),
    };

    match platform {
        "netease" => super::netease::playlist_tracks(&playlist_id, &cookie)
            .await
            .map_err(|e| format!("获取歌单详情失败: {}", e)),
        "qq" => super::qqmusic::playlist_tracks(&playlist_id, &cookie)
            .await
            .map_err(|e| format!("获取歌单详情失败: {}", e)),
        _ => Err(format!("不支持的音乐平台: {}", platform)),
    }
}

/// 每日推荐（F-MU-36，仅网易云）- 占位桩
#[tauri::command]
pub async fn music_recommend_songs(envelope: CmdEnvelope) -> Result<Vec<Song>, String> {
    warn!(
        correlation_id = &envelope.correlation_id,
        command = "music_recommend_songs",
        action = "stub",
        result = "not_implemented",
        duration_ms = 0,
        error_code = "STUB",
        user_id = "",
        "每日推荐功能尚未实现（占位桩）"
    );
    Ok(vec![])
}

/// 推荐歌单（F-MU-36，仅 QQ）
#[tauri::command]
pub async fn music_recommend_resource(envelope: CmdEnvelope) -> Result<Vec<Playlist>, String> {
    // QQ 音乐推荐不需要登录
    super::qqmusic::recommend_playlists()
        .await
        .map_err(|e| format!("获取推荐失败: {}", e))
}

/// 官方榜单列表（F-MU-37，仅 QQ）
#[tauri::command]
pub async fn music_rank_list(
    envelope: CmdEnvelope,
    platform: String,
) -> Result<Vec<Playlist>, String> {
    let correlation_id = &envelope.correlation_id;
    let platform = normalized_platform("music_rank_list", correlation_id, &platform)?;

    // QQ 音乐榜单不需要登录
    if platform == "qq" {
        return super::qqmusic::rank_list()
            .await
            .map_err(|e| format!("获取榜单失败: {}", e));
    }

    warn!(
        correlation_id = correlation_id,
        command = "music_rank_list",
        platform = %platform,
        action = "not_supported",
        result = "warning",
        duration_ms = 0,
        error_code = "NOT_SUPPORTED",
        user_id = "",
        "该平台不支持榜单功能"
    );
    Ok(vec![])
}

/// 榜单曲目（F-MU-37）
#[tauri::command]
pub async fn music_rank_songs(
    envelope: CmdEnvelope,
    rank_id: String,
    platform: String,
    limit: Option<u32>,
) -> Result<Vec<Song>, String> {
    let correlation_id = &envelope.correlation_id;
    let platform = normalized_platform("music_rank_songs", correlation_id, &platform)?;
    let limit = limit.unwrap_or(30);

    // 加载凭据
    let cookie = match platform {
        "netease" => load_credential(MusicPlatform::Netease, correlation_id).unwrap_or_default(),
        "qq" => load_credential(MusicPlatform::QQ, correlation_id).unwrap_or_default(),
        _ => return Err(format!("不支持的音乐平台: {}", platform)),
    };

    if platform == "qq" {
        return super::qqmusic::rank_songs(&rank_id, limit, &cookie)
            .await
            .map_err(|e| format!("获取榜单歌曲失败: {}", e));
    }

    warn!(
        correlation_id = correlation_id,
        command = "music_rank_songs",
        platform = %platform,
        action = "not_supported",
        result = "warning",
        duration_ms = 0,
        error_code = "NOT_SUPPORTED",
        user_id = "",
        "该平台不支持榜单功能"
    );
    Ok(vec![])
}

// ═══════════════════════════════════════════════════════
//  MU-4.10: 登录相关命令
// ═══════════════════════════════════════════════════════

/// 查询指定平台的登录状态
#[tauri::command]
pub async fn music_login_status(
    envelope: CmdEnvelope,
    platform: String,
) -> Result<LoginInfo, String> {
    let correlation_id = &envelope.correlation_id;
    let platform = normalized_platform("music_login_status", correlation_id, &platform)?;

    debug!(
        correlation_id = correlation_id,
        command = "music_login_status",
        platform = %platform,
        action = "check_start",
        result = "pending",
        duration_ms = 0,
        error_code = "",
        user_id = "",
        "检查登录状态开始"
    );

    // 加载凭据
    let cookie = match platform {
        "netease" => load_credential(MusicPlatform::Netease, correlation_id).unwrap_or_default(),
        "qq" => load_credential(MusicPlatform::QQ, correlation_id).unwrap_or_default(),
        _ => return Err(format!("不支持的音乐平台: {}", platform)),
    };

    if cookie.is_empty() {
        warn!(
            correlation_id = correlation_id,
            command = "music_login_status",
            platform = %platform,
            action = "no_cookie",
            result = "not_logged_in",
            duration_ms = 0,
            error_code = "",
            user_id = "",
            "未找到凭据（未登录）"
        );
        return Ok(LoginInfo {
            provider: platform.into(),
            ..Default::default()
        });
    }

    // 调用实际 API 检查登录状态
    let result = match platform {
        "netease" => super::netease::login_status(&cookie).await,
        "qq" => super::qqmusic::login_info(&cookie).await,
        _ => Err(format!("不支持的音乐平台: {}", platform)),
    };

    match result {
        Ok(info) => {
            debug!(
                correlation_id = correlation_id,
                command = "music_login_status",
                platform = %platform,
                action = "check_complete",
                result = if info.logged_in { "success" } else { "not_logged_in" },
                duration_ms = 0,
                error_code = "",
                user_id = &info.user_id,
                "登录状态检查完成: logged_in={}",
                info.logged_in
            );
            Ok(info)
        }
        Err(e) => {
            warn!(
                correlation_id = correlation_id,
                command = "music_login_status",
                platform = %platform,
                action = "check_error",
                result = "failure",
                duration_ms = 0,
                error_code = "API_ERROR",
                user_id = "",
                "登录状态检查失败: {}",
                e
            );
            Err(format!("检查登录状态失败: {}", e))
        }
    }
}

/// 使用 Cookie 登录
#[tauri::command]
pub async fn music_login_cookie(
    envelope: CmdEnvelope,
    platform: String,
    cookie: String,
) -> Result<LoginInfo, String> {
    let correlation_id = &envelope.correlation_id;
    let platform = normalized_platform("music_login_cookie", correlation_id, &platform)?;

    debug!(
        correlation_id = correlation_id,
        command = "music_login_cookie",
        platform = %platform,
        action = "login_start",
        result = "pending",
        duration_ms = 0,
        error_code = "",
        user_id = "",
        "Cookie 登录开始"
    );

    // 验证 Cookie 格式
    let is_valid = match platform {
        "netease" => super::cookie::netease_cookie_has_login(&cookie),
        "qq" => super::cookie::qq_cookie_has_login(&cookie),
        _ => return Err(format!("不支持的音乐平台: {}", platform)),
    };

    if !is_valid {
        warn!(
            correlation_id = correlation_id,
            command = "music_login_cookie",
            platform = %platform,
            action = "validate_failed",
            result = "failure",
            duration_ms = 0,
            error_code = "INVALID_COOKIE",
            user_id = "",
            "Cookie 格式无效"
        );
        return Ok(LoginInfo {
            provider: platform.into(),
            ..Default::default()
        });
    }

    // 保存凭据到 Credential Manager
    let platform_enum = match platform {
        "netease" => MusicPlatform::Netease,
        "qq" => MusicPlatform::QQ,
        _ => return Err(format!("不支持的音乐平台: {}", platform)),
    };

    match auth::save_credential(platform_enum, &cookie, correlation_id) {
        auth::CredentialResult::Success => {
            info!(
                correlation_id = correlation_id,
                command = "music_login_cookie",
                platform = %platform,
                action = "credential_saved",
                result = "success",
                duration_ms = 0,
                error_code = "",
                user_id = "",
                "凭据保存成功"
            );
        }
        auth::CredentialResult::SystemError(e) => {
            error!(
                correlation_id = correlation_id,
                command = "music_login_cookie",
                platform = %platform,
                action = "credential_save_error",
                result = "failure",
                duration_ms = 0,
                error_code = "CREDENTIAL_SAVE_FAILED",
                user_id = "",
                "凭据保存失败: {}",
                e
            );
            return Err(format!("保存凭据失败: {}", e));
        }
        _ => {}
    }

    // 获取登录后的用户信息
    let result = match platform {
        "netease" => super::netease::login_status(&cookie).await,
        "qq" => super::qqmusic::login_info(&cookie).await,
        _ => Err(format!("不支持的音乐平台: {}", platform)),
    };

    match result {
        Ok(info) => {
            info!(
                correlation_id = correlation_id,
                command = "music_login_cookie",
                platform = %platform,
                action = "login_complete",
                result = "success",
                duration_ms = 0,
                error_code = "",
                user_id = &info.user_id,
                "Cookie 登录成功: user_id={}, nickname={}",
                info.user_id,
                info.nickname
            );
            Ok(info)
        }
        Err(e) => {
            warn!(
                correlation_id = correlation_id,
                command = "music_login_cookie",
                platform = %platform,
                action = "login_api_error",
                result = "warning",
                duration_ms = 0,
                error_code = "API_ERROR",
                user_id = "",
                "Cookie 已保存但获取用户信息失败: {}",
                e
            );
            // Cookie 已保存，返回部分信息
            Ok(LoginInfo {
                provider: platform.into(),
                logged_in: true,
                ..Default::default()
            })
        }
    }
}

/// 退出登录
#[tauri::command]
pub async fn music_logout(envelope: CmdEnvelope, platform: String) -> Result<(), String> {
    let correlation_id = &envelope.correlation_id;
    let platform = normalized_platform("music_logout", correlation_id, &platform)?;

    debug!(
        correlation_id = correlation_id,
        command = "music_logout",
        platform = %platform,
        action = "logout_start",
        result = "pending",
        duration_ms = 0,
        error_code = "",
        user_id = "",
        "退出登录开始"
    );

    let platform_enum = match platform {
        "netease" => MusicPlatform::Netease,
        "qq" => MusicPlatform::QQ,
        _ => return Err(format!("不支持的音乐平台: {}", platform)),
    };

    match auth::clear_credential(platform_enum, correlation_id) {
        auth::CredentialResult::Success => {
            info!(
                correlation_id = correlation_id,
                command = "music_logout",
                platform = %platform,
                action = "logout_complete",
                result = "success",
                duration_ms = 0,
                error_code = "",
                user_id = "",
                "退出登录成功"
            );
            Ok(())
        }
        auth::CredentialResult::SystemError(e) => {
            error!(
                correlation_id = correlation_id,
                command = "music_logout",
                platform = %platform,
                action = "logout_error",
                result = "failure",
                duration_ms = 0,
                error_code = "CREDENTIAL_CLEAR_FAILED",
                user_id = "",
                "清除凭据失败: {}",
                e
            );
            Err(format!("退出登录失败: {}", e))
        }
        _ => Ok(()),
    }
}

// ═══════════════════════════════════════════════════════
//  MU-4.11: 在线歌词获取
// ═══════════════════════════════════════════════════════

/// 获取在线歌词
#[tauri::command]
pub async fn music_lyric_online(
    envelope: CmdEnvelope,
    song_id: String,
    platform: String,
) -> Result<Lyrics, String> {
    let correlation_id = &envelope.correlation_id;
    let platform = normalized_platform("music_lyric_online", correlation_id, &platform)?;

    debug!(
        correlation_id = correlation_id,
        command = "music_lyric_online",
        platform = %platform,
        action = "lyric_fetch_start",
        result = "pending",
        duration_ms = 0,
        error_code = "",
        user_id = "",
        "获取歌词开始: song_id={}",
        song_id
    );

    // 加载凭据
    let cookie = match platform {
        "netease" => load_credential(MusicPlatform::Netease, correlation_id).unwrap_or_default(),
        "qq" => load_credential(MusicPlatform::QQ, correlation_id).unwrap_or_default(),
        _ => return Err(format!("不支持的音乐平台: {}", platform)),
    };

    // 调用实际 API 获取歌词
    let result = match platform {
        "netease" => super::netease::lyric(&song_id, &cookie).await,
        "qq" => {
            // QQ 音乐需要 mid 和 id，这里假设 song_id 是 mid
            super::qqmusic::lyric(&song_id, "0", &cookie).await
        }
        _ => Err(format!("不支持的音乐平台: {}", platform)),
    };

    match result {
        Ok(lyrics) => {
            debug!(
                correlation_id = correlation_id,
                command = "music_lyric_online",
                platform = %platform,
                action = "lyric_fetch_complete",
                result = "success",
                duration_ms = 0,
                error_code = "",
                user_id = "",
                "获取歌词完成: has_lyric={}, has_translation={}",
                !lyrics.lyric.is_empty(),
                lyrics.translation.is_some()
            );
            Ok(lyrics)
        }
        Err(e) => {
            warn!(
                correlation_id = correlation_id,
                command = "music_lyric_online",
                platform = %platform,
                action = "lyric_fetch_error",
                result = "failure",
                duration_ms = 0,
                error_code = "API_ERROR",
                user_id = "",
                "获取歌词失败: {}",
                e
            );
            Err(format!("获取歌词失败: {}", e))
        }
    }
}

// ═══════════════════════════════════════════════════════
//  MU-4.12: 社交功能扩展
// ═══════════════════════════════════════════════════════

/// 收藏/取消收藏歌曲
#[tauri::command]
pub async fn music_like_song(
    envelope: CmdEnvelope,
    song_id: String,
    like: bool,
    platform: String,
) -> Result<(), String> {
    let correlation_id = &envelope.correlation_id;
    let platform = normalized_platform("music_like_song", correlation_id, &platform)?;

    debug!(
        correlation_id = correlation_id,
        command = "music_like_song",
        platform = %platform,
        action = "like_start",
        result = "pending",
        duration_ms = 0,
        error_code = "",
        user_id = "",
        "{}歌曲: song_id={}",
        if like { "收藏" } else { "取消收藏" },
        song_id
    );

    // 加载凭据
    let cookie = match platform {
        "netease" => load_credential(MusicPlatform::Netease, correlation_id).unwrap_or_default(),
        "qq" => load_credential(MusicPlatform::QQ, correlation_id).unwrap_or_default(),
        _ => return Err(format!("不支持的音乐平台: {}", platform)),
    };

    if cookie.is_empty() {
        return Err("请先登录".to_string());
    }

    // 调用实际 API
    let result = match platform {
        "netease" => super::netease::like_song(&song_id, like, &cookie).await,
        "qq" => {
            // QQ 音乐需要完整的 Song 对象，这里构造一个最小对象
            let song = Song {
                provider: MusicPlatform::QQ.slug().into(),
                id: song_id.clone(),
                mid: Some(song_id.clone()),
                qq_song_id: None,
                ..Default::default()
            };
            super::qqmusic::like_toggle(&song, like, &cookie)
                .await
                .map(|_| ())
        }
        _ => Err(format!("不支持的音乐平台: {}", platform)),
    };

    match result {
        Ok(_) => {
            info!(
                correlation_id = correlation_id,
                command = "music_like_song",
                platform = %platform,
                action = "like_complete",
                result = "success",
                duration_ms = 0,
                error_code = "",
                user_id = "",
                "{}歌曲成功: song_id={}",
                if like { "收藏" } else { "取消收藏" },
                song_id
            );
            Ok(())
        }
        Err(e) => {
            warn!(
                correlation_id = correlation_id,
                command = "music_like_song",
                platform = %platform,
                action = "like_error",
                result = "failure",
                duration_ms = 0,
                error_code = "API_ERROR",
                user_id = "",
                "{}歌曲失败: {}",
                if like { "收藏" } else { "取消收藏" },
                e
            );
            Err(format!("操作失败: {}", e))
        }
    }
}

/// 获取我喜欢的歌曲列表
#[tauri::command]
pub async fn music_liked_songs(
    envelope: CmdEnvelope,
    platform: String,
) -> Result<Vec<String>, String> {
    let correlation_id = &envelope.correlation_id;
    let platform = normalized_platform("music_liked_songs", correlation_id, &platform)?;

    debug!(
        correlation_id = correlation_id,
        command = "music_liked_songs",
        platform = %platform,
        action = "liked_fetch_start",
        result = "pending",
        duration_ms = 0,
        error_code = "",
        user_id = "",
        "获取喜欢列表开始"
    );

    // 加载凭据
    let cookie = match platform {
        "netease" => load_credential(MusicPlatform::Netease, correlation_id).unwrap_or_default(),
        "qq" => load_credential(MusicPlatform::QQ, correlation_id).unwrap_or_default(),
        _ => return Err(format!("不支持的音乐平台: {}", platform)),
    };

    if cookie.is_empty() {
        return Err("请先登录".to_string());
    }

    // 对于网易云，需要先获取用户 UID
    let result = match platform {
        "netease" => {
            let status = super::netease::login_status(&cookie).await?;
            if status.user_id.is_empty() {
                return Err("无法获取用户 ID".to_string());
            }
            super::netease::likelist(&status.user_id, &cookie).await
        }
        "qq" => super::qqmusic::liked_hashes(&cookie).await,
        _ => Err(format!("不支持的音乐平台: {}", platform)),
    };

    match result {
        Ok(ids) => {
            info!(
                correlation_id = correlation_id,
                command = "music_liked_songs",
                platform = %platform,
                action = "liked_fetch_complete",
                result = "success",
                duration_ms = 0,
                error_code = "",
                user_id = "",
                "获取喜欢列表完成: count={}",
                ids.len()
            );
            Ok(ids)
        }
        Err(e) => {
            warn!(
                correlation_id = correlation_id,
                command = "music_liked_songs",
                platform = %platform,
                action = "liked_fetch_error",
                result = "failure",
                duration_ms = 0,
                error_code = "API_ERROR",
                user_id = "",
                "获取喜欢列表失败: {}",
                e
            );
            Err(format!("获取喜欢列表失败: {}", e))
        }
    }
}

/// 获取歌曲评论
#[tauri::command]
pub async fn music_song_comments(
    envelope: CmdEnvelope,
    song_id: String,
    platform: String,
    page: Option<u32>,
    page_size: Option<u32>,
) -> Result<CommentPage, String> {
    let correlation_id = &envelope.correlation_id;
    let platform = normalized_platform("music_song_comments", correlation_id, &platform)?;
    let page = page.unwrap_or(1);
    let page_size = page_size.unwrap_or(20);

    debug!(
        correlation_id = correlation_id,
        command = "music_song_comments",
        platform = %platform,
        action = "comments_fetch_start",
        result = "pending",
        duration_ms = 0,
        error_code = "",
        user_id = "",
        "获取评论开始: song_id={}, page={}, page_size={}",
        song_id, page, page_size
    );

    // 加载凭据
    let cookie = match platform {
        "netease" => load_credential(MusicPlatform::Netease, correlation_id).unwrap_or_default(),
        "qq" => load_credential(MusicPlatform::QQ, correlation_id).unwrap_or_default(),
        _ => return Err(format!("不支持的音乐平台: {}", platform)),
    };

    // 目前仅支持网易云评论
    if platform != "netease" {
        return Err("该平台暂不支持评论功能".to_string());
    }

    let result = super::netease::song_comments(&song_id, page, page_size, &cookie).await;

    match result {
        Ok(comments) => {
            info!(
                correlation_id = correlation_id,
                command = "music_song_comments",
                platform = %platform,
                action = "comments_fetch_complete",
                result = "success",
                duration_ms = 0,
                error_code = "",
                user_id = "",
                "获取评论完成: total={}, has_more={}",
                comments.total, comments.has_more
            );
            Ok(comments)
        }
        Err(e) => {
            warn!(
                correlation_id = correlation_id,
                command = "music_song_comments",
                platform = %platform,
                action = "comments_fetch_error",
                result = "failure",
                duration_ms = 0,
                error_code = "API_ERROR",
                user_id = "",
                "获取评论失败: {}",
                e
            );
            Err(format!("获取评论失败: {}", e))
        }
    }
}

/// 发送评论
#[tauri::command]
pub async fn music_send_comment(
    envelope: CmdEnvelope,
    song_id: String,
    content: String,
    platform: String,
) -> Result<(), String> {
    let correlation_id = &envelope.correlation_id;
    let platform = normalized_platform("music_send_comment", correlation_id, &platform)?;

    debug!(
        correlation_id = correlation_id,
        command = "music_send_comment",
        platform = %platform,
        action = "comment_send_start",
        result = "pending",
        duration_ms = 0,
        error_code = "",
        user_id = "",
        "发送评论开始: song_id={}, content_len={}",
        song_id,
        content.len()
    );

    // 加载凭据
    let cookie = match platform {
        "netease" => load_credential(MusicPlatform::Netease, correlation_id).unwrap_or_default(),
        "qq" => load_credential(MusicPlatform::QQ, correlation_id).unwrap_or_default(),
        _ => return Err(format!("不支持的音乐平台: {}", platform)),
    };

    if cookie.is_empty() {
        return Err("请先登录".to_string());
    }

    // 目前仅支持网易云评论
    if platform != "netease" {
        return Err("该平台暂不支持评论功能".to_string());
    }

    let result = super::netease::send_comment(&song_id, &content, &cookie).await;

    match result {
        Ok(_) => {
            info!(
                correlation_id = correlation_id,
                command = "music_send_comment",
                platform = %platform,
                action = "comment_send_complete",
                result = "success",
                duration_ms = 0,
                error_code = "",
                user_id = "",
                "发送评论成功: song_id={}",
                song_id
            );
            Ok(())
        }
        Err(e) => {
            warn!(
                correlation_id = correlation_id,
                command = "music_send_comment",
                platform = %platform,
                action = "comment_send_error",
                result = "failure",
                duration_ms = 0,
                error_code = "API_ERROR",
                user_id = "",
                "发送评论失败: {}",
                e
            );
            Err(format!("发送评论失败: {}", e))
        }
    }
}

/// 获取歌手详情
#[tauri::command]
pub async fn music_artist_detail(
    envelope: CmdEnvelope,
    artist_id: String,
    platform: String,
) -> Result<ArtistDetail, String> {
    let correlation_id = &envelope.correlation_id;
    let platform = normalized_platform("music_artist_detail", correlation_id, &platform)?;

    debug!(
        correlation_id = correlation_id,
        command = "music_artist_detail",
        platform = %platform,
        action = "artist_detail_start",
        result = "pending",
        duration_ms = 0,
        error_code = "",
        user_id = "",
        "获取歌手详情开始: artist_id={}",
        artist_id
    );

    // 加载凭据
    let cookie = match platform {
        "netease" => load_credential(MusicPlatform::Netease, correlation_id).unwrap_or_default(),
        "qq" => load_credential(MusicPlatform::QQ, correlation_id).unwrap_or_default(),
        _ => return Err(format!("不支持的音乐平台: {}", platform)),
    };

    // 目前仅支持网易云
    if platform != "netease" {
        return Err("该平台暂不支持歌手详情功能".to_string());
    }

    let result = super::netease::artist_detail(&artist_id, &cookie).await;

    match result {
        Ok(detail) => {
            info!(
                correlation_id = correlation_id,
                command = "music_artist_detail",
                platform = %platform,
                action = "artist_detail_complete",
                result = "success",
                duration_ms = 0,
                error_code = "",
                user_id = "",
                "获取歌手详情完成: name={}",
                detail.name
            );
            Ok(detail)
        }
        Err(e) => {
            warn!(
                correlation_id = correlation_id,
                command = "music_artist_detail",
                platform = %platform,
                action = "artist_detail_error",
                result = "failure",
                duration_ms = 0,
                error_code = "API_ERROR",
                user_id = "",
                "获取歌手详情失败: {}",
                e
            );
            Err(format!("获取歌手详情失败: {}", e))
        }
    }
}

/// 获取专辑详情 + 曲目列表
#[tauri::command]
pub async fn music_album_detail(
    envelope: CmdEnvelope,
    album_id: String,
    platform: String,
) -> Result<(Album, Vec<Song>), String> {
    let correlation_id = &envelope.correlation_id;
    let platform = normalized_platform("music_album_detail", correlation_id, &platform)?;

    debug!(
        correlation_id = correlation_id,
        command = "music_album_detail",
        platform = %platform,
        action = "album_detail_start",
        result = "pending",
        duration_ms = 0,
        error_code = "",
        user_id = "",
        "获取专辑详情开始: album_id={}",
        album_id
    );

    // 加载凭据
    let cookie = match platform {
        "netease" => load_credential(MusicPlatform::Netease, correlation_id).unwrap_or_default(),
        "qq" => load_credential(MusicPlatform::QQ, correlation_id).unwrap_or_default(),
        _ => return Err(format!("不支持的音乐平台: {}", platform)),
    };

    // 目前仅支持网易云
    if platform != "netease" {
        return Err("该平台暂不支持专辑详情功能".to_string());
    }

    let result = super::netease::album_detail(&album_id, &cookie).await;

    match result {
        Ok((album, songs)) => {
            info!(
                correlation_id = correlation_id,
                command = "music_album_detail",
                platform = %platform,
                action = "album_detail_complete",
                result = "success",
                duration_ms = 0,
                error_code = "",
                user_id = "",
                "获取专辑详情完成: name={}, track_count={}",
                album.name,
                songs.len()
            );
            Ok((album, songs))
        }
        Err(e) => {
            warn!(
                correlation_id = correlation_id,
                command = "music_album_detail",
                platform = %platform,
                action = "album_detail_error",
                result = "failure",
                duration_ms = 0,
                error_code = "API_ERROR",
                user_id = "",
                "获取专辑详情失败: {}",
                e
            );
            Err(format!("获取专辑详情失败: {}", e))
        }
    }
}

/// 获取 MV 播放地址
#[tauri::command]
pub async fn music_mv_url(
    envelope: CmdEnvelope,
    mv_id: String,
    platform: String,
    resolution: Option<u32>,
) -> Result<String, String> {
    let correlation_id = &envelope.correlation_id;
    let platform = normalized_platform("music_mv_url", correlation_id, &platform)?;
    let resolution = resolution.unwrap_or(1080);

    debug!(
        correlation_id = correlation_id,
        command = "music_mv_url",
        platform = %platform,
        action = "mv_url_start",
        result = "pending",
        duration_ms = 0,
        error_code = "",
        user_id = "",
        "获取 MV 地址开始: mv_id={}, resolution={}",
        mv_id, resolution
    );

    // 加载凭据
    let cookie = match platform {
        "netease" => load_credential(MusicPlatform::Netease, correlation_id).unwrap_or_default(),
        "qq" => load_credential(MusicPlatform::QQ, correlation_id).unwrap_or_default(),
        _ => return Err(format!("不支持的音乐平台: {}", platform)),
    };

    // 目前仅支持网易云
    if platform != "netease" {
        return Err("该平台暂不支持 MV 功能".to_string());
    }

    let result = super::netease::mv_url(&mv_id, resolution, &cookie).await;

    match result {
        Ok(url) => {
            info!(
                correlation_id = correlation_id,
                command = "music_mv_url",
                platform = %platform,
                action = "mv_url_complete",
                result = "success",
                duration_ms = 0,
                error_code = "",
                user_id = "",
                "获取 MV 地址完成"
            );
            Ok(url)
        }
        Err(e) => {
            warn!(
                correlation_id = correlation_id,
                command = "music_mv_url",
                platform = %platform,
                action = "mv_url_error",
                result = "failure",
                duration_ms = 0,
                error_code = "API_ERROR",
                user_id = "",
                "获取 MV 地址失败: {}",
                e
            );
            Err(format!("获取 MV 地址失败: {}", e))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::normalized_platform;

    #[test]
    fn platform_alias_and_case_are_normalized() {
        assert_eq!(normalized_platform("music_search", "cid", "qq"), Ok("qq"));
        assert_eq!(
            normalized_platform("music_search", "cid", "qqmusic"),
            Ok("qq")
        );
        assert_eq!(normalized_platform("music_search", "cid", " QQ "), Ok("qq"));
        assert_eq!(
            normalized_platform("music_search", "cid", "netease"),
            Ok("netease")
        );
    }

    #[test]
    fn unknown_platform_is_rejected_not_defaulted() {
        // 禁静默兜底成默认平台：这是 P1 根因②的防线
        assert!(normalized_platform("music_search", "cid", "kugou").is_err());
        assert!(normalized_platform("music_search", "cid", "").is_err());
    }
}
