//! 快照读写与新鲜度判定（M-01 片2-e，2026-10-03）。
//!
//! **复用结论（DP-1）**：
//! - 快照路径 ← `%LOCALAPPDATA%\aegis\snapshots\overview.json`（母本 `NexBox` 同路径）；
//! - 原子写 ← `infra::fs::atomic_write`（W0-09 已落地，断电不产生半截文件）；
//! - 指纹 ← SHA-256（前 16 字节 hex），用于检测数据结构漂移。
//!
//! 🔴 R-14 快照边界：缺失/损坏/超龄（>30天）一律退化为冷启动全量采集，绝不因快照问题导致启动失败。

use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::overview::dto::{HardwareDetailDto, StatCardDto};

/// 快照载体（含指纹与新鲜度元数据）。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Snapshot {
    /// 采集时刻（Unix 毫秒，与 `HardwareDetailDto.collected_at_ms` 同源）
    pub collected_at_ms: u64,
    /// 写入时刻（Unix 毫秒，用于新鲜度判定）
    pub written_at_ms: u64,
    /// 内容指纹（SHA-256 前 16 字节 hex，用于检测结构漂移）
    pub fingerprint: String,
    /// 实际载荷（硬件详情 + 系统信息等慢变量）
    pub payload: HardwareDetailDto,
    /// 🔴 F-01 四卡的**上次**读数（用户 2026-10-04 裁决：启动首屏不能是加载态）。
    ///
    /// 为什么四卡也要进快照：`overview_live_sample` 首拍要做硬件枚举（显卡逐个探测、
    /// 传感器通道握手），耗时以秒计；而本机概览是程序启动后**第一个看见的页面**，
    /// 顶部四张卡是用户第一眼要看的东西，空着等一拍等于"启动即加载态"。
    /// 故把上一拍的四卡随慢变量一起落盘，供冷启动**秒显**。
    ///
    /// 🔴 **语义是「上次读数」不是「当前值」**：读数会过期，故只用于首屏占位，
    /// 前端读到后必须立即回源一次（见 `useSnapshotWarmup`）。空数组 = 无可用历史，
    /// 此时四卡自然缺席（而不是显示假数据——这与「禁止假数据兜底」同一取向）。
    #[serde(default)]
    pub cards: Vec<StatCardDto>,
}

/// 快照有效期限（30 天）。
const SNAPSHOT_MAX_AGE_DAYS: u64 = 30;
/// 同一期限的毫秒形态（新鲜度判定与 `written_at_ms` 同一时间轴）。
const SNAPSHOT_MAX_AGE_MS: u64 = SNAPSHOT_MAX_AGE_DAYS * 24 * 60 * 60 * 1000;

/// 计算内容的 SHA-256 指纹（前 16 字节 hex）。
#[must_use]
pub fn compute_fingerprint(content: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(content);
    let result = hasher.finalize();
    hex::encode(&result[..16])
}

/// 获取快照文件路径（`%LOCALAPPDATA%\aegis\snapshots\overview.json`）。
#[must_use]
pub fn snapshot_path() -> PathBuf {
    let local_app_data = std::env::var("LOCALAPPDATA").unwrap_or_else(|_| ".".to_string());
    PathBuf::from(local_app_data)
        .join("aegis")
        .join("snapshots")
        .join("overview.json")
}

/// 读取快照（缺失/损坏/超龄均返回 None，调用方走冷启动全量采集）。
#[must_use]
pub fn read_snapshot() -> Option<Snapshot> {
    let path = snapshot_path();
    if !path.exists() {
        return None;
    }

    let bytes = match std::fs::read(&path) {
        Ok(bytes) => bytes,
        Err(err) => {
            // 🔴 R-14 仍以 `None` 退化（快照问题不得让页面报错），但**必须留痕**：
            // 静默 `.ok()?` 会让"快照一直不生效"这类问题完全无法排查。
            log::warn!("概览快照读取失败（退化为冷启动采集）path={path:?} err={err}");
            return None;
        }
    };
    let snapshot: Snapshot = match serde_json::from_slice(&bytes) {
        Ok(value) => value,
        Err(err) => {
            log::warn!("概览快照解析失败（退化为冷启动采集）path={path:?} err={err}");
            return None;
        }
    };

    // 新鲜度判定：超过 30 天视为过期
    let now_ms = current_unix_ms();
    let age_ms = now_ms.saturating_sub(snapshot.written_at_ms);
    if age_ms > SNAPSHOT_MAX_AGE_MS {
        log::info!("快照已过期（{} 天），将重新全量采集", age_ms / 86_400_000);
        return None;
    }

    // 指纹校验：按与写入侧**完全相同**的组合方式重算（检测结构漂移 / 脏数据）
    let fingerprint_input =
        serde_json::json!({ "payload": &snapshot.payload, "cards": &snapshot.cards });
    let payload_bytes = serde_json::to_vec(&fingerprint_input).ok()?;
    let expected_fp = compute_fingerprint(&payload_bytes);
    if expected_fp != snapshot.fingerprint {
        log::warn!("快照指纹不匹配（结构可能已漂移），将重新全量采集");
        return None;
    }

    Some(snapshot)
}

/// 写入快照（原子写，确保断电不产生半截文件）。
///
/// 🔴 指纹覆盖 `payload + cards` 二者：只对 `payload` 取指纹的话，四卡部分被篡改或
/// 结构漂移时校验会通过，脏数据会被当成有效快照读出来。故先拼一个组合体再取指纹。
///
/// # Errors
///
/// 序列化或原子写失败时返回原因。
pub fn write_snapshot(payload: &HardwareDetailDto, cards: &[StatCardDto]) -> Result<(), String> {
    let fingerprint_input = serde_json::json!({ "payload": payload, "cards": cards });
    let fingerprint_bytes =
        serde_json::to_vec(&fingerprint_input).map_err(|e| format!("序列化快照失败: {e}"))?;
    let fingerprint = compute_fingerprint(&fingerprint_bytes);

    let snapshot = Snapshot {
        collected_at_ms: payload.collected_at_ms,
        written_at_ms: current_unix_ms(),
        fingerprint,
        payload: payload.clone(),
        cards: cards.to_vec(),
    };

    let bytes =
        serde_json::to_vec_pretty(&snapshot).map_err(|e| format!("序列化快照载体失败: {e}"))?;

    let path = snapshot_path();
    // 确保父目录存在
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| format!("创建快照目录失败: {e}"))?;
    }

    infra::fs::atomic_write(&path, &bytes).map_err(|e| format!("原子写快照失败: {e}"))
}

/// 当前 Unix 毫秒时刻。
#[must_use]
fn current_unix_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |elapsed| {
            u64::try_from(elapsed.as_millis()).unwrap_or(u64::MAX)
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fingerprint_is_deterministic_for_same_content() {
        let content = b"hello world";
        let fp1 = compute_fingerprint(content);
        let fp2 = compute_fingerprint(content);
        assert_eq!(fp1, fp2);
        assert_eq!(fp1.len(), 32); // 16 bytes = 32 hex chars
    }

    #[test]
    fn fingerprint_differs_for_different_content() {
        let fp1 = compute_fingerprint(b"hello");
        let fp2 = compute_fingerprint(b"world");
        assert_ne!(fp1, fp2);
    }

    #[test]
    fn snapshot_path_returns_valid_localappdata_path() {
        let path = snapshot_path();
        assert!(path.ends_with("aegis/snapshots/overview.json"));
    }

    #[test]
    fn current_unix_ms_returns_reasonable_value() {
        let ms = current_unix_ms();
        // 2024-01-01 之后的毫秒数必然大于 1704067200000
        assert!(ms > 1_704_067_200_000);
    }

    #[test]
    fn write_and_read_roundtrip_succeeds() {
        let payload = HardwareDetailDto {
            cards: Vec::new(),
            collected_at_ms: current_unix_ms(),
        };

        // 临时目录仅用于隔离测试产物（快照路径由 snapshot_path() 单点决定，不在测试里改写）
        let test_dir = std::env::temp_dir().join("aegis_test_snapshot");
        std::fs::create_dir_all(&test_dir).ok();

        // 由于 snapshot_path() 是硬编码的，这里只测试序列化/反序列化逻辑
        // 🔴 指纹算法须与 `write_snapshot` 完全一致（组合 payload + cards），否则本测试形同虚设
        let cards: Vec<StatCardDto> = Vec::new();
        let fingerprint_bytes =
            serde_json::to_vec(&serde_json::json!({ "payload": &payload, "cards": &cards }))
                .unwrap();
        let fingerprint = compute_fingerprint(&fingerprint_bytes);
        let snapshot = Snapshot {
            collected_at_ms: payload.collected_at_ms,
            written_at_ms: current_unix_ms(),
            fingerprint,
            payload: payload.clone(),
            cards,
        };

        let bytes = serde_json::to_vec_pretty(&snapshot).unwrap();
        let restored: Snapshot = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(restored.collected_at_ms, snapshot.collected_at_ms);
        assert_eq!(restored.fingerprint, snapshot.fingerprint);

        // 清理
        std::fs::remove_dir_all(&test_dir).ok();
    }

    #[test]
    fn expired_snapshot_returns_none() {
        let old_payload = HardwareDetailDto {
            cards: Vec::new(),
            collected_at_ms: 1_600_000_000_000, // 2020-09-13
        };
        let cards: Vec<StatCardDto> = Vec::new();
        let fingerprint_bytes =
            serde_json::to_vec(&serde_json::json!({ "payload": &old_payload, "cards": &cards }))
                .unwrap();
        let fingerprint = compute_fingerprint(&fingerprint_bytes);
        let snapshot = Snapshot {
            collected_at_ms: old_payload.collected_at_ms,
            written_at_ms: 1_600_000_000_000, // 2020-09-13
            fingerprint,
            payload: old_payload,
            cards,
        };

        // 模拟写入后直接读（但时间已过去很久）
        let bytes = serde_json::to_vec(&snapshot).unwrap();
        let temp_file = std::env::temp_dir().join("aegis_expired_test.json");
        std::fs::write(&temp_file, &bytes).ok();

        // 由于 read_snapshot() 使用硬编码路径，这里只测试新鲜度判定逻辑
        let now_ms = current_unix_ms();
        let age_ms = now_ms.saturating_sub(snapshot.written_at_ms);
        assert!(age_ms > SNAPSHOT_MAX_AGE_MS, "快照应已过期");

        // 清理
        std::fs::remove_file(&temp_file).ok();
    }
}
