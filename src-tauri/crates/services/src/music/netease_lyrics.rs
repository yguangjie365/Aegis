#[derive(Default, Clone)]
pub struct Snapshot {
    pub current_lyric: Option<String>,
    pub song_title: Option<String>,
    pub song_artist: Option<String>,
}

#[cfg(not(target_os = "windows"))]
pub fn collect_snapshot() -> Snapshot {
    Snapshot::default()
}

#[cfg(target_os = "windows")]
mod imp {
    use std::cell::RefCell;
    use std::collections::HashSet;
    use std::time::Duration;

    // 🔴 EAPI 加密/请求头复用 `music_api::crypto`（2026-10-03 收敛重复实现）
    use super::super::music_api::crypto;

    use regex::Regex;
    use reqwest::blocking::Client;
    use reqwest::header::{COOKIE, REFERER, USER_AGENT};
    use serde_json::{json, Map, Value};
    use sysinfo::System;
    use windows::{
        core::{GUID, VARIANT},
        Media::Control::{
            GlobalSystemMediaTransportControlsSession,
            GlobalSystemMediaTransportControlsSessionManager,
            GlobalSystemMediaTransportControlsSessionPlaybackStatus,
        },
        Win32::{
            System::Com::{CoCreateInstance, CLSCTX_ALL},
            UI::Accessibility::{
                IUIAutomation, TreeScope_Children, TreeScope_Descendants,
                UIA_ControlTypePropertyId, UIA_ProcessIdPropertyId, UIA_TextControlTypeId,
            },
        },
    };

    use super::Snapshot;

    // 🔴 EAPI 密钥已随本地加密实现一并移除（2026-10-03），改用
    //    `music_api::crypto` 的公共实现，避免密钥常量在两处漂移。
    const NETEASE_USER_AGENT: &str = "Mozilla/5.0 (Linux; Android 9; PCT-AL10) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/70.0.3538.64 HuaweiBrowser/10.0.3.311 Mobile Safari/537.36";
    const SEARCH_API_URL: &str = "https://interface3.music.163.com/eapi/search/get";
    const SEARCH_API_PATH: &str = "/api/search/get";
    const LYRIC_API_URL: &str = "https://interface3.music.163.com/eapi/song/lyric/v1";
    const LYRIC_API_PATH: &str = "/api/song/lyric/v1";
    const MATCH_THRESHOLD: f64 = 0.60;
    const WINDOWS_TICK_PER_MILLISECOND: i64 = 10_000;
    const WINDOWS_EPOCH_OFFSET_TICKS: i64 = 116_444_736_000_000_000;

    #[derive(Clone)]
    struct LyricLine {
        time_ms: i64,
        text: String,
    }

    #[derive(Default)]
    struct CachedTrackLyrics {
        track_key: String,
        lines: Vec<LyricLine>,
    }

    pub fn collect_snapshot() -> Snapshot {
        thread_local! {
            static COLLECTOR: RefCell<Option<NeteaseLyricsCollector>> = const { RefCell::new(None) };
        }

        COLLECTOR.with(|slot| {
            let mut slot = slot.borrow_mut();
            if slot.is_none() {
                match NeteaseLyricsCollector::new() {
                    Ok(collector) => {
                        *slot = Some(collector);
                    }
                    Err(error) => {
                        log::warn!("初始化网易云歌词采集器失败: {error}");
                        return Snapshot::default();
                    }
                }
            }

            match slot.as_mut() {
                Some(collector) => collector.collect(),
                None => Snapshot::default(),
            }
        })
    }

    struct NeteaseLyricsCollector {
        session_manager: GlobalSystemMediaTransportControlsSessionManager,
        automation: IUIAutomation,
        http_client: Client,
        cached_track: Option<CachedTrackLyrics>,

        // Track local timer to avoid calling UIA continuously and for smooth interpolation
        last_uia_progress_ms: i64,
        last_uia_update_ticks: i64,
    }

    impl NeteaseLyricsCollector {
        fn new() -> Result<Self, String> {
            // Prefer CUIAutomation8 (Windows 8+) for better modern UI support
            const CUIAUTOMATION8_CLSID: GUID =
                GUID::from_u128(0xe22ad333_b25f_460c_83d0_0581107395c9);
            const CUIAUTOMATION_CLSID: GUID =
                GUID::from_u128(0xff48dba4_60ef_4201_aa87_54103eef594e);

            let session_manager = GlobalSystemMediaTransportControlsSessionManager::RequestAsync()
                .map_err(|error| error.to_string())?
                .get()
                .map_err(|error| error.to_string())?;

            let automation: IUIAutomation = unsafe {
                CoCreateInstance(&CUIAUTOMATION8_CLSID, None, CLSCTX_ALL)
                    .or_else(|_| CoCreateInstance(&CUIAUTOMATION_CLSID, None, CLSCTX_ALL))
                    .map_err(|error| error.to_string())?
            };

            let http_client = Client::builder()
                .timeout(Duration::from_secs(8))
                .build()
                .map_err(|error| error.to_string())?;

            Ok(Self {
                session_manager,
                automation,
                http_client,
                cached_track: None,
                last_uia_progress_ms: -1,
                last_uia_update_ticks: 0,
            })
        }

        fn collect(&mut self) -> Snapshot {
            let session = match self.session_manager.GetCurrentSession() {
                Ok(session) => session,
                Err(_) => {
                    self.cached_track = None;
                    return Snapshot::default();
                }
            };

            if !is_netease_session(&session) {
                self.cached_track = None;
                return Snapshot::default();
            }

            let media = match session
                .TryGetMediaPropertiesAsync()
                .and_then(|task| task.get())
            {
                Ok(media) => media,
                Err(error) => {
                    log::warn!("读取网易云媒体属性失败: {error}");
                    return Snapshot::default();
                }
            };

            let song_title = normalize_text(
                &media
                    .Title()
                    .map(|value| value.to_string())
                    .unwrap_or_default(),
            );
            let song_artist = normalize_text(
                &media
                    .Artist()
                    .map(|value| value.to_string())
                    .unwrap_or_default(),
            );

            if song_title.is_none() && song_artist.is_none() {
                self.cached_track = None;
                return Snapshot::default();
            }

            let track_key = build_track_key(song_title.as_deref(), song_artist.as_deref());
            let current_lyric = self.get_current_lyric_for_track(
                &track_key,
                song_title.as_deref().unwrap_or_default(),
                song_artist.as_deref().unwrap_or_default(),
                &session,
            );

            Snapshot {
                current_lyric,
                song_title,
                song_artist,
            }
        }

        fn get_current_lyric_for_track(
            &mut self,
            track_key: &str,
            song_title: &str,
            song_artist: &str,
            session: &GlobalSystemMediaTransportControlsSession,
        ) -> Option<String> {
            let needs_refresh = self
                .cached_track
                .as_ref()
                .map(|cache| cache.track_key != track_key)
                .unwrap_or(true);

            if needs_refresh {
                match self.fetch_track_lyrics(song_title, song_artist) {
                    Ok(lines) => {
                        log::info!(
                            "网易云歌词已加载: {} - {}，共 {} 行",
                            song_title,
                            song_artist,
                            lines.len()
                        );
                        self.cached_track = Some(CachedTrackLyrics {
                            track_key: track_key.to_string(),
                            lines,
                        });
                        self.last_uia_progress_ms = -1;
                        self.last_uia_update_ticks = 0;
                    }
                    Err(error) => {
                        log::warn!("获取网易云歌词失败: {error}");
                        self.cached_track = Some(CachedTrackLyrics {
                            track_key: track_key.to_string(),
                            lines: Vec::new(),
                        });
                    }
                }
            }

            let position_ms = self.get_accurate_position_ms(session);

            self.cached_track
                .as_ref()
                .and_then(|cache| lyric_at_position(&cache.lines, position_ms))
        }

        fn get_accurate_position_ms(
            &mut self,
            session: &GlobalSystemMediaTransportControlsSession,
        ) -> i64 {
            let now_ticks = current_windows_ticks();
            let mut result_ms = -1;

            // 为了避免卡顿，每隔 250ms 通过 UIA 抓取一次 UI 进度文本
            if now_ticks - self.last_uia_update_ticks > 250 * WINDOWS_TICK_PER_MILLISECOND {
                if let Some(uia_ms) = self.extract_progress_from_window() {
                    // C# 中是提取出精确的 seconds (例如 01:23 = 83 秒)
                    // 如果 UIA 读取的秒数变了，更新记录；否则维持平滑递增
                    let uia_sec = uia_ms / 1000;
                    let last_sec = self.last_uia_progress_ms / 1000;

                    if self.last_uia_progress_ms == -1 || uia_sec != last_sec {
                        self.last_uia_progress_ms = uia_ms;
                        self.last_uia_update_ticks = now_ticks;
                    }
                } else {
                    // Fallback to GSMTC extraction if UI reading fails
                    let gsmtc_ms = extract_position_ms(session).unwrap_or(0);
                    let gsmtc_sec = gsmtc_ms / 1000;
                    let last_sec = self.last_uia_progress_ms / 1000;
                    if self.last_uia_progress_ms == -1 || gsmtc_sec != last_sec {
                        self.last_uia_progress_ms = gsmtc_ms;
                        self.last_uia_update_ticks = now_ticks;
                    }
                }
            }

            // 平滑插值计算当前真实的播放进度 (ms)
            if self.last_uia_progress_ms >= 0 {
                let playback_status = session
                    .GetPlaybackInfo()
                    .ok()
                    .and_then(|info| info.PlaybackStatus().ok());
                if playback_status
                    == Some(GlobalSystemMediaTransportControlsSessionPlaybackStatus::Playing)
                {
                    let elapsed_ms =
                        (now_ticks - self.last_uia_update_ticks) / WINDOWS_TICK_PER_MILLISECOND;
                    result_ms = self.last_uia_progress_ms + elapsed_ms;
                } else {
                    result_ms = self.last_uia_progress_ms;
                }
            }

            if result_ms < 0 {
                0
            } else {
                result_ms
            }
        }

        fn extract_progress_from_window(&self) -> Option<i64> {
            let root = self.find_cloudmusic_window_uia()?;

            // 网易云主窗口内的文本控件
            let condition = unsafe {
                self.automation
                    .CreatePropertyCondition(
                        UIA_ControlTypePropertyId,
                        &VARIANT::from(UIA_TextControlTypeId.0 as i32),
                    )
                    .ok()?
            };
            let elements = unsafe { root.FindAll(TreeScope_Descendants, &condition).ok()? };
            let count = unsafe { elements.Length().ok()? };

            for index in 0..count {
                let element = match unsafe { elements.GetElement(index) } {
                    Ok(element) => element,
                    Err(_) => continue,
                };

                let name = unsafe { element.CurrentName().ok() };
                if let Some(text) = name {
                    let text_str = text.to_string();
                    if text_str.contains('/') || text_str.contains('|') {
                        if let Some((current_sec, _total_sec)) = parse_progress_text(&text_str) {
                            return Some(current_sec as i64 * 1000);
                        }
                    }
                }
            }

            None
        }

        fn find_cloudmusic_window_uia(
            &self,
        ) -> Option<windows::Win32::UI::Accessibility::IUIAutomationElement> {
            let mut system = System::new_all();
            system.refresh_processes();

            let process_ids = system
                .processes()
                .values()
                .filter_map(|process| {
                    let name = process.name().to_string().to_lowercase();
                    if name.contains("cloudmusic") {
                        Some(process.pid().as_u32())
                    } else {
                        None
                    }
                })
                .collect::<HashSet<_>>();

            if process_ids.is_empty() {
                return None;
            }

            let desktop = unsafe { self.automation.GetRootElement().ok()? };

            for pid in process_ids {
                let pid_condition = unsafe {
                    self.automation
                        .CreatePropertyCondition(
                            UIA_ProcessIdPropertyId,
                            &VARIANT::from(pid as i32),
                        )
                        .ok()?
                };
                let windows = unsafe { desktop.FindAll(TreeScope_Children, &pid_condition).ok()? };
                let count = unsafe { windows.Length().ok().unwrap_or(0) };

                for i in 0..count {
                    if let Ok(win) = unsafe { windows.GetElement(i) } {
                        if let Ok(name) = unsafe { win.CurrentName() } {
                            let title = name.to_string();
                            if title.contains(" - ") && !title.contains("MediaPlayer") {
                                return Some(win);
                            }
                        }
                    }
                }
            }

            None
        }

        fn fetch_track_lyrics(
            &self,
            song_title: &str,
            song_artist: &str,
        ) -> Result<Vec<LyricLine>, String> {
            let keyword = build_keyword(song_title, song_artist);
            let song_id = self.search_best_match(&keyword, song_title, song_artist)?;
            let response = self.post_eapi(
                LYRIC_API_URL,
                LYRIC_API_PATH,
                map_from_pairs([
                    ("id", json!(song_id)),
                    ("cp", json!("false")),
                    ("lv", json!("0")),
                    ("kv", json!("0")),
                    ("tv", json!("0")),
                    ("rv", json!("0")),
                    ("yv", json!("0")),
                    ("ytv", json!("0")),
                    ("yrv", json!("0")),
                    ("csrf_token", json!("")),
                ]),
            )?;

            if response.get("code").and_then(Value::as_i64) != Some(200) {
                return Err("网易云歌词接口返回失败".to_string());
            }

            let lrc = response
                .get("lrc")
                .and_then(Value::as_object)
                .and_then(|value| value.get("lyric"))
                .and_then(Value::as_str)
                .unwrap_or_default();

            if lrc.is_empty() || lrc.contains("纯音乐，请欣赏") {
                return Ok(Vec::new());
            }

            Ok(parse_lrc(lrc))
        }

        fn search_best_match(
            &self,
            keyword: &str,
            song_title: &str,
            song_artist: &str,
        ) -> Result<String, String> {
            let response = self.post_eapi(
                SEARCH_API_URL,
                SEARCH_API_PATH,
                map_from_pairs([
                    ("s", json!(keyword)),
                    ("limit", json!("5")),
                    ("offset", json!("0")),
                    ("type", json!("1")),
                    ("csrf_token", json!("")),
                ]),
            )?;

            if response.get("code").and_then(Value::as_i64) != Some(200) {
                return Err("网易云搜索接口返回失败".to_string());
            }

            let songs = response
                .get("result")
                .and_then(Value::as_object)
                .and_then(|value| value.get("songs"))
                .and_then(Value::as_array)
                .ok_or_else(|| "网易云搜索结果为空".to_string())?;

            let mut best_song_id: Option<String> = None;
            let mut best_score = 0.0f64;

            for song in songs {
                let cloud_title = song
                    .get("name")
                    .and_then(Value::as_str)
                    .map(normalize_owned)
                    .unwrap_or_default();
                let cloud_artist = song
                    .get("artists")
                    .and_then(Value::as_array)
                    .map(|artists| join_artist_names(artists))
                    .unwrap_or_default();

                let score = score_track_match(song_title, song_artist, &cloud_title, &cloud_artist);
                if score > best_score {
                    best_score = score;
                    best_song_id = extract_song_id(song);
                }
            }

            if best_score < MATCH_THRESHOLD {
                return Err(format!(
                    "未找到足够匹配的网易云歌曲，最高匹配度为 {:.0}%",
                    best_score * 100.0
                ));
            }

            best_song_id.ok_or_else(|| "网易云搜索结果缺少歌曲 ID".to_string())
        }

        fn post_eapi(
            &self,
            url: &str,
            api_path: &str,
            mut payload: Map<String, Value>,
        ) -> Result<Value, String> {
            let header_payload = crypto::build_eapi_header();
            payload.insert(
                "header".to_string(),
                Value::String(
                    serde_json::to_string(&header_payload).map_err(|error| error.to_string())?,
                ),
            );

            let payload_text =
                serde_json::to_string(&payload).map_err(|error| error.to_string())?;
            let encrypted = crypto::encrypt_eapi_payload(api_path, &payload_text)?;
            let cookie = header_payload
                .iter()
                .map(|(key, value)| format!("{key}={value}"))
                .collect::<Vec<_>>()
                .join("; ");

            let response_text = self
                .http_client
                .post(url)
                .header(USER_AGENT, NETEASE_USER_AGENT)
                .header(REFERER, "https://music.163.com/")
                .header(COOKIE, cookie)
                .form(&[("params", encrypted)])
                .send()
                .and_then(|response| response.error_for_status())
                .map_err(|error| error.to_string())?
                .text()
                .map_err(|error| error.to_string())?;

            serde_json::from_str(&response_text).map_err(|error| error.to_string())
        }
    }

    /// 解析进度字符串，如 "01:23 / 04:56"
    fn parse_progress_text(text: &str) -> Option<(i32, i32)> {
        let cleaned = text.replace(" ", "");
        let parts: Vec<&str> = if cleaned.contains('/') {
            cleaned.split('/').collect()
        } else {
            cleaned.split('|').collect()
        };

        if parts.len() != 2 {
            return None;
        }

        let current = parse_time_string(parts[0])?;
        let total = parse_time_string(parts[1])?;

        if total <= 0 || current < 0 || current > total + 2 {
            return None;
        }

        Some((current, total))
    }

    fn parse_time_string(time_str: &str) -> Option<i32> {
        let parts: Vec<&str> = time_str.split(':').collect();
        if parts.len() != 2 {
            return None;
        }

        let minutes = parts[0].parse::<i32>().ok()?;
        let seconds = parts[1].parse::<i32>().ok()?;

        if minutes < 0 || seconds < 0 || seconds >= 60 {
            return None;
        }

        Some(minutes * 60 + seconds)
    }

    fn extract_position_ms(session: &GlobalSystemMediaTransportControlsSession) -> Option<i64> {
        let timeline = session.GetTimelineProperties().ok()?;
        let mut position_ticks = timeline.Position().ok()?.Duration.max(0);
        let last_updated_ticks = timeline.LastUpdatedTime().ok()?.UniversalTime;

        let playback_status = session
            .GetPlaybackInfo()
            .ok()
            .and_then(|info| info.PlaybackStatus().ok());

        if playback_status == Some(GlobalSystemMediaTransportControlsSessionPlaybackStatus::Playing)
        {
            let now_ticks = current_windows_ticks();
            if now_ticks > last_updated_ticks {
                position_ticks += now_ticks - last_updated_ticks;
            }
        }

        Some((position_ticks / WINDOWS_TICK_PER_MILLISECOND).max(0))
    }

    fn current_windows_ticks() -> i64 {
        chrono::Utc::now().timestamp_millis() * WINDOWS_TICK_PER_MILLISECOND
            + WINDOWS_EPOCH_OFFSET_TICKS
    }

    fn build_track_key(song_title: Option<&str>, song_artist: Option<&str>) -> String {
        match (song_title, song_artist) {
            (Some(title), Some(artist)) if !artist.is_empty() => format!("{title} - {artist}"),
            (Some(title), _) => title.to_string(),
            (None, Some(artist)) => artist.to_string(),
            (None, None) => String::new(),
        }
    }

    fn build_keyword(song_title: &str, song_artist: &str) -> String {
        if song_artist.is_empty() {
            song_title.to_string()
        } else {
            format!("{song_title} - {song_artist}")
        }
    }

    fn normalize_text(value: &str) -> Option<String> {
        let normalized = normalize_owned(value);
        if normalized.is_empty() {
            None
        } else {
            Some(normalized)
        }
    }

    fn normalize_owned(value: &str) -> String {
        value
            .split_whitespace()
            .collect::<Vec<_>>()
            .join(" ")
            .trim()
            .to_string()
    }

    fn is_netease_session(session: &GlobalSystemMediaTransportControlsSession) -> bool {
        let source = session
            .SourceAppUserModelId()
            .map(|value| value.to_string().to_lowercase())
            .unwrap_or_default();

        source.contains("cloudmusic")
            || source.contains("netease")
            || source.contains("music.163")
            || source.contains("163music")
    }

    // 🔴 以下 4 个 EAPI 加密实现已于 2026-10-03 删除，改为复用
    //    `music_api::crypto` 的公共实现（本文件原为逐字副本，密钥常量
    //    `NETEASE_EAPI_KEY` 与算法重复两份，改一处漏一处有漂移风险）：
    //      - md5_bytes / aes_ecb_encrypt_pkcs7 / encrypt_eapi_payload → `crypto::*`
    //      - build_eapi_header                              → `crypto::build_eapi_header`
    //    保留位置仅作行为说明，实际调用见上方 `fetch_lyrics` 段。

    fn join_artist_names(artists: &[Value]) -> String {
        artists
            .iter()
            .filter_map(|artist| artist.get("name").and_then(Value::as_str))
            .map(normalize_owned)
            .filter(|name| !name.is_empty())
            .collect::<Vec<_>>()
            .join(" / ")
    }

    fn extract_song_id(song: &Value) -> Option<String> {
        if let Some(id) = song.get("id").and_then(Value::as_i64) {
            return Some(id.to_string());
        }

        song.get("id")
            .and_then(Value::as_str)
            .map(|value| value.to_string())
    }

    fn score_track_match(
        local_title: &str,
        local_artist: &str,
        cloud_title: &str,
        cloud_artist: &str,
    ) -> f64 {
        let local_title = normalize_compare_text(local_title);
        let local_artist = normalize_compare_text(local_artist);
        let cloud_title = normalize_compare_text(cloud_title);
        let cloud_artist = normalize_compare_text(cloud_artist);

        let title_score = similarity_score(
            &extract_base_title(&local_title),
            &extract_base_title(&cloud_title),
        );
        if title_score < 0.45 {
            return title_score * 0.5;
        }

        let artist_score = if local_artist.is_empty() || cloud_artist.is_empty() {
            0.6
        } else {
            similarity_score(&local_artist, &cloud_artist)
        };

        title_score * 0.72 + artist_score * 0.28
    }

    fn normalize_compare_text(value: &str) -> String {
        let lower = value.to_lowercase();
        let unified = lower
            .replace('（', "(")
            .replace('）', ")")
            .replace('【', "(")
            .replace('】', ")")
            .replace('[', "(")
            .replace(']', ")")
            .replace('「', "(")
            .replace('」', ")");

        unified.split_whitespace().collect::<Vec<_>>().join(" ")
    }

    fn extract_base_title(value: &str) -> String {
        let mut depth = 0u32;
        let mut output = String::new();

        for ch in value.chars() {
            match ch {
                '(' => depth += 1,
                ')' => depth = depth.saturating_sub(1),
                _ if depth == 0 => output.push(ch),
                _ => {}
            }
        }

        output
            .split_whitespace()
            .collect::<Vec<_>>()
            .join(" ")
            .trim()
            .to_string()
    }

    fn similarity_score(left: &str, right: &str) -> f64 {
        if left.is_empty() || right.is_empty() {
            return 0.0;
        }
        if left == right {
            return 1.0;
        }
        if left.contains(right) || right.contains(left) {
            let short = left.len().min(right.len()) as f64;
            let long = left.len().max(right.len()) as f64;
            return 0.75 + (short / long) * 0.25;
        }

        let distance = levenshtein(left, right) as f64;
        let max_len = left.chars().count().max(right.chars().count()) as f64;
        (1.0 - distance / max_len).max(0.0)
    }

    fn levenshtein(left: &str, right: &str) -> usize {
        let left_chars = left.chars().collect::<Vec<_>>();
        let right_chars = right.chars().collect::<Vec<_>>();
        let mut prev = (0..=right_chars.len()).collect::<Vec<_>>();
        let mut curr = vec![0; right_chars.len() + 1];

        for (i, left_char) in left_chars.iter().enumerate() {
            curr[0] = i + 1;
            for (j, right_char) in right_chars.iter().enumerate() {
                let cost = usize::from(left_char != right_char);
                curr[j + 1] = (prev[j + 1] + 1).min(curr[j] + 1).min(prev[j] + cost);
            }
            prev.clone_from(&curr);
        }

        prev[right_chars.len()]
    }

    fn parse_lrc(lrc: &str) -> Vec<LyricLine> {
        let tag_regex = Regex::new(r"\[(\d{1,2}):(\d{1,2})(?:[.:](\d{1,3}))?]").unwrap();
        let mut lines = Vec::new();

        for raw_line in lrc.lines() {
            let text = normalize_owned(tag_regex.replace_all(raw_line, "").trim());
            if text.is_empty() {
                continue;
            }

            for capture in tag_regex.captures_iter(raw_line) {
                let minutes = capture
                    .get(1)
                    .and_then(|value| value.as_str().parse::<i64>().ok())
                    .unwrap_or(0);
                let seconds = capture
                    .get(2)
                    .and_then(|value| value.as_str().parse::<i64>().ok())
                    .unwrap_or(0);
                let fraction = capture.get(3).map(|value| value.as_str()).unwrap_or("0");
                let millis = parse_fraction_to_millis(fraction);
                let time_ms = minutes * 60_000 + seconds * 1000 + millis;
                lines.push(LyricLine {
                    time_ms,
                    text: text.clone(),
                });
            }
        }

        lines.sort_by_key(|line| line.time_ms);
        lines.dedup_by(|left, right| left.time_ms == right.time_ms && left.text == right.text);
        lines
    }

    fn parse_fraction_to_millis(fraction: &str) -> i64 {
        match fraction.len() {
            0 => 0,
            1 => fraction.parse::<i64>().unwrap_or(0) * 100,
            2 => fraction.parse::<i64>().unwrap_or(0) * 10,
            _ => fraction[..3].parse::<i64>().unwrap_or(0),
        }
    }

    fn lyric_at_position(lines: &[LyricLine], position_ms: i64) -> Option<String> {
        if lines.is_empty() {
            return None;
        }

        let index = lines.partition_point(|line| line.time_ms <= position_ms);
        if index == 0 {
            return None;
        }

        Some(lines[index - 1].text.clone())
    }

    fn map_from_pairs<const N: usize>(pairs: [(&str, Value); N]) -> Map<String, Value> {
        let mut map = Map::new();
        for (key, value) in pairs {
            map.insert(key.to_string(), value);
        }
        map
    }

    // 🔴 2026-10-03 清理：`mod hex`（EAPI 的 encode / encode_upper / encode_lower /
    // encode_upper_slice）随 M-3「4 个 EAPI 函数改调 music_api::crypto」整体删除——
    // EAPI 签名本段不用，留着只会产生 dead_code 警告并误导「EAPI 仍启用」。
    // 若日后要恢复 EAPI 路径，须同时补 `services::music::music_api::crypto` 的调用。
}

#[cfg(target_os = "windows")]
pub use imp::collect_snapshot;
