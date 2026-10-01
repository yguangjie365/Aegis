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
