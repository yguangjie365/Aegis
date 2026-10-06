//! 内容指纹：SHA-256，以及**复制单遍I/O** 的拷贝+计算（③ 思路复用 ← `Core/Utilities/Sha256Hasher.cs`）。
//!
//! 母本的关键设计：**边复制边算哈希**，只读一遍源文件。
//! 若先 `File::copy` 再单独读一遍算哈希，备份 1 TB 数据就要 2 TB 的磁盘读取 ——
//! 母本因此把 `CopyFileWithHash` 做成单遍。本实现同款。
//!
//! 🔴 `sha2` 已在 `services` 依赖里（M-01 已引，ADR-003 五步登记见 `DEPENDENCIES.md §三`），
//! **本模块不引入新依赖**。
//!
//! 🔴 哈希采用**小写十六进制**（母本 `BitConverter` 输出大写后 `.ToLowerInvariant()`；
//! 比对侧一律 `eq_ignore_ascii_case`，见 [`verify`]）。

use sha2::{Digest, Sha256};
use std::fs::File;
use std::io::{self, BufReader, BufWriter, Read, Write};
use std::path::Path;

/// 读缓冲大小（64 KiB：Windows 管道/文件 IO 的经验值，过小会放大系统调用，过大浪费内存）。
const READ_BUF: usize = 64 * 1024;

/// 单个文件的 SHA-256（读一遍）。
///
/// # Errors
/// 文件不存在 / 无读权限 / 读中断 ⇒ 原样返回 `io::Error`（🔴 **不得**把它当"内容为空"或"未变"）。
pub fn sha256_file(path: &Path) -> io::Result<String> {
    let file = File::open(path)?;
    hash_reader(BufReader::with_capacity(READ_BUF, file))
}

/// 从任意 reader 计算（便于测试与流式场景复用）。
///
/// # Errors
/// 读取源出错时返回 `io::Error`。
pub fn hash_reader<R: Read>(mut reader: R) -> io::Result<String> {
    let mut hasher = Sha256::new();
    let mut buf = vec![0u8; READ_BUF];
    loop {
        let n = reader.read(&mut buf)?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
    }
    Ok(to_hex(&hasher.finalize()))
}

/// 小写十六进制。
#[must_use]
pub fn to_hex(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        out.push(char::from_digit(u32::from(byte >> 4), 16).unwrap_or('0'));
        out.push(char::from_digit(u32::from(byte & 0x0F), 16).unwrap_or('0'));
    }
    out
}

/// **单遍拷贝 + 计算SHA-256**（母本 `CopyFileWithHash` 的 Rust 对应）。
///
/// 🔴 为什么必须单遍：备份动辄是百 GB 级，两遍 I/O 会把备份时间翻倍。
///
/// # Errors
/// 源打不开、目标写不动、目标父目录缺失、中途磁盘满 ⇒ 返回 `io::Error`；
/// 失败时**已写入的部分不删除**（由调用方决定半成品去留—— 备份侧的整体清理由
/// [`crate::backup::engine`] 负责，见 R-14 取消语义）。
pub fn copy_and_hash(source: &Path, target: &Path) -> io::Result<String> {
    if let Some(parent) = target.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let mut reader = BufReader::with_capacity(READ_BUF, File::open(source)?);
    // 🔴 写侧必须是 `BufWriter`（`BufReader` 只包装 reader，当 writer 用会没有 write_all）
    let file = File::create(target).map_err(|e| annotate(e, target, "创建目标文件失败"))?;
    let mut writer = BufWriter::with_capacity(READ_BUF, file);
    let mut hasher = Sha256::new();
    let mut buf = vec![0u8; READ_BUF];
    loop {
        let n = read_interruptible(&mut reader, &mut buf)?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
        if let Err(error) = writer.write_all(&buf[..n]) {
            return Err(annotate(error, target, "写入目标文件失败"));
        }
    }
    // 🔴 必须 flush + sync：哈希已算但数据还在页缓存里就报成功，会让"备份完成"名不副实。
    //   🔴 `BufWriter` 必须先 `flush` 把缓冲推给内核，`get_ref().sync_all()` 才有意义 ——
    //   顺序反了等于只同步了空文件。
    writer
        .flush()
        .map_err(|e| annotate(e, target, "刷新缓冲失败"))?;
    writer
        .get_ref()
        .sync_all()
        .map_err(|e| annotate(e, target, "刷新目标文件到磁盘失败"))?;
    Ok(to_hex(&hasher.finalize()))
}

/// 读系统调用可被 Ctrl-C打断；`read` 返回 `ErrorKind::Interrupted` 时重试。
fn read_interruptible<R: Read>(reader: &mut R, buf: &mut [u8]) -> io::Result<usize> {
    loop {
        match reader.read(buf) {
            Err(error) if error.kind() == io::ErrorKind::Interrupted => {}
            other => return other,
        }
    }
}

/// 给 `io::Error` 附上**用户可读**的上下文（规格 §六「失败可读」）。
///
/// 🔴 不改错误内容（`io::Error` 的消息会被 UI 直接展示），而是加一层说明后重新构造。
// `needless_pass_by_value` 豁免：函数体里的 `format!("…{error}")` **按引用**用掉了它，
//   clippy 的这条lint只看签名不读函数体，故此处是误报；改成 `&io::Error` 反而
//   会让调用点多一次显式取址（3 处），收益为零。
#[allow(clippy::needless_pass_by_value)]
fn annotate(error: io::Error, path: &Path, what: &str) -> io::Error {
    io::Error::new(
        error.kind(),
        format!("{what}（{}）：{error}", path.display()),
    )
}

#[cfg(test)]
mod tests;
