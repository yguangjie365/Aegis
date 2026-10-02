//! music 域命令（命名 `<域>_<动词>_<对象>`，01 编码规范；🔴 pub 命令禁放 crate 根，E0255）。
//!
//! 载荷统一走 `contracts::CmdEnvelope`（correlationId 串联日志）；
//! 错误返回 `Err(String)`（用户可读原因，禁静默失败；结构化错误模型随 W0-07 定档）。
#![allow(clippy::unused_async)] // Tauri command 约定：async 派发到异步池，命令面统一为将来含 await 留位

use super::library::{Library, MusicState};
use super::scan::spawn_scan;
use contracts::CmdEnvelope;
use serde::{Deserialize, Serialize};
use std::path::Path;
use tauri::{AppHandle, Manager, State};

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ImportRootPayload {
    path: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct LyricReadPayload {
    path: String,
}

/// 导入结果（F-MU-01：取消选择由前端侧发生，后端不会收到空路径）
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportRootResult {
    pub correlation_id: String,
    /// 本次是否新增（false = 同一目录已登记过，R-2 不重复登记）
    pub added: bool,
    pub roots: Vec<String>,
    /// 是否已触发扫描（已有扫描在跑时 false，前端须等待其完成）
    pub scan_started: bool,
}

/// 曲库快照（F-MU-07/08 数据出口）
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LibraryView {
    pub correlation_id: String,
    pub library: Library,
    pub scanning: bool,
    /// 曲库缓存降级警告（R-7：可见，不静默）
    pub cache_warning: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ScanAck {
    pub correlation_id: String,
    pub started: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LyricResult {
    pub correlation_id: String,
    /// 歌词原文；无同名歌词文件时为空串（F-MU-19 由界面给"未找到"文案）
    pub text: String,
}

pub(crate) fn payload_as<T: serde::de::DeserializeOwned>(
    envelope: &CmdEnvelope,
) -> Result<T, String> {
    let raw = envelope
        .payload
        .clone()
        .ok_or_else(|| "缺少命令载荷".to_string())?;
    serde_json::from_value(raw).map_err(|e| format!("载荷解析失败: {e}"))
}

/// asset 协议动态授权（MU-2 / Q-16）：把曲库根目录（含子目录）加入允许清单，
/// 前端 `convertFileSrc(song.path)` 才能经 Range/206 直读音频文件。
/// 🔴 比 `NexBox` 静态 `scope: ["**"]` 收紧——只放开用户自行登记的曲库根。
///
/// 授权失败不阻断导入流程（扫描/元数据不依赖它），但播放会 403，必须留日志。
pub fn allow_asset_root(app: &AppHandle, root: &str) {
    if let Err(e) = app
        .asset_protocol_scope()
        .allow_directory(Path::new(root), true)
    {
        eprintln!("[music] asset 协议授权失败 {root}: {e}");
    }
}

/// 导入音乐文件夹：登记根目录并触发扫描（F-MU-01）。目录不存在/非目录 → 明确报错。
///
/// # Errors
/// 载荷解析失败、所选路径不是有效文件夹时返回用户可读原因。
#[tauri::command]
pub async fn music_scan_import_root(
    app: AppHandle,
    state: State<'_, MusicState>,
    envelope: CmdEnvelope,
) -> Result<ImportRootResult, String> {
    let payload: ImportRootPayload = payload_as(&envelope)?;
    let path = Path::new(&payload.path);
    if !path.is_dir() {
        eprintln!("[music] 导入失败：不是有效目录 {}", payload.path);
        return Err("所选路径不是有效文件夹，请重新选择".to_string());
    }
    let canonical = match std::fs::canonicalize(path) {
        Ok(p) => p.to_string_lossy().to_string(),
        Err(_) => payload.path.clone(),
    };
    // 去掉 \\?\ 前缀（长路径/UNC 规范化产物，展示与匹配都不该带）
    let root = strip_verbatim_prefix(&canonical);
    let added = state.add_root(&root);
    allow_asset_root(&app, &root);
    let snapshot = state.snapshot();
    state.store_and_persist(&snapshot);
    let scan_started = if state.try_begin_scan() {
        spawn_scan(app.clone());
        true
    } else {
        false
    };
    Ok(ImportRootResult {
        correlation_id: envelope.correlation_id,
        added,
        roots: snapshot.roots,
        scan_started,
    })
}

/// 手动重扫曲库（F-MU-02"扫描曲库"入口）。已有扫描在跑时拒绝。
///
/// # Errors
/// 曲库未登记任何根目录时返回用户可读原因。
#[tauri::command]
pub async fn music_scan_start(
    app: AppHandle,
    state: State<'_, MusicState>,
    envelope: CmdEnvelope,
) -> Result<ScanAck, String> {
    if state.snapshot().roots.is_empty() {
        return Err("曲库还没有导入任何文件夹，无法扫描".to_string());
    }
    let started = state.try_begin_scan();
    if started {
        spawn_scan(app.clone());
    }
    Ok(ScanAck {
        correlation_id: envelope.correlation_id,
        started,
    })
}

/// 取消扫描（F-MU-03：部分保留语义在扫描线程收尾处实现）
///
/// # Errors
/// 当前没有进行中的扫描时返回用户可读原因。
#[tauri::command]
pub async fn music_scan_cancel(
    state: State<'_, MusicState>,
    envelope: CmdEnvelope,
) -> Result<ScanAck, String> {
    if !state.is_scanning() {
        return Err("当前没有进行中的扫描".to_string());
    }
    state.request_cancel();
    Ok(ScanAck {
        correlation_id: envelope.correlation_id,
        started: false,
    })
}

/// 读取曲库快照（进入模块/扫描完成后刷新）
///
/// # Errors
/// 本命令不校验输入，理论上恒成功；`Result` 形状保留以统一 command 契约。
#[tauri::command]
pub async fn music_library_get(
    state: State<'_, MusicState>,
    envelope: CmdEnvelope,
) -> Result<LibraryView, String> {
    Ok(LibraryView {
        correlation_id: envelope.correlation_id,
        library: state.snapshot(),
        scanning: state.is_scanning(),
        cache_warning: state.cache_warning(),
    })
}

/// 读取本地歌词（F-MU-19 本地来源）：同名外挂文本（`.lrc` 优先回退 `.txt`，移植 `NexBox`）→ 内嵌歌词（规格补写）→ 空文本。
///
/// # Errors
/// 载荷解析失败、或请求路径不在曲库登记曲目内（防任意路径读取）时返回用户可读原因。
#[tauri::command]
pub async fn music_lyric_read(
    state: State<'_, MusicState>,
    envelope: CmdEnvelope,
) -> Result<LyricResult, String> {
    let payload: LyricReadPayload = payload_as(&envelope)?;
    let audio_path = std::path::Path::new(&payload.path);
    // 只允许读曲库内已登记曲目（防任意路径读取）
    let known = state
        .snapshot()
        .songs
        .iter()
        .any(|s| s.path.eq_ignore_ascii_case(&payload.path));
    if !known {
        return Err("该曲目不在曲库中，拒绝读取歌词".to_string());
    }
    let text = match super::meta::find_local_lyric_path(audio_path) {
        Some(candidate) => std::fs::read_to_string(&candidate).unwrap_or_else(|error| {
            eprintln!("[music] 歌词文件读取失败 {}: {error}", candidate.display());
            String::new()
        }),
        // 同名外挂落空 → 内嵌歌词（F-MU-19 第二优先级）
        None => super::meta::read_embedded_lyric(audio_path).unwrap_or_default(),
    };
    Ok(LyricResult {
        correlation_id: envelope.correlation_id,
        text,
    })
}

fn strip_verbatim_prefix(path: &str) -> String {
    path.strip_prefix(r"\\?\UNC\")
        .map(|rest| format!(r"\\{rest}"))
        .or_else(|| {
            path.strip_prefix(r"\\?\")
                .map(std::borrow::ToOwned::to_owned)
        })
        .unwrap_or_else(|| path.to_string())
}
