//! 原因码：机器可读码 + 中文说明成对携带（F-FT-31 / §五「原因码对照」）。
//!
//! 复用结论（DP-1）：③思路复用 ←
//! `Source/SystemToolkit/src/SystemToolkit.Core/FileTransfer/Models/TransferReasonCodes.cs`
//! （GPL-3.0 同源，ADR-003 五步登记见 `Aegis/DEPENDENCIES.md §三`）。**16 项逐条对照、未增未减**，
//! 中文说明照抄母本 `Describe()` 的措辞（用户界面文案，不得由实现者改写）。
//!
//! 🔴 这些字符串是**线协议契约**：两条通道（电脑↔电脑 / 手机浏览器）与历史条目、
//! 操作日志、界面"原因"列共用同一套码，同一码在两通道含义必须一致（R-16「以对端回传的原因码为权威」）。
//! **改动任一码的拼写即破坏兼容**（历史条目里存的是字符串），故本模块带码表稳定性测试。
//!
//! 🔴 未知码**原样显示、不吞、不硬翻**（F-FT-31 末句）：[`describe`] 返回 `None` 时，
//! 调用方应把原始码直接展示，而不是替换为"未知错误"——母本 `Describe()` 即原样返回码本身。

/// 一个原因码：机器可读码 + 面向用户的中文说明。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Reason {
    /// 线协议上的机器可读码（全大写下划线；🔴 不得改写）
    pub code: &'static str,
    /// 界面"原因"列展示的中文说明（🔴 照抄母本措辞）
    pub text: &'static str,
}

// ── 16 个码（常量供代码内引用，避免调用点散落字面量） ──────────────────────────
/// 第 1 层白名单拦下：对端未被发现、或不在已知清单内。
pub const PEER_NOT_DISCOVERED: &str = "PEER_NOT_DISCOVERED";
/// 配对码无效、已使用或已过期（R-2 一次性消费）。
pub const PAIRING_INVALID: &str = "PAIRING_INVALID";
/// 并发已满（发送 3 / 接收 8，F-FT-28）。
pub const CONCURRENCY_LIMIT: &str = "CONCURRENCY_LIMIT";
/// 分片大小与接收端约定不一致（F-FT-28 准入闸门第 4 关）。
pub const CHUNK_SIZE_MISMATCH: &str = "CHUNK_SIZE_MISMATCH";
/// 文件大小非法或超出单文件上限。
pub const INVALID_FILE_SIZE: &str = "INVALID_FILE_SIZE";
/// 接收目录剩余空间不足（含 128 MB 安全余量，F-FT-22）。
pub const INSUFFICIENT_DISK: &str = "INSUFFICIENT_DISK";
/// 同名文件已存在，按策略跳过未接收（F-FT-23；终态为"已跳过"而非"已完成"）。
pub const CONFLICT_SKIP: &str = "CONFLICT_SKIP";
/// 无确认闸道的通道上"每次询问"已降级为改名（F-FT-23 / R-10）。
pub const CONFLICT_ASK_UNAVAILABLE: &str = "CONFLICT_ASK_UNAVAILABLE";
/// 用户在接收确认中拒绝（F-FT-16，默认动作）。
pub const USER_REJECT: &str = "USER_REJECT";
/// 接收确认超时，按拒绝处理（F-FT-16；🔴 不是业务失败）。
pub const CONFIRM_TIMEOUT: &str = "CONFIRM_TIMEOUT";
/// 全文件校验不一致，损坏的未完成文件已删除（F-FT-20）。
pub const HASH_MISMATCH: &str = "HASH_MISMATCH";
/// 用户主动取消（F-FT-29；已完整接收的文件保留）。
pub const USER_CANCEL: &str = "USER_CANCEL";
/// 对端断开连接（R-16；与取消/超时各自独立，绝不合并成"失败"）。
pub const PEER_DISCONNECTED: &str = "PEER_DISCONNECTED";
/// 本地暂停超过上限后自动取消（F-FT-27；🔴 不得被"用户取消"掩盖）。
pub const PAUSE_TIMEOUT: &str = "PAUSE_TIMEOUT";
/// 文本超出单条 256 KB（UTF-8 字节）上限（F-FT-32；拒绝而非截断）。
pub const TEXT_TOO_LONG: &str = "TEXT_TOO_LONG";
/// 剪贴板写入失败，接收未成立（F-FT-34；回执 ≠ 交付）。
pub const CLIPBOARD_WRITE_FAILED: &str = "CLIPBOARD_WRITE_FAILED";

/// 码表全集（顺序与规格 §五「原因码对照」一致；🔴 16 项）。
pub const ALL: [Reason; 16] = [
    Reason {
        code: PEER_NOT_DISCOVERED,
        text: "来源设备不在已发现在线列表",
    },
    Reason {
        code: PAIRING_INVALID,
        text: "配对码无效或已过期",
    },
    Reason {
        code: CONCURRENCY_LIMIT,
        text: "接收方并发传输已达上限",
    },
    Reason {
        code: CHUNK_SIZE_MISMATCH,
        text: "分片大小与接收端不一致",
    },
    Reason {
        code: INVALID_FILE_SIZE,
        text: "文件大小非法",
    },
    Reason {
        code: INSUFFICIENT_DISK,
        text: "接收方磁盘空间不足",
    },
    Reason {
        code: CONFLICT_SKIP,
        text: "同名文件已存在，按策略跳过",
    },
    Reason {
        code: CONFLICT_ASK_UNAVAILABLE,
        text: "策略为询问但无法询问，已按自动改名处理",
    },
    Reason {
        code: USER_REJECT,
        text: "接收方用户拒绝",
    },
    Reason {
        code: CONFIRM_TIMEOUT,
        text: "接收方未在时限内确认",
    },
    Reason {
        code: HASH_MISMATCH,
        text: "SHA-256 校验不通过",
    },
    Reason {
        code: USER_CANCEL,
        text: "已取消",
    },
    Reason {
        code: PEER_DISCONNECTED,
        text: "对端断开连接",
    },
    Reason {
        code: PAUSE_TIMEOUT,
        text: "暂停超时，已自动取消",
    },
    Reason {
        code: TEXT_TOO_LONG,
        text: "文本超出单条上限",
    },
    Reason {
        code: CLIPBOARD_WRITE_FAILED,
        text: "接收方写入剪贴板失败（文本未交付）",
    },
];

/// 查中文说明；**未知码返回 `None`**（调用方须原样显示码本身，不得替换为"未知错误"）。
#[must_use]
pub fn describe(code: &str) -> Option<&'static str> {
    ALL.iter()
        .find(|reason| reason.code == code)
        .map(|reason| reason.text)
}

/// 该码是否在 16 项内置码表内（F-FT-31 的"未知码原样显示"靠它分流）。
#[must_use]
pub fn is_known(code: &str) -> bool {
    ALL.iter().any(|reason| reason.code == code)
}

/// 界面"原因"列的最终展示文本：已知码给中文说明，未知码**原样返回码本身**。
///
/// 每次调用都分配一次 `String`，但"原因"列只在任务终态与历史渲染时取值，不在热路径上。
#[must_use]
pub fn display_text(code: &str) -> String {
    match describe(code) {
        Some(text) => text.to_owned(),
        None => code.to_owned(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 🔴 码表是**线协议契约**：数量、拼写、唯一性任一变都破坏与历史条目的兼容。
    /// 本用例把 16 项逐条钉死（对照母本 `TransferReasonCodes.cs`）。
    #[test]
    fn code_table_is_frozen_contract() {
        assert_eq!(ALL.len(), 16, "规格 §五 明列 16 项，不得增删");
        let expected = [
            "PEER_NOT_DISCOVERED",
            "PAIRING_INVALID",
            "CONCURRENCY_LIMIT",
            "CHUNK_SIZE_MISMATCH",
            "INVALID_FILE_SIZE",
            "INSUFFICIENT_DISK",
            "CONFLICT_SKIP",
            "CONFLICT_ASK_UNAVAILABLE",
            "USER_REJECT",
            "CONFIRM_TIMEOUT",
            "HASH_MISMATCH",
            "USER_CANCEL",
            "PEER_DISCONNECTED",
            "PAUSE_TIMEOUT",
            "TEXT_TOO_LONG",
            "CLIPBOARD_WRITE_FAILED",
        ];
        for (index, code) in expected.iter().enumerate() {
            assert_eq!(
                ALL[index].code,
                *code,
                "第 {} 项的码拼写被改动（历史条目按字符串存储，改拼写即破坏兼容）",
                index + 1
            );
        }
    }

    #[test]
    fn every_code_is_unique_and_has_chinese_text() {
        let mut seen = std::collections::HashSet::new();
        for reason in ALL {
            assert!(seen.insert(reason.code), "码重复：{}", reason.code);
            assert!(
                !reason.text.is_empty()
                    && reason
                        .text
                        .chars()
                        .any(|c| ('\u{4e00}'..='\u{9fff}').contains(&c)),
                "{} 的中文说明缺失或非中文：{}",
                reason.code,
                reason.text
            );
        }
    }

    /// F-FT-31 末句：「未知码原样显示（不吞掉、不硬翻）」。这两条是它的回归闸。
    #[test]
    fn unknown_code_is_shown_verbatim_never_swallowed() {
        assert_eq!(describe("SOME_FUTURE_CODE"), None);
        assert!(!is_known("SOME_FUTURE_CODE"));
        assert_eq!(
            display_text("SOME_FUTURE_CODE"),
            "SOME_FUTURE_CODE",
            "未知码必须原样显示，不得替换为『未知错误』"
        );
    }

    #[test]
    fn known_codes_resolve_to_their_text() {
        assert_eq!(describe(USER_CANCEL), Some("已取消"));
        assert_eq!(display_text(PAUSE_TIMEOUT), "暂停超时，已自动取消");
        assert!(is_known(CLIPBOARD_WRITE_FAILED));
    }
}
