//! 音乐模块凭据管理（F-MU-27/F-MU-28）：跨平台 keyring 封装（Windows 侧走 Credential Manager）。
//!
//! 存储策略：
//! - 网易云：service = `Aegis/Music/Netease`，保存 `MUSIC_U` Cookie 值（单条 `cookie`）
//! - QQ 音乐：service = `Aegis/Music/QQ`，保存完整 Cookie 字符串（含 uin + music_key）；
//!   超长值自动分片：`cookie.0..n` + `cookie.meta`（分片数，最后写入，作提交标记）
//! - 读取：有 `cookie.meta` 按分片依序拼接；无则回退旧单条 `cookie`（兼容短凭据）
//!
//! 分片原因：Windows Credential Manager 单条凭据明文上限 2560 字节
//! （`CRED_MAX_CREDENTIAL_BLOB_SIZE`），keyring 以 UTF-16 写入并在写入前按该长度校验
//! （每码元 2 字节 → 单条约 1280 码元）。QQ 完整 Cookie 实测约 2.1 KB，不分片必被拒。
//!
//! 安全边界：
//! - 凭据仅本机可见，不上传、不分发、不写入日志明文
//! - 损坏/无法解密一律按"未登录"处理并删除失效项
//! - 写入先清旧条目（单条 + 分片 + meta）再写新值；meta 最后写，保证半途失败不会被误读为完整值

use tracing::{debug, error, info, warn};

/// 支持的音乐平台枚举
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MusicPlatform {
    Netease,
    QQ,
}

impl MusicPlatform {
    /// 🔴 provider 词表入站归一单点（Q-20）：接受规范值，并兼容别名 `qqmusic`
    /// （NexBox 词表遗留，移植期部分接口与历史数据仍带该值）。无法识别即 `None`，
    /// 由调用方拒绝并留日志，禁静默兜底成默认平台。
    pub fn from_slug(raw: &str) -> Option<Self> {
        match raw.trim().to_ascii_lowercase().as_str() {
            "netease" => Some(Self::Netease),
            "qq" | "qqmusic" => Some(Self::QQ),
            _ => None,
        }
    }

    /// 出站线格式：与前端 `MusicProvider` 联合类型逐字对齐（新增平台须同步两端）
    pub const fn slug(self) -> &'static str {
        match self {
            Self::Netease => "netease",
            Self::QQ => "qq",
        }
    }

    /// keyring service 名称
    fn credential_service(&self) -> &'static str {
        match self {
            Self::Netease => "Aegis/Music/Netease",
            Self::QQ => "Aegis/Music/QQ",
        }
    }
}

/// 凭据操作结果
#[derive(Debug)]
pub enum CredentialResult {
    /// 成功
    Success,
    /// 凭据不存在（未登录）
    NotFound,
    /// 凭据损坏或无法解析
    Corrupted,
    /// 系统级错误（权限不足/Credential Manager 不可用）
    SystemError(String),
}

/// 单条 Windows 凭据明文上限（UTF-16 码元数）
///
/// `CRED_MAX_CREDENTIAL_BLOB_SIZE` = 2560 字节，keyring 按 UTF-16 长度校验（2 字节/码元），
/// 取 1200 码元留余量。
const MAX_CREDENTIAL_UTF16_UNITS: usize = 1200;

/// 分片数上限（防状态损坏导致无界读写循环；1200 × 16 ≈ 19K 码元，远超任何真实 Cookie）
const MAX_CHUNKS: usize = 16;

/// 单条（未分片）凭据的 user 名；分片为 `cookie.<n>`，提交标记为 `cookie.meta`
const LEGACY_USER: &str = "cookie";
const META_USER: &str = "cookie.meta";

fn chunk_user(index: usize) -> String {
    format!("{LEGACY_USER}.{index}")
}

/// 按 UTF-16 码元切分（不劈开代理对），保证每片 ≤ MAX_CREDENTIAL_UTF16_UNITS
fn split_by_utf16_units(value: &str) -> Vec<String> {
    let mut chunks: Vec<String> = Vec::new();
    let mut current = String::new();
    let mut units = 0usize;
    for ch in value.chars() {
        let ch_units = ch.len_utf16();
        if units + ch_units > MAX_CREDENTIAL_UTF16_UNITS && !current.is_empty() {
            chunks.push(std::mem::take(&mut current));
            units = 0;
        }
        current.push(ch);
        units += ch_units;
    }
    if !current.is_empty() {
        chunks.push(current);
    }
    chunks
}

/// 删除该 service 下全部凭据条目（旧单条 + meta + 全部分片），幂等（不存在即跳过）
fn delete_all_entries(service: &str) {
    if let Ok(entry) = keyring::Entry::new(service, LEGACY_USER) {
        let _ = entry.delete_credential();
    }
    if let Ok(entry) = keyring::Entry::new(service, META_USER) {
        let _ = entry.delete_credential();
    }
    for index in 0..MAX_CHUNKS {
        if let Ok(entry) = keyring::Entry::new(service, &chunk_user(index)) {
            let _ = entry.delete_credential();
        }
    }
}

/// 写入值：≤1200 码元单条直存；超长分片存储，meta 最后写（提交标记）
fn write_value(service: &str, value: &str) -> keyring::Result<()> {
    if value.encode_utf16().count() <= MAX_CREDENTIAL_UTF16_UNITS {
        return keyring::Entry::new(service, LEGACY_USER)?.set_password(value);
    }
    let chunks = split_by_utf16_units(value);
    if chunks.len() > MAX_CHUNKS {
        return Err(keyring::Error::TooLong(
            LEGACY_USER.to_string(),
            (MAX_CREDENTIAL_UTF16_UNITS * MAX_CHUNKS) as u32,
        ));
    }
    for (index, chunk) in chunks.iter().enumerate() {
        keyring::Entry::new(service, &chunk_user(index))?.set_password(chunk)?;
    }
    keyring::Entry::new(service, META_USER)?.set_password(&chunks.len().to_string())
}

/// 读取完整值：有 meta 按分片依序拼接；无 meta 回退旧单条 `cookie`；都没有 = 未登录
fn read_value(service: &str) -> keyring::Result<Option<String>> {
    match keyring::Entry::new(service, META_USER).and_then(|e| e.get_password()) {
        Ok(meta) => {
            let count: usize = meta.trim().parse().map_err(|_| {
                keyring::Error::Invalid(META_USER.to_string(), format!("分片数无法解析: {meta}"))
            })?;
            if count == 0 || count > MAX_CHUNKS {
                return Err(keyring::Error::Invalid(
                    META_USER.to_string(),
                    format!("分片数越界: {count}"),
                ));
            }
            let mut value = String::new();
            for index in 0..count {
                match keyring::Entry::new(service, &chunk_user(index))
                    .and_then(|e| e.get_password())
                {
                    Ok(part) => value.push_str(&part),
                    Err(keyring::Error::NoEntry) => {
                        return Err(keyring::Error::Invalid(
                            chunk_user(index),
                            "分片缺失（写入中断）".to_string(),
                        ));
                    }
                    Err(e) => return Err(e),
                }
            }
            Ok(Some(value))
        }
        Err(keyring::Error::NoEntry) => {
            match keyring::Entry::new(service, LEGACY_USER).and_then(|e| e.get_password()) {
                Ok(value) => Ok(Some(value)),
                Err(keyring::Error::NoEntry) => Ok(None),
                Err(e) => Err(e),
            }
        }
        Err(e) => Err(e),
    }
}

/// 保存凭据（原子写：先删旧再写新）
///
/// # 参数
/// * `platform` - 目标平台
/// * `cookie_value` - Cookie 字符串（已规范化，不含敏感明文如密码）
/// * `correlation_id` - 关联 ID，用于日志追踪
///
/// # 返回
/// * `CredentialResult` - 操作结果
pub fn save_credential(
    platform: MusicPlatform,
    cookie_value: &str,
    correlation_id: &str,
) -> CredentialResult {
    let service = platform.credential_service();

    // 八字段日志：保存开始
    debug!(
        correlation_id = correlation_id,
        command = "music_save_credential",
        platform = ?platform,
        action = "save_start",
        result = "pending",
        duration_ms = 0,
        error_code = "",
        user_id = "",
        "保存凭据开始"
    );

    // 1. 先清旧凭据（单条 + 分片 + meta），避免旧分片参与拼接
    delete_all_entries(service);

    // 2. 写入新凭据（超长自动分片）
    match write_value(service, cookie_value) {
        Ok(_) => {
            info!(
                correlation_id = correlation_id,
                command = "music_save_credential",
                platform = ?platform,
                action = "save_complete",
                result = "success",
                duration_ms = 0,
                error_code = "",
                user_id = extract_user_id(platform, cookie_value),
                "凭据保存成功"
            );
            CredentialResult::Success
        }
        Err(e) => {
            error!(
                correlation_id = correlation_id,
                command = "music_save_credential",
                platform = ?platform,
                action = "save_write",
                result = "failure",
                duration_ms = 0,
                error_code = format!("{:?}", e),
                user_id = "",
                "凭据写入失败: {:?}",
                e
            );
            CredentialResult::SystemError(format!("Keyring 写入失败: {}", e))
        }
    }
}

/// 加载凭据
///
/// # 参数
/// * `platform` - 目标平台
/// * `correlation_id` - 关联 ID
///
/// # 返回
/// * `Option<String>` - Cookie 字符串（None 表示未登录或凭据损坏）
pub fn load_credential(platform: MusicPlatform, correlation_id: &str) -> Option<String> {
    let service = platform.credential_service();

    debug!(
        correlation_id = correlation_id,
        command = "music_load_credential",
        platform = ?platform,
        action = "load_start",
        result = "pending",
        duration_ms = 0,
        error_code = "",
        user_id = "",
        "加载凭据开始"
    );

    match read_value(service) {
        Ok(Some(cookie)) => {
            // 验证 Cookie 有效性（非空且包含必要字段）
            if validate_cookie(platform, &cookie) {
                info!(
                    correlation_id = correlation_id,
                    command = "music_load_credential",
                    platform = ?platform,
                    action = "load_complete",
                    result = "success",
                    duration_ms = 0,
                    error_code = "",
                    user_id = extract_user_id(platform, &cookie),
                    "凭据加载成功"
                );
                Some(cookie)
            } else {
                warn!(
                    correlation_id = correlation_id,
                    command = "music_load_credential",
                    platform = ?platform,
                    action = "load_validate",
                    result = "corrupted",
                    duration_ms = 0,
                    error_code = "INVALID_COOKIE_FORMAT",
                    user_id = "",
                    "凭据格式无效，视为损坏"
                );
                // 删除损坏的凭据（单条 + 分片 + meta）
                delete_all_entries(service);
                None
            }
        }
        Ok(None) => {
            debug!(
                correlation_id = correlation_id,
                command = "music_load_credential",
                platform = ?platform,
                action = "load_not_found",
                result = "not_logged_in",
                duration_ms = 0,
                error_code = "",
                user_id = "",
                "凭据不存在（未登录）"
            );
            None
        }
        // 损坏类（meta 不可解析/分片缺失/编码异常/超限）：按未登录处理并清除失效项
        Err(
            e @ (keyring::Error::Invalid(..)
            | keyring::Error::BadEncoding(..)
            | keyring::Error::TooLong(..)),
        ) => {
            warn!(
                correlation_id = correlation_id,
                command = "music_load_credential",
                platform = ?platform,
                action = "load_corrupted",
                result = "corrupted",
                duration_ms = 0,
                error_code = format!("{:?}", e),
                user_id = "",
                "凭据损坏（{e}），已清除失效项"
            );
            delete_all_entries(service);
            None
        }
        Err(e) => {
            error!(
                correlation_id = correlation_id,
                command = "music_load_credential",
                platform = ?platform,
                action = "load_error",
                result = "system_error",
                duration_ms = 0,
                error_code = format!("{:?}", e),
                user_id = "",
                "凭据读取失败: {:?}",
                e
            );
            None
        }
    }
}

/// 清除凭据（退出登录）
///
/// # 参数
/// * `platform` - 目标平台
/// * `correlation_id` - 关联 ID
///
/// # 返回
/// * `CredentialResult` - 操作结果（幂等：条目不存在也视为成功）
pub fn clear_credential(platform: MusicPlatform, correlation_id: &str) -> CredentialResult {
    let service = platform.credential_service();

    debug!(
        correlation_id = correlation_id,
        command = "music_clear_credential",
        platform = ?platform,
        action = "clear_start",
        result = "pending",
        duration_ms = 0,
        error_code = "",
        user_id = "",
        "清除凭据开始"
    );

    // 单条 + meta + 全部分片一并清除（各条目不存在属正常态）
    delete_all_entries(service);

    info!(
        correlation_id = correlation_id,
        command = "music_clear_credential",
        platform = ?platform,
        action = "clear_complete",
        result = "success",
        duration_ms = 0,
        error_code = "",
        user_id = "",
        "凭据清除成功（退出登录）"
    );
    CredentialResult::Success
}

/// 验证 Cookie 格式有效性
fn validate_cookie(platform: MusicPlatform, cookie: &str) -> bool {
    if cookie.is_empty() {
        return false;
    }

    match platform {
        MusicPlatform::Netease => {
            // 网易云：必须包含 MUSIC_U（与 cookie.rs 谓词同源）
            super::cookie::netease_cookie_has_login(cookie)
        }
        MusicPlatform::QQ => {
            // QQ 音乐：与 cookie.rs 谓词对齐（覆盖 wxuin/微信扫码、qm_keyst 等 cookie 名变体），
            // 避免登录窗口写入的合法 cookie 在加载时被误判损坏而删除
            super::cookie::qq_cookie_has_login(cookie)
        }
    }
}

/// 从 Cookie 中提取用户标识（用于日志，脱敏处理）
fn extract_user_id(platform: MusicPlatform, cookie: &str) -> String {
    match platform {
        MusicPlatform::Netease => {
            // 网易云：MUSIC_U 值为哈希，直接取前 8 位
            cookie
                .split(';')
                .find(|pair| pair.trim().starts_with("MUSIC_U="))
                .and_then(|pair| pair.split('=').nth(1))
                .map(|val| {
                    let trimmed = val.trim();
                    if trimmed.len() > 8 {
                        format!("{}...", &trimmed[..8])
                    } else {
                        trimmed.to_string()
                    }
                })
                .unwrap_or_else(|| "unknown".to_string())
        }
        MusicPlatform::QQ => {
            // QQ：uin 为数字 ID，直接取
            cookie
                .split(';')
                .find(|pair| pair.trim().starts_with("uin="))
                .and_then(|pair| pair.split('=').nth(1))
                .map(|val| val.trim().to_string())
                .unwrap_or_else(|| "unknown".to_string())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_platform_slug_roundtrip() {
        assert_eq!(MusicPlatform::Netease.slug(), "netease");
        assert_eq!(MusicPlatform::QQ.slug(), "qq");
        for p in [MusicPlatform::Netease, MusicPlatform::QQ] {
            assert_eq!(MusicPlatform::from_slug(p.slug()), Some(p));
        }
    }

    #[test]
    fn test_platform_from_slug_accepts_alias_and_case() {
        assert_eq!(MusicPlatform::from_slug("qqmusic"), Some(MusicPlatform::QQ));
        assert_eq!(MusicPlatform::from_slug(" QQ "), Some(MusicPlatform::QQ));
        assert_eq!(
            MusicPlatform::from_slug("Netease"),
            Some(MusicPlatform::Netease)
        );
        assert_eq!(MusicPlatform::from_slug("kugou"), None);
        assert_eq!(MusicPlatform::from_slug(""), None);
    }

    #[test]
    fn test_validate_netease_cookie() {
        let valid = "MUSIC_U=abc123; __csrf=xyz";
        let invalid = "__csrf=xyz";
        assert!(validate_cookie(MusicPlatform::Netease, valid));
        assert!(!validate_cookie(MusicPlatform::Netease, invalid));
    }

    #[test]
    fn test_validate_qq_cookie() {
        let valid = "uin=123456; music_key=abc";
        let valid_alt = "uin=123456; p_skey=xyz";
        let invalid = "uin=123456";
        assert!(validate_cookie(MusicPlatform::QQ, valid));
        assert!(validate_cookie(MusicPlatform::QQ, valid_alt));
        assert!(!validate_cookie(MusicPlatform::QQ, invalid));
    }

    #[test]
    fn test_extract_user_id_netease() {
        let cookie = "MUSIC_U=abcdef1234567890; __csrf=xyz";
        let user_id = extract_user_id(MusicPlatform::Netease, cookie);
        assert_eq!(user_id, "abcdef12...");
    }

    #[test]
    fn test_extract_user_id_qq() {
        let cookie = "uin=987654321; music_key=abc";
        let user_id = extract_user_id(MusicPlatform::QQ, cookie);
        assert_eq!(user_id, "987654321");
    }

    #[test]
    fn test_split_short_value_single_chunk() {
        let chunks = split_by_utf16_units("uin=1; music_key=k");
        assert_eq!(chunks.len(), 1);
        assert_eq!(chunks[0], "uin=1; music_key=k");
    }

    #[test]
    fn test_split_boundary() {
        let exact = "a".repeat(MAX_CREDENTIAL_UTF16_UNITS);
        assert_eq!(split_by_utf16_units(&exact).len(), 1);

        let over = "a".repeat(MAX_CREDENTIAL_UTF16_UNITS + 1);
        let chunks = split_by_utf16_units(&over);
        assert_eq!(chunks.len(), 2);
        assert_eq!(chunks[0].len(), MAX_CREDENTIAL_UTF16_UNITS);
        assert_eq!(chunks[1].len(), 1);
    }

    #[test]
    fn test_split_qq_cookie_size() {
        // QQ 完整 cookie 实测约 2.1 KB → 2 片，且拼接还原
        let value = "a".repeat(2152);
        let chunks = split_by_utf16_units(&value);
        assert_eq!(chunks.len(), 2);
        assert_eq!(chunks.concat(), value);
        for chunk in &chunks {
            assert!(chunk.encode_utf16().count() <= MAX_CREDENTIAL_UTF16_UNITS);
        }
    }

    #[test]
    fn test_split_surrogate_pairs() {
        // 非 BMP 字符占 2 码元：切分不得劈开代理对，且每片不超限
        let value = "\u{1d11e}".repeat(1300);
        let chunks = split_by_utf16_units(&value);
        assert!(chunks.len() >= 3);
        assert_eq!(chunks.concat(), value);
        for chunk in &chunks {
            assert!(chunk.encode_utf16().count() <= MAX_CREDENTIAL_UTF16_UNITS);
        }
    }

    /// 真实 Credential Manager 分片往返（手动执行，默认忽略）：
    /// `cargo test -p services --lib -- --ignored e2e_qq_chunked_roundtrip`
    ///
    /// 🔴 会写入并清除 `Aegis/Music/QQ` 真实凭据项；检测到已有凭据即中止，避免误删
    #[test]
    #[ignore = "读写真实 Credential Manager，需手动执行"]
    fn e2e_qq_chunked_roundtrip() {
        if load_credential(MusicPlatform::QQ, "e2e-test").is_some() {
            panic!("检测到已有 QQ 凭据，中止测试以免误删");
        }

        let mut value = String::from("uin=123456789; music_key=TESTKEY123; ");
        for i in 0..200 {
            value.push_str(&format!("k{i}=v{i};"));
        }
        assert!(value.encode_utf16().count() > MAX_CREDENTIAL_UTF16_UNITS);

        assert!(matches!(
            save_credential(MusicPlatform::QQ, &value, "e2e-test"),
            CredentialResult::Success
        ));
        assert_eq!(
            load_credential(MusicPlatform::QQ, "e2e-test").as_deref(),
            Some(value.as_str())
        );
        assert!(matches!(
            clear_credential(MusicPlatform::QQ, "e2e-test"),
            CredentialResult::Success
        ));
        assert_eq!(load_credential(MusicPlatform::QQ, "e2e-test"), None);
    }
}
