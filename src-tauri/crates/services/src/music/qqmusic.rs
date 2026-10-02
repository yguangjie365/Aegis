//! QQ 音乐 API 客户端（完全移植自 NexBox `music_api/qqmusic.rs`）
//!
//! 所有 API 端点、签名算法、请求参数、降级策略均与 server.js/NexBox 完全一致。

use std::time::{Duration, SystemTime, UNIX_EPOCH};

use base64::Engine;
use reqwest::header::{COOKIE, REFERER, USER_AGENT};
use serde_json::{json, Value};
use tracing::{debug, info, warn};

use super::cookie::{
    parse_cookie_string, qq_cookie_has_login, qq_cookie_has_playback, qq_extract_music_key,
    qq_extract_playback_key, qq_extract_uin,
};
use super::models::*;
use crate::music::auth::MusicPlatform;

/// 出站 provider 规范值（词表单点见 `MusicPlatform::from_slug` / `slug`；勿在此处写字面量）
const PROVIDER: &str = MusicPlatform::QQ.slug();

// ============================================================
//  常量 (对照 server.js / NexBox)
// ============================================================

const QQ_MUSICU_URL: &str = "https://u.y.qq.com/cgi-bin/musicu.fcg";
const QQ_SMARTBOX_URL: &str = "https://c.y.qq.com/splcloud/fcgi-bin/smartbox_new.fcg";
const QQ_PLAYLIST_CREATED_URL: &str = "https://c.y.qq.com/rsc/fcgi-bin/fcg_user_created_diss";
const QQ_PLAYLIST_COLLECTED_URL: &str =
    "https://c.y.qq.com/fav/fcgi-bin/fcg_get_profile_order_asset.fcg";
const QQ_PLAYLIST_TRACKS_URL: &str =
    "https://c.y.qq.com/qzone/fcg-bin/fcg_ucc_getcdinfo_byids_cp.fcg";
const QQ_PROFILE_URL: &str = "https://c.y.qq.com/rsc/fcgi-bin/fcg_get_profile_homepage.fcg";
const QQ_LYRIC_LEGACY_URL: &str = "https://c.y.qq.com/lyric/fcgi-bin/fcg_query_lyric_new.fcg";

const QQ_HEADERS_UA: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36";
const QQ_SEARCH_UA: &str = "QQMusic 14090508(android 12)";
const QQ_HEADERS_REFERER: &str = "https://y.qq.com/";

const QQ_LIKED_DIRID: i64 = 201;
const QQ_LIKED_PLAYLIST_ID: &str = "liked";
const QQ_LIKED_PLAYLIST_NAME: &str = "QQ 音乐·我的喜欢";
const QQ_LIKED_PLAYLIST_COVER: &str = "https://y.gtimg.cn/mediastyle/global/img/cover_like.png";

const QQ_PLAYLIST_SYNC_PAGE_SIZE: u32 = 200;
const QQ_PLAYLIST_SYNC_MAX_PAGES: usize = 25;

const QQ_VKEY_REQUEST_TIMEOUT_MS: u64 = 6000;
const AUDIO_URL_PROBE_BYTES: usize = 8192;

// QQ 音质模板 (prefix, ext, level, label)
// level 名须与 normalize_quality 输出同词汇（hires/lossless/exhigh/standard），否则降级链查位恒失败
const QQ_QUALITY_TEMPLATES: &[(&str, &str, &str, &str)] = &[
    ("RS01", ".flac", "hires", "Hi-Res FLAC"),
    ("F000", ".flac", "lossless", "无损 FLAC"),
    ("M800", ".mp3", "exhigh", "320k MP3"),
    ("M500", ".mp3", "standard", "128k MP3"),
    ("C400", ".m4a", "aac", "AAC/M4A"),
];

fn now_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as i64
}

// ============================================================
//  HTTP 请求工具
// ============================================================

fn build_client_with_timeout(timeout_ms: u64) -> reqwest::Client {
    reqwest::Client::builder()
        .timeout(Duration::from_millis(timeout_ms))
        .build()
        .expect("Failed to build HTTP client")
}

/// 发送请求并返回文本
async fn request_text(
    url: &str,
    method: &str,
    headers: &[(&str, &str)],
    body: Option<&str>,
    timeout_ms: u64,
) -> Result<String, String> {
    let client = build_client_with_timeout(timeout_ms);
    let method = match method {
        "POST" => reqwest::Method::POST,
        _ => reqwest::Method::GET,
    };
    let mut req = client.request(method, url);
    for (key, value) in headers {
        req = req.header(*key, *value);
    }
    if let Some(b) = body {
        req = req.body(b.to_string());
    }
    let resp = req
        .send()
        .await
        .map_err(|e| format!("Request failed: {e}"))?;
    let status = resp.status();
    let text = resp
        .text()
        .await
        .map_err(|e| format!("Failed to read response: {e}"))?;
    if !status.is_success() {
        return Err(format!(
            "HTTP {}: {}",
            status.as_u16(),
            &text[..text.len().min(200)]
        ));
    }
    Ok(text)
}

/// 解析 JSON 文本，处理 callback 包裹
fn parse_json_text(text: &str) -> Result<Value, String> {
    let raw = text.trim();
    let is_jsonp = raw.ends_with(')') || raw.ends_with(");");
    let has_callback_prefix = raw.starts_with("callback")
        || raw.starts_with("MusicJsonCallback")
        || raw.starts_with("jsonCallback")
        || raw.starts_with("Callback");

    let json_str = if is_jsonp && has_callback_prefix {
        let start = raw.find('(').map(|i| i + 1).unwrap_or(0);
        let end = raw.rfind(')').unwrap_or(raw.len());
        if start < end {
            &raw[start..end]
        } else {
            raw
        }
    } else {
        raw
    };
    serde_json::from_str(json_str).map_err(|e| {
        format!(
            "Failed to parse JSON: {e}, body: {}",
            &text[..text.len().min(300)]
        )
    })
}

/// 发送 musicu.fcg 请求
async fn qq_musicu_request(
    payload: &Value,
    cookie: &str,
    timeout_ms: u64,
) -> Result<Value, String> {
    let body = serde_json::to_string(payload).map_err(|e| e.to_string())?;
    let mut headers = vec![
        ("Referer", QQ_HEADERS_REFERER),
        ("User-Agent", QQ_HEADERS_UA),
        ("Content-Type", "application/json;charset=UTF-8"),
    ];
    if !cookie.is_empty() {
        headers.push(("Cookie", cookie));
    }
    let text = request_text(QQ_MUSICU_URL, "POST", &headers, Some(&body), timeout_ms).await?;
    debug!(
        "[QQMusicu] response preview: {}",
        &text[..text.len().min(500)]
    );
    parse_json_text(&text)
}

/// 发送 GET JSON 请求
async fn qq_get_json(url: &str, params: &[(&str, &str)], cookie: &str) -> Result<Value, String> {
    let mut full_url = url.to_string();
    if !params.is_empty() {
        let query: Vec<String> = params
            .iter()
            .map(|(k, v)| format!("{}={}", urlencoding::encode(k), urlencoding::encode(v)))
            .collect();
        full_url = format!("{}?{}", full_url, query.join("&"));
    }
    let mut headers = vec![
        (REFERER.as_str(), QQ_HEADERS_REFERER),
        (USER_AGENT.as_str(), QQ_HEADERS_UA),
    ];
    if !cookie.is_empty() {
        headers.push((COOKIE.as_str(), cookie));
    }
    let text = request_text(&full_url, "GET", &headers, None, 10000).await?;
    parse_json_text(&text)
}

// ============================================================
//  搜索签名 (对照 qqSearchSign)
// ============================================================

/// QQ 音乐搜索签名
fn qq_search_sign(text: &str) -> String {
    use sha1::{Digest, Sha1};
    let mut hasher = Sha1::new();
    hasher.update(text.as_bytes());
    let hash = hex::encode(hasher.finalize());
    let hash_bytes = hash.as_bytes();

    let part1_indices = [23usize, 14, 6, 36, 16, 40, 7, 19];
    let part2_indices = [16usize, 1, 32, 12, 19, 27, 8, 5];
    let scramble = [
        89u8, 39, 179, 150, 218, 82, 58, 252, 177, 52, 186, 123, 120, 64, 242, 133, 143, 161, 121,
        179,
    ];

    let part1: String = part1_indices
        .iter()
        .filter_map(|&i| hash_bytes.get(i).map(|&b| b as char))
        .collect();
    let part2: String = part2_indices
        .iter()
        .filter_map(|&i| hash_bytes.get(i).map(|&b| b as char))
        .collect();

    let bytes: Vec<u8> = scramble
        .iter()
        .enumerate()
        .map(|(i, &v)| {
            let hex_pair = &hash[i * 2..i * 2 + 2];
            v ^ u8::from_str_radix(hex_pair, 16).unwrap_or(0)
        })
        .collect();

    let middle = base64::engine::general_purpose::STANDARD
        .encode(&bytes)
        .chars()
        .filter(|c| *c != '\\' && *c != '/' && *c != '+' && *c != '=')
        .collect::<String>();

    format!("zzc{}{}{}", part1, middle, part2).to_lowercase()
}

// ============================================================
//  Cookie / 认证
// ============================================================

/// QQ 音乐认证信息
#[derive(Debug, Clone, Default)]
pub struct QQAuth {
    pub uin: String,
    pub music_key: String,
    pub playback_key: String,
    pub login_type: i32,
    pub nickname: String,
    pub avatar: String,
    pub logged_in: bool,
    pub playback_ready: bool,
}

/// 从 Cookie 提取认证信息
pub fn extract_qq_auth(cookie: &str) -> QQAuth {
    let uin = qq_extract_uin(cookie);
    let music_key = qq_extract_music_key(cookie);
    let playback_key = qq_extract_playback_key(cookie);
    let login_type = parse_cookie_string(cookie)
        .get("login_type")
        .map(|v| v.trim().parse::<i32>().unwrap_or(0))
        .unwrap_or(0);

    let logged_in = !uin.is_empty() && !music_key.is_empty();
    let playback_ready = !uin.is_empty() && !playback_key.is_empty();

    QQAuth {
        uin,
        music_key,
        playback_key,
        login_type,
        logged_in,
        playback_ready,
        ..Default::default()
    }
}

// ============================================================
//  工具函数
// ============================================================

/// 专辑封面 URL
fn qq_album_cover(album_mid: &str, size: u32) -> String {
    if album_mid.is_empty() {
        return String::new();
    }
    format!(
        "https://y.qq.com/music/photo_new/T002R{}x{}M000{}.jpg?max_age=2592000",
        size, size, album_mid
    )
}

/// 歌手头像 URL
fn qq_singer_avatar(singer_mid: &str, size: u32) -> String {
    if singer_mid.is_empty() {
        return String::new();
    }
    format!(
        "https://y.qq.com/music/photo_new/T001R{}x{}M000{}.jpg?max_age=2592000",
        size, size, singer_mid
    )
}

/// 映射 QQ 歌手列表
fn map_qq_artists(raw: &Value) -> Vec<Artist> {
    raw.as_array()
        .map(|arr| {
            arr.iter()
                .map(|a| Artist {
                    id: a.get("id").and_then(|v| v.as_i64()).map(|n| n.to_string()),
                    mid: a.get("mid").and_then(|v| v.as_str()).map(|s| s.to_string()),
                    name: a
                        .get("name")
                        .or_else(|| a.get("title"))
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string(),
                    pic_url: None,
                    music_size: None,
                })
                .filter(|a| !a.name.is_empty())
                .collect()
        })
        .unwrap_or_default()
}

/// 映射 QQ 歌曲详情
fn map_qq_track(track: &Value, fallback: &Song) -> Song {
    let album = track.get("album").unwrap_or(&Value::Null);
    let artists = map_qq_artists(track.get("singer").unwrap_or(&Value::Null));
    let mid = track
        .get("mid")
        .or_else(|| track.get("songmid"))
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    let fallback_mid = fallback.mid.as_deref().unwrap_or("");
    let song_mid = if !mid.is_empty() {
        mid.as_str()
    } else {
        fallback_mid
    };

    let album_mid = album
        .get("mid")
        .or_else(|| album.get("pmid"))
        .and_then(|v| v.as_str())
        .or_else(|| track.get("albummid").and_then(|v| v.as_str()))
        .unwrap_or("")
        .to_string();

    let media_mid = track
        .get("file")
        .and_then(|f| f.get("media_mid"))
        .and_then(|v| v.as_str())
        .or_else(|| track.get("strMediaMid").and_then(|v| v.as_str()))
        .unwrap_or("")
        .to_string();

    let name = track
        .get("name")
        .or_else(|| track.get("title"))
        .or_else(|| track.get("songname"))
        .and_then(|v| v.as_str())
        .unwrap_or(&fallback.name)
        .to_string();

    let artist_str = if !artists.is_empty() {
        artists
            .iter()
            .map(|a| a.name.clone())
            .collect::<Vec<_>>()
            .join(" / ")
    } else {
        fallback.artist.clone()
    };

    let album_name = album
        .get("name")
        .or_else(|| album.get("title"))
        .and_then(|v| v.as_str())
        .or_else(|| track.get("albumname").and_then(|v| v.as_str()))
        .unwrap_or(&fallback.album)
        .to_string();

    let duration = track.get("interval").and_then(|v| v.as_u64()).unwrap_or(0) * 1000;

    let fee = track
        .get("pay")
        .and_then(|p| p.get("pay_play"))
        .and_then(|v| v.as_i64())
        .map(|n| if n > 0 { 1 } else { 0 })
        .unwrap_or(0);

    let id = if !song_mid.is_empty() {
        song_mid.to_string()
    } else {
        track
            .get("id")
            .or_else(|| track.get("songid"))
            .and_then(|v| v.as_i64())
            .map(|n| n.to_string())
            .unwrap_or_else(|| fallback.id.clone())
    };
    let qq_song_id = track
        .get("id")
        .or_else(|| track.get("songid"))
        .and_then(|v| v.as_i64());

    Song {
        provider: PROVIDER.into(),
        id,
        mid: if !song_mid.is_empty() {
            Some(song_mid.into())
        } else {
            fallback.mid.clone()
        },
        media_mid: if !media_mid.is_empty() {
            Some(media_mid)
        } else {
            fallback.media_mid.clone()
        },
        name,
        artist: artist_str,
        artists: if !artists.is_empty() {
            artists
        } else {
            fallback.artists.clone()
        },
        album: album_name,
        cover: {
            let c = qq_album_cover(&album_mid, 800);
            if !c.is_empty() {
                c
            } else {
                fallback.cover.clone()
            }
        },
        duration,
        fee,
        playable: false,
        language: 0,
        qq_song_id,
        ..Default::default()
    }
}

/// 映射 QQ 歌单曲目
fn map_qq_playlist_track(raw: &Value) -> Song {
    let track = if raw.get("songid").is_some()
        || raw.get("songmid").is_some()
        || raw.get("mid").is_some()
    {
        raw.clone()
    } else {
        raw.get("track_info")
            .or_else(|| raw.get("songInfo"))
            .or_else(|| raw.get("songinfo"))
            .or_else(|| raw.get("song"))
            .cloned()
            .unwrap_or_else(|| raw.clone())
    };

    let album = track.get("album").cloned().unwrap_or(Value::Null);
    let artists = map_qq_artists(
        track
            .get("singer")
            .or_else(|| track.get("singers"))
            .unwrap_or(&Value::Null),
    );

    let mid = track
        .get("mid")
        .or_else(|| track.get("songmid"))
        .or_else(|| raw.get("mid"))
        .or_else(|| raw.get("songmid"))
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();

    let album_mid = album
        .get("mid")
        .or_else(|| track.get("albummid"))
        .or_else(|| raw.get("albummid"))
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();

    let media_mid = track
        .get("file")
        .and_then(|f| f.get("media_mid"))
        .or_else(|| track.get("strMediaMid"))
        .or_else(|| track.get("media_mid"))
        .or_else(|| raw.get("strMediaMid"))
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();

    let name = track
        .get("name")
        .or_else(|| track.get("songname"))
        .or_else(|| raw.get("songname"))
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();

    let artist_str = if !artists.is_empty() {
        artists
            .iter()
            .map(|a| a.name.clone())
            .collect::<Vec<_>>()
            .join(" / ")
    } else {
        track
            .get("singername")
            .or_else(|| raw.get("singername"))
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string()
    };

    let album_name = album
        .get("name")
        .or_else(|| album.get("title"))
        .or_else(|| track.get("albumname"))
        .or_else(|| raw.get("albumname"))
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();

    let duration = track
        .get("interval")
        .or_else(|| raw.get("interval"))
        .and_then(|v| v.as_u64())
        .unwrap_or(0)
        * 1000;

    let fee = track
        .get("pay")
        .and_then(|p| p.get("pay_play"))
        .and_then(|v| v.as_i64())
        .map(|n| if n > 0 { 1 } else { 0 })
        .unwrap_or(0);

    let id = if !mid.is_empty() {
        mid.clone()
    } else {
        track
            .get("id")
            .or_else(|| track.get("songid"))
            .or_else(|| raw.get("id"))
            .or_else(|| raw.get("songid"))
            .and_then(|v| v.as_i64())
            .map(|n| n.to_string())
            .unwrap_or_default()
    };
    let qq_song_id = track
        .get("id")
        .or_else(|| track.get("songid"))
        .or_else(|| raw.get("songid"))
        .and_then(|v| v.as_i64());

    Song {
        provider: PROVIDER.into(),
        id,
        mid: if !mid.is_empty() { Some(mid) } else { None },
        media_mid: if !media_mid.is_empty() {
            Some(media_mid)
        } else {
            None
        },
        name,
        artist: artist_str,
        artists,
        album: album_name,
        cover: qq_album_cover(&album_mid, 800),
        duration,
        fee,
        playable: false,
        language: 0,
        qq_song_id,
        ..Default::default()
    }
}

/// 映射 QQ 歌单
fn map_qq_playlist(pl: &Value, kind: &str) -> Playlist {
    let dirid_num = pl
        .get("dirid")
        .or_else(|| pl.get("dir_id"))
        .or_else(|| pl.get("dirId"))
        .and_then(|v| v.as_i64())
        .unwrap_or(0);
    let liked = dirid_num == QQ_LIKED_DIRID || is_qq_favorite_playlist(pl);

    let id = if liked {
        QQ_LIKED_PLAYLIST_ID.to_string()
    } else {
        pl.get("dissid")
            .or_else(|| pl.get("tid"))
            .or_else(|| pl.get("dissId"))
            .or_else(|| pl.get("id"))
            .or_else(|| pl.get("diss_id"))
            .and_then(|v| match v {
                Value::String(s) => Some(s.clone()),
                Value::Number(n) => Some(n.to_string()),
                _ => None,
            })
            .unwrap_or_else(|| dirid_num.to_string())
    };

    let raw_name = pl
        .get("diss_name")
        .or_else(|| pl.get("dissname"))
        .or_else(|| pl.get("name"))
        .or_else(|| pl.get("title"))
        .and_then(|v| v.as_str())
        .unwrap_or("");

    let name = if liked {
        QQ_LIKED_PLAYLIST_NAME.to_string()
    } else {
        raw_name.to_string()
    };

    let cover = if liked {
        QQ_LIKED_PLAYLIST_COVER.to_string()
    } else {
        pl.get("diss_cover")
            .or_else(|| pl.get("dissCover"))
            .or_else(|| pl.get("logo"))
            .or_else(|| pl.get("picurl"))
            .or_else(|| pl.get("cover"))
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string()
    };

    let track_count = pl
        .get("song_cnt")
        .or_else(|| pl.get("songCnt"))
        .or_else(|| pl.get("songnum"))
        .or_else(|| pl.get("songNum"))
        .or_else(|| pl.get("total_song_num"))
        .or_else(|| pl.get("song_count"))
        .or_else(|| pl.get("songCount"))
        .and_then(|v| v.as_u64())
        .unwrap_or(0) as u32;

    let creator = pl
        .get("hostname")
        .or_else(|| pl.get("nick"))
        .or_else(|| pl.get("creator"))
        .or_else(|| pl.get("nickname"))
        .and_then(|v| match v {
            Value::String(s) => Some(s.clone()),
            Value::Object(_) => v
                .get("name")
                .and_then(|n| n.as_str())
                .map(|s| s.to_string()),
            _ => None,
        })
        .unwrap_or_else(|| "QQ 音乐".to_string());

    Playlist {
        provider: PROVIDER.into(),
        id,
        name,
        cover,
        track_count,
        creator,
        subscribed: kind == "collect",
    }
}

/// 检查是否为 QQ 喜欢歌单
fn is_qq_liked_playlist_id(id: &str) -> bool {
    let value = id.trim().to_lowercase();
    value == QQ_LIKED_PLAYLIST_ID || value == "qq-liked" || value == QQ_LIKED_DIRID.to_string()
}

/// 检查是否为 QQ 收藏歌单
fn is_qq_favorite_playlist(pl: &Value) -> bool {
    let dirid = pl
        .get("dirid")
        .or_else(|| pl.get("dir_id"))
        .or_else(|| pl.get("dirId"))
        .and_then(|v| v.as_i64())
        .unwrap_or(0);
    if dirid == QQ_LIKED_DIRID {
        return true;
    }
    if let Some(id_str) = pl.get("id").and_then(|v| v.as_str()) {
        if is_qq_liked_playlist_id(id_str) {
            return true;
        }
    }
    if let Some(tid_str) = pl
        .get("tid")
        .or_else(|| pl.get("dissid"))
        .and_then(|v| v.as_str())
    {
        if is_qq_liked_playlist_id(tid_str) {
            return true;
        }
    }
    let name = pl
        .get("diss_name")
        .or_else(|| pl.get("dissname"))
        .or_else(|| pl.get("name"))
        .or_else(|| pl.get("title"))
        .and_then(|v| v.as_str())
        .unwrap_or("");
    let normalized = name
        .replace(['·', '•', '・', '_', '-', ' ', '\t', '\n'], "")
        .to_lowercase();
    matches!(
        normalized.as_str(),
        "我喜欢"
            | "我的喜欢"
            | "喜欢的音乐"
            | "qq音乐我喜欢"
            | "qq音乐我的喜欢"
            | "qq音乐喜欢的音乐"
    )
}

/// 去除搜索结果名称中的 HTML 高亮标签
fn strip_html_tags(text: &str) -> String {
    if text.contains('<') || text.contains('>') {
        regex::Regex::new(r"<[^>]*>")
            .map(|re| re.replace_all(text, "").to_string())
            .unwrap_or_else(|_| text.to_string())
    } else {
        text.to_string()
    }
}

// ============================================================
//  搜索
// ============================================================

/// 智能搜索
async fn smartbox_search(keywords: &str, limit: u32) -> Result<Vec<Song>, String> {
    let limit = limit.clamp(1, 10);
    let params: Vec<(&str, &str)> = vec![
        ("format", "json"),
        ("key", keywords),
        ("g_tk", "5381"),
        ("loginUin", "0"),
        ("hostUin", "0"),
        ("inCharset", "utf8"),
        ("outCharset", "utf-8"),
        ("notice", "0"),
        ("platform", "yqq.json"),
        ("needNewCode", "0"),
    ];
    let full_url = format!(
        "{}?{}",
        QQ_SMARTBOX_URL,
        params
            .iter()
            .map(|(k, v)| format!("{}={}", urlencoding::encode(k), urlencoding::encode(v)))
            .collect::<Vec<_>>()
            .join("&")
    );
    let headers = vec![
        (REFERER.as_str(), QQ_HEADERS_REFERER),
        (USER_AGENT.as_str(), QQ_HEADERS_UA),
    ];
    let text = request_text(&full_url, "GET", &headers, None, 10000).await?;
    let json = parse_json_text(&text)?;
    let items = json
        .get("data")
        .and_then(|d| d.get("song"))
        .and_then(|s| s.get("itemlist"))
        .and_then(|l| l.as_array());

    let songs = items
        .map(|arr| {
            arr.iter()
                .take(limit as usize)
                .map(|item| {
                    let mid = item
                        .get("mid")
                        .or_else(|| item.get("songmid"))
                        .or_else(|| item.get("id"))
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string();
                    let name = strip_html_tags(
                        &item
                            .get("name")
                            .or_else(|| item.get("title"))
                            .and_then(|v| v.as_str())
                            .unwrap_or(""),
                    );
                    let singer = item
                        .get("singer")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string();
                    let id = mid.clone();
                    Song {
                        provider: PROVIDER.into(),
                        id,
                        mid: if !mid.is_empty() { Some(mid) } else { None },
                        name,
                        artist: singer.clone(),
                        artists: if !singer.is_empty() {
                            vec![Artist {
                                name: singer,
                                ..Default::default()
                            }]
                        } else {
                            vec![]
                        },
                        ..Default::default()
                    }
                })
                .collect()
        })
        .unwrap_or_default();
    Ok(songs)
}

/// 完整搜索
async fn full_song_search(keywords: &str, limit: u32, offset: u32) -> Result<Vec<Song>, String> {
    let limit = limit.clamp(1, 30);
    let page_num = offset / limit + 1;
    let payload = json!({
        "comm": {
            "ct": "11",
            "cv": "14090508",
            "v": "14090508",
            "tmeAppID": "qqmusic",
            "phonetype": "EBG-AN10",
            "os_ver": "12",
            "OpenUDID": "0",
            "QIMEI36": "0",
            "udid": "0",
            "chid": "0",
            "aid": "0",
            "oaid": "0",
            "taid": "0",
            "tid": "0",
            "wid": "0",
            "uid": "0",
            "sid": "0",
            "modeSwitch": "6",
            "teenMode": "0",
            "ui_mode": "2",
            "nettype": "1020"
        },
        "req": {
            "module": "music.search.SearchCgiService",
            "method": "DoSearchForQQMusicMobile",
            "param": {
                "search_type": 0,
                "searchid": format!("{}{:06}", chrono::Utc::now().timestamp_millis(), rand::random::<u32>() % 1000000),
                "query": keywords,
                "page_num": page_num,
                "num_per_page": limit,
                "highlight": 0,
                "nqc_flag": 0,
                "multi_zhida": 0,
                "cat": 2,
                "grp": 1,
                "sin": offset,
                "sem": 0
            }
        }
    });

    let body_text = serde_json::to_string(&payload).map_err(|e| e.to_string())?;
    let sign = qq_search_sign(&body_text);
    let url = format!("https://u.y.qq.com/cgi-bin/musics.fcg?sign={}", sign);
    let headers = vec![
        ("User-Agent", QQ_SEARCH_UA),
        ("Content-Type", "application/json"),
    ];
    let text = request_text(&url, "POST", &headers, Some(&body_text), 10000).await?;
    let json = parse_json_text(&text)?;

    let data = json
        .get("req")
        .and_then(|r| r.get("data"))
        .unwrap_or(&Value::Null);
    let body = data.get("body").unwrap_or(data);
    let items = body
        .get("item_song")
        .or_else(|| body.get("song").and_then(|s| s.get("list")))
        .or_else(|| body.get("list"))
        .and_then(|l| l.as_array());

    let songs = items
        .map(|arr| {
            arr.iter()
                .map(|item| {
                    let track = item
                        .get("track_info")
                        .or_else(|| item.get("songInfo"))
                        .or_else(|| item.get("songinfo"))
                        .or_else(|| item.get("song"))
                        .unwrap_or(item);
                    let mut song = map_qq_track(track, &Song::default());
                    song.name = strip_html_tags(&song.name);
                    song
                })
                .filter(|s| !s.name.is_empty() && (s.mid.is_some() || !s.id.is_empty()))
                .collect()
        })
        .unwrap_or_default();
    Ok(songs)
}

/// 歌曲详情
async fn song_detail(mid: &str, fallback: &Song) -> Song {
    if mid.is_empty() {
        return fallback.clone();
    }
    let payload = json!({
        "comm": { "ct": 24, "cv": 0 },
        "songinfo": {
            "module": "music.pf_song_detail_svr",
            "method": "get_song_detail_yqq",
            "param": { "song_mid": mid }
        }
    });
    match qq_musicu_request(&payload, "", 10000).await {
        Ok(json) => {
            let data = json
                .get("songinfo")
                .and_then(|s| s.get("data"))
                .unwrap_or(&Value::Null);
            let track_info = data.get("track_info").unwrap_or(&Value::Null);
            map_qq_track(track_info, fallback)
        }
        Err(_) => fallback.clone(),
    }
}

/// 搜索 (公开 API)
pub async fn search(keywords: &str, limit: u32, _cookie: &str) -> Result<Vec<Song>, String> {
    let kw = keywords.trim();
    if kw.is_empty() {
        return Ok(vec![]);
    }
    let limit = limit.clamp(1, 30);

    let mut base = Vec::new();
    match full_song_search(kw, limit, 0).await {
        Ok(results) => base = results,
        Err(e) => warn!("[QQSearch] full search failed: {e}"),
    }

    if base.is_empty() {
        match smartbox_search(kw, limit).await {
            Ok(results) => base = results,
            Err(e) => warn!("[QQSearch] smartbox failed: {e}"),
        }
    }

    // 补全详情 (并行执行)
    let futures: Vec<_> = base
        .iter()
        .map(|item| {
            let mid = item.mid.as_deref().unwrap_or("").to_string();
            let fallback = item.clone();
            async move {
                if !mid.is_empty() {
                    song_detail(&mid, &fallback).await
                } else {
                    fallback
                }
            }
        })
        .collect();
    let detailed = futures_util::future::join_all(futures).await;

    // 去重
    let mut seen = std::collections::HashSet::new();
    let songs: Vec<Song> = detailed
        .into_iter()
        .filter(|s| {
            let key = s.mid.as_deref().unwrap_or(&s.id).to_string();
            let name_key = &s.name;
            if name_key.is_empty() {
                return false;
            }
            if seen.contains(&key) {
                return false;
            }
            seen.insert(key);
            true
        })
        .collect();

    Ok(songs)
}

// ============================================================
//  播放地址
// ============================================================

/// 探测音频 URL
async fn probe_audio_url(audio_url: &str, timeout_ms: u64) -> bool {
    if audio_url.is_empty() {
        return false;
    }
    let client = build_client_with_timeout(timeout_ms);
    let range = format!("bytes=0-{}", AUDIO_URL_PROBE_BYTES - 1);
    let resp = match client
        .get(audio_url)
        .header("Range", &range)
        .header("Referer", QQ_HEADERS_REFERER)
        .header("User-Agent", QQ_HEADERS_UA)
        .send()
        .await
    {
        Ok(r) => r,
        Err(_) => return false,
    };

    let status = resp.status().as_u16();
    if status != 200 && status != 206 {
        return false;
    }

    let content_type = resp
        .headers()
        .get("content-type")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("")
        .to_lowercase();
    if content_type.contains("text/html")
        || content_type.contains("application/json")
        || content_type.contains("application/xml")
        || content_type.contains("text/plain")
    {
        return false;
    }

    let body = match resp.bytes().await {
        Ok(b) => b,
        Err(_) => return false,
    };

    if body.len() < 512 {
        return false;
    }

    let is_mp3 = body.starts_with(b"ID3");
    let is_flac = body.starts_with(b"fLaC");
    let is_ogg = body.starts_with(b"OggS");
    let is_wav = body.len() >= 12 && &body[0..4] == b"RIFF" && &body[8..12] == b"WAVE";
    let is_mp4 = body.len() >= 8 && &body[4..8] == b"ftyp";
    let is_mpeg = (0..body.len().saturating_sub(1).min(2048))
        .any(|i| body[i] == 0xff && (body[i + 1] & 0xe0) == 0xe0);

    is_mp3 || is_flac || is_ogg || is_wav || is_mp4 || is_mpeg
}

/// 获取播放地址 (公开 API)
///
/// `media_mid` 用于构建无损 filename 候选（部分曲目 media_mid ≠ song_mid，缺它拿不到 FLAC）
pub async fn song_url(
    song_id: &str,
    media_mid: &str,
    quality: &str,
    cookie: &str,
) -> Result<SongUrlResult, String> {
    let songmid = song_id.trim();
    if songmid.is_empty() {
        return Ok(SongUrlResult {
            url: None,
            playable: false,
            ..Default::default()
        });
    }

    let auth = extract_qq_auth(cookie);
    let uin = if auth.uin.is_empty() {
        "0".to_string()
    } else {
        auth.uin.clone()
    };
    let music_key = &auth.music_key;

    // 生成随机 guid
    let guid = format!("{}", 10000000 + rand::random::<u32>() % 90000000);
    let file_media_mid = media_mid.trim().to_string();

    // 按音质模板构建 filename 候选列表
    let requested_quality = normalize_quality(quality);
    let quality_start = QQ_QUALITY_TEMPLATES
        .iter()
        .position(|(_, _, level, _)| *level == requested_quality)
        .unwrap_or(0);
    let templates = &QQ_QUALITY_TEMPLATES[quality_start..];

    // 构建 mediaIds 列表
    let mut media_ids = Vec::new();
    if !file_media_mid.is_empty() {
        media_ids.push(file_media_mid.clone());
    }
    if !songmid.is_empty() && !media_ids.contains(&songmid.to_string()) {
        media_ids.push(songmid.to_string());
    }

    // 构建 filename 候选列表
    let mut file_candidates: Vec<(String, String, String)> = Vec::new();
    for media_id in &media_ids {
        for (prefix, ext, level, label) in templates {
            let filename = format!("{}{}{}", prefix, media_id, ext);
            file_candidates.push((filename, level.to_string(), label.to_string()));
        }
    }

    let filenames: Vec<String> = file_candidates.iter().map(|(f, _, _)| f.clone()).collect();
    let songmids: Vec<String> = filenames.iter().map(|_| songmid.to_string()).collect();
    let songtypes: Vec<i32> = filenames.iter().map(|_| 0).collect();

    let comm = if !music_key.is_empty() {
        json!({
            "uin": uin,
            "format": "json",
            "ct": 19,
            "cv": 0,
            "authst": music_key
        })
    } else {
        json!({
            "uin": uin,
            "format": "json",
            "ct": 24,
            "cv": 0
        })
    };

    let mut param = json!({
        "guid": guid,
        "songmid": songmids,
        "songtype": songtypes,
        "uin": uin,
        "loginflag": 1,
        "platform": "20",
    });
    if !filenames.is_empty() {
        param["filename"] = json!(filenames);
    }

    let payload = json!({
        "comm": comm,
        "req_0": {
            "module": "vkey.GetVkeyServer",
            "method": "CgiGetVkey",
            "param": param
        }
    });

    let json = qq_musicu_request(&payload, cookie, QQ_VKEY_REQUEST_TIMEOUT_MS).await?;
    let data = json
        .get("req_0")
        .and_then(|r| r.get("data"))
        .unwrap_or(&Value::Null);

    let infos = data
        .get("midurlinfo")
        .and_then(|m| m.as_array())
        .cloned()
        .unwrap_or_default();
    let purl_infos: Vec<&Value> = infos
        .iter()
        .filter(|item| {
            item.get("purl")
                .and_then(|v| v.as_str())
                .map(|s| !s.is_empty())
                .unwrap_or(false)
        })
        .collect();

    let sips: Vec<String> = data
        .get("sip")
        .and_then(|s| s.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str().map(|s| s.to_string()))
                .filter(|s| !s.is_empty())
                .collect()
        })
        .unwrap_or_else(|| vec!["https://ws.stream.qqmusic.qq.com/".into()]);

    // 探测音频 URL
    let probe_deadline_ms = 2000u64;
    let probe_attempt_ms = 800u64;
    let start_time = std::time::Instant::now();

    for candidate_info in &purl_infos {
        for sip in &sips {
            if start_time.elapsed().as_millis() as u64 > probe_deadline_ms - 200 {
                break;
            }
            let purl = candidate_info
                .get("purl")
                .and_then(|v| v.as_str())
                .unwrap_or("");
            if purl.is_empty() {
                continue;
            }
            let candidate_url = format!("{}{}", sip, purl);
            let remaining_ms =
                probe_deadline_ms.saturating_sub(start_time.elapsed().as_millis() as u64);
            let probe_timeout = probe_attempt_ms.min(remaining_ms);
            if probe_audio_url(&candidate_url, probe_timeout).await {
                let filename = candidate_info
                    .get("filename")
                    .and_then(|v| v.as_str())
                    .unwrap_or("");
                let file_meta = file_candidates.iter().find(|(f, _, _)| f == filename);
                let level = file_meta.map(|(_, l, _)| l.clone()).unwrap_or_default();
                let label = file_meta.map(|(_, _, lb)| lb.clone()).unwrap_or_default();

                return Ok(SongUrlResult {
                    url: Some(candidate_url),
                    playable: true,
                    trial: false,
                    level,
                    quality: label,
                    br: 0,
                    ..Default::default()
                });
            }
        }
    }

    // 探测全部失败，返回第一个候选 URL
    if let Some(first_info) = purl_infos.first() {
        if let Some(purl) = first_info.get("purl").and_then(|v| v.as_str()) {
            if !purl.is_empty() {
                let sip = sips
                    .first()
                    .map(|s| s.as_str())
                    .unwrap_or("https://ws.stream.qqmusic.qq.com/");
                let fallback_url = format!("{}{}", sip, purl);
                let filename = first_info
                    .get("filename")
                    .and_then(|v| v.as_str())
                    .unwrap_or("");
                let file_meta = file_candidates.iter().find(|(f, _, _)| f == filename);
                let level = file_meta.map(|(_, l, _)| l.clone()).unwrap_or_default();
                let label = file_meta.map(|(_, _, lb)| lb.clone()).unwrap_or_default();

                return Ok(SongUrlResult {
                    url: Some(fallback_url),
                    playable: true,
                    trial: false,
                    level,
                    quality: label,
                    br: 0,
                    ..Default::default()
                });
            }
        }
    }

    // 无可用 URL
    let restriction_msg = if auth.logged_in && auth.playback_ready {
        "QQ 音乐没有返回播放地址，可能受版权、会员或官方客户端限制"
    } else if auth.logged_in {
        "QQ 音乐当前只拿到了网页登录状态，还缺少播放授权，请重新登录"
    } else {
        "QQ 音乐需要登录或授权后才能获取播放地址"
    };

    Ok(SongUrlResult {
        url: None,
        playable: false,
        trial: false,
        level: requested_quality,
        quality: String::new(),
        br: 0,
        reason: Some("QQ_URL_UNAVAILABLE".into()),
        message: Some(restriction_msg.into()),
        fee: None,
        ..Default::default()
    })
}

// ============================================================
//  歌单
// ============================================================

/// 获取用户创建的歌单
async fn fetch_created_playlists(uin: &str, cookie: &str) -> Vec<Value> {
    let mut out = Vec::new();
    for page in 0..QQ_PLAYLIST_SYNC_MAX_PAGES {
        let sin = page * QQ_PLAYLIST_SYNC_PAGE_SIZE as usize;
        let sin_str = sin.to_string();
        let size_str = QQ_PLAYLIST_SYNC_PAGE_SIZE.to_string();
        let params: Vec<(&str, &str)> = vec![
            ("hostUin", "0"),
            ("hostuin", uin),
            ("sin", &sin_str),
            ("size", &size_str),
            ("g_tk", "5381"),
            ("loginUin", uin),
            ("format", "json"),
            ("inCharset", "utf8"),
            ("outCharset", "utf-8"),
            ("notice", "0"),
            ("platform", "yqq.json"),
            ("needNewCode", "0"),
        ];
        let headers = vec![("Referer", "https://y.qq.com/portal/profile.html")];
        match qq_get_json(QQ_PLAYLIST_CREATED_URL, &params, cookie).await {
            Ok(body) => {
                let rows = body
                    .get("data")
                    .and_then(|d| d.get("disslist"))
                    .and_then(|l| l.as_array())
                    .cloned()
                    .unwrap_or_default();
                let len = rows.len();
                out.extend(rows);
                if len < QQ_PLAYLIST_SYNC_PAGE_SIZE as usize {
                    break;
                }
            }
            Err(e) => {
                warn!("[QQPlaylist] created page {} failed: {e}", page);
                break;
            }
        }
    }
    out
}

/// 获取用户收藏的歌单
async fn fetch_collected_playlists(uin: &str, cookie: &str) -> Vec<Value> {
    let mut out = Vec::new();
    for page in 0..QQ_PLAYLIST_SYNC_MAX_PAGES {
        let sin = page * QQ_PLAYLIST_SYNC_PAGE_SIZE as usize;
        let ein = sin + QQ_PLAYLIST_SYNC_PAGE_SIZE as usize - 1;
        let sin_str = sin.to_string();
        let ein_str = ein.to_string();
        let params: Vec<(&str, &str)> = vec![
            ("ct", "20"),
            ("cid", "205360956"),
            ("userid", uin),
            ("reqtype", "3"),
            ("sin", &sin_str),
            ("ein", &ein_str),
        ];
        let headers = vec![("Referer", "https://y.qq.com/portal/profile.html")];
        match qq_get_json(QQ_PLAYLIST_COLLECTED_URL, &params, cookie).await {
            Ok(body) => {
                let data = body.get("data").unwrap_or(&Value::Null);
                let rows = data
                    .get("cdlist")
                    .or_else(|| data.get("v_playlist"))
                    .or_else(|| data.get("playlist"))
                    .or_else(|| data.get("disslist"))
                    .and_then(|l| l.as_array())
                    .cloned()
                    .unwrap_or_default();
                let len = rows.len();
                out.extend(rows);
                if len < QQ_PLAYLIST_SYNC_PAGE_SIZE as usize {
                    break;
                }
            }
            Err(e) => {
                warn!("[QQPlaylist] collected page {} failed: {e}", page);
                break;
            }
        }
    }
    out
}

/// 获取我喜欢歌单卡片
async fn get_liked_playlist_card(cookie: &str) -> Playlist {
    let auth = extract_qq_auth(cookie);
    if !auth.playback_ready {
        return Playlist {
            provider: PROVIDER.into(),
            id: QQ_LIKED_PLAYLIST_ID.into(),
            name: QQ_LIKED_PLAYLIST_NAME.into(),
            cover: QQ_LIKED_PLAYLIST_COVER.into(),
            track_count: 0,
            creator: if !auth.uin.is_empty() {
                auth.uin.clone()
            } else {
                "QQ 音乐".into()
            },
            subscribed: false,
        };
    }
    let payload = json!({
        "comm": { "ct": 24, "cv": 0 },
        "req_0": {
            "module": "music.srfDissInfo.DissInfo",
            "method": "CgiGetDiss",
            "param": {
                "disstid": 0,
                "dirid": QQ_LIKED_DIRID,
                "tag": 1,
                "song_begin": 0,
                "song_num": 1,
                "userinfo": 1,
                "orderlist": 1
            }
        }
    });
    let mut count = 0u32;
    if let Ok(json) = qq_musicu_request(&payload, cookie, 10000).await {
        let data = json
            .get("req_0")
            .and_then(|r| r.get("data"))
            .unwrap_or(&Value::Null);
        count = data
            .get("total_song_num")
            .and_then(|v| v.as_u64())
            .unwrap_or(0) as u32;
    }
    Playlist {
        provider: PROVIDER.into(),
        id: QQ_LIKED_PLAYLIST_ID.into(),
        name: QQ_LIKED_PLAYLIST_NAME.into(),
        cover: QQ_LIKED_PLAYLIST_COVER.into(),
        track_count: count,
        creator: if !auth.uin.is_empty() {
            auth.uin.clone()
        } else {
            "QQ 音乐".into()
        },
        subscribed: false,
    }
}

/// 获取用户歌单 (公开 API)
pub async fn user_playlists(cookie: &str) -> Result<Vec<Playlist>, String> {
    let auth = extract_qq_auth(cookie);
    if !auth.logged_in || auth.uin.is_empty() {
        warn!("[QQPlaylist] user_playlists: not logged in");
        return Ok(vec![]);
    }
    let uin = auth.uin.clone();
    info!(
        "[QQPlaylist] user_playlists: uin={}, logged_in={}, playback_ready={}",
        uin, auth.logged_in, auth.playback_ready
    );

    let created_cookie = cookie.to_string();
    let collected_cookie = cookie.to_string();
    let liked_cookie = cookie.to_string();

    let (created_raw, collect_raw, liked_card) = tokio::join!(
        fetch_created_playlists(&uin, &created_cookie),
        fetch_collected_playlists(&uin, &collected_cookie),
        get_liked_playlist_card(&liked_cookie),
    );

    info!(
        "[QQPlaylist] raw counts: created={}, collected={}, liked_card.id={}",
        created_raw.len(),
        collect_raw.len(),
        liked_card.id
    );

    let created: Vec<Playlist> = created_raw
        .iter()
        .map(|pl| map_qq_playlist(pl, "created"))
        .collect();
    let collected: Vec<Playlist> = collect_raw
        .iter()
        .map(|pl| map_qq_playlist(pl, "collect"))
        .collect();

    info!(
        "[QQPlaylist] mapped counts: created={}, collected={}",
        created.len(),
        collected.len()
    );

    let mut base: Vec<Playlist> = created
        .into_iter()
        .chain(collected.into_iter())
        .filter(|pl| pl.id != QQ_LIKED_PLAYLIST_ID)
        .collect();
    base.insert(0, liked_card);

    // 过滤重复
    let mut seen = std::collections::HashSet::new();
    let playlists: Vec<Playlist> = base
        .into_iter()
        .filter(|pl| {
            if pl.id.is_empty() || pl.name.is_empty() {
                return false;
            }
            if seen.contains(&pl.id) {
                debug!("[QQPlaylist] dedup removing duplicate id={}", pl.id);
                return false;
            }
            seen.insert(pl.id.clone());
            true
        })
        .collect();

    info!(
        "[QQPlaylist] user_playlists result: {} playlists",
        playlists.len()
    );
    Ok(playlists)
}

/// 获取歌单曲目 (公开 API)
pub async fn playlist_tracks(id: &str, cookie: &str) -> Result<(Playlist, Vec<Song>), String> {
    let pid = id.trim();
    if pid.is_empty() {
        return Ok((Playlist::default(), vec![]));
    }

    // 如果是我喜欢歌单
    if is_qq_liked_playlist_id(pid) {
        return liked_playlist_tracks(cookie).await;
    }

    // 旧版 CGI 歌单曲目接口
    playlist_tracks_legacy(pid, cookie).await
}

/// 旧版 CGI 歌单曲目接口
async fn playlist_tracks_legacy(pid: &str, cookie: &str) -> Result<(Playlist, Vec<Song>), String> {
    let auth = extract_qq_auth(cookie);
    let uin = if auth.uin.is_empty() {
        "0".to_string()
    } else {
        auth.uin.clone()
    };
    let params: Vec<(&str, &str)> = vec![
        ("type", "1"),
        ("utf8", "1"),
        ("disstid", pid),
        ("song_begin", "0"),
        ("song_num", "100"),
        ("loginUin", &uin),
        ("format", "json"),
        ("inCharset", "utf8"),
        ("outCharset", "utf-8"),
        ("notice", "0"),
        ("platform", "yqq.json"),
        ("needNewCode", "0"),
    ];
    let headers = vec![("Referer", "https://y.qq.com/n/yqq/playlist")];
    let body = qq_get_json(QQ_PLAYLIST_TRACKS_URL, &params, cookie).await?;

    let detail = body
        .get("cdlist")
        .and_then(|c| c.as_array())
        .and_then(|arr| arr.first())
        .cloned()
        .unwrap_or(Value::Null);

    let raw_tracks = detail
        .get("songlist")
        .and_then(|s| s.as_array())
        .cloned()
        .unwrap_or_default();

    let tracks: Vec<Song> = raw_tracks
        .iter()
        .map(|raw| map_qq_playlist_track(raw))
        .filter(|s| !s.name.is_empty() && (s.mid.is_some() || !s.id.is_empty()))
        .collect();

    let total = detail
        .get("total_song_num")
        .or_else(|| detail.get("songnum"))
        .or_else(|| detail.get("song_cnt"))
        .or_else(|| detail.get("song_count"))
        .and_then(|v| v.as_u64())
        .unwrap_or(0) as u32;

    let playlist = Playlist {
        provider: PROVIDER.into(),
        id: pid.to_string(),
        name: detail
            .get("dissname")
            .or_else(|| detail.get("diss_name"))
            .or_else(|| detail.get("name"))
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string(),
        cover: detail
            .get("logo")
            .or_else(|| detail.get("diss_cover"))
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string(),
        track_count: total,
        creator: "QQ 音乐".into(),
        subscribed: false,
    };

    Ok((playlist, tracks))
}

/// 获取我喜欢歌单的曲目
async fn liked_playlist_tracks(cookie: &str) -> Result<(Playlist, Vec<Song>), String> {
    let auth = extract_qq_auth(cookie);
    if !auth.playback_ready {
        return Ok((
            Playlist {
                provider: PROVIDER.into(),
                id: QQ_LIKED_PLAYLIST_ID.into(),
                name: QQ_LIKED_PLAYLIST_NAME.into(),
                cover: QQ_LIKED_PLAYLIST_COVER.into(),
                track_count: 0,
                creator: if !auth.uin.is_empty() {
                    auth.uin.clone()
                } else {
                    "QQ 音乐".into()
                },
                subscribed: false,
            },
            vec![],
        ));
    }

    let payload = json!({
        "comm": { "ct": 24, "cv": 0 },
        "req_0": {
            "module": "music.srfDissInfo.DissInfo",
            "method": "CgiGetDiss",
            "param": {
                "disstid": 0,
                "dirid": QQ_LIKED_DIRID,
                "tag": 1,
                "song_begin": 0,
                "song_num": 100,
                "userinfo": 1,
                "orderlist": 1
            }
        }
    });

    match qq_musicu_request(&payload, cookie, 10000).await {
        Ok(json) => {
            let block = json.get("req_0").unwrap_or(&Value::Null);
            let data = block.get("data").unwrap_or(&Value::Null);
            let raw_tracks = data
                .get("songlist")
                .and_then(|s| s.as_array())
                .cloned()
                .unwrap_or_default();
            let tracks: Vec<Song> = raw_tracks
                .iter()
                .map(|raw| map_qq_playlist_track(raw))
                .filter(|s| !s.name.is_empty() && (s.mid.is_some() || !s.id.is_empty()))
                .collect();
            let total = data
                .get("total_song_num")
                .and_then(|v| v.as_u64())
                .unwrap_or(0) as u32;

            Ok((
                Playlist {
                    provider: PROVIDER.into(),
                    id: QQ_LIKED_PLAYLIST_ID.into(),
                    name: QQ_LIKED_PLAYLIST_NAME.into(),
                    cover: QQ_LIKED_PLAYLIST_COVER.into(),
                    track_count: total,
                    creator: if !auth.uin.is_empty() {
                        auth.uin.clone()
                    } else {
                        "QQ 音乐".into()
                    },
                    subscribed: false,
                },
                tracks,
            ))
        }
        Err(e) => {
            warn!("[QQLiked] tracks failed: {e}");
            Ok((
                Playlist {
                    provider: PROVIDER.into(),
                    id: QQ_LIKED_PLAYLIST_ID.into(),
                    name: QQ_LIKED_PLAYLIST_NAME.into(),
                    cover: QQ_LIKED_PLAYLIST_COVER.into(),
                    track_count: 0,
                    creator: if !auth.uin.is_empty() {
                        auth.uin.clone()
                    } else {
                        "QQ 音乐".into()
                    },
                    subscribed: false,
                },
                vec![],
            ))
        }
    }
}

// ============================================================
//  排行榜
// ============================================================

/// QQ 音乐排行榜预设列表
const QQ_RANK_PRESETS: &[(&str, &str, &str)] = &[
    (
        "26",
        "巅峰榜·热歌",
        "https://y.gtimg.cn/music/photo_new/T003R300x300M000003kErY34CR2zg.jpg",
    ),
    (
        "27",
        "巅峰榜·新歌",
        "https://y.gtimg.cn/music/photo_new/T003R300x300M000003lDPtw0sKXJK.jpg",
    ),
    (
        "4",
        "巅峰榜·流行指数",
        "https://y.gtimg.cn/music/photo_new/T003R300x300M000002aA7GQ0YoZYe.jpg",
    ),
    (
        "62",
        "飙升榜",
        "https://y.gtimg.cn/music/photo_new/T003R300x300M000002DWfWl0cjhjP.jpg",
    ),
    (
        "5",
        "巅峰榜·内地",
        "https://y.gtimg.cn/music/photo_new/T003R300x300M000003V5yNn4TIRVP.jpg",
    ),
    (
        "6",
        "巅峰榜·港台",
        "https://y.gtimg.cn/music/photo_new/T003R300x300M000000CLXb916ik5d.jpg",
    ),
    (
        "105",
        "日本公信榜",
        "https://y.gtimg.cn/music/photo_new/T003R300x300M000003EpI9F1a0tFY.jpg",
    ),
    (
        "51",
        "巅峰榜·明日之子",
        "https://y.gtimg.cn/music/common/upload/iphone_order_channel/toplist_51_300_203451421.jpg",
    ),
    (
        "52",
        "巅峰榜·腾讯音乐人原创榜",
        "https://y.gtimg.cn/music/photo_new/T003R300x300M000000UTVyf0BnyCn.jpg",
    ),
    (
        "53",
        "机车",
        "https://y.gtimg.cn/music/photo_new/T003R300x300M000002nNMsB2S520Y.jpg",
    ),
];

/// 获取 QQ 音乐排行榜列表 (公开 API)
pub async fn rank_list() -> Result<Vec<Playlist>, String> {
    // 使用预设列表（官方榜单列表接口已失效）
    Ok(QQ_RANK_PRESETS
        .iter()
        .map(|(id, name, cover)| Playlist {
            provider: PROVIDER.into(),
            id: id.to_string(),
            name: name.to_string(),
            cover: cover.to_string(),
            track_count: 0,
            creator: "QQ 音乐".into(),
            subscribed: false,
        })
        .collect())
}

/// 获取排行榜歌曲 (公开 API)
pub async fn rank_songs(rank_id: &str, limit: u32, cookie: &str) -> Result<Vec<Song>, String> {
    let limit = limit.max(1) as usize;
    let mut songs = Vec::new();

    // CGI 端点 page=detail&topid=X
    let headers = vec![
        ("Referer", QQ_HEADERS_REFERER),
        ("User-Agent", QQ_HEADERS_UA),
    ];
    let url = format!(
        "https://c.y.qq.com/v8/fcg-bin/fcg_v8_toplist_cp.fcg?\
        g_tk=5381&loginUin=0&hostUin=0&inCharset=utf8&outCharset=utf-8&\
        notice=0&platform=yqq&needNewCode=0&\
        type=top&topid={}&format=json&tpl=3&page=detail",
        rank_id
    );

    info!("[QQRank] fetching songs from CGI, rank_id={}", rank_id);
    match request_text(&url, "GET", &headers, None, 10000).await {
        Ok(text) => {
            match parse_json_text(&text) {
                Ok(json) => {
                    let code = json.get("code").and_then(|v| v.as_i64()).unwrap_or(-1);
                    info!("[QQRank] CGI songs code={}, rank_id={}", code, rank_id);

                    // 尝试多种路径解析歌曲列表
                    let song_list = json
                        .get("data")
                        .and_then(|d| d.get("songInfoList"))
                        .and_then(|s| s.as_array())
                        .or_else(|| {
                            json.get("data")
                                .and_then(|d| d.get("song"))
                                .and_then(|s| s.get("songInfoList"))
                                .and_then(|s| s.as_array())
                        })
                        .or_else(|| {
                            json.get("data")
                                .and_then(|d| d.get("songlist"))
                                .and_then(|s| s.as_array())
                        })
                        .or_else(|| json.get("songlist").and_then(|s| s.as_array()));

                    if let Some(arr) = song_list {
                        songs = arr
                            .iter()
                            .take(limit)
                            .map(|item| {
                                let track = item
                                    .get("data")
                                    .or_else(|| item.get("songInfo"))
                                    .or_else(|| item.get("song"))
                                    .unwrap_or(item);
                                map_qq_track(track, &Song::default())
                            })
                            .filter(|s| !s.name.is_empty() && (s.mid.is_some() || !s.id.is_empty()))
                            .collect();
                        info!("[QQRank] CGI songs parsed {} songs", songs.len());
                    } else {
                        let preview =
                            serde_json::to_string(json.get("data").unwrap_or(&Value::Null))
                                .unwrap_or_default();
                        warn!(
                            "[QQRank] CGI songs empty, data: {}",
                            &preview[..preview.len().min(500)]
                        );
                    }
                }
                Err(e) => warn!("[QQRank] CGI songs parse failed: {e}"),
            }
        }
        Err(e) => warn!("[QQRank] CGI songs request failed: {e}"),
    }

    Ok(songs)
}

// ============================================================
//  推荐歌单
// ============================================================

/// QQ 音乐推荐歌单 (公开 API)
pub async fn recommend_playlists() -> Result<Vec<Playlist>, String> {
    let url = format!(
        "https://c.y.qq.com/splcloud/fcgi-bin/fcg_get_diss_by_tag.fcg?\
        g_tk=5381&loginUin=0&hostUin=0&inCharset=utf8&outCharset=utf-8&\
        notice=0&platform=yqq&needNewCode=0&\
        categoryId=10000000&sin=0&size=20&order=play&format=json"
    );
    let headers = vec![
        (REFERER.as_str(), QQ_HEADERS_REFERER),
        (USER_AGENT.as_str(), QQ_HEADERS_UA),
    ];
    let text = request_text(&url, "GET", &headers, None, 10000).await?;
    let json = parse_json_text(&text)?;
    let list = json
        .get("data")
        .and_then(|d| d.get("list"))
        .and_then(|l| l.as_array())
        .cloned()
        .unwrap_or_default();

    Ok(list
        .iter()
        .filter_map(|item| {
            let id = item
                .get("dissid")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string())
                .or_else(|| {
                    item.get("dissid")
                        .and_then(|v| v.as_i64())
                        .map(|n| n.to_string())
                })
                .unwrap_or_default();
            if id.is_empty() {
                return None;
            }
            let creator = item
                .get("creator")
                .map(|c| {
                    if let Some(s) = c.as_str() {
                        s.to_string()
                    } else {
                        c.get("name")
                            .and_then(|v| v.as_str())
                            .unwrap_or("")
                            .to_string()
                    }
                })
                .unwrap_or_default();
            Some(Playlist {
                provider: PROVIDER.into(),
                id,
                name: item
                    .get("dissname")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string(),
                cover: item
                    .get("imgurl")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string(),
                track_count: item
                    .get("songnum")
                    .and_then(|v| v.as_u64().or_else(|| v.as_i64().map(|n| n as u64)))
                    .map(|n| n as u32)
                    .unwrap_or(0),
                creator,
                subscribed: false,
            })
        })
        .collect())
}

// ═══════════════════════════════════════════════════════
//  歌词功能 (F-MU-19~24)
// ═══════════════════════════════════════════════════════

/// Base64 解码 QQ 歌词文本
fn decode_qq_lyric_text(encoded: &str) -> String {
    if encoded.is_empty() {
        return String::new();
    }
    // 尝试 base64 解码
    match base64::Engine::decode(&base64::engine::general_purpose::STANDARD, encoded) {
        Ok(bytes) => String::from_utf8_lossy(&bytes).to_string(),
        Err(_) => encoded.to_string(), // 如果不是 base64，直接返回原文本
    }
}

/// 获取歌词 (参考 NexBox lyric: music.musichallSong.PlayLyricInfo.GetPlayLyricInfo)
pub async fn lyric(mid: &str, id: &str, cookie: &str) -> Result<Lyrics, String> {
    let song_mid = mid.trim();
    let song_id: i64 = id.trim().parse::<i64>().unwrap_or(0);

    if song_mid.is_empty() && song_id == 0 {
        return Ok(Lyrics {
            lyric: String::new(),
            ..Default::default()
        });
    }

    let mut param = json!({});
    if !song_mid.is_empty() {
        param["songMID"] = json!(song_mid);
    }
    if song_id > 0 {
        param["songID"] = json!(song_id);
    }

    let payload = json!({
        "comm": { "ct": 24, "cv": 0 },
        "lyric": {
            "module": "music.musichallSong.PlayLyricInfo",
            "method": "GetPlayLyricInfo",
            "param": param
        }
    });

    let mut lyric_text = String::new();
    let mut trans_text = String::new();
    let mut roma_text = String::new();
    let mut qrc_text = String::new();

    match qq_musicu_request(&payload, cookie, 10000).await {
        Ok(json) => {
            let data = json
                .get("lyric")
                .and_then(|l| l.get("data"))
                .unwrap_or(&Value::Null);
            lyric_text =
                decode_qq_lyric_text(data.get("lyric").and_then(|v| v.as_str()).unwrap_or(""));
            trans_text =
                decode_qq_lyric_text(data.get("trans").and_then(|v| v.as_str()).unwrap_or(""));
            qrc_text = decode_qq_lyric_text(data.get("qrc").and_then(|v| v.as_str()).unwrap_or(""));
            roma_text =
                decode_qq_lyric_text(data.get("roma").and_then(|v| v.as_str()).unwrap_or(""));
        }
        Err(e) => {
            warn!("[QQLyric] musicu failed: {e}");
        }
    }

    // 降级到旧版 API
    if lyric_text.is_empty() && !song_mid.is_empty() {
        let login_uin = super::cookie::qq_extract_uin(cookie);
        let params: Vec<(&str, &str)> = vec![
            ("songmid", song_mid),
            ("songtype", "0"),
            ("format", "json"),
            ("nobase64", "1"),
            ("g_tk", "5381"),
            ("loginUin", &login_uin),
            ("hostUin", "0"),
            ("inCharset", "utf8"),
            ("outCharset", "utf-8"),
            ("notice", "0"),
            ("platform", "yqq.json"),
            ("needNewCode", "0"),
        ];
        match qq_get_json(QQ_LYRIC_LEGACY_URL, &params, cookie).await {
            Ok(body) => {
                lyric_text =
                    decode_qq_lyric_text(body.get("lyric").and_then(|v| v.as_str()).unwrap_or(""));
                if trans_text.is_empty() {
                    trans_text = decode_qq_lyric_text(
                        body.get("trans")
                            .or_else(|| body.get("tlyric"))
                            .and_then(|v| v.as_str())
                            .unwrap_or(""),
                    );
                }
            }
            Err(e) => {
                warn!("[QQLyric] legacy failed: {e}");
            }
        }
    }

    Ok(Lyrics {
        lyric: lyric_text,
        translation: if trans_text.is_empty() {
            None
        } else {
            Some(trans_text)
        },
        roma: if roma_text.is_empty() {
            None
        } else {
            Some(roma_text)
        },
        yrc: if qrc_text.is_empty() {
            None
        } else {
            Some(qrc_text)
        },
    })
}

// ═══════════════════════════════════════════════════════
//  喜欢功能 (F-MU-41)
// ═══════════════════════════════════════════════════════

/// 获取喜欢的歌曲 mid 列表
pub async fn liked_hashes(cookie: &str) -> Result<Vec<String>, String> {
    let auth = extract_qq_auth(cookie);
    if !auth.playback_ready {
        return Ok(vec![]);
    }

    let payload = json!({
        "comm": { "ct": 24, "cv": 0 },
        "req_0": {
            "module": "music.srfDissInfo.DissInfo",
            "method": "CgiGetDiss",
            "param": {
                "disstid": 0,
                "dirid": QQ_LIKED_DIRID,
                "tag": 1,
                "song_begin": 0,
                "song_num": 500,
                "userinfo": 1,
                "orderlist": 1
            }
        }
    });

    match qq_musicu_request(&payload, cookie, 10000).await {
        Ok(json) => {
            let block = json.get("req_0").unwrap_or(&Value::Null);
            let data = block.get("data").unwrap_or(&Value::Null);
            let raw_tracks = data
                .get("songlist")
                .and_then(|s| s.as_array())
                .cloned()
                .unwrap_or_default();
            let mids: Vec<String> = raw_tracks
                .iter()
                .filter_map(|raw| {
                    let track = raw
                        .get("track_info")
                        .or_else(|| raw.get("songInfo"))
                        .or_else(|| raw.get("songinfo"))
                        .or_else(|| raw.get("song"))
                        .unwrap_or(raw);
                    track
                        .get("mid")
                        .or_else(|| track.get("songmid"))
                        .and_then(|v| v.as_str())
                        .map(|s| s.to_string())
                })
                .collect();
            Ok(mids)
        }
        Err(e) => {
            warn!("[QQLiked] hashes failed: {e}");
            Ok(vec![])
        }
    }
}

/// 喜欢/取消喜欢歌曲
pub async fn like_toggle(song: &Song, like: bool, cookie: &str) -> Result<bool, String> {
    let auth = extract_qq_auth(cookie);
    info!(
        "[QQLike] toggle called: id={}, mid={:?}, like={}, playback_ready={}",
        song.id, song.mid, like, auth.playback_ready
    );

    if !auth.playback_ready {
        warn!("[QQLike] playback not ready, rejecting");
        return Err("QQ 音乐需要完整授权才能操作喜欢".into());
    }

    let mid = song.mid.as_deref().unwrap_or("");
    let song_id = song.qq_song_id.unwrap_or(0);
    info!("[QQLike] mid='{}', song_id={}", mid, song_id);

    if mid.is_empty() && song_id == 0 {
        return Err("Missing QQ song mid or id".into());
    }

    let mut param = json!({
        "dirId": QQ_LIKED_DIRID,
    });
    if song_id > 0 {
        param["songId"] = json!(song_id);
    }
    if !mid.is_empty() {
        param["songmid"] = json!(mid);
    }

    // 收藏/取消收藏歌曲到「我喜欢」
    let method = if like {
        "CgiCollectSong"
    } else {
        "CgiRemoveCollectSong"
    };
    let payload = json!({
        "comm": {
            "uin": auth.uin,
            "format": "json",
            "ct": 19,
            "cv": 0,
            "authst": auth.music_key
        },
        "req_0": {
            "module": "music.musicInfoCollection",
            "method": method,
            "param": param
        }
    });

    match qq_musicu_request(&payload, cookie, 10000).await {
        Ok(json) => {
            let code = json
                .get("req_0")
                .and_then(|r| r.get("code"))
                .and_then(|v| v.as_i64())
                .unwrap_or(-1);
            info!("[QQLike] API response code={}", code);
            if code == 0 {
                Ok(true)
            } else {
                let msg = format!("QQ API returned code {}", code);
                warn!("[QQLike] {}", msg);
                Err(msg)
            }
        }
        Err(e) => {
            warn!("[QQLike] toggle failed: {e}");
            Err(e)
        }
    }
}

// ═══════════════════════════════════════════════════════
//  登录信息
// ═══════════════════════════════════════════════════════

/// 获取 QQ 音乐登录信息
pub async fn login_info(cookie: &str) -> Result<LoginInfo, String> {
    let auth = extract_qq_auth(cookie);

    if !auth.logged_in || auth.uin.is_empty() {
        return Ok(LoginInfo {
            provider: PROVIDER.into(),
            ..Default::default()
        });
    }

    // 尝试获取用户详细信息
    let mut nickname = format!("QQ {}", auth.uin);
    let mut avatar = format!("https://q1.qlogo.cn/g?b=qq&nk={}&s=100", auth.uin);
    let mut is_vip = false;
    let mut is_svip = false;
    let mut vip_level = "none".to_string();
    let mut vip_type = 0;

    // 尝试从 profile 接口获取用户信息
    let payload = json!({
        "comm": { "ct": 24, "cv": 0 },
        "req_0": {
            "module": "userInfo.BaseUserInfoServer",
            "method": "cgi_get_user_profile",
            "param": { "uin": auth.uin.clone() }
        }
    });

    if let Ok(json) = qq_musicu_request(&payload, cookie, 5000).await {
        if let Some(data) = json.get("req_0").and_then(|r| r.get("data")) {
            if let Some(nick) = data.get("nick").and_then(|v| v.as_str()) {
                nickname = nick.to_string();
            }
            if let Some(avatar_url) = data.get("headurl").and_then(|v| v.as_str()) {
                if !avatar_url.is_empty() {
                    avatar = avatar_url.to_string();
                }
            }
            // VIP 信息
            if let Some(vip) = data.get("vip") {
                is_vip = vip.get("isVip").and_then(|v| v.as_bool()).unwrap_or(false);
                is_svip = vip.get("isSvip").and_then(|v| v.as_bool()).unwrap_or(false);
                vip_type = vip.get("vipType").and_then(|v| v.as_i64()).unwrap_or(0) as i32;
                vip_level = if is_svip {
                    "svip".to_string()
                } else if is_vip {
                    "vip".to_string()
                } else {
                    "none".to_string()
                };
            }
        }
    }

    info!(
        "[QQLogin] login_info success: uin={}, nickname={}",
        auth.uin, nickname
    );

    Ok(LoginInfo {
        provider: PROVIDER.into(),
        logged_in: true,
        user_id: auth.uin,
        nickname,
        avatar,
        vip_type,
        vip_level,
        is_vip,
        is_svip,
    })
}
