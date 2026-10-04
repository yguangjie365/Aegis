//! 文件原子写原语（W0-09 提前子集，随 MU-1 曲库缓存落地）。
//!
//! 口径：同目录临时文件 `.staging-*` + write + fsync + rename；失败时清理临时文件、
//! 保留原内容不动（规格 07 R-25 / 02 专题 §五）。

use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::Path;

/// 原子写入 `path`：内容完整落盘后才可见；中途失败不破坏原文件。
///
/// # Errors
/// 父目录创建、临时文件写入/`sync_all`/原子 rename 任一步失败时返回底层 `io::Error`；
/// 失败路径上临时文件已尽力清理，原文件保持不动。
pub fn atomic_write(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    let dir = path.parent().ok_or_else(|| {
        std::io::Error::new(std::io::ErrorKind::InvalidInput, "path has no parent dir")
    })?;
    fs::create_dir_all(dir)?;

    let file_name = path
        .file_name()
        .ok_or_else(|| std::io::Error::new(std::io::ErrorKind::InvalidInput, "no file name"))?;
    let staging = dir.join(format!(".staging-{}", file_name.to_string_lossy()));

    write_and_flush(&staging, bytes)?;
    if let Err(error) = fs::rename(&staging, path) {
        let _ = fs::remove_file(&staging);
        return Err(error);
    }
    sync_dir_best_effort(dir);
    Ok(())
}

fn write_and_flush(staging: &Path, bytes: &[u8]) -> std::io::Result<()> {
    let mut file = OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(true)
        .open(staging)?;
    file.write_all(bytes)?;
    match file.sync_all() {
        Ok(()) => Ok(()),
        Err(error) => {
            let _ = fs::remove_file(staging);
            Err(error)
        }
    }
}

/// Windows 上打开目录句柄不被常规 API 支持，父目录 fsync 为尽力而为；
/// 文件句柄的 `sync_all` 已保证内容落盘，rename 在同卷上是事务性的。
fn sync_dir_best_effort(_dir: &Path) {}

/// 把文件的**修改时间**设为 `unix_ms`（Unix 毫秒）。
///
/// 规格依据：F-FT-24「接收完成后把文件的修改时间还原为对端声明的原始时间」。
///
/// 🔴 **FFI 只允许出现在 `infra` 与 `elevated-helper`**（02 册 KN-04），故本函数是
/// "设文件时间"的**唯一入口**；域侧只负责决定"要不要设、设的值合不合理"
/// （合理窗口判定属域逻辑，见 `services::transfer` 的 mtime 窗口）。
///
/// 🔴 `std::fs` **没有**设置时间的能力（只有读取），故必须走 Win32 `SetFileTime`。
/// 这不需要新增依赖：本 crate 已因 SMART 引入 `windows-sys` 的 `Win32_Storage_FileSystem`。
///
/// # Errors
/// 文件不存在、无 `FILE_WRITE_ATTRIBUTES` 权限、或 `SetFileTime` 失败时返回 `io::Error`。
// `unsafe_code` 在本函数上是**不可避免**的：`std::fs` 只能读时间、不能写时间。
// 工作区 lint 把它设为 `warn`（非 deny），既有 FFI 模块（`smart.rs` / `display.rs`）都是让它响；
// 这里加**函数级** allow 是为了让本批不新增告警——不是放宽全仓姿态。
// 安全性由函数体内的 `SAFETY` 注释逐条论证。
#[cfg(windows)]
#[allow(unsafe_code)]
pub fn set_modified_time(path: &Path, unix_ms: i64) -> std::io::Result<()> {
    use std::os::windows::ffi::OsStrExt;
    use windows_sys::Win32::Foundation::{CloseHandle, FILETIME, INVALID_HANDLE_VALUE};
    use windows_sys::Win32::Storage::FileSystem::{
        CreateFileW, SetFileTime, FILE_ATTRIBUTE_NORMAL, FILE_SHARE_READ, FILE_SHARE_WRITE,
        OPEN_EXISTING,
    };

    /// `FILE_WRITE_ATTRIBUTES`（Win32 头文件值 0x0100）。
    ///
    /// 写常量字面量而非引用 `windows-sys` 的符号：该常量在不同版本里的导出路径不稳，
    /// 而值是 ABI 固定的。
    const FILE_WRITE_ATTRIBUTES: u32 = 0x0100;

    /// 1601-01-01 → 1970-01-01 的秒数（`FILETIME` 纪元是 1601）。
    const EPOCH_DELTA_SECONDS: i64 = 11_644_473_600;
    /// `FILETIME` 的单位是 100 纳秒。
    const TICKS_PER_SECOND: i64 = 10_000_000;

    let wide: Vec<u16> = path
        .as_os_str()
        .encode_wide()
        .chain(std::iter::once(0))
        .collect();

    // SAFETY: `wide` 是以 NUL 结尾的合法 UTF-16 缓冲，在其生命周期内有效；
    // 句柄由本函数创建并在返回前关闭；`FILETIME` 是 POD。
    unsafe {
        let handle = CreateFileW(
            wide.as_ptr(),
            FILE_WRITE_ATTRIBUTES,
            FILE_SHARE_READ | FILE_SHARE_WRITE,
            std::ptr::null(),
            OPEN_EXISTING,
            FILE_ATTRIBUTE_NORMAL,
            std::ptr::null_mut(),
        );
        if handle == INVALID_HANDLE_VALUE {
            return Err(std::io::Error::last_os_error());
        }

        let ticks = (unix_ms / 1000 + EPOCH_DELTA_SECONDS)
            .saturating_mul(TICKS_PER_SECOND)
            .saturating_add((unix_ms % 1000) * 10_000);
        // 不走 `as u32`：clippy 会把"i64 截断到 u32"当隐患（这里虽被掩码保证安全，
        // 但用 try_from 表达意图更清楚，也让本函数保持零告警）
        let ticks_u64 = u64::try_from(ticks).unwrap_or(0);
        let file_time = FILETIME {
            dwLowDateTime: u32::try_from(ticks_u64 & 0xFFFF_FFFF).unwrap_or(0),
            dwHighDateTime: u32::try_from(ticks_u64 >> 32).unwrap_or(0),
        };
        // 只设修改时间：创建时间与访问时间传 null 表示"不改"
        let ok = SetFileTime(handle, std::ptr::null(), std::ptr::null(), &file_time);
        let error = if ok == 0 {
            Some(std::io::Error::last_os_error())
        } else {
            None
        };
        CloseHandle(handle);
        match error {
            Some(error) => Err(error),
            None => Ok(()),
        }
    }
}

/// 非 Windows 平台的占位：本工程只发布 Windows（规格 §一 平台约束）。
///
/// # Errors
/// 恒返回 `Unsupported`——不给"静默当成功"的机会。
#[cfg(not(windows))]
pub fn set_modified_time(_path: &Path, _unix_ms: i64) -> std::io::Result<()> {
    Err(std::io::Error::new(
        std::io::ErrorKind::Unsupported,
        "set_modified_time 目前仅实现 Windows",
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn atomic_write_roundtrip_and_no_staging_left() {
        let dir = std::env::temp_dir().join("aegis_atomic_write_test");
        let file = dir.join("payload.json");
        atomic_write(&file, b"{\"v\":1}").unwrap();
        assert_eq!(fs::read(&file).unwrap(), b"{\"v\":1}");
        assert!(!dir.join(".staging-payload.json").exists());
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn rewrite_keeps_new_content() {
        let dir = std::env::temp_dir().join("aegis_atomic_write_test2");
        let file = dir.join("payload.json");
        atomic_write(&file, b"old").unwrap();
        atomic_write(&file, b"new").unwrap();
        assert_eq!(fs::read(&file).unwrap(), b"new");
        fs::remove_dir_all(&dir).unwrap();
    }
}

#[cfg(windows)]
#[test]
fn set_modified_time_round_trips() {
    use std::time::{Duration, UNIX_EPOCH};
    let dir = std::env::temp_dir().join("aegis_set_mtime_test");
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    let file = dir.join("stamp.txt");
    fs::write(&file, b"x").unwrap();

    // 2021-01-01T00:00:00Z
    let target_ms: i64 = 1_609_459_200_000;
    set_modified_time(&file, target_ms).unwrap();
    let got = fs::metadata(&file).unwrap().modified().unwrap();
    let got_ms =
        i64::try_from(got.duration_since(UNIX_EPOCH).unwrap().as_millis()).unwrap_or(i64::MAX);
    // FAT/NTFS 的 100ns 精度足以表达整秒，允许 2 秒容差（文件系统时间粒度差异）
    assert!(
        (got_ms - target_ms).abs() < 2000,
        "期望约 {target_ms}，实得 {got_ms}"
    );

    // 不存在的文件必须报错，不得静默当成功
    assert!(set_modified_time(&dir.join("nope.txt"), target_ms).is_err());
    let _ = fs::remove_dir_all(&dir);
    let _ = Duration::from_secs(0);
}
