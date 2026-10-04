//! 电脑↔电脑通道的线协议：帧格式与消息类型（片1 核心，F-FT-02 / F-FT-13 / F-FT-19~21 / F-FT-34）。
//!
//! ## 🔴 本协议是本工程**自定**的，不是移植
//!
//! 母本电脑通道走 `WatsonTcp`（.NET 专有库，Rust 无对应物；且本工程**不要求与旧版互通**——
//! 两端都是 Aegis）。故帧格式与序列化都由本模块定义，`DEPENDENCIES.md §三` 据此**不登记移植**，
//! 只登记"消息类型与语义③思路复用自 `TransferProtocol.cs`"。
//!
//! ## 帧格式
//!
//! ```text
//! 偏移  长度  字段
//! 0     4    magic      = b"AGFT"
//! 4     1    version    = 1
//! 5     4    header_len （u32，**大端**）JSON 头的字节数
//! 9     4    body_len   （u32，大端）紧随其后的裸字节数
//! 13    H    header     UTF-8 JSON（[`TransferMessage`]）
//! 13+H  B    body       裸字节（分块数据 / 文本 UTF-8；无载荷时为 0 字节）
//! ```
//!
//! 设计取舍：
//! * **定长前缀把所有长度都放在 JSON 之外** —— 解析器能在**分配任何缓冲之前**校验
//!   `header_len` / `body_len` 上限。若把长度放进 JSON，就必须先解析完不可信的 JSON
//!   才知道该留多大缓冲（长度前置是防内存放大的关键）。
//! * **大端** —— 网络序，抓包时肉眼可读。
//! * **控制头用 JSON** —— 可抓包调试、可跨语言复核；块数据走裸字节，**不**做 base64
//!   （那会放大约 33% 且多一次编解码）。代价是头部有解析开销，但头部每块才一次。
//! * 🔴 **不带 `ChunkAck`**：母本协议里声明了该类型但全仓零引用（规格 §七 限制 10 也这么记）。
//!   新协议**不设**该变体——一个永远不发的消息类型只会诱导对端去发它。
//!
//! ## 与母本的消息名对齐
//!
//! 变体名（`Handshake` / `HandshakeAck` / `Chunk` / `Complete` / `CompleteAck` / `Cancel` /
//! `Pause` / `Resume` / `Text` / `TextAck` / `Error`）与母本 `TransferMessageType` **逐个对齐**，
//! 便于双向复核；字段名按本仓惯例用 camelCase。

use serde::{Deserialize, Serialize};

/// 帧魔数。用于让"误连到本端口的路人"在第一帧就被拒，而不是让我们去解析它的垃圾。
pub const MAGIC: [u8; 4] = *b"AGFT";

/// 协议版本。🔴 变更语义必须递增；对端版本不符即整连接拒绝（不做降级协商）。
pub const PROTOCOL_VERSION: u8 = 1;

/// 定长前缀长度：`magic`(4) + `version`(1) + `header_len`(4) + `body_len`(4)。
pub const PREFIX_LEN: usize = 13;

/// JSON 头上限。控制头只装元数据（文件名、大小、哈希、原因码…），64 KiB 绰绰有余。
pub const MAX_HEADER_BYTES: u32 = 64 * 1024;

/// 裸载荷上限。v1 一帧最多承载一个分块，故与 [`crate::transfer::chunk::MAX_CHUNK_SIZE`] 同值
/// （由 `max_body_matches_chunk_cap` 用例钉住两者不得漂移）。
pub const MAX_BODY_BYTES: u32 = 4 * 1024 * 1024;

/// 消息类型（🔴 变体名即线协议上的 `type` 值，与母本逐个对齐；改名即破坏兼容）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(
    tag = "type",
    rename_all = "PascalCase",
    rename_all_fields = "camelCase"
)]
pub enum TransferMessage {
    /// 发送方开场：文件元数据 + 本次运行期的配对凭据（F-FT-13）。
    Handshake {
        task_id: String,
        file_name: String,
        file_size: u64,
        /// 发送方对**整个文件**的 SHA-256（十六进制小写，F-FT-20）
        file_hash: String,
        /// 发送方声明的分块大小（🔴 接收方须按 `chunk::chunk_size_mismatch` 校验，超限即拒）
        chunk_size: u64,
        /// 对端声明的原始修改时间（Unix 毫秒，F-FT-24；超出合理窗口时静默忽略）
        file_modified_at_ms: Option<i64>,
        /// 本次运行期所需的配对凭据（免码地址可省）
        pair_code: Option<String>,
    },
    /// 接收方回执：准入结论 + 续传起点 + 最终采用的分块大小（F-FT-19/28）。
    HandshakeAck {
        task_id: String,
        accepted: bool,
        /// 拒收时的原因码（[`super::reasons`]）
        reason_code: Option<String>,
        /// 续传起点（F-FT-19）；`None` 表示从 0 开始
        resume_from: Option<u64>,
        /// 接收方最终采用的分块大小（可能与请求值不同）
        chunk_size: Option<u64>,
    },
    /// 一个数据块（偏移见字段；裸字节在 body）。
    Chunk {
        task_id: String,
        /// 块序号（从 0 起）
        index: u64,
        /// 本块在文件中的起始偏移
        offset: u64,
    },
    /// 发送完毕，附全文件摘要供接收方比对（F-FT-20）。
    Complete { task_id: String, file_hash: String },
    /// 接收方对 `Complete` 的终态回执（F-FT-21/30）。
    CompleteAck {
        task_id: String,
        ok: bool,
        reason_code: Option<String>,
    },
    /// 双向取消（F-FT-29；原因码区分用户取消 / 超时 / 断连）。
    Cancel {
        task_id: String,
        reason_code: String,
    },
    /// 本端暂停（语义："我这边暂停了，你不要再发数据"，在块边界生效，F-FT-26）。
    Pause { task_id: String },
    /// 本端恢复。
    Resume { task_id: String },
    /// 文本消息（裸字节在 body，UTF-8；字符/字节数一并携带供对端如实显示，F-FT-15/32）。
    Text {
        task_id: String,
        chars: u64,
        bytes: u64,
        from_device: Option<String>,
    },
    /// 文本回执。🔴 电脑通道上 `ok` 的含义是**剪贴板写入成功**（F-FT-34：回执 = 交付）。
    TextAck {
        task_id: String,
        ok: bool,
        reason_code: Option<String>,
    },
    /// 通用错误（无法归入上述终态的异常路径）。
    Error {
        task_id: Option<String>,
        reason_code: String,
        message: Option<String>,
    },
}

/// 帧头解析结果（**不含** body 字节本身）。
///
/// body 在缓冲区中的起点由 [`parse_head`] 一并返回（见其文档）——**不**在本结构上挂
/// `body_offset()` 之类的便捷方法：那需要重算一次 JSON 长度，多一个会与解析路径漂移的口径。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FrameHead {
    pub message: TransferMessage,
    /// 紧随头之后的裸字节数
    pub body_len: u32,
}

/// 帧错误（🔴 全部**不携带任何对端可控内容**，避免把攻击者的字节回显进日志/界面）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FrameError {
    /// 魔数不符（对端不是本协议，或连错了端口）
    BadMagic,
    /// 协议版本不符（不做降级协商）
    UnsupportedVersion(u8),
    /// 头长度超上限
    HeaderTooLarge(u32),
    /// 载荷长度超上限
    BodyTooLarge(u32),
    /// 头无法解析为合法 JSON / 消息形状不符
    MalformedHeader(String),
}

impl std::fmt::Display for FrameError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::BadMagic => write!(f, "帧魔数不符（对端不是本协议或连错端口）"),
            Self::UnsupportedVersion(v) => {
                write!(f, "协议版本不符：对端 v{v}，本端 v{PROTOCOL_VERSION}")
            }
            Self::HeaderTooLarge(n) => write!(f, "帧头上限 {MAX_HEADER_BYTES} 字节，对端声明 {n}"),
            Self::BodyTooLarge(n) => write!(f, "载荷上限 {MAX_BODY_BYTES} 字节，对端声明 {n}"),
            Self::MalformedHeader(why) => write!(f, "帧头无法解析：{why}"),
        }
    }
}

impl std::error::Error for FrameError {}

/// 把一条消息与可选载荷编码成一帧。
///
/// # Errors
/// 头 JSON 序列化失败、或载荷超过 [`MAX_BODY_BYTES`] 时返回 [`FrameError`]。
pub fn encode_frame(msg: &TransferMessage, body: &[u8]) -> Result<Vec<u8>, FrameError> {
    let header = serde_json::to_vec(msg).map_err(|e| FrameError::MalformedHeader(e.to_string()))?;
    let header_len =
        u32::try_from(header.len()).map_err(|_| FrameError::HeaderTooLarge(u32::MAX))?;
    if header_len > MAX_HEADER_BYTES {
        return Err(FrameError::HeaderTooLarge(header_len));
    }
    let body_len = u32::try_from(body.len()).map_err(|_| FrameError::BodyTooLarge(u32::MAX))?;
    if body_len > MAX_BODY_BYTES {
        return Err(FrameError::BodyTooLarge(body_len));
    }

    let mut out = Vec::with_capacity(PREFIX_LEN + header.len() + body.len());
    out.extend_from_slice(&MAGIC);
    out.push(PROTOCOL_VERSION);
    out.extend_from_slice(&header_len.to_be_bytes());
    out.extend_from_slice(&body_len.to_be_bytes());
    out.extend_from_slice(&header);
    out.extend_from_slice(body);
    Ok(out)
}

/// 解析帧头。
///
/// 返回值：
/// * `Ok(None)` —— **缓冲区还不够**（不足定长前缀，或头未收全）→ 调用方继续读，**不报错**
/// * `Ok(Some((head, body_offset)))` —— 头已解析；body 自 `body_offset` 起共 `head.body_len` 字节，
///   调用方需再确保缓冲区长度 ≥ `body_offset + body_len` 才能取到完整载荷
/// * `Err(_)` —— 硬错误，整连接应拒绝
///
/// 🔴 长度上限在**分配任何缓冲之前**校验（见模块头"设计取舍"）。
///
/// # Errors
/// 魔数 / 版本不符、长度超上限、头 JSON 非法时返回 [`FrameError`]。
pub fn parse_head(buf: &[u8]) -> Result<Option<(FrameHead, usize)>, FrameError> {
    if buf.len() < PREFIX_LEN {
        return Ok(None);
    }
    if buf[0..4] != MAGIC {
        return Err(FrameError::BadMagic);
    }
    let version = buf[4];
    if version != PROTOCOL_VERSION {
        return Err(FrameError::UnsupportedVersion(version));
    }
    let header_len = u32::from_be_bytes([buf[5], buf[6], buf[7], buf[8]]);
    let body_len = u32::from_be_bytes([buf[9], buf[10], buf[11], buf[12]]);
    if header_len > MAX_HEADER_BYTES {
        return Err(FrameError::HeaderTooLarge(header_len));
    }
    if body_len > MAX_BODY_BYTES {
        return Err(FrameError::BodyTooLarge(body_len));
    }

    let header_len_usize = header_len as usize;
    let body_offset = PREFIX_LEN + header_len_usize;
    if buf.len() < body_offset {
        return Ok(None);
    }
    let header = &buf[PREFIX_LEN..body_offset];
    let message: TransferMessage =
        serde_json::from_slice(header).map_err(|e| FrameError::MalformedHeader(e.to_string()))?;
    Ok(Some((FrameHead { message, body_len }, body_offset)))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_messages() -> Vec<(TransferMessage, Vec<u8>)> {
        vec![
            (
                TransferMessage::Handshake {
                    task_id: "t1".into(),
                    file_name: "报告 2026 🚀.pdf".into(),
                    file_size: 123_456_789,
                    file_hash: "a".repeat(64),
                    chunk_size: super::super::chunk::DEFAULT_CHUNK_SIZE,
                    file_modified_at_ms: Some(1_760_000_000_000),
                    pair_code: Some("A2C4E6".into()),
                },
                Vec::new(),
            ),
            (
                TransferMessage::HandshakeAck {
                    task_id: "t1".into(),
                    accepted: true,
                    reason_code: None,
                    resume_from: Some(2_097_152),
                    chunk_size: Some(super::super::chunk::DEFAULT_CHUNK_SIZE),
                },
                Vec::new(),
            ),
            (
                TransferMessage::Chunk {
                    task_id: "t1".into(),
                    index: 3,
                    offset: 6_291_456,
                },
                vec![7_u8; 1024],
            ),
            (
                TransferMessage::Complete {
                    task_id: "t1".into(),
                    file_hash: "b".repeat(64),
                },
                Vec::new(),
            ),
            (
                TransferMessage::CompleteAck {
                    task_id: "t1".into(),
                    ok: false,
                    reason_code: Some(super::super::reasons::HASH_MISMATCH.into()),
                },
                Vec::new(),
            ),
            (
                TransferMessage::Cancel {
                    task_id: "t1".into(),
                    reason_code: super::super::reasons::USER_CANCEL.into(),
                },
                Vec::new(),
            ),
            (
                TransferMessage::Pause {
                    task_id: "t1".into(),
                },
                Vec::new(),
            ),
            (
                TransferMessage::Resume {
                    task_id: "t1".into(),
                },
                Vec::new(),
            ),
            (
                TransferMessage::Text {
                    task_id: "t2".into(),
                    chars: 5,
                    bytes: 12,
                    from_device: Some("书房台式机".into()),
                },
                "你好🚀".as_bytes().to_vec(),
            ),
            (
                TransferMessage::TextAck {
                    task_id: "t2".into(),
                    ok: false,
                    reason_code: Some(super::super::reasons::CLIPBOARD_WRITE_FAILED.into()),
                },
                Vec::new(),
            ),
            (
                TransferMessage::Error {
                    task_id: None,
                    reason_code: super::super::reasons::PEER_DISCONNECTED.into(),
                    message: Some("对端在半块处断开".into()),
                },
                Vec::new(),
            ),
        ]
    }

    /// 每种消息都能**原样**往返（含非 ASCII 文件名、emoji 文本、裸载荷）。
    #[test]
    fn every_message_round_trips_with_its_body() {
        for (msg, body) in sample_messages() {
            let frame = encode_frame(&msg, &body).expect("编码应成功");
            let (head, body_offset) = parse_head(&frame)
                .expect("解析不应硬失败")
                .expect("完整帧应可解析");
            assert_eq!(head.message, msg, "消息往返不一致");
            assert_eq!(head.body_len as usize, body.len(), "载荷长度不一致");
            assert_eq!(&frame[body_offset..], &body[..], "载荷字节不一致");
        }
    }

    /// 增量读取：**缓冲区不足时返回 Ok(None)，不得当错误**（否则半个 TCP 包就会断连）。
    #[test]
    fn partial_buffers_yield_none_not_error() {
        let msg = TransferMessage::Pause {
            task_id: "t".into(),
        };
        let frame = encode_frame(&msg, b"xy").unwrap();
        for cut in 0..PREFIX_LEN {
            assert_eq!(
                parse_head(&frame[..cut]).unwrap(),
                None,
                "前缀缺 {cut} 字节时应等待"
            );
        }
        // 前缀齐了但头没齐
        let head_only_len = frame.len() - 2; // 缺 body
        assert!(
            parse_head(&frame[..head_only_len]).unwrap().is_some(),
            "头本身是自洽的：body 未到不影响头解析（调用方另读 body）"
        );
        // 头缺 1 字节
        let short_header = PREFIX_LEN + 1;
        assert_eq!(parse_head(&frame[..short_header]).unwrap(), None);
    }

    /// 🔴 长度上限必须在分配前生效——这是"把长度放在 JSON 之外"的全部理由。
    #[test]
    fn oversized_lengths_are_rejected_before_any_parse() {
        // 伪造一个 header_len 超上限的前缀（JSON 故意不放，因为根本不该被解析）
        let mut buf = Vec::new();
        buf.extend_from_slice(&MAGIC);
        buf.push(PROTOCOL_VERSION);
        buf.extend_from_slice(&(MAX_HEADER_BYTES + 1).to_be_bytes());
        buf.extend_from_slice(&0_u32.to_be_bytes());
        assert_eq!(
            parse_head(&buf),
            Err(FrameError::HeaderTooLarge(MAX_HEADER_BYTES + 1))
        );

        let mut buf2 = Vec::new();
        buf2.extend_from_slice(&MAGIC);
        buf2.push(PROTOCOL_VERSION);
        buf2.extend_from_slice(&0_u32.to_be_bytes());
        buf2.extend_from_slice(&(MAX_BODY_BYTES + 1).to_be_bytes());
        assert_eq!(
            parse_head(&buf2),
            Err(FrameError::BodyTooLarge(MAX_BODY_BYTES + 1))
        );
    }

    #[test]
    fn wrong_magic_and_version_are_rejected() {
        let frame = encode_frame(
            &TransferMessage::Pause {
                task_id: "t".into(),
            },
            b"",
        )
        .unwrap();

        let mut bad_magic = frame.clone();
        bad_magic[0] = b'X';
        assert_eq!(parse_head(&bad_magic), Err(FrameError::BadMagic));

        let mut bad_version = frame;
        bad_version[4] = PROTOCOL_VERSION + 1;
        assert_eq!(
            parse_head(&bad_version),
            Err(FrameError::UnsupportedVersion(PROTOCOL_VERSION + 1))
        );
    }

    #[test]
    fn garbage_header_is_malformed_not_panic() {
        let mut buf = Vec::new();
        buf.extend_from_slice(&MAGIC);
        buf.push(PROTOCOL_VERSION);
        let junk = b"not json at all";
        buf.extend_from_slice(&u32::try_from(junk.len()).unwrap().to_be_bytes());
        buf.extend_from_slice(&0_u32.to_be_bytes());
        buf.extend_from_slice(junk);
        assert!(matches!(
            parse_head(&buf),
            Err(FrameError::MalformedHeader(_))
        ));
    }

    /// 编码侧也要挡：载荷超上限时不得先分配再报错。
    #[test]
    fn encode_rejects_oversized_body() {
        let huge = vec![0_u8; MAX_BODY_BYTES as usize + 1];
        assert_eq!(
            encode_frame(
                &TransferMessage::Pause {
                    task_id: "t".into()
                },
                &huge
            ),
            Err(FrameError::BodyTooLarge(MAX_BODY_BYTES + 1))
        );
    }

    /// 🔴 载荷上限必须与分块上限同值，否则"块比帧能装的还大"会出现莫名其妙的中断。
    #[test]
    fn max_body_matches_chunk_cap() {
        assert_eq!(
            u64::from(MAX_BODY_BYTES),
            super::super::chunk::MAX_CHUNK_SIZE
        );
    }

    /// 变体名即线协议上的 `type` 值 —— 改名即破坏兼容，故逐条钉死。
    #[test]
    fn message_type_names_are_frozen_wire_contract() {
        let cases: [(TransferMessage, &str); 11] = [
            (
                TransferMessage::Handshake {
                    task_id: String::new(),
                    file_name: String::new(),
                    file_size: 0,
                    file_hash: String::new(),
                    chunk_size: 0,
                    file_modified_at_ms: None,
                    pair_code: None,
                },
                "Handshake",
            ),
            (
                TransferMessage::HandshakeAck {
                    task_id: String::new(),
                    accepted: false,
                    reason_code: None,
                    resume_from: None,
                    chunk_size: None,
                },
                "HandshakeAck",
            ),
            (
                TransferMessage::Chunk {
                    task_id: String::new(),
                    index: 0,
                    offset: 0,
                },
                "Chunk",
            ),
            (
                TransferMessage::Complete {
                    task_id: String::new(),
                    file_hash: String::new(),
                },
                "Complete",
            ),
            (
                TransferMessage::CompleteAck {
                    task_id: String::new(),
                    ok: false,
                    reason_code: None,
                },
                "CompleteAck",
            ),
            (
                TransferMessage::Cancel {
                    task_id: String::new(),
                    reason_code: String::new(),
                },
                "Cancel",
            ),
            (
                TransferMessage::Pause {
                    task_id: String::new(),
                },
                "Pause",
            ),
            (
                TransferMessage::Resume {
                    task_id: String::new(),
                },
                "Resume",
            ),
            (
                TransferMessage::Text {
                    task_id: String::new(),
                    chars: 0,
                    bytes: 0,
                    from_device: None,
                },
                "Text",
            ),
            (
                TransferMessage::TextAck {
                    task_id: String::new(),
                    ok: false,
                    reason_code: None,
                },
                "TextAck",
            ),
            (
                TransferMessage::Error {
                    task_id: None,
                    reason_code: String::new(),
                    message: None,
                },
                "Error",
            ),
        ];
        for (msg, expected) in cases {
            let json = serde_json::to_value(&msg).unwrap();
            assert_eq!(
                json.get("type").and_then(|v| v.as_str()),
                Some(expected),
                "type 名被改动（线协议契约）"
            );
        }
    }

    /// 🔴 协议里**不得**出现母本那个从未使用的 `ChunkAck`。
    #[test]
    fn chunk_ack_variant_is_deliberately_absent() {
        let json = r#"{"type":"ChunkAck","taskId":"t"}"#;
        assert!(
            serde_json::from_str::<TransferMessage>(json).is_err(),
            "ChunkAck 已刻意移除（母本声明但全仓零引用）；不得被重新引入"
        );
    }
}
