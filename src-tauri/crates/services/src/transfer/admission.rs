//! 准入闸门（F-FT-28 / R-13）：**顺序不可乱**，且拒绝必须发生在产生副作用之前。
//!
//! 规格 R-13 原文顺序：**白名单 → 配对 → 并发 → 分块约定 → 大小合法性 → 磁盘预检 →
//! 冲突裁决 → 建未完成文件**。前六关在本模块裁决（第 7 关是 [`super::conflict`]，
//! 第 8 关是调用方的落盘动作）。
//!
//! 🔴 **为什么顺序必须写死在同一个函数里**：顺序错了不会报错，只会让"拒绝"发生在
//! 已经建了 `.part`、已经占了并发槽之后 —— 表现为磁盘上多出垃圾、并发槽泄漏，
//! 而且要等到压力上来才暴露。故本模块把顺序做成**一个不可绕过的门**，
//! 调用方只能问它"过不过"，不能自己挑着判。
//!
//! 复用结论（DP-1）：③思路复用 ← `FileTransferService.HandleHandshake`
//! （`Source/SystemToolkit/src/SystemToolkit.Core/FileTransfer/Services/FileTransferService.cs`）
//! 的准入分支序列与 `TransferSettings` 的并发上限。

use super::{chunk, reasons};

/// 单文件大小上限。
///
/// 🟠 **本值是本工程自定的**：规格 §二 只在**手机通道**（F-FT-47）给了"同一文件上限 10 GB"，
/// **电脑↔电脑通道未给单文件上限**（F-FT-28 只说"单文件大小非法（负数、超限）→ `INVALID_FILE_SIZE`"）。
/// 本工程取与手机通道一致的 10 GiB，避免两条通道对"能传多大"给出不同答案。
/// 🔴 **待用户确认**（已登记变更记录）。
pub const MAX_FILE_SIZE: u64 = 10 * 1024 * 1024 * 1024;

/// 发送并发上限（F-FT-28；母本 `MaxConcurrentTransfers` 默认 3）。
pub const MAX_SEND_CONCURRENCY: u32 = 3;

/// 接收并发上限（F-FT-28；母本 `MaxConcurrentReceives` 默认 8）。
pub const MAX_RECEIVE_CONCURRENCY: u32 = 8;

/// 磁盘预检结论（F-FT-22 / R-12）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiskSpace {
    /// 剩余空间够（含 128 MB 安全余量）
    Sufficient,
    /// 不够 → 拒收 `INSUFFICIENT_DISK`
    Insufficient,
    /// **无法判定**（网络路径 / 卷未就绪）→ 🔴 **放行但记录**（R-12 明示的保守取舍）
    Unknown,
}

/// 闸门标识（用于把"卡在哪一关"如实回报，而不是笼统说"被拒"）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Gate {
    Whitelist,
    Pairing,
    Concurrency,
    ChunkSize,
    FileSize,
    DiskSpace,
}

impl Gate {
    /// 该关的名称（日志与排查用；🔴 不是界面文案——界面文案取原因码的中文说明）。
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Whitelist => "whitelist",
            Self::Pairing => "pairing",
            Self::Concurrency => "concurrency",
            Self::ChunkSize => "chunk-size",
            Self::FileSize => "file-size",
            Self::DiskSpace => "disk-space",
        }
    }
}

/// 一次准入判定所需的全部事实（调用方不得漏填——漏填即顺序失效）。
#[derive(Debug, Clone, Copy)]
pub struct AdmissionRequest {
    /// 对端是否在"已发现在线"列表内（第 1 关的输入）
    pub peer_discovered: bool,
    /// "仅接受已发现设备"开关（F-FT-09，默认开）
    pub require_known_peer: bool,
    /// 配对是否已通过（第 2 关；🔴 关闭白名单**不会**放松这一关，R-1）
    pub paired: bool,
    /// 当前进行中的接收数
    pub active_receives: u32,
    /// 对端在本块声明的分块大小
    pub declared_chunk_size: u64,
    /// 对端声明的完整文件大小
    pub file_size: u64,
    /// 磁盘预检结论
    pub disk: DiskSpace,
}

/// 准入结论。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Admission {
    /// 放行。`disk_unknown` 为真表示"空间无法判定但按 R-12 放行"，**调用方必须记录**该事实。
    Admitted { disk_unknown: bool },
    /// 拒绝：卡在哪一关 + 回传哪个原因码。🔴 此时**不得有任何副作用**。
    Rejected { gate: Gate, reason: &'static str },
}

impl Admission {
    #[must_use]
    pub fn is_admitted(self) -> bool {
        matches!(self, Self::Admitted { .. })
    }
}

/// **唯一准入门**（F-FT-28 / R-13）。按规范顺序逐关判定，返回**第一个**失败关。
///
/// 🔴 调用方必须是"先问它、再动手"：本函数不做 IO，也就不可能替调用方保证"没建 `.part`"——
/// 顺序纪律由调用点遵守（先 `evaluate` 再落盘）。
#[must_use]
pub fn evaluate(req: &AdmissionRequest) -> Admission {
    // 第 1 关：白名单（仅"便利过滤"，见 R-1；关闭它不放松第 2 关）
    if req.require_known_peer && !req.peer_discovered {
        return Admission::Rejected {
            gate: Gate::Whitelist,
            reason: reasons::PEER_NOT_DISCOVERED,
        };
    }
    // 第 2 关：配对——唯一信任根，任何跨机写入前都必须在场
    if !req.paired {
        return Admission::Rejected {
            gate: Gate::Pairing,
            reason: reasons::PAIRING_INVALID,
        };
    }
    // 第 3 关：并发（接收侧；发送侧由发送方自己那把闸管，见 MAX_SEND_CONCURRENCY）
    if req.active_receives >= MAX_RECEIVE_CONCURRENCY {
        return Admission::Rejected {
            gate: Gate::Concurrency,
            reason: reasons::CONCURRENCY_LIMIT,
        };
    }
    // 第 4 关：分块约定（安全边界，见 chunk::chunk_size_mismatch）
    if let Some(reason) = chunk::chunk_size_mismatch(req.declared_chunk_size) {
        return Admission::Rejected {
            gate: Gate::ChunkSize,
            reason,
        };
    }
    // 第 5 关：大小合法性（u64 表达不出负数；负数在 serde 反序列化阶段即被拒，
    // 故 INVALID_FILE_SIZE 在此覆盖"超上限"与手机通道的声明值异常）
    if req.file_size > MAX_FILE_SIZE {
        return Admission::Rejected {
            gate: Gate::FileSize,
            reason: reasons::INVALID_FILE_SIZE,
        };
    }
    // 第 6 关：磁盘预检（不足即拒；无法判定则放行但记录 —— R-12）
    match req.disk {
        DiskSpace::Insufficient => Admission::Rejected {
            gate: Gate::DiskSpace,
            reason: reasons::INSUFFICIENT_DISK,
        },
        DiskSpace::Sufficient => Admission::Admitted {
            disk_unknown: false,
        },
        DiskSpace::Unknown => Admission::Admitted { disk_unknown: true },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 一份"全绿"的请求，各用例只改待测的那一项。
    fn ok_request() -> AdmissionRequest {
        AdmissionRequest {
            peer_discovered: true,
            require_known_peer: true,
            paired: true,
            active_receives: 0,
            declared_chunk_size: chunk::DEFAULT_CHUNK_SIZE,
            file_size: 1024,
            disk: DiskSpace::Sufficient,
        }
    }

    #[test]
    fn all_green_is_admitted_without_disk_caveat() {
        assert_eq!(
            evaluate(&ok_request()),
            Admission::Admitted {
                disk_unknown: false
            }
        );
    }

    /// 🔴 R-13 的核心性质：**多关同时失败时，返回的是顺序更靠前的那一关**。
    /// 这条用例是"顺序不可乱"的回归闸——把任何两关对调都会让它红。
    #[test]
    fn gate_order_is_enforced_when_everything_fails() {
        let all_bad = AdmissionRequest {
            peer_discovered: false,
            require_known_peer: true,
            paired: false,
            active_receives: MAX_RECEIVE_CONCURRENCY, // 并发满
            declared_chunk_size: 0,                   // 分块非法
            file_size: MAX_FILE_SIZE + 1,             // 超大小
            disk: DiskSpace::Insufficient,            // 磁盘不足
        };
        assert_eq!(
            evaluate(&all_bad),
            Admission::Rejected {
                gate: Gate::Whitelist,
                reason: reasons::PEER_NOT_DISCOVERED
            },
            "第 1 关必须最先报"
        );

        // 逐关放行，下一关接班
        let mut req = all_bad;
        req.peer_discovered = true;
        assert_eq!(
            evaluate(&req),
            Admission::Rejected {
                gate: Gate::Pairing,
                reason: reasons::PAIRING_INVALID
            }
        );
        req.paired = true;
        assert_eq!(
            evaluate(&req),
            Admission::Rejected {
                gate: Gate::Concurrency,
                reason: reasons::CONCURRENCY_LIMIT
            }
        );
        req.active_receives = 0;
        assert_eq!(
            evaluate(&req),
            Admission::Rejected {
                gate: Gate::ChunkSize,
                reason: reasons::CHUNK_SIZE_MISMATCH
            }
        );
        req.declared_chunk_size = chunk::DEFAULT_CHUNK_SIZE;
        assert_eq!(
            evaluate(&req),
            Admission::Rejected {
                gate: Gate::FileSize,
                reason: reasons::INVALID_FILE_SIZE
            }
        );
        req.file_size = 1024;
        assert_eq!(
            evaluate(&req),
            Admission::Rejected {
                gate: Gate::DiskSpace,
                reason: reasons::INSUFFICIENT_DISK
            }
        );
        req.disk = DiskSpace::Sufficient;
        assert!(evaluate(&req).is_admitted(), "全部放行后应通过");
    }

    /// 🔴 R-1 原文："关闭白名单**不会**放松配对要求"。这条把该语义钉死。
    #[test]
    fn disabling_whitelist_does_not_relax_pairing() {
        let mut req = ok_request();
        req.require_known_peer = false;
        req.peer_discovered = false; // 白名单关了，这一项无关
        req.paired = false; // 但没配对
        assert_eq!(
            evaluate(&req),
            Admission::Rejected {
                gate: Gate::Pairing,
                reason: reasons::PAIRING_INVALID
            },
            "白名单关闭后仍必须要求配对"
        );

        // 关了白名单 + 已配对 → 即使未发现也应放行（便利过滤不是安全边界）
        req.paired = true;
        assert!(evaluate(&req).is_admitted());
    }

    /// R-12：空间**无法判定**时放行，但必须把该事实传出去（调用方要记录）。
    #[test]
    fn unknown_disk_is_admitted_but_flagged() {
        let mut req = ok_request();
        req.disk = DiskSpace::Unknown;
        assert_eq!(
            evaluate(&req),
            Admission::Admitted { disk_unknown: true },
            "无法判定 → 放行但标记（不得谎称空间充足）"
        );
    }

    #[test]
    fn receive_concurrency_upper_bound_is_exclusive() {
        let mut req = ok_request();
        req.active_receives = MAX_RECEIVE_CONCURRENCY - 1;
        assert!(evaluate(&req).is_admitted(), "上限前一个仍可受理");
        req.active_receives = MAX_RECEIVE_CONCURRENCY;
        assert!(!evaluate(&req).is_admitted(), "达到上限即拒");
    }

    #[test]
    fn zero_length_file_is_allowed_and_oversize_is_not() {
        let mut req = ok_request();
        req.file_size = 0;
        assert!(
            evaluate(&req).is_admitted(),
            "空文件是合法文件，不按非法大小拒"
        );
        req.file_size = MAX_FILE_SIZE;
        assert!(evaluate(&req).is_admitted(), "正好等于上限应放行");
        req.file_size = MAX_FILE_SIZE + 1;
        assert!(!evaluate(&req).is_admitted());
    }

    #[test]
    fn gate_labels_are_stable_lowercase_ids() {
        for (gate, label) in [
            (Gate::Whitelist, "whitelist"),
            (Gate::Pairing, "pairing"),
            (Gate::Concurrency, "concurrency"),
            (Gate::ChunkSize, "chunk-size"),
            (Gate::FileSize, "file-size"),
            (Gate::DiskSpace, "disk-space"),
        ] {
            assert_eq!(gate.as_str(), label);
        }
    }
}
