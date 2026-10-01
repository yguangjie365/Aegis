//! 曲库扫描编排（规格 07 F-MU-02~05、R-2/R-3/R-6）：
//! 栈式递归收集 → 跨根路径去重 → 受限并行解析（NexBox scope+mpsc 形态移植）→
//! R-6 合并 → 原子回写缓存。可取消、部分保留；进度事件节流约 5 次/秒。

use super::library::{Library, MusicState, SCAN_PROGRESS_EVENT};
use super::meta::{build_local_song, cover_cache_dir, is_supported_extension, LocalSong};
use serde::Serialize;
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, PoisonError};
use tauri::{AppHandle, Emitter, Manager};

/// 扫描进度事件载荷（camelCase，前端状态行消费）
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ScanProgress {
    /// collecting（收集文件）/ parsing（解析元数据）/ done / cancelled / failed
    pub phase: &'static str,
    pub processed: usize,
    pub total: usize,
    /// 0-100；total 未知时为 0
    pub percent: u8,
    pub songs: usize,
    pub failed: usize,
    pub unreachable: usize,
}

fn emit_progress(app: &AppHandle, payload: &ScanProgress) {
    if let Err(error) = app.emit(SCAN_PROGRESS_EVENT, payload) {
        eprintln!("[music] 进度事件发送失败: {error}");
    }
}

/// 一次扫描的产物
#[derive(Debug, Default)]
struct ScanResult {
    songs: Vec<LocalSong>,
    /// 收集到但解析失败的绝对路径（R-6：失败 ≠ 已删除，须保留旧条目）
    failed_paths: Vec<String>,
    unreachable: usize,
    cancelled: bool,
}

/// 收集单个根下的音频文件（栈式遍历；R-3：单目录失败只计入不可达、不中断整棵；
/// 不跟随符号链接/联接点防成环；取消位随时中断）
fn collect_from_root(
    root: &Path,
    state: &MusicState,
    out: &mut Vec<String>,
    unreachable: &mut usize,
) {
    let mut stack: Vec<PathBuf> = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        if state.cancel_requested() {
            return;
        }
        let Ok(entries) = std::fs::read_dir(&dir) else {
            // 禁静默失败：不可达目录计数并留痕（F-MU-04）
            *unreachable += 1;
            eprintln!("[music] 目录不可达，已跳过: {}", dir.display());
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            let Ok(file_type) = entry.file_type() else {
                continue;
            };
            if file_type.is_symlink() {
                continue;
            }
            #[cfg(windows)]
            {
                // junction / 重解析点统一识别：FILE_ATTRIBUTE_REPARSE_POINT(0x400)。
                // std 稳定版无 `is_junction`，用元数据属性位代替（01 总览 §二：禁手写 Win32 绑定，此为标准库通道）。
                use std::os::windows::fs::MetadataExt as _;
                let is_reparse = entry
                    .metadata()
                    .is_ok_and(|m| m.file_attributes() & 0x400 != 0);
                if is_reparse {
                    continue;
                }
            }
            if file_type.is_dir() {
                stack.push(path);
            } else if file_type.is_file() {
                let ext = path
                    .extension()
                    .and_then(|v| v.to_str())
                    .map(str::to_lowercase)
                    .unwrap_or_default();
                if is_supported_extension(&ext) {
                    out.push(path.to_string_lossy().to_string());
                }
            }
        }
    }
}

/// 并行解析元数据（NexBox `import_local_music` 的 scope+mpsc 分片形态移植）。
/// worker 只做解析与发送；进度计数与节流在收集线程，避免跨线程可变闭包。
fn parse_songs(
    paths: &[String],
    state: &MusicState,
    cache_dir: Option<&Path>,
    app: &AppHandle,
) -> ScanResult {
    let total = paths.len();
    let workers = match std::thread::available_parallelism() {
        Ok(n) => n.get(),
        Err(_) => 4,
    };
    let folder_cover_cache: Arc<Mutex<HashMap<PathBuf, String>>> =
        Arc::new(Mutex::new(HashMap::new()));

    let mut result = ScanResult::default();
    std::thread::scope(|scope| {
        let (tx, rx) = std::sync::mpsc::channel::<Option<LocalSong>>();
        let chunk_size = total.div_ceil(workers).max(1);
        for chunk in paths.chunks(chunk_size) {
            let chunk = chunk.to_vec();
            let tx = tx.clone();
            let folder_cover_cache = Arc::clone(&folder_cover_cache);
            scope.spawn(move || {
                let mut local_cache = HashMap::new();
                for raw_path in chunk {
                    // F-MU-03 取消：worker 逐文件检查取消位，已扫到的照常产出（部分保留）
                    if state.cancel_requested() {
                        break;
                    }
                    let _ = tx.send(build_local_song(&raw_path, &mut local_cache, cache_dir));
                }
                let mut cache = folder_cover_cache
                    .lock()
                    .unwrap_or_else(PoisonError::into_inner);
                for (dir, cover) in local_cache {
                    if !cover.is_empty() {
                        cache.entry(dir).or_insert(cover);
                    }
                }
            });
        }
        drop(tx);

        // 收集线程：计数 + 节流刷新（约 5 次/秒，100% 必达，F-MU-02）
        let mut last_emit = std::time::Instant::now();
        for (index, item) in rx.into_iter().enumerate() {
            if let Some(song) = item {
                result.songs.push(song);
            } // None = 收集后文件消失/扩展名不符 = 正常缺席，不计失败
            let processed = index + 1;
            if processed == total || last_emit.elapsed().as_millis() >= 200 {
                last_emit = std::time::Instant::now();
                emit_progress(
                    app,
                    &ScanProgress {
                        phase: "parsing",
                        processed,
                        total,
                        percent: percent_of(processed, total),
                        songs: result.songs.len(),
                        failed: 0,
                        unreachable: 0,
                    },
                );
            }
        }
    });
    result.unreachable = 0;
    result.cancelled = state.cancel_requested();
    result
}

fn percent_of(processed: usize, total: usize) -> u8 {
    if total == 0 {
        return 0;
    }
    u8::try_from(processed.saturating_mul(100) / total).unwrap_or(100)
}

/// 启动一次全量后台扫描（导入根后自动触发 / 手动重扫共用）。
/// 同一时刻只允许一个扫描（调用方经 `try_begin_scan` 保证）；本函数立即返回。
pub fn spawn_scan(app: AppHandle) {
    std::thread::spawn(move || {
        let state = app.state::<MusicState>();
        let roots = state.snapshot().roots;
        let cache_root = app.path().app_cache_dir().ok();
        let cover_dir = cache_root.as_deref().map(cover_cache_dir);
        let cover_dir = cover_dir.as_deref();

        let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            run_scan(&state, &roots, cover_dir, &app)
        }));
        if let Ok(result) = outcome {
            finish_scan(&app, &state, result);
        } else {
            eprintln!("[music] 扫描线程 panic，已复位扫描锁");
            state.end_scan();
            emit_progress(
                &app,
                &ScanProgress {
                    phase: "failed",
                    processed: 0,
                    total: 0,
                    percent: 0,
                    songs: 0,
                    failed: 0,
                    unreachable: 0,
                },
            );
        }
    });
}

fn run_scan(
    state: &MusicState,
    roots: &[String],
    cache_dir: Option<&Path>,
    app: &AppHandle,
) -> ScanResult {
    // 阶段一：收集（跨根按路径小写去重，R-2）
    let mut paths: Vec<String> = Vec::new();
    let mut unreachable = 0usize;
    for root in roots {
        collect_from_root(Path::new(root), state, &mut paths, &mut unreachable);
        emit_progress(
            app,
            &ScanProgress {
                phase: "collecting",
                processed: paths.len(),
                total: 0,
                percent: 0,
                songs: 0,
                failed: 0,
                unreachable,
            },
        );
        if state.cancel_requested() {
            break;
        }
    }
    let mut seen: HashSet<String> = HashSet::with_capacity(paths.len());
    paths.retain(|p| seen.insert(p.to_lowercase()));

    // 阶段二：并行解析
    let mut result = parse_songs(&paths, state, cache_dir, app);
    result.unreachable = unreachable;
    let parsed: HashSet<String> = result.songs.iter().map(|s| s.path.to_lowercase()).collect();
    result.failed_paths = paths
        .iter()
        .filter(|p| !parsed.contains(&p.to_lowercase()) && Path::new(p).is_file())
        .cloned()
        .collect();
    result
}

fn finish_scan(app: &AppHandle, state: &MusicState, result: ScanResult) {
    let old = state.snapshot();
    let merged = if result.cancelled {
        // R-6 取消：旧库 + 本轮已扫到的合并保留，不覆盖式清空
        let mut by_id: HashMap<String, LocalSong> =
            old.songs.into_iter().map(|s| (s.id.clone(), s)).collect();
        for song in result.songs {
            by_id.insert(song.id.clone(), song);
        }
        Library {
            roots: old.roots,
            songs: by_id.into_values().collect(),
        }
    } else {
        // R-6 完整扫描：整体替换，但解析失败的文件按路径保留旧条目
        let new_paths: HashSet<String> =
            result.songs.iter().map(|s| s.path.to_lowercase()).collect();
        let failed: HashSet<String> = result
            .failed_paths
            .iter()
            .map(|p| p.to_lowercase())
            .collect();
        let mut songs = result.songs;
        songs.extend(old.songs.into_iter().filter(|s| {
            failed.contains(&s.path.to_lowercase()) && !new_paths.contains(&s.path.to_lowercase())
        }));
        Library {
            roots: old.roots,
            songs,
        }
    };
    let songs = merged.songs.len();
    let failed = result.failed_paths.len();
    let unreachable = result.unreachable;
    state.store_and_persist(&merged);
    state.end_scan();
    let phase = if unreachable > 0 && songs == 0 && failed == 0 {
        "failed"
    } else if result.cancelled {
        "cancelled"
    } else {
        "done"
    };
    emit_progress(
        app,
        &ScanProgress {
            phase,
            processed: songs,
            total: songs,
            percent: 100,
            songs,
            failed,
            unreachable,
        },
    );
    eprintln!("[music] 扫描{phase}：{songs} 首，失败 {failed}，目录不可达 {unreachable}");
}
