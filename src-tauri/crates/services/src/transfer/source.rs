//! 发送侧读盘器：全文件摘要 → 分块流式读出（F-FT-19/20/28）。
//!
//! ## 为什么先单独哈希一遍
//!
//! 协议把**全文件摘要放在开场握手**里（`Handshake.file_hash`，见 [`super::protocol`]），
//! 故发送方必须在发第一个数据块**之前**就知道它 ⇒ 必须独立读一遍文件。
//! 代价是一次额外读盘；收益是接收方**从第一帧起**就持有校验依据，
//! 且 `Complete` 里再报一次同一个值可以暴露"传输期间文件被改过"
//! （两次摘要不一致 = 内容变了，而不是静默发出一份混合内容）。
//!
//! 🔴 **全文件摘要无法用来提前校验已收前缀**：前缀的哈希推不出全文件哈希。
//! 所以 F-FT-19 的"重算已收部分作为校验起点"确实只能作为**最终比对**的起点——
//! 接续中若前缀是坏的，只有发完才能发现。这是全文件摘要方案的固有代价，
//! 如实登记，不假装有早期校验。
//!
//! ## 发送期间文件被改动怎么办
//!
//! 一律**如实报错**（[`SourceError::ChangedDuringTransfer`]），不发"半新半旧"的内容。
//! 判据是**大小 + 修改时间**双双比对（只看大小会漏掉同长度的改写）。
//!
//! 复用结论（DP-1）：③思路复用 ← 母本 `FileTransferService.SendFileAsync`
//! （先算 SHA-256 填 `FileTransferItem.Hash`，再按 `ChunkSize` 分块读发）与
//! `FileTransferProtocol` 的偏移/序号字段。GPL-3.0 同向，ADR-003 五步登记见 `DEPENDENCIES.md §三`。

use std::fs::File;
use std::io::{BufReader, Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use sha2::{Digest, Sha256};

use super::admission::MAX_FILE_SIZE;
use super::chunk;

/// 哈希与读盘用的缓冲大小（64 KiB：与 `std::io::copy` 默认量级一致，避免大文件上的 syscall 放大）。
pub const READ_BUFFER_BYTES: usize = 64 * 1024;

/// 发送侧失败原因。
#[derive(Debug)]
pub enum SourceError {
    Io(std::io::Error),
    /// 路径不是常规文件（目录 / 设备 / 不存在）
    NotAFile,
    /// 超过准入上限（与接收侧同一常量，避免两端对"能传多大"给出不同答案）
    TooLarge {
        size: u64,
        max: u64,
    },
    /// 分块大小不合规（复用 [`chunk::chunk_size_mismatch`] 的判据）
    ChunkSize {
        declared: u64,
    },
    /// 续传起点越界（必须落在 `0..=file_size`）
    ResumeOutOfRange {
        requested: u64,
        size: u64,
    },
    /// 传输期间文件被改动
    ChangedDuringTransfer {
        expected_size: u64,
        actual_size: u64,
        size_changed: bool,
        time_changed: bool,
    },
}

impl std::fmt::Display for SourceError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io(error) => write!(f, "读盘失败：{error}"),
            Self::NotAFile => f.write_str("不是常规文件"),
            Self::TooLarge { size, max } => {
                write!(f, "文件 {size} 字节超过上限 {max}")
            }
            Self::ChunkSize { declared } => {
                write!(f, "分块大小 {declared} 不合规")
            }
            Self::ResumeOutOfRange { requested, size } => {
                write!(f, "续传起点 {requested} 越界（文件 {size} 字节）")
            }
            Self::ChangedDuringTransfer {
                expected_size,
                actual_size,
                size_changed,
                time_changed,
            } => {
                write!(
                    f,
                    "传输期间文件被改动（原 {expected_size} → 现 {actual_size}；大小变={size_changed} 时间变={time_changed}）"
                )
            }
        }
    }
}

impl std::error::Error for SourceError {}

impl From<std::io::Error> for SourceError {
    fn from(error: std::io::Error) -> Self {
        Self::Io(error)
    }
}

/// 独立一遍计算全文件 SHA-256（十六进制小写）——握手要用它。
///
/// # Errors
/// 文件不存在/不可读、或不是常规文件时返回 [`SourceError`]。
pub fn hash_file(path: &Path) -> Result<String, SourceError> {
    let metadata = std::fs::metadata(path)?;
    if !metadata.is_file() {
        return Err(SourceError::NotAFile);
    }
    let mut reader = BufReader::with_capacity(READ_BUFFER_BYTES, File::open(path)?);
    let mut hasher = Sha256::new();
    let mut buffer = vec![0_u8; READ_BUFFER_BYTES];
    loop {
        let read = reader.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }
    Ok(hex::encode(hasher.finalize()))
}

/// 待发出的一帧数据块。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OutgoingChunk {
    /// 本块在文件中的序号（由偏移导出：`offset / chunk_size`）。
    /// 🔴 **接收方必须按 `offset` 归位，不得按 `index`**——续传起点可能不是块边界，
    /// 那时首个块的 `index` 会与接收方"已有的半块"重合。
    pub index: u64,
    /// 本块在文件中的起始偏移
    pub offset: u64,
    /// 本块字节（末块可短）
    pub bytes: Vec<u8>,
}

/// 一次发送的读盘器。
///
/// 生命周期：`open` →（可选 `seek_to`）→ 反复 `next_chunk` → `verify_unchanged`。
#[derive(Debug)]
pub struct SendSource {
    reader: BufReader<File>,
    path: PathBuf,
    file_name: String,
    file_size: u64,
    chunk_size: u64,
    cursor: u64,
    opened_modified: SystemTime,
}

impl SendSource {
    /// 打开待发送文件并校验分块大小。
    ///
    /// 🔴 分块大小用 [`chunk::chunk_size_mismatch`] 校验（0 与超上限都拒）——
    /// 与接收侧**同一个判据**，避免两端对"合法块大小"给出不同答案。
    ///
    /// # Errors
    /// 非文件、超上限、分块大小不合规、IO 失败时返回 [`SourceError`]。
    pub fn open(path: &Path, chunk_size: u64) -> Result<Self, SourceError> {
        if chunk::chunk_size_mismatch(chunk_size).is_some() {
            return Err(SourceError::ChunkSize {
                declared: chunk_size,
            });
        }
        let metadata = std::fs::metadata(path)?;
        if !metadata.is_file() {
            return Err(SourceError::NotAFile);
        }
        let file_size = metadata.len();
        if file_size > MAX_FILE_SIZE {
            return Err(SourceError::TooLarge {
                size: file_size,
                max: MAX_FILE_SIZE,
            });
        }
        let file_name = path
            .file_name()
            .map_or_else(String::new, |n| n.to_string_lossy().into_owned());
        if file_name.is_empty() {
            return Err(SourceError::NotAFile);
        }
        let opened_modified = metadata.modified().unwrap_or(SystemTime::UNIX_EPOCH);
        Ok(Self {
            reader: BufReader::with_capacity(READ_BUFFER_BYTES, File::open(path)?),
            path: path.to_path_buf(),
            file_name,
            file_size,
            chunk_size,
            cursor: 0,
            opened_modified,
        })
    }

    #[must_use]
    pub fn file_name(&self) -> &str {
        &self.file_name
    }

    #[must_use]
    pub fn file_size(&self) -> u64 {
        self.file_size
    }

    #[must_use]
    pub fn chunk_size(&self) -> u64 {
        self.chunk_size
    }

    /// 当前读到的偏移。
    #[must_use]
    pub fn cursor(&self) -> u64 {
        self.cursor
    }

    /// 定位到续传起点（F-FT-19：接收方在 `HandshakeAck.resume_from` 里给）。
    ///
    /// 接受 `0..=file_size`；`file_size` 表示"已收满，无需再发数据块"
    /// （接收方会直接等 `Complete`）。
    ///
    /// 🔴 起点**不必**是块边界：接收方的未完成文件长度就是它的真实进度，
    /// 崩溃可能留下半块。故偏移一律以字节为准，序号仅作展示。
    ///
    /// # Errors
    /// 起点大于文件大小时返回 [`SourceError::ResumeOutOfRange`]。
    pub fn seek_to(&mut self, offset: u64) -> Result<(), SourceError> {
        if offset > self.file_size {
            return Err(SourceError::ResumeOutOfRange {
                requested: offset,
                size: self.file_size,
            });
        }
        self.reader.seek(SeekFrom::Start(offset))?;
        self.cursor = offset;
        Ok(())
    }

    /// 读出下一块；已到末尾返回 `None`（正常收尾，不是错误）。
    ///
    /// # Errors
    /// IO 失败时返回 [`SourceError`]。
    pub fn next_chunk(&mut self) -> Result<Option<OutgoingChunk>, SourceError> {
        if self.cursor >= self.file_size {
            return Ok(None);
        }
        let remaining = self.file_size - self.cursor;
        let want = usize::try_from(remaining.min(self.chunk_size)).unwrap_or(0);
        let mut bytes = vec![0_u8; want];
        self.reader.read_exact(&mut bytes)?;
        let chunk = OutgoingChunk {
            index: chunk::chunk_index_for_offset(self.cursor, self.chunk_size),
            offset: self.cursor,
            bytes,
        };
        self.cursor += want as u64;
        Ok(Some(chunk))
    }

    /// 结束前校验文件**自打开以来未被改动**。
    ///
    /// 判据是**大小 + 修改时间**：只看大小会漏掉"同长度改写"，
    /// 那会发出一份半新半旧的内容而两端都不报错——最难查的一类问题。
    ///
    /// # Errors
    /// 文件已被改动时返回 [`SourceError::ChangedDuringTransfer`]。
    pub fn verify_unchanged(&self) -> Result<(), SourceError> {
        let metadata = std::fs::metadata(&self.path)?;
        let actual_size = metadata.len();
        let actual_modified = metadata.modified().unwrap_or(SystemTime::UNIX_EPOCH);
        let size_changed = actual_size != self.file_size;
        let time_changed = actual_modified != self.opened_modified;
        if size_changed || time_changed {
            return Err(SourceError::ChangedDuringTransfer {
                expected_size: self.file_size,
                actual_size,
                size_changed,
                time_changed,
            });
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::transfer::{conflict::ConflictPolicy, sink::ReceiveSink};
    use std::fs;

    fn temp_dir(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("aegis-source-{}-{}", std::process::id(), tag));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).expect("建临时目录");
        dir
    }

    fn write_file(dir: &Path, name: &str, bytes: &[u8]) -> PathBuf {
        let path = dir.join(name);
        fs::write(&path, bytes).expect("写测试文件");
        path
    }

    /// SHA-256("abc") 是公开的标准测试向量，用它证明哈希**算法对不对**，
    /// 而不是"两边用同一个错函数所以一致"。
    #[test]
    fn hash_file_matches_the_published_sha256_test_vector() {
        let dir = temp_dir("hash");
        let path = write_file(&dir, "abc.txt", b"abc");
        assert_eq!(
            hash_file(&path).unwrap(),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
        // 空文件的已知向量
        let empty = write_file(&dir, "empty.txt", b"");
        assert_eq!(
            hash_file(&empty).unwrap(),
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
        // 目录不是文件
        assert!(matches!(hash_file(&dir), Err(SourceError::NotAFile)));
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn open_rejects_bad_chunk_size_and_non_files() {
        let dir = temp_dir("open");
        let path = write_file(&dir, "a.bin", &[0_u8; 16]);

        // 0 与超上限都拒（与接收侧同一判据）
        assert!(matches!(
            SendSource::open(&path, 0),
            Err(SourceError::ChunkSize { declared: 0 })
        ));
        assert!(matches!(
            SendSource::open(&path, chunk::MAX_CHUNK_SIZE + 1),
            Err(SourceError::ChunkSize { .. })
        ));
        // 目录不是文件
        assert!(matches!(
            SendSource::open(&dir, chunk::DEFAULT_CHUNK_SIZE),
            Err(SourceError::NotAFile)
        ));

        let source = SendSource::open(&path, chunk::DEFAULT_CHUNK_SIZE).unwrap();
        assert_eq!(source.file_name(), "a.bin");
        assert_eq!(source.file_size(), 16);
        assert_eq!(source.cursor(), 0);
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn chunks_tile_the_file_exactly_with_a_short_last_chunk() {
        let dir = temp_dir("chunks");
        // 5 字节，块大小 2 → 2+2+1
        let payload = b"ABCDE";
        let path = write_file(&dir, "b.bin", payload);
        let mut source = SendSource::open(&path, 2).unwrap();

        let mut collected = Vec::new();
        let mut offsets = Vec::new();
        while let Some(chunk) = source.next_chunk().unwrap() {
            offsets.push(chunk.offset);
            collected.extend_from_slice(&chunk.bytes);
        }
        assert_eq!(collected, payload, "字节必须无缺无重");
        assert_eq!(offsets, vec![0, 2, 4]);
        assert_eq!(source.cursor(), 5);
        // 末尾再取仍为 None（幂等）
        assert!(source.next_chunk().unwrap().is_none());
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn empty_file_yields_no_chunks_and_is_not_an_error() {
        let dir = temp_dir("empty");
        let path = write_file(&dir, "empty.bin", b"");
        let mut source = SendSource::open(&path, chunk::DEFAULT_CHUNK_SIZE).unwrap();
        assert_eq!(source.file_size(), 0);
        assert!(
            source.next_chunk().unwrap().is_none(),
            "空文件合法，不该报错"
        );
        source.verify_unchanged().unwrap();
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn seek_to_resumes_from_the_receiver_reported_offset() {
        let dir = temp_dir("resume");
        let payload: Vec<u8> = (0..10_u8).collect();
        let path = write_file(&dir, "c.bin", &payload);
        let mut source = SendSource::open(&path, 4).unwrap();

        // 接收方已有前 6 字节（不是块边界：4 的倍数里没有 6）
        source.seek_to(6).unwrap();
        assert_eq!(source.cursor(), 6);
        let mut rest = Vec::new();
        while let Some(chunk) = source.next_chunk().unwrap() {
            assert!(chunk.offset >= 6);
            rest.extend_from_slice(&chunk.bytes);
        }
        assert_eq!(rest, &payload[6..], "只发剩余部分");

        // 起点等于文件大小 = 已收满，不再发块
        source.seek_to(10).unwrap();
        assert!(source.next_chunk().unwrap().is_none());

        // 越界必须拒
        assert!(matches!(
            source.seek_to(11),
            Err(SourceError::ResumeOutOfRange {
                requested: 11,
                size: 10
            })
        ));
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn verify_unchanged_detects_a_size_change() {
        let dir = temp_dir("changed");
        let path = write_file(&dir, "d.bin", &[0_u8; 8]);
        let source = SendSource::open(&path, chunk::DEFAULT_CHUNK_SIZE).unwrap();
        source.verify_unchanged().unwrap();

        fs::write(&path, [0_u8; 9]).unwrap();
        match source.verify_unchanged() {
            Err(SourceError::ChangedDuringTransfer {
                expected_size,
                actual_size,
                size_changed,
                ..
            }) => {
                assert_eq!((expected_size, actual_size), (8, 9));
                assert!(size_changed);
            }
            other => panic!("应报文件被改动，实得 {other:?}"),
        }
        let _ = fs::remove_dir_all(&dir);
    }

    /// 🔴 同长度改写也必须被抓到——只看大小会漏掉，那会发出一份"半新半旧"的内容。
    #[test]
    fn verify_unchanged_detects_a_same_size_rewrite() {
        let dir = temp_dir("rewrite");
        let path = write_file(&dir, "e.bin", &[1_u8; 64]);
        let source = SendSource::open(&path, chunk::DEFAULT_CHUNK_SIZE).unwrap();

        // 不用 sleep（本仓 clippy.toml 明令禁止 `std::thread::sleep`，且它慢又不确定）：
        // 直接改写内容并把修改时间**确定性地**设成另一个值。
        fs::write(&path, [2_u8; 64]).unwrap();
        infra::fs::set_modified_time(&path, 1_600_000_000_000).unwrap();

        match source.verify_unchanged() {
            Err(SourceError::ChangedDuringTransfer {
                size_changed,
                time_changed,
                expected_size,
                actual_size,
            }) => {
                assert!(!size_changed, "长度没变");
                assert!(time_changed, "时间变了必须抓到");
                assert_eq!((expected_size, actual_size), (64, 64));
            }
            other => panic!("同长度改写必须被抓到，实得 {other:?}"),
        }
        let _ = fs::remove_dir_all(&dir);
    }

    // ── 端到端（无网络）：发送侧 → 接收侧 ───────────────────────────────────

    /// 🔴 本批最有价值的一条：把 [`SendSource`] 的块**真喂进** [`ReceiveSink`]，
    /// 验证"发出去的东西能原样落地并通过校验"。网络层只是给这条链路套一个传输而已。
    #[test]
    fn source_feeds_sink_end_to_end_for_various_sizes() {
        let payloads: Vec<(&str, Vec<u8>)> = vec![
            ("空文件", Vec::new()),
            ("单块不足", b"short".to_vec()),
            ("恰好整块", vec![7_u8; 8]),
            ("整块+余数", (0..21_u8).collect()),
        ];
        for (tag, payload) in payloads {
            let dir = temp_dir(&format!("e2e-{}", tag.len()));
            let send_dir = dir.join("send");
            let recv_dir = dir.join("recv");
            fs::create_dir_all(&send_dir).unwrap();
            fs::create_dir_all(&recv_dir).unwrap();
            let path = write_file(&send_dir, "payload.bin", &payload);

            // 发送方：先哈希（握手用），再流式发块
            let full_hash = hash_file(&path).unwrap();
            assert_eq!(full_hash, full_hash_of(&payload), "摘要必须与内容一致");

            let mut source = SendSource::open(&path, 4).unwrap();
            let mut sink =
                ReceiveSink::open(&recv_dir, "payload.bin", "tok", source.file_size()).unwrap();
            while let Some(chunk) = source.next_chunk().unwrap() {
                sink.append(chunk.offset, &chunk.bytes).unwrap();
            }
            source.verify_unchanged().unwrap();

            let outcome = sink
                .finish(
                    &full_hash,
                    ConflictPolicy::Rename,
                    None,
                    crate::transfer::sink::now_unix_ms(),
                )
                .unwrap();
            match outcome {
                crate::transfer::sink::FinishOutcome::Committed { path, .. } => {
                    assert_eq!(
                        fs::read(&path).unwrap(),
                        payload,
                        "[{tag}] 内容必须逐字节一致"
                    );
                }
                other => panic!("[{tag}] 应入位，实得 {other:?}"),
            }
            let _ = fs::remove_dir_all(&dir);
        }
    }

    /// 续传的端到端：先落一半、断掉、再从中断处接着发，最终仍要校验通过。
    #[test]
    fn source_resumes_into_a_partial_sink_end_to_end() {
        let dir = temp_dir("e2e-resume");
        let send_dir = dir.join("send");
        let recv_dir = dir.join("recv");
        fs::create_dir_all(&send_dir).unwrap();
        fs::create_dir_all(&recv_dir).unwrap();
        let payload: Vec<u8> = (0..30_u8).collect();
        let path = write_file(&send_dir, "big.bin", &payload);
        let full_hash = hash_file(&path).unwrap();
        let chunk_size = 4_u64;

        // 第一段：发前 12 字节（3 块）后"断线"
        let received;
        {
            let mut source = SendSource::open(&path, chunk_size).unwrap();
            let mut sink = ReceiveSink::open(&recv_dir, "big.bin", "tok", 30).unwrap();
            for _ in 0..3 {
                let chunk = source.next_chunk().unwrap().unwrap();
                sink.append(chunk.offset, &chunk.bytes).unwrap();
            }
            received = sink.received();
            // 不 finish、不 discard —— 模拟连接中断
        }
        assert_eq!(received, 12);

        // 第二段：重新打开两端，从 12 接着发
        let mut source = SendSource::open(&path, chunk_size).unwrap();
        let mut sink = ReceiveSink::open(&recv_dir, "big.bin", "tok", 30).unwrap();
        assert_eq!(sink.received(), 12, "接收方应报告真实进度");
        source.seek_to(sink.received()).unwrap();
        while let Some(chunk) = source.next_chunk().unwrap() {
            sink.append(chunk.offset, &chunk.bytes).unwrap();
        }

        let outcome = sink
            .finish(
                &full_hash,
                ConflictPolicy::Rename,
                None,
                crate::transfer::sink::now_unix_ms(),
            )
            .unwrap();
        match outcome {
            crate::transfer::sink::FinishOutcome::Committed { path, .. } => {
                assert_eq!(fs::read(&path).unwrap(), payload, "续传后必须逐字节完整");
            }
            other => panic!("续传应成功，实得 {other:?}"),
        }
        let _ = fs::remove_dir_all(&dir);
    }

    fn full_hash_of(bytes: &[u8]) -> String {
        let mut hasher = Sha256::new();
        hasher.update(bytes);
        hex::encode(hasher.finalize())
    }
}
