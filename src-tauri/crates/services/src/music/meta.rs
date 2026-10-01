//! 本地曲目元数据与封面提取（移植自 `NexBox` `src-tauri/src/music.rs`，GPL-3.0 同源衍生）。
//!
//! 适配点（相对源实现）：字段按规格 07 F-MU-06 补全（专辑艺术家/流派/音轨/年份/码率/
//! 采样率/声道/修改时间/内嵌封面与歌词标记）；R-5 兜底在整理环节一次完成；
//! 日志暂走 stderr 单行（八字段 JSONL 总线随 W0-10 接线）。

use image::{GenericImageView, ImageEncoder};
use lofty::file::{AudioFile, TaggedFileExt};
use lofty::tag::{Accessor, ItemKey};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// 支持的本地音频扩展名（与 `NexBox` 同口径）
pub const SUPPORTED_AUDIO_EXTENSIONS: [&str; 11] = [
    "mp3", "wav", "ogg", "m4a", "flac", "aac", "opus", "wma", "aiff", "ape", "oga",
];

/// 检查某个扩展名是否受支持（传入时已去掉点号并小写）
#[must_use]
pub fn is_supported_extension(ext: &str) -> bool {
    SUPPORTED_AUDIO_EXTENSIONS.contains(&ext)
}

/// 本地曲库单曲条目（provider = local；规格 07 §五 数据视图的落点）
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LocalSong {
    /// 稳定唯一 ID = 规范化绝对路径（R-4：不随内容改动变化）
    pub id: String,
    /// 展示名 = 文件名去扩展名（缺失兜底链的起点）
    pub name: String,
    pub path: String,
    pub size: u64,
    pub extension: String,
    pub title: String,
    pub artist: String,
    pub album: String,
    pub album_artist: String,
    pub genre: String,
    pub track: u32,
    pub year: u32,
    pub duration_ms: u64,
    pub bitrate: u32,
    pub sample_rate: u32,
    pub channels: u8,
    /// 文件修改时间（unix 秒；0 = 未知）
    pub modified_at: u64,
    /// 封面缓存文件绝对路径（可能为空）
    pub cover_path: String,
    /// 封面来源："embedded" 内嵌 / "folder" 同目录图片 / "none" 无
    pub cover_source: String,
    pub has_embedded_cover: bool,
    pub has_embedded_lyric: bool,
    /// 是否存在同目录同名歌词文件（.lrc 优先，回退 .txt）
    pub has_lyric: bool,
}

/// 封面缓存目录：应用缓存目录下的 covers 子目录
#[must_use]
pub fn cover_cache_dir(cache_root: &Path) -> PathBuf {
    cache_root.join("covers")
}

/// 稳定的 FNV-1a 64 位哈希：保证同一输入在多次进程运行中得到相同结果。
/// 不能用 DefaultHasher（随机种子导致跨会话 hash 变化、重启后缓存路径失效）。
fn stable_hash(s: &str) -> u64 {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for byte in s.as_bytes() {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x0100_0000_01b3);
    }
    hash
}

/// 把压缩后的封面数据写入缓存目录，返回文件绝对路径。
/// 文件已存在且长度一致时跳过写入直接返回路径，支持重复导入去重；
/// 长度不一致视为半截/损坏文件，覆盖重写以自修复。
fn save_cover_file(cache_dir: &Path, id: &str, data: &[u8], mime: &str) -> String {
    if data.is_empty() {
        return String::new();
    }
    let ext = match mime {
        "image/png" => "png",
        "image/gif" => "gif",
        "image/bmp" => "bmp",
        "image/webp" => "webp",
        "image/avif" => "avif",
        _ => "jpg",
    };
    let name = format!("{:016x}.{ext}", stable_hash(id));
    let file_path = cache_dir.join(name);
    // 缓存根由 Tauri 运行期给出，covers 子目录不保证存在（全新安装/手工清理后即为缺失态），写入前补齐
    if !cache_dir.exists() {
        if let Err(err) = std::fs::create_dir_all(cache_dir) {
            eprintln!(
                "[music] 封面缓存目录创建失败 {}: {err}",
                cache_dir.display()
            );
            return String::new();
        }
    }
    let intact = std::fs::metadata(&file_path)
        .is_ok_and(|meta| meta.len() == u64::try_from(data.len()).unwrap_or(u64::MAX));
    if !intact {
        if let Err(error) = std::fs::write(&file_path, data) {
            eprintln!("[music] 封面缓存写入失败 {}: {error}", file_path.display());
            return String::new();
        }
    }
    file_path.to_string_lossy().to_string()
}

/// 常见的同目录专辑封面文件名（小写）
const FOLDER_COVER_NAMES: [&str; 8] = [
    "cover.jpg",
    "folder.jpg",
    "album.jpg",
    "front.jpg",
    "cover.png",
    "folder.png",
    "album.png",
    "front.png",
];

/// 在某个目录下查找同目录封面图片，写入封面缓存并返回缓存文件路径。
/// 缓存 key 使用目录路径，同一目录的歌曲共享同一张通用封面缓存文件。
fn find_folder_generic_cover(dir: &Path, cache_dir: &Path) -> String {
    let key = dir.to_string_lossy().to_string();
    for name in FOLDER_COVER_NAMES {
        let candidate = dir.join(name);
        if candidate.is_file() {
            let cover = save_cover_from_file(cache_dir, &key, &candidate);
            if !cover.is_empty() {
                return cover;
            }
        }
    }
    String::new()
}

/// 从音频文件提取封面并写入缓存目录：内嵌封面优先，回退同目录图片。
/// 返回 `(cover_path, cover_source, has_embedded_cover)`。
///
/// `folder_cover_cache`：同目录通用封面缓存（目录 → 封面缓存文件路径），避免对同一目录下
/// 每首歌重复磁盘探测与写入。`cache_dir` 为 None 时降级为不提取封面（不阻塞主流程）。
fn extract_song_cover(
    path: &Path,
    folder_cover_cache: &mut HashMap<PathBuf, String>,
    cache_dir: Option<&Path>,
    id: &str,
) -> (String, String, bool) {
    let mut cover_path = String::new();
    let mut cover_source = "none".to_string();
    let mut has_embedded_cover = false;

    // 1. 内嵌封面：必须先压缩再判结果——高清内嵌封面（无损 FLAC 常见 >1MB）压缩后仅数十 KB，
    //    按原始字节拦截会导致大封面全部丢失（NexBox 历史缺陷教训，原样保留）。
    if let Ok(tagged_file) = lofty::read_from_path(path) {
        for t in tagged_file.tags() {
            for pic in t.pictures() {
                let data = pic.data();
                has_embedded_cover = true;
                // 仅拦截空数据与极端异常的巨型 blob（防御损坏数据拖垮解码器），正常高清封面放行
                if data.is_empty() || data.len() > COVER_INPUT_MAX_BYTES {
                    continue;
                }
                let mime = detect_image_mime(data);
                let (data, mime) = downscale_cover(data, mime);
                if let Some(cache_dir) = cache_dir {
                    cover_path = save_cover_file(cache_dir, id, &data, &mime);
                }
                if !cover_path.is_empty() {
                    cover_source = "embedded".to_string();
                    break;
                }
            }
            if !cover_path.is_empty() {
                break;
            }
        }
    }

    // 2. 兜底：无内嵌封面时读同目录图片（固定封面名 → 与音频同名的图片）
    if cover_path.is_empty() {
        if let (Some(dir), Some(cache_dir)) = (path.parent(), cache_dir) {
            if !folder_cover_cache.contains_key(dir) {
                let cached = find_folder_generic_cover(dir, cache_dir);
                folder_cover_cache.insert(dir.to_path_buf(), cached);
            }
            let generic = folder_cover_cache.get(dir).cloned().unwrap_or_default();
            if !generic.is_empty() {
                cover_path = generic;
                cover_source = "folder".to_string();
            } else if let Some(stem) = path.file_stem().and_then(|v| v.to_str()) {
                for ext in ["jpg", "png", "jpeg", "webp", "bmp"] {
                    let candidate = dir.join(format!("{stem}.{ext}"));
                    if candidate.is_file() {
                        cover_path = save_cover_from_file(
                            cache_dir,
                            &candidate.to_string_lossy(),
                            &candidate,
                        );
                        if !cover_path.is_empty() {
                            cover_source = "folder".to_string();
                            break;
                        }
                    }
                }
            }
        }
    }

    (cover_path, cover_source, has_embedded_cover)
}

/// 读取单个音频文件的内嵌元数据并组装成 `LocalSong`（R-5 兜底在此一次完成）。
/// 返回 None 表示该路径不可入库（不存在/扩展名不支持）；标签读取失败不丢弃条目。
#[must_use]
// 调用方固定使用标准 SipHash 构建的缓存表，无需泛化哈希器
#[allow(clippy::implicit_hasher)]
pub fn build_local_song(
    raw_path: &str,
    folder_cover_cache: &mut HashMap<PathBuf, String>,
    cache_dir: Option<&Path>,
) -> Option<LocalSong> {
    let path = Path::new(raw_path);
    if !path.is_file() {
        return None;
    }
    let extension = path
        .extension()
        .and_then(|value| value.to_str())
        .map(str::to_lowercase)
        .unwrap_or_default();
    if !is_supported_extension(&extension) {
        return None;
    }

    let name = path
        .file_stem()
        .and_then(|value| value.to_str())
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .unwrap_or_default();

    let size = std::fs::metadata(path).map_or(0, |meta| meta.len());
    let modified_at = std::fs::metadata(path)
        .ok()
        .and_then(|meta| meta.modified().ok())
        .and_then(|time| time.duration_since(std::time::UNIX_EPOCH).ok())
        .map_or(0, |d| d.as_secs());

    // 以规范化绝对路径作为稳定 ID（R-4），不同位置的同名文件可区分
    let id = match std::fs::canonicalize(path) {
        Ok(canon) => canon.to_string_lossy().to_string(),
        Err(_) => raw_path.to_string(),
    };

    let mut title = String::new();
    let mut artist = String::new();
    let mut album = String::new();
    let mut album_artist = String::new();
    let mut genre = String::new();
    let mut track: u32 = 0;
    let mut year: u32 = 0;
    let mut duration_ms: u64 = 0;
    let mut bitrate: u32 = 0;
    let mut sample_rate: u32 = 0;
    let mut channels: u8 = 0;
    let mut has_embedded_lyric = false;

    if let Ok(tagged_file) = lofty::read_from_path(path) {
        let props = tagged_file.properties();
        duration_ms = props.duration().as_millis().try_into().unwrap_or(u64::MAX);
        bitrate = props.audio_bitrate().unwrap_or(0);
        sample_rate = props.sample_rate().unwrap_or(0);
        channels = props.channels().unwrap_or(0);
        let tag = tagged_file
            .primary_tag()
            .or_else(|| tagged_file.first_tag());
        if let Some(tag) = tag {
            title = tag.title().map_or_else(String::new, |v| v.to_string());
            artist = tag.artist().map_or_else(String::new, |v| v.to_string());
            album = tag.album().map_or_else(String::new, |v| v.to_string());
            // lofty 0.21 的 Tag 无 album_artist/lyrics 便捷方法，走 ItemKey 通道
            album_artist = tag
                .get_string(&ItemKey::AlbumArtist)
                .unwrap_or_default()
                .to_string();
            genre = tag.genre().map_or_else(String::new, |v| v.to_string());
            track = tag.track().unwrap_or(0);
            year = tag.year().unwrap_or(0);
            has_embedded_lyric = tag
                .get_string(&ItemKey::Lyrics)
                .is_some_and(|l| !l.trim().is_empty());
        }
    }

    let (cover_path, cover_source, has_embedded_cover) =
        extract_song_cover(path, folder_cover_cache, cache_dir, &id);

    // R-5：标题缺失用文件名兜底、艺术家缺失用"未知艺术家"兜底；专辑/流派/专辑艺术家留空不臆断
    if title.trim().is_empty() {
        title.clone_from(&name);
    }
    if artist.trim().is_empty() {
        artist = "未知艺术家".to_string();
    }

    Some(LocalSong {
        id,
        name,
        path: raw_path.to_string(),
        size,
        extension,
        title,
        artist,
        album,
        album_artist,
        genre,
        track,
        year,
        duration_ms,
        bitrate,
        sample_rate,
        channels,
        modified_at,
        cover_path,
        cover_source,
        has_embedded_cover,
        has_embedded_lyric,
        has_lyric: find_local_lyric_path(path).is_some() || has_embedded_lyric,
    })
}

/// 查找与音频同目录的同名歌词文件：.lrc 优先，回退 .txt（F-MU-19 来源优先级的前半段）。
#[must_use]
pub fn find_local_lyric_path(audio_path: &Path) -> Option<PathBuf> {
    let dir = audio_path.parent()?;
    let stem = audio_path.file_stem()?.to_str()?;
    for ext in ["lrc", "txt"] {
        let candidate = dir.join(format!("{stem}.{ext}"));
        if candidate.is_file() {
            return Some(candidate);
        }
    }
    None
}

/// 读取音频文件内嵌歌词（F-MU-19 第二优先级：同名外挂缺失时回退内嵌；`NexBox` 无此级，本工程按规格补写）。
#[must_use]
pub fn read_embedded_lyric(audio_path: &Path) -> Option<String> {
    let tagged_file = lofty::read_from_path(audio_path).ok()?;
    let tag = tagged_file
        .primary_tag()
        .or_else(|| tagged_file.first_tag())?;
    tag.get_string(&ItemKey::Lyrics)
        .map(std::string::ToString::to_string)
        .filter(|v| !v.trim().is_empty())
}

/// 通过图片数据的 magic bytes 判断 MIME 类型，比标签声明的 MIME 更可靠
#[must_use]
pub fn detect_image_mime(data: &[u8]) -> &'static str {
    if data.len() >= 8 && data[..8] == [0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A] {
        "image/png"
    } else if data.len() >= 3 && data[..3] == [0xFF, 0xD8, 0xFF] {
        "image/jpeg"
    } else if data.len() >= 6 && (&data[..6] == b"GIF87a" || &data[..6] == b"GIF89a") {
        "image/gif"
    } else if data.len() >= 12 && &data[..4] == b"RIFF" && &data[8..12] == b"WEBP" {
        "image/webp"
    } else if data.len() >= 2 && data[..2] == [0x42, 0x4D] {
        "image/bmp"
    } else {
        // 无法识别时回退到标签声明的类型
        "image/jpeg"
    }
}

/// 压缩封面：最大边 `COVER_MAX_PX`，输出 JPEG（PNG 源输出 PNG 保留透明）。
/// 数据 ≤ `COVER_MIN_BYTES`、尺寸已足够小、或解码/编码失败时原样返回。
const COVER_MAX_PX: u32 = 256;
const COVER_MIN_BYTES: usize = 64 * 1024;
/// 封面进入解码前的硬上限：仅防御极端异常数据，正常高清内嵌封面（≤ 几 MB）不受影响。
const COVER_INPUT_MAX_BYTES: usize = 16 * 1024 * 1024;

#[must_use]
// 像素尺寸量级远小于 2^24，u32↔f32 转换在本函数内无精度风险
#[allow(
    clippy::cast_precision_loss,
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss
)]
pub fn downscale_cover(data: &[u8], mime: &str) -> (Vec<u8>, String) {
    if data.len() <= COVER_MIN_BYTES {
        return (data.to_vec(), mime.to_string());
    }
    let format = match mime {
        "image/png" => image::ImageFormat::Png,
        _ => image::ImageFormat::Jpeg,
    };
    let Ok(img) = image::load_from_memory_with_format(data, format) else {
        return (data.to_vec(), mime.to_string());
    };
    let (w, h) = img.dimensions();
    let max = w.max(h);
    if max <= COVER_MAX_PX {
        return (data.to_vec(), mime.to_string());
    }
    let scale = COVER_MAX_PX as f32 / max as f32;
    let nw = ((w as f32) * scale).round().max(1.0) as u32;
    let nh = ((h as f32) * scale).round().max(1.0) as u32;
    let resized = img.resize(nw, nh, image::imageops::FilterType::Triangle);
    let mut out = Vec::new();
    let encoded = if mime == "image/png" {
        image::codecs::png::PngEncoder::new(&mut std::io::Cursor::new(&mut out))
            .write_image(
                resized.as_bytes(),
                resized.width(),
                resized.height(),
                resized.color().into(),
            )
            .map(|()| "image/png")
    } else {
        image::codecs::jpeg::JpegEncoder::new_with_quality(&mut std::io::Cursor::new(&mut out), 80)
            .write_image(
                resized.as_bytes(),
                resized.width(),
                resized.height(),
                resized.color().into(),
            )
            .map(|()| "image/jpeg")
    };
    match encoded {
        Ok(m) if !out.is_empty() => (out, m.to_string()),
        _ => (data.to_vec(), mime.to_string()),
    }
}

/// 将图片文件压缩后写入封面缓存，返回缓存文件绝对路径。
fn save_cover_from_file(cache_dir: &Path, cache_key: &str, path: &Path) -> String {
    let mime = match path
        .extension()
        .and_then(|v| v.to_str())
        .map(str::to_lowercase)
        .as_deref()
    {
        Some("png") => "image/png",
        Some("gif") => "image/gif",
        Some("bmp") => "image/bmp",
        Some("webp") => "image/webp",
        _ => "image/jpeg",
    };
    match std::fs::read(path) {
        // 先压缩再写入（与内嵌封面一致）；仅拦截空文件与极端异常的巨型图片
        Ok(data) if !data.is_empty() && data.len() <= COVER_INPUT_MAX_BYTES => {
            let (data, mime) = downscale_cover(&data, mime);
            save_cover_file(cache_dir, cache_key, &data, &mime)
        }
        Ok(_) | Err(_) => String::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    /// >1MB 的高清内嵌封面应压缩后正常写入缓存（NexBox 历史缺陷回归测试，随移植保留）
    #[test]
    fn large_embedded_cover_is_downscaled_and_saved() {
        let dir = std::env::temp_dir().join("aegis_cover_test");
        let _ = std::fs::create_dir_all(&dir);
        // 2048x2048 噪声 PNG（随机纹理不可压缩，编码后必然 >1MB）
        let mut pixels = vec![0u8; 2048 * 2048 * 3];
        let mut seed = 0x1234_5678_u64;
        for chunk in pixels.chunks_mut(3) {
            seed = seed
                .wrapping_mul(6_364_136_223_846_793_005)
                .wrapping_add(1_442_695_040_888_963_407);
            chunk[0] = ((seed >> 33) & 0xFF) as u8;
            chunk[1] = ((seed >> 41) & 0xFF) as u8;
            chunk[2] = ((seed >> 49) & 0xFF) as u8;
        }
        let img = image::RgbImage::from_raw(2048, 2048, pixels).unwrap();
        let img = image::DynamicImage::ImageRgb8(img);
        let mut raw = Vec::new();
        image::codecs::png::PngEncoder::new(&mut std::io::Cursor::new(&mut raw))
            .write_image(
                img.as_bytes(),
                img.width(),
                img.height(),
                img.color().into(),
            )
            .unwrap();
        assert!(raw.len() > 1024 * 1024, "测试前置条件：原始封面应大于 1MB");

        assert!(raw.len() <= COVER_INPUT_MAX_BYTES);
        let mime = detect_image_mime(&raw);
        let (out, out_mime) = downscale_cover(&raw, mime);
        assert_ne!(out, Vec::<u8>::new(), "压缩产物不应为空");
        assert!(out.len() < raw.len(), "压缩后体积应显著缩小");
        let path = save_cover_file(&dir, "test-id", &out, &out_mime);
        assert_ne!(path, String::new(), "封面缓存路径不应为空");
        let meta = std::fs::metadata(&path).unwrap();
        assert_eq!(
            usize::try_from(meta.len()).unwrap_or(usize::MAX),
            out.len(),
            "缓存文件应完整写入"
        );
        std::fs::remove_dir_all(&dir).unwrap();
    }

    /// 损坏的半截缓存文件（长度不一致）应被覆盖重写而非跳过
    #[test]
    fn corrupt_cache_file_is_repaired() {
        let dir = std::env::temp_dir().join("aegis_cover_test2");
        let _ = std::fs::create_dir_all(&dir);
        let data = vec![7u8; 1000];
        let first = save_cover_file(&dir, "repair-id", &data, "image/jpeg");
        std::fs::write(&first, &data[..500]).unwrap(); // 模拟半截写入
        let second = save_cover_file(&dir, "repair-id", &data, "image/jpeg");
        assert_eq!(first, second);
        let meta = std::fs::metadata(&second).unwrap();
        assert_eq!(meta.len(), 1000, "损坏文件应被重写为完整数据");
        std::fs::remove_dir_all(&dir).unwrap();
    }

    /// covers 子目录尚不存在时应自建（全新安装/清理缓存后首次扫描的历史缺陷回归钉）
    #[test]
    fn cover_file_is_written_when_cache_dir_missing() {
        let dir = std::env::temp_dir().join("aegis_cover_test_missing_dir");
        let _ = std::fs::remove_dir_all(&dir);
        let nested = dir.join("covers");
        assert!(!nested.exists());
        let written = save_cover_file(&nested, "fresh-id", &[9u8; 64], "image/jpeg");
        assert!(!written.is_empty(), "目录缺失不应静默返回空路径");
        assert_eq!(std::fs::metadata(&written).unwrap().len(), 64);
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn mime_detection() {
        assert_eq!(
            detect_image_mime(&[0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A]),
            "image/png"
        );
        assert_eq!(detect_image_mime(&[0xFF, 0xD8, 0xFF, 0xE0]), "image/jpeg");
        assert_eq!(detect_image_mime(b"GIF89a...."), "image/gif");
    }

    #[test]
    fn stable_hash_is_deterministic() {
        assert_eq!(stable_hash("abc"), stable_hash("abc"));
        assert_ne!(stable_hash("abc"), stable_hash("abd"));
    }

    /// 构造带 `ID3v2` 内嵌歌词的最小 mp3：先写两帧合法 `MPEG1` `Layer3` 128kbps 44.1kHz mono（帧头 FF FB 90 64，417 字节/帧；
    /// 单帧会被 lofty 的 `MPEG` 探针判为 invalid frame），再交给 lofty 写标签——读写两侧都与生产路径一致。
    fn make_mp3_with_embedded_lyric(path: &Path, lyric: &str) {
        use lofty::config::WriteOptions;
        use lofty::tag::{Tag, TagType};

        let mut audio = Vec::new();
        for _ in 0..2 {
            audio.extend_from_slice(&[0xFFu8, 0xFB, 0x90, 0x64]);
            audio.resize(audio.len() + 413, 0);
        }
        std::fs::write(path, &audio).unwrap();

        let mut tagged_file = lofty::read_from_path(path).unwrap();
        let mut tag = Tag::new(TagType::Id3v2);
        assert!(tag.insert_text(ItemKey::Lyrics, lyric.to_string()));
        tagged_file.insert_tag(tag);
        tagged_file
            .save_to_path(path, WriteOptions::default())
            .unwrap();
    }

    /// F-MU-19 第二优先级：同名外挂落空 → 内嵌歌词文本（NexBox 无此级，规格补写回归钉）
    #[test]
    fn embedded_lyric_is_read_when_external_missing() {
        let dir = std::env::temp_dir().join("aegis_lyric_test1");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let mp3 = dir.join("song.mp3");
        make_mp3_with_embedded_lyric(&mp3, "[00:01.00]内嵌一行");
        assert_eq!(find_local_lyric_path(&mp3), None);
        assert_eq!(
            read_embedded_lyric(&mp3).as_deref(),
            Some("[00:01.00]内嵌一行")
        );
        std::fs::remove_dir_all(&dir).unwrap();
    }

    /// F-MU-19 来源优先级：同名外挂 .lrc 存在时压过内嵌歌词
    #[test]
    fn external_lyric_takes_priority_over_embedded() {
        let dir = std::env::temp_dir().join("aegis_lyric_test2");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let mp3 = dir.join("song.mp3");
        make_mp3_with_embedded_lyric(&mp3, "内嵌文本");
        std::fs::write(dir.join("song.lrc"), "[00:02.00]外挂文本").unwrap();
        let found = find_local_lyric_path(&mp3).unwrap();
        assert_eq!(found.file_name().unwrap(), "song.lrc");
        assert_eq!(
            std::fs::read_to_string(&found).unwrap(),
            "[00:02.00]外挂文本"
        );
        std::fs::remove_dir_all(&dir).unwrap();
    }

    /// 无内嵌歌词的 mp3 返回 None（不误报有词）
    #[test]
    fn missing_embedded_lyric_returns_none() {
        let dir = std::env::temp_dir().join("aegis_lyric_test3");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let mp3 = dir.join("bare.mp3");
        std::fs::write(&mp3, [0xFFu8, 0xFB, 0x90, 0x64, 0, 0, 0, 0, 0, 0]).unwrap();
        std::fs::OpenOptions::new()
            .append(true)
            .open(&mp3)
            .unwrap()
            .write_all(&[0u8; 407])
            .unwrap();
        assert_eq!(read_embedded_lyric(&mp3), None);
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
