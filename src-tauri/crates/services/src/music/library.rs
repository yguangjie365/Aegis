//! 曲库状态与持久缓存（规格 07 R-1/R-7：多根清单 + 曲库缓存，原子写、启动自愈）。

use super::meta::LocalSong;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex, PoisonError};

/// 扫描进度事件名（payload 见 `super::scan::ScanProgress`）
pub const SCAN_PROGRESS_EVENT: &str = "music:scan-progress";

/// 持久化的曲库快照（`<appData>/music/library.json`，原子写）
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Library {
    /// 曲库根清单（R-1 多值；按规范化路径登记，展示用）
    pub roots: Vec<String>,
    /// 曲目集合（id = 规范化绝对路径，R-4）
    pub songs: Vec<LocalSong>,
}

/// 缓存载入结果（F-MU-07 三态：首启 / 正常 / 损坏降级）
#[derive(Debug, Default)]
pub struct LoadOutcome {
    pub library: Library,
    /// 缓存缺失 = 首启（false 且无警告）；不可读/损坏 = true（界面须显警告）
    pub cache_warning: bool,
    /// 启动自愈剔除的"磁盘已消失"曲目数
    pub pruned: usize,
}

/// 音乐模块托管状态（src-tauri `setup` 里 `manage` + 载入缓存）
#[derive(Debug, Default)]
pub struct MusicState {
    library: Mutex<Library>,
    cache_warning: AtomicBool,
    scanning: AtomicBool,
    cancel: AtomicBool,
    data_dir: Mutex<Option<PathBuf>>,
}

impl MusicState {
    /// 新建空状态（尚未载入缓存）
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// 缓存文件路径：`<appData>/music/library.json`
    fn cache_path(data_dir: &Path) -> PathBuf {
        data_dir.join("music").join("library.json")
    }

    /// 从磁盘载入曲库并做启动自愈（剔除已消失文件）。返回剔除数与降级警告。
    pub fn load_from_disk(&self, app_data_dir: &Path) -> LoadOutcome {
        let mut outcome = LoadOutcome::default();
        *self.data_dir.lock().unwrap_or_else(PoisonError::into_inner) =
            Some(app_data_dir.to_path_buf());
        let file = Self::cache_path(app_data_dir);
        if !file.exists() {
            // 首启：空库、无警告（R-7）
            return outcome;
        }
        let loaded = std::fs::read_to_string(&file)
            .map_err(|e| format!("读取失败: {e}"))
            .and_then(|text| {
                serde_json::from_str::<Library>(&text).map_err(|e| format!("解析失败: {e}"))
            });
        let library = match loaded {
            Ok(library) => library,
            Err(reason) => {
                // 缓存不可读/损坏 = 空库 + 可见警告 + 留痕（R-7，禁静默）
                eprintln!("[music] 曲库缓存不可用，已重置——请重新扫描目录（{reason}）");
                outcome.cache_warning = true;
                Library::default()
            }
        };
        let (kept, pruned) = Self::prune_missing(library);
        outcome.pruned = pruned;
        outcome.library = kept;
        self.cache_warning
            .store(outcome.cache_warning, Ordering::Relaxed);
        *self.library.lock().unwrap_or_else(PoisonError::into_inner) = outcome.library.clone();
        outcome
    }

    fn prune_missing(library: Library) -> (Library, usize) {
        let mut pruned = 0usize;
        let songs = library
            .songs
            .into_iter()
            .filter(|song| {
                let exists = Path::new(&song.path).is_file();
                if !exists {
                    pruned += 1;
                }
                exists
            })
            .collect();
        (
            Library {
                roots: library.roots,
                songs,
            },
            pruned,
        )
    }

    /// 当前曲库快照（invoke 返回用）
    #[must_use]
    pub fn snapshot(&self) -> Library {
        self.library
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }

    /// 是否正在扫描
    #[must_use]
    pub fn is_scanning(&self) -> bool {
        self.scanning.load(Ordering::Relaxed)
    }

    /// 置位扫描中；已有扫描在跑时返回 false（调用方须拒绝并发扫描）
    #[must_use]
    pub fn try_begin_scan(&self) -> bool {
        !self.scanning.swap(true, Ordering::SeqCst)
    }

    /// 请求取消当前扫描（F-MU-03）
    pub fn request_cancel(&self) {
        if self.scanning.load(Ordering::Relaxed) {
            self.cancel.store(true, Ordering::SeqCst);
        }
    }

    #[must_use]
    pub fn cancel_requested(&self) -> bool {
        self.cancel.load(Ordering::Relaxed)
    }

    /// 缓存降级警告（F-MU-07 状态区展示）
    #[must_use]
    pub fn cache_warning(&self) -> bool {
        self.cache_warning.load(Ordering::Relaxed)
    }

    /// 写入/替换曲库并回写缓存（R-6 合并语义由调用方完成后传入）。
    /// 缓存写失败不改变内存态，但必须留痕（R-7 禁静默）。
    pub fn store_and_persist(&self, library: &Library) {
        *self.library.lock().unwrap_or_else(PoisonError::into_inner) = library.clone();
        let data_dir = self
            .data_dir
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone();
        let Some(data_dir) = data_dir else {
            eprintln!("[music] 缓存未落盘：data_dir 尚未初始化");
            return;
        };
        let file = Self::cache_path(&data_dir);
        let json = serde_json::to_string(library);
        match json {
            Ok(json) => {
                if let Err(error) = infra::fs::atomic_write(&file, json.as_bytes()) {
                    eprintln!("[music] 曲库缓存写入失败 {}: {error}", file.display());
                }
            }
            Err(error) => eprintln!("[music] 曲库缓存序列化失败: {error}"),
        }
    }

    /// 扫描结束复位（释放扫描锁 + 清取消位）
    pub fn end_scan(&self) {
        self.cancel.store(false, Ordering::SeqCst);
        self.scanning.store(false, Ordering::SeqCst);
    }

    /// 登记根目录（R-2：按完整路径大小写不敏感去重）。返回 true = 新增成功，false = 已存在。
    pub fn add_root(&self, root: &str) -> bool {
        let key = root.to_lowercase();
        let mut library = self.library.lock().unwrap_or_else(PoisonError::into_inner);
        if library.roots.iter().any(|r| r.to_lowercase() == key) {
            return false;
        }
        library.roots.push(root.to_string());
        true
    }
}
