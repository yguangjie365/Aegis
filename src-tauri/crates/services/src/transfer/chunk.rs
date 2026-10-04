//! 分块与断点续传的算术（F-FT-19 / F-FT-20 / R-20）。
//!
//! 复用结论（DP-1）：③思路复用 ←
//! `Source/SystemToolkit/src/SystemToolkit.Core/FileTransfer/Models/TransferSettings.cs`
//! （`ChunkSize = 2 MiB`）与 `.../Services/FileTransferService.cs` 的续传协商段
//! （`.part` 长度即偏移、`resumeFrom` 仅在 `0 < existing < FileSize` 时成立、发送方越界归零）。
//! GPL-3.0 同源，ADR-003 五步登记见 `Aegis/DEPENDENCIES.md §三`。
//!
//! 🔴 **本模块是纯算术**：不做 IO、不碰路径、不知道 `.part` 文件叫什么。
//! 谁去读磁盘、`.part` 怎么命名，由传输引擎决定（且路径拼接必须走 `infra::safe`，见 `mod.rs`）。
//!
//! ## 为什么续传口径要写死成函数
//!
//! 母本在**发送方与接收方各算一遍**偏移，两侧口径只要有一点不同，就会表现为
//! "传完了但校验失败"（`HASH_MISMATCH`）——这是最难查的一类缺陷。故把判定收敛到本模块，
//! 两侧都调同一函数。

/// 默认分块大小：**2 MiB**（F-FT-19 原文"文件按默认 2 MB 一块流式传输"；母本 `ChunkSize` 同值）。
///
/// 注：规格写"2 MB"，母本实现是 `2 * 1024 * 1024`（2 MiB）。以母本为准并在此登记该字面差异。
pub const DEFAULT_CHUNK_SIZE: u64 = 2 * 1024 * 1024;

/// 分块大小上限（**安全边界，不是优化**）。
///
/// 母本在握手期强制校验对端声明的 `ChunkSize` 不得超过本端值——否则对端可用一个超大"块"
/// 让本端一次性分配巨量内存。本工程取 4 MiB 作为硬上限（默认值的 2 倍，留一档协商余量）。
pub const MAX_CHUNK_SIZE: u64 = 4 * 1024 * 1024;

/// 分块数：向上取整。`file_size = 0` 时为 0（空文件由上层按 `INVALID_FILE_SIZE` 拒绝）。
#[must_use]
pub fn total_chunks(file_size: u64, chunk_size: u64) -> u64 {
    if chunk_size == 0 {
        return 0;
    }
    file_size.div_ceil(chunk_size)
}

/// 第 `index` 块的字节区间 `(offset, len)`；越界返回 `None`。
///
/// 🔴 末块**短于** `chunk_size`（余数字节），调用方不得假定每块等长。
#[must_use]
pub fn chunk_range(index: u64, chunk_size: u64, file_size: u64) -> Option<(u64, u64)> {
    if chunk_size == 0 {
        return None;
    }
    let offset = index.checked_mul(chunk_size)?;
    if offset >= file_size {
        return None;
    }
    let remaining = file_size - offset;
    let len = remaining.min(chunk_size);
    Some((offset, len))
}

/// 某偏移落在第几块（`offset == file_size` 时归最后一块的下一块，故调用方应先判完成）。
#[must_use]
pub fn chunk_index_for_offset(offset: u64, chunk_size: u64) -> u64 {
    if chunk_size == 0 {
        return 0;
    }
    offset / chunk_size
}

/// 续传起点的**唯一裁决**（F-FT-19 / R-20）。
///
/// 口径（照母本，改动须同步两侧）：
/// * `existing == 0` → 从头传（`0`）
/// * `0 < existing < file_size` → 从 `existing` 接续
/// * `existing >= file_size` → **归零重传**（母本条件是严格小于；`== file_size` 也归零，
///   因为按 F-FT-21 校验通过后 `.part` 会被改名入位，此时不该存在"正好收满"的 `.part`，
///   出现即说明该 `.part` 不可信）
///
/// 🔴 返回 `None` 表示"**调用方应删除残留 `.part` 后从零开始**"，与返回 `Some(0)` 的
/// "本来就该从零开始"在**副作用上不同**，故用 `Option` 区分而不是统一成 0。
#[must_use]
pub fn resume_from(existing: u64, file_size: u64) -> Option<u64> {
    if existing > 0 && existing < file_size {
        Some(existing)
    } else {
        None
    }
}

/// 对端声明的分块大小是否可接受（F-FT-28 准入闸门第 4 关）。
///
/// 拒收条件：`0`（无意义）或 `> MAX_CHUNK_SIZE`（内存放大攻击面）。返回该码或 `None`。
#[must_use]
pub fn chunk_size_mismatch(declared: u64) -> Option<&'static str> {
    if declared == 0 || declared > MAX_CHUNK_SIZE {
        Some(super::reasons::CHUNK_SIZE_MISMATCH)
    } else {
        None
    }
}

/// 传输进度百分比（0–100，保留一位小数；F-FT-25 "进度"列）。
///
/// 🔴 `total == 0` 时返回 `None` → 界面显示"—"，不得显示 0%（否则与"还没开始"混淆）。
#[must_use]
pub fn progress_percent(done: u64, total: u64) -> Option<f64> {
    if total == 0 {
        return None;
    }
    // 用 f64 只为展示；先夹住 done 以免对端谎报导致 >100%
    let clamped = done.min(total);
    #[allow(clippy::cast_precision_loss)] // 展示用，字节数远小于 2^53
    let value = (clamped as f64 / total as f64) * 100.0;
    Some((value * 10.0).round() / 10.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn total_chunks_rounds_up_and_handles_empty() {
        assert_eq!(total_chunks(0, DEFAULT_CHUNK_SIZE), 0);
        assert_eq!(total_chunks(1, DEFAULT_CHUNK_SIZE), 1);
        assert_eq!(total_chunks(DEFAULT_CHUNK_SIZE, DEFAULT_CHUNK_SIZE), 1);
        assert_eq!(total_chunks(DEFAULT_CHUNK_SIZE + 1, DEFAULT_CHUNK_SIZE), 2);
        assert_eq!(total_chunks(5 * DEFAULT_CHUNK_SIZE, DEFAULT_CHUNK_SIZE), 5);
        assert_eq!(total_chunks(123, 0), 0, "块大小为 0 时不得除零");
    }

    #[test]
    fn chunk_range_covers_file_without_gaps_or_overlap() {
        // 用一个小块大小做穷举：文件 10 字节 / 块 3 字节 → 3+3+3+1
        let (size, chunk) = (10_u64, 3_u64);
        let n = total_chunks(size, chunk);
        assert_eq!(n, 4);
        let mut covered = 0_u64;
        let mut expected_offset = 0_u64;
        for index in 0..n {
            let (offset, len) = chunk_range(index, chunk, size).expect("块应存在");
            assert_eq!(offset, expected_offset, "第 {index} 块偏移不连续");
            assert!(len > 0 && len <= chunk);
            expected_offset += len;
            covered += len;
        }
        assert_eq!(covered, size, "各块必须无重叠无空隙地覆盖整个文件");
        assert_eq!(chunk_range(n, chunk, size), None, "越界块必须返回 None");
    }

    #[test]
    fn last_chunk_may_be_short() {
        let (offset, len) = chunk_range(3, 3, 10).unwrap();
        assert_eq!((offset, len), (9, 1), "末块短于块大小");
        let (offset2, len2) = chunk_range(1, DEFAULT_CHUNK_SIZE, DEFAULT_CHUNK_SIZE + 5).unwrap();
        assert_eq!((offset2, len2), (DEFAULT_CHUNK_SIZE, 5));
    }

    #[test]
    fn chunk_index_and_range_are_inverse() {
        let (size, chunk) = (100_u64, 7_u64);
        for index in 0..total_chunks(size, chunk) {
            let (offset, _) = chunk_range(index, chunk, size).unwrap();
            assert_eq!(chunk_index_for_offset(offset, chunk), index);
        }
        assert_eq!(chunk_index_for_offset(0, 0), 0, "块大小为 0 时不得除零");
    }

    /// F-FT-19 / R-20：续传起点的唯一裁决。三条分支各自钉住。
    #[test]
    fn resume_policy_matches_reference_semantics() {
        assert_eq!(resume_from(0, 100), None, "空 .part → 从零（无残留可删）");
        assert_eq!(resume_from(1, 100), Some(1));
        assert_eq!(resume_from(99, 100), Some(99), "差一字节也接续");
        assert_eq!(
            resume_from(100, 100),
            None,
            "正好收满 → 归零（该 .part 不可信）"
        );
        assert_eq!(resume_from(200, 100), None, "超出文件大小 → 归零");
        assert_eq!(resume_from(5, 0), None, "空文件无续传可言");
    }

    /// F-FT-28 第 4 关：分块大小是**安全边界**——0 与超上限都必须以 `CHUNK_SIZE_MISMATCH` 拒收。
    #[test]
    fn declared_chunk_size_is_a_security_bound() {
        use crate::transfer::reasons;
        assert_eq!(chunk_size_mismatch(0), Some(reasons::CHUNK_SIZE_MISMATCH));
        assert_eq!(chunk_size_mismatch(1), None);
        assert_eq!(chunk_size_mismatch(DEFAULT_CHUNK_SIZE), None);
        assert_eq!(
            chunk_size_mismatch(MAX_CHUNK_SIZE),
            None,
            "正好等于上限应放行"
        );
        assert_eq!(
            chunk_size_mismatch(MAX_CHUNK_SIZE + 1),
            Some(reasons::CHUNK_SIZE_MISMATCH),
            "超过上限即拒（否则对端可用超大块逼本端巨量分配）"
        );
    }

    /// F-FT-25：无法判定进度时给 None（界面显示"—"），不得用 0% 冒充。
    #[test]
    fn progress_is_none_when_undeterminable_and_clamped_at_100() {
        assert_eq!(progress_percent(0, 0), None);
        assert_eq!(progress_percent(50, 100), Some(50.0));
        assert_eq!(progress_percent(1, 3), Some(33.3));
        assert_eq!(progress_percent(100, 100), Some(100.0));
        assert_eq!(
            progress_percent(150, 100),
            Some(100.0),
            "对端谎报不得 >100%"
        );
        assert_eq!(progress_percent(0, 100), Some(0.0));
    }
}
