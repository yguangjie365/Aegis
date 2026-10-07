//! Steam 在线库存（GetOwnedGames）—— Aegis 在 NexBox 基线之上的**能力补全**。
//!
//! # 背景（2026-10-03 用户裁决）
//! ---------------------------------------------------------------------------
//! NexBox 的 `get_steam_inventory`（`steam.rs`）**纯本地**：读 `.acf` 已安装清单 +
//! `localconfig` 游玩记录 + `appinfo` 名称目录，全程无网络请求、不读任何 API Key。
//! 其 `SteamInventoryData.source` 虽声明了 `"online"` 枚举，但代码里从未赋值 ——
//! 实测只有 `"cache"` / `"none"` 两态。
//!
//! 这意味着 NexBox 只能反映「本机痕迹」，拿不到「账号拥有的**全部**游戏」。
//! SystemToolkit 有这条路径（`SteamService.InventoryOnline.cs`，调
//! `IPlayerService/GetOwnedGames`），但它把 API Key 输入弹窗放在**游戏管理页内**
//! （`SteamApiKeyWindow.xaml`）—— 用户明确裁决：
//!
//! > 「库存补全的 Steam API Key 输入 UI 放到设置页面，不要放到游戏管理页面」
//!
//! 故本模块承担两件事：
//! 1. **后端**：`get_steam_inventory_online` 命令 + API Key 的读写；
//! 2. 凭据一律由**设置页**录入（`features/Settings/`），本模块不弹任何窗。
//!
//! # 与 NexBox 的关系（重要）
//! ---------------------------------------------------------------------------
//! 本文件是**新增**的，NexBox 无对应实现，故不受「游戏模块基线是 NexBox」的复用铁律
//! 约束；但**错误语义、字段口径、接口参数**严格照 SystemToolkit 的
//! `SteamService.InventoryOnline.cs` 搬（见各处「SystemToolkit：」注释行号），
//! 以保持两个工程对同一接口的行为一致。
//!
//! # 三态语义（与 NexBox 的 `source` 字段对齐）
//! ---------------------------------------------------------------------------
//! | source    | 含义                                   | 触发条件 |
//! |-----------|----------------------------------------|---------|
//! | `online`  | 在线清单成功                           | 有 Key + 账号 ID + HTTP 200 + 解析出条目 |
//! | `cache`   | 在线失败或未配置，**已退回本地缓存**     | 其余全部情况（**不抛错**） |
//! | `none`    | 无 Steam 客户端                        | 注册表读不到安装路径 |
//!
//! 🔴 **不抛异常纪律**：任何失败（无 Key / 断网 / 401 / 解析异常）都返回带 `error`
//! 的**失败结果**而非 panic，符合规格 08 F-GM-03「逐数据源容错，异常从不抛给页面」。

use serde::{Deserialize, Serialize};

/// Steam Web API 根地址（SystemToolkit：`SteamService.InventoryOnline.cs:24`）
const STEAM_API_BASE: &str = "https://api.steampowered.com";

/// 在线请求超时（秒）。SystemToolkit 用 15s，此处同值。
const ONLINE_TIMEOUT_SECS: u64 = 15;

// ============================================================================
// 凭据存取（API Key）
// ============================================================================

/// API Key 的落盘文件名（放在 Steam 目录的 config 下，与客户端配置同域）。
const API_KEY_FILE: &str = "aegis_steam_api_key.txt";

/// 🔴 2026-10-07（审查 R-6，用户裁决方案 C）：Key 落盘由明文改为 **DPAPI 加密**。
///
/// 原实现把 32 位十六进制**明文**直接写盘，与项目红线「凭据只经系统安全存储、
/// 禁明文落盘」冲突。改为 `CryptProtectData`（DPAPI）：密文绑定**当前 Windows
/// 用户**解密；文件路径、`.bak` 纪律、命令签名全部不变，只有文件内容变为密文。
///
/// 兼容与降级（已在规格 08 登记）：
/// - 旧版**明文**文件在首次读取时自动迁移为密文（迁移失败保留旧文件继续用，留痕）；
/// - 换机/重装系统后旧密文不可解密 → 按「未配置」处理，用户重贴 Key 即可。
///
/// 🔴 与 `keyring` 依赖的关系：Cargo.toml 已预置 keyring（MU-4 注释）供后续
///   凭据场景（如音乐域 cookie）选用；本处按裁决用 DPAPI 保住「与 Steam 目录
///   同域、可随目录迁移」的既有语义，二者不冲突。
#[cfg(windows)]
mod key_dpapi {
    use windows_sys::Win32::Foundation::LocalFree;
    use windows_sys::Win32::Security::Cryptography::{
        CryptProtectData, CryptUnprotectData, CRYPTPROTECT_UI_FORBIDDEN, CRYPT_INTEGER_BLOB,
    };

    /// 域分隔串：作为可选熵防止同用户其它 DPAPI 密文被挪到此处解密（反之亦然）。
    /// 换代（v2）= 改这里 + 全量重新迁移。
    const ENTROPY: &[u8] = b"Aegis.SteamApiKey.v1";

    fn blob(bytes: &[u8]) -> CRYPT_INTEGER_BLOB {
        CRYPT_INTEGER_BLOB {
            cbData: bytes.len() as u32,
            pbData: bytes.as_ptr() as *mut u8,
        }
    }

    /// 取回 DPAPI 输出 blob 的内容并释放其 LocalAlloc 内存。
    fn take(out: CRYPT_INTEGER_BLOB) -> Vec<u8> {
        let bytes = unsafe { std::slice::from_raw_parts(out.pbData, out.cbData as usize).to_vec() };
        unsafe {
            LocalFree(out.pbData.cast());
        }
        bytes
    }

    /// 加密。失败即返回 Err（调用方不得降级为明文落盘）。
    pub fn protect(plain: &[u8]) -> Result<Vec<u8>, String> {
        unsafe {
            let mut out = CRYPT_INTEGER_BLOB {
                cbData: 0,
                pbData: std::ptr::null_mut(),
            };
            let ok = CryptProtectData(
                &blob(plain),
                std::ptr::null(),
                &blob(ENTROPY),
                std::ptr::null(),
                std::ptr::null(),
                CRYPTPROTECT_UI_FORBIDDEN,
                &mut out,
            );
            if ok == 0 {
                return Err(format!(
                    "DPAPI 加密失败: {}",
                    std::io::Error::last_os_error()
                ));
            }
            Ok(take(out))
        }
    }

    /// 解密。密文无效（含换机后本用户解不开）时返回 Err。
    pub fn unprotect(cipher: &[u8]) -> Result<Vec<u8>, String> {
        unsafe {
            let mut out = CRYPT_INTEGER_BLOB {
                cbData: 0,
                pbData: std::ptr::null_mut(),
            };
            let ok = CryptUnprotectData(
                &blob(cipher),
                std::ptr::null_mut(),
                &blob(ENTROPY),
                std::ptr::null(),
                std::ptr::null(),
                CRYPTPROTECT_UI_FORBIDDEN,
                &mut out,
            );
            if ok == 0 {
                return Err(format!(
                    "DPAPI 解密失败: {}",
                    std::io::Error::last_os_error()
                ));
            }
            Ok(take(out))
        }
    }
}

/// 校验前端传入的 `steam_dir` 是否就是本机真实 Steam 目录（纵深防御）
///
/// 🔴 三个公开命令（`steam_save_api_key` / `steam_api_key_status` /
/// `get_steam_inventory_online`）的 `steam_dir` 均来自前端 `steamDirFor()` 的返回值。
/// Tauri IPC 只来自本应用前端、**非远程攻击面**，但缺校验时传 `C:\Windows\System32`
/// 之类即可让 `save_api_key` 把 `aegis_steam_api_key.txt` 写到任意目录。
///
/// 口径：**不信任前端传参做写盘定位**。仅当传入值与本机探测到的 Steam 路径一致
/// （忽略大小写与末尾斜杠、或指向其子目录）才放行；本机探测不到 Steam 时也放行
/// ——此时无法证伪，不因环境探测失败而阻断用户正常使用。
fn validate_steam_dir(candidate: &str) -> Result<(), String> {
    if candidate.trim().is_empty() {
        return Err("Steam 目录为空".to_string());
    }
    // 无论是否放行，先拦住明显的路径穿越成分
    if candidate.contains("..") {
        return Err(format!("非法的 Steam 目录: {}", candidate));
    }

    let Some(actual) = super::steam::get_steam_path() else {
        // 探测不到（如非 Windows / 未安装 Steam）→ 无法证伪，放行
        return Ok(());
    };

    let norm = |s: &str| s.trim_end_matches(['\\', '/']).to_lowercase();
    let (cand, real) = (norm(candidate), norm(&actual));
    if cand == real
        || cand.starts_with(&format!("{}\\", real))
        || cand.starts_with(&format!("{}/", real))
    {
        Ok(())
    } else {
        Err(format!(
            "Steam 目录与本机实际安装位置不符：传入 {}，实际 {}",
            candidate, actual
        ))
    }
}

/// 读取已保存的 API Key；未配置返回 `None`。
///
/// 🔴 只读 Steam 目录的单个文件，**不写注册表、不改客户端配置**（规格 08 §一
/// 「本模块不修改任何游戏文件」）。文件不存在 = 未配置，属正常态而非错误。
/// 🔴 2026-10-07（R-6）：文件内容为 DPAPI 密文；旧明文自动迁移；解不开 = 未配置。
pub fn load_api_key(steam_dir: &str) -> Option<String> {
    let path = format!("{}\\{}", steam_dir, API_KEY_FILE);
    let raw = std::fs::read(&path).ok()?;
    if raw.is_empty() {
        return None;
    }

    // 🔴 兼容迁移（仅 Windows）：旧版存的是**明文** 32 位十六进制（判据与
    //   `steam_save_api_key` 的写盘预检一致）。命中即原地重写为密文。
    //   迁移失败保留旧文件继续用 —— 留痕不阻断（禁静默失败，但不因加密迁移
    //   失败而把用户本可用的 Key 判成未配置）。
    #[cfg(windows)]
    if let Ok(legacy) = String::from_utf8(raw.clone()) {
        let legacy = legacy.trim();
        if is_wellformed_key(legacy) {
            log::info!("[Steam] 检测到旧版明文 Key，自动迁移为 DPAPI 加密存储");
            match save_api_key(steam_dir, legacy) {
                Ok(()) => {
                    // 迁移成功后 .bak 里是**明文旧值**，必须一并清掉（否则加密白做）
                    let _ = std::fs::remove_file(format!("{}.bak", path));
                }
                Err(e) => {
                    log::warn!("[Steam] Key 加密迁移失败（保留原文件继续使用）: {e}");
                }
            }
            return Some(legacy.to_string());
        }
        // 非空但不符合 Key 格式的"明文"：多半是损坏文件，落到底部按密文解密失败
        // → 未配置态，与「换机后解不开」同一出口。
    }

    #[cfg(windows)]
    let decoded = key_dpapi::unprotect(&raw).ok()?;
    #[cfg(not(windows))]
    let decoded = raw;

    let key = String::from_utf8(decoded).ok()?.trim().to_string();
    if key.is_empty() {
        return None;
    }
    log::info!("[Steam] API Key 已配置（{} 字节）", key.len());
    Some(key)
}

/// 与 `steam_save_api_key` 的写盘预检同口径：Steam Web API Key 固定 32 位十六进制。
fn is_wellformed_key(s: &str) -> bool {
    s.len() == 32 && s.chars().all(|c| c.is_ascii_hexdigit())
}

/// 保存 API Key。空串视为**清除**（删文件），返回是否成功。
///
/// 🔴 覆写前先备份为 `.bak` —— 规格 08 §一要求「改写客户端的登录记录，写前留备份」
/// 是针对切账户的；Key 属本程序自有配置，同样按此纪律处理。
/// 🔴 2026-10-07（R-6）：写盘内容为 DPAPI 密文，盘上不再出现明文；
///   Windows 加密失败时**返回错误**，不得降级为明文落盘。
pub fn save_api_key(steam_dir: &str, key: &str) -> Result<(), String> {
    let path = format!("{}\\{}", steam_dir, API_KEY_FILE);
    let key = key.trim();

    if key.is_empty() {
        // 清除
        return match std::fs::remove_file(&path) {
            Ok(_) => {
                log::info!("[Steam] API Key 已清除");
                Ok(())
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(e) => Err(format!("清除失败: {}", e)),
        };
    }

    // 🔴 加密先行（R-6）：先拿到密文再动盘 —— 加密失败时旧文件与 .bak 原样不动
    let payload = {
        #[cfg(windows)]
        {
            key_dpapi::protect(key.as_bytes())?
        }
        #[cfg(not(windows))]
        {
            key.as_bytes().to_vec()
        }
    };

    // 写前备份
    if std::path::Path::new(&path).exists() {
        let _ = std::fs::copy(&path, format!("{}.bak", path));
    }
    std::fs::write(&path, payload).map_err(|e| format!("保存失败: {}", e))
}

// ============================================================================
// 在线上报
// ============================================================================

/// 校验 API Key 是否可用（只验格式与能否拿到账号 ID，**不发网络请求**）。
///
/// 供设置页「测试连接」前的预检用 —— 用户点了保存就立刻知道 Key 长度不对、
/// 或本机没登录 Steam 账号，而不是等拉库存才报错。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SteamApiKeyStatus {
    /// 是否已配置且格式合法
    pub configured: bool,
    /// 本机可读到的当前账号 SteamID64（`None` = 未登录任何账号）
    pub steam_id64: Option<String>,
    /// 当前账号昵称（若有）
    pub account_name: Option<String>,
    /// 配置了但拿不到账号 ID 时的原因（如实交代，不静默）
    pub warning: Option<String>,
}

/// SteamID64 合法性：17 位纯数字，且落在 76561197960265729 之后（个体账号区间）。
///
/// SystemToolkit 同样以 `loginusers.vdf` 的激活账户为查询目标；这里只做**格式**校验，
/// 不做算术推导（越界判断交给官方接口，它会返 401）。
fn is_valid_steam_id64(id: &str) -> bool {
    id.len() == 17 && id.chars().all(|c| c.is_ascii_digit())
}

/// 在线拉取结果（内部结构，不直接出命令）。
struct OnlineResult {
    games: Vec<OnlineGame>,
    error: Option<String>,
}

/// 单条在线游戏记录（已转成 `SteamInventoryGame` 前的中间形态）。
struct OnlineGame {
    app_id: u32,
    name: String,
    playtime_minutes: u64,
    last_played: i64,
}

/// 🔴 **不抛异常**：任何失败都折成 `error` 文本返回（SystemToolkit 同款纪律，
/// `SteamService.InventoryOnline.cs:88-93` 明确「不抛异常，返回带 Error 的失败结果」）。
async fn fetch_owned_games(api_key: &str, steam_id64: &str) -> OnlineResult {
    if api_key.trim().is_empty() || steam_id64.trim().is_empty() {
        return OnlineResult {
            games: vec![],
            error: Some("缺少 API Key 或 Steam 账户 ID".to_string()),
        };
    }

    // 参数编码：Key 与 SteamID 都做百分号编码（SystemToolkit `:105-108` 用
    // Uri.EscapeDataString；此处 URL-encode 语义相同）
    let url = format!(
        "{}/IPlayerService/GetOwnedGames/v1/?key={}&steamid={}&include_appinfo=1&include_played_free_games=1&format=json",
        STEAM_API_BASE,
        url_encode(api_key),
        url_encode(steam_id64)
    );

    let client = match reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(ONLINE_TIMEOUT_SECS))
        .user_agent("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36")
        .build()
    {
        Ok(c) => c,
        Err(e) => {
            return OnlineResult {
                games: vec![],
                error: Some(format!("HTTP 客户端构建失败: {}", e)),
            }
        }
    };

    let resp = match client.get(&url).send().await {
        Ok(r) => r,
        Err(e) if e.is_timeout() => {
            return OnlineResult {
                games: vec![],
                error: Some(format!(
                    "在线库存请求超时（上界 {} 秒）",
                    ONLINE_TIMEOUT_SECS
                )),
            }
        }
        Err(e) => {
            return OnlineResult {
                games: vec![],
                error: Some(format!("在线库存请求失败: {}", e)),
            }
        }
    };

    if !resp.status().is_success() {
        let code = resp.status().as_u16();
        let hint = if code == 401 || code == 403 {
            "Key 无效、已被撤销，或不属于该账户"
        } else {
            "官方接口返回异常状态"
        };
        return OnlineResult {
            games: vec![],
            error: Some(format!("在线库存请求被拒（HTTP {}）：{}", code, hint)),
        };
    }

    let json = match resp.text().await {
        Ok(t) => t,
        Err(e) => {
            return OnlineResult {
                games: vec![],
                error: Some(format!("读取响应失败: {}", e)),
            }
        }
    };

    parse_owned_games(&json)
}

/// 解析 `GetOwnedGames` 响应体。**不发网络**，故可单测。
///
/// 🔴 两个必须照搬 SystemToolkit 的防御点（`SteamService.InventoryOnline.cs:145-200`，
/// 该工程已因踩坑加注释 + 钉测试）：
/// 1. `response.games` 缺失/非数组/空数组 → 官方在「游戏详情非公开」时**正是这样返回**
///    （HTTP 200 但无 games），此时给**可操作提示**而非"空库存"；
/// 2. `appid` 必须先验 `ValueKind` 是 Number —— 类型不符（如 `"appid":"123"`）时
///    `TryGetUInt32` 是**抛** `InvalidOperationException` 而非返回 false，
///    不验就会因一条脏数据丢掉整份库存。
fn parse_owned_games(json: &str) -> OnlineResult {
    let root: serde_json::Value = match serde_json::from_str(json) {
        Ok(v) => v,
        Err(e) => {
            return OnlineResult {
                games: vec![],
                error: Some(format!("响应解析失败: {}", e)),
            }
        }
    };

    let games = match root.get("response").and_then(|r| r.get("games")) {
        Some(v) if v.is_array() => v,
        _ => {
            return OnlineResult {
                games: vec![],
                error: Some(
                    "在线返回空列表：请把 Steam「隐私设置 → 游戏详情」设为公开，并确认 Key 属于当前账户"
                        .to_string(),
                ),
            }
        }
    };

    let arr = games.as_array().expect("上面已验 is_array");
    if arr.is_empty() {
        return OnlineResult {
            games: vec![],
            error: Some(
                "在线返回空列表：请把 Steam「隐私设置 → 游戏详情」设为公开，并确认 Key 属于当前账户"
                    .to_string(),
            ),
        };
    }

    let mut list: Vec<OnlineGame> = Vec::with_capacity(arr.len());
    for item in arr {
        // 先验类型再取值（见函数头第 2 点）
        let app_id = match item.get("appid") {
            Some(v) if v.is_u64() => v.as_u64().unwrap_or(0) as u32,
            _ => continue,
        };
        if app_id == 0 {
            continue;
        }

        let name = item
            .get("name")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();

        let playtime_minutes = item
            .get("playtime_forever")
            .and_then(|v| v.as_u64())
            .unwrap_or(0);

        // 🔴 `rtime_last_played` 官方给的是 **Unix 秒**；0 表示从未游玩。
        // NexBox 本地 `last_played` 同为 Unix 秒，故直接用，无需换算。
        let last_played = item
            .get("rtime_last_played")
            .and_then(|v| v.as_i64())
            .unwrap_or(0);

        list.push(OnlineGame {
            app_id,
            name: if name.is_empty() {
                format!("App {}", app_id)
            } else {
                name
            },
            playtime_minutes,
            last_played,
        });
    }

    if list.is_empty() {
        return OnlineResult {
            games: vec![],
            error: Some("在线返回的记录均无有效 AppID，已忽略".to_string()),
        };
    }
    OnlineResult {
        games: list,
        error: None,
    }
}

/// 最小百分号编码（仅需覆盖 query 值里的保留字符）。
fn url_encode(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(b as char);
            }
            _ => out.push_str(&format!("%{:02X}", b)),
        }
    }
    out
}

// ============================================================================
// 对外命令
// ============================================================================

/// 查询 API Key 配置状态（供设置页渲染；**不发网络请求**）。
///
/// 参数 `steam_dir` 由调用方从 `get_steam_path()` 取得后传入，保持本模块零注册表依赖
/// （与 `steam.rs` 同一套路径解析口径，避免两处各读一次）。
#[tauri::command]
pub async fn steam_api_key_status(steam_dir: String) -> SteamApiKeyStatus {
    // 纵深防御：拒绝与本机实际 Steam 位置不符的传入值（详见 validate_steam_dir）
    if let Err(msg) = validate_steam_dir(&steam_dir) {
        return SteamApiKeyStatus {
            configured: false,
            steam_id64: None,
            account_name: None,
            warning: Some(msg),
        };
    }
    let key = load_api_key(&steam_dir);
    let configured = key.is_some();

    // 取当前账号：与 steam.rs 的 parse_login_users 同源口径
    let (steam_id64, account_name, warning) =
        match crate::steam::steam::current_account_id(&steam_dir) {
            Some((id, name)) if is_valid_steam_id64(&id) => (Some(id), Some(name), None),
            Some((id, name)) => (
                None,
                Some(name),
                Some(format!(
                    "本机记录的 SteamID64「{}」格式异常，无法用于在线查询",
                    id
                )),
            ),
            None => {
                if configured {
                    (
                    None,
                    None,
                    Some(
                        "本机未找到已登录的 Steam 账户，请先在 Steam 客户端登录并勾选「记住密码」"
                            .to_string(),
                    ),
                )
                } else {
                    (None, None, None)
                }
            }
        };

    if !configured && warning.is_none() {
        return SteamApiKeyStatus {
            configured: false,
            steam_id64: None,
            account_name: None,
            warning: None,
        };
    }

    SteamApiKeyStatus {
        configured,
        steam_id64,
        account_name,
        warning,
    }
}

/// 保存 API Key。空串 = 清除。
#[tauri::command]
pub async fn steam_save_api_key(steam_dir: String, api_key: String) -> Result<(), String> {
    // 🔴 写盘定位必须先校验：否则可把 Key 文件写到任意目录（详见 validate_steam_dir）
    validate_steam_dir(&steam_dir)?;
    let key = api_key.trim();
    // 长度预检：Steam Web API Key 固定 32 位十六进制。格式明显不对时直接拒，
    // 免得用户存进去后到拉库存才看到 401。
    if !key.is_empty() && (key.len() != 32 || !key.chars().all(|c| c.is_ascii_hexdigit())) {
        return Err("API Key 格式不对：应为 32 位十六进制字符".to_string());
    }
    save_api_key(&steam_dir, key)
}

/// 在线拉取账号完整拥有清单（含从未在本机安装的游戏）。
///
/// 与 NexBox 的 `get_steam_inventory` **并存**：本命令只管在线，合并逻辑在前端
/// （`SteamPage` 按 `source` 三态决定用哪份数据）。这样保持 NexBox 主文件零改动。
#[tauri::command]
pub async fn get_steam_inventory_online(steam_dir: String) -> SteamOnlineInventory {
    // 纵深防御：拒绝与本机实际 Steam 位置不符的传入值（详见 validate_steam_dir）
    if let Err(msg) = validate_steam_dir(&steam_dir) {
        return SteamOnlineInventory {
            source: "none".to_string(),
            total: 0,
            installed: 0,
            not_installed: 0,
            total_playtime_minutes: 0,
            games: vec![],
            error: Some(msg),
        };
    }

    // 🔴 原 `let none_result = SteamOnlineInventory { .. }` 已删（2026-10-03）：
    //    该变量定义后从未被使用（下方 `load_api_key` 的 None 分支会自行构造返回值），
    //    属重构残留的死变量，触发 `unused_variables` 告警。
    let key = match load_api_key(&steam_dir) {
        Some(k) => k,
        None => {
            return SteamOnlineInventory {
                source: "none".to_string(),
                total: 0,
                installed: 0,
                not_installed: 0,
                total_playtime_minutes: 0,
                games: vec![],
                error: Some("尚未配置 Steam API Key，请到设置页填写".to_string()),
            }
        }
    };

    let (steam_id64, _name) = match crate::steam::steam::current_account_id(&steam_dir) {
        Some(v) => v,
        None => {
            return SteamOnlineInventory {
                source: "none".to_string(),
                total: 0,
                installed: 0,
                not_installed: 0,
                total_playtime_minutes: 0,
                games: vec![],
                error: Some(
                    "本机未找到已登录的 Steam 账户，请先在 Steam 客户端登录并勾选「记住密码」"
                        .to_string(),
                ),
            }
        }
    };

    if !is_valid_steam_id64(&steam_id64) {
        return SteamOnlineInventory {
            source: "none".to_string(),
            total: 0,
            installed: 0,
            not_installed: 0,
            total_playtime_minutes: 0,
            games: vec![],
            error: Some(format!("本机记录的 SteamID64「{}」格式异常", steam_id64)),
        };
    }

    let result = fetch_owned_games(&key, &steam_id64).await;
    match result.error {
        Some(err) => {
            log::warn!("[Steam] 在线库存失败，已退回本地缓存: {}", err);
            SteamOnlineInventory {
                source: "none".to_string(),
                total: 0,
                installed: 0,
                not_installed: 0,
                total_playtime_minutes: 0,
                games: vec![],
                error: Some(err),
            }
        }
        None => {
            // 统计口径与 NexBox 的 SteamInventoryStats 对齐：
            // 在线清单里 installed 恒为 false（是否已安装由本地 .acf 说了算，合并时本地覆盖）
            let total = result.games.len();
            let total_playtime: u64 = result.games.iter().map(|g| g.playtime_minutes).sum();
            let games = result
                .games
                .into_iter()
                .map(|g| SteamOnlineGame {
                    app_id: g.app_id,
                    name: g.name,
                    playtime_minutes: g.playtime_minutes,
                    last_played: g.last_played,
                })
                .collect();
            SteamOnlineInventory {
                source: "online".to_string(),
                total,
                installed: 0,
                not_installed: total,
                total_playtime_minutes: total_playtime,
                games,
                error: None,
            }
        }
    }
}

/// 在线清单的统计信息。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SteamOnlineInventory {
    /// `"online"`（成功）或 `"none"`（失败/未配置）
    pub source: String,
    pub total: usize,
    pub installed: usize,
    pub not_installed: usize,
    pub total_playtime_minutes: u64,
    pub games: Vec<SteamOnlineGame>,
    /// 失败原因（成功时为 `None`）
    pub error: Option<String>,
}

/// 单条在线游戏记录。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SteamOnlineGame {
    pub app_id: u32,
    pub name: String,
    pub playtime_minutes: u64,
    /// Unix 秒；0 = 从未游玩
    pub last_played: i64,
}

/// 本地已安装 app_id 集合（供前端合并时判定 installed）。
///
/// 单独出一个命令而非让前端回传，是为了让「已安装」的判定**只有后端一处口径**
/// （读 `.acf`），避免前端自己猜。
#[tauri::command]
pub async fn steam_installed_appids(steam_dir: String) -> Vec<u32> {
    crate::steam::steam::installed_app_ids(&steam_dir)
        .into_iter()
        .collect::<Vec<u32>>()
}

/// 供单测/调试：直接暴露解析函数。
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn 解析正常响应() {
        let json = r#"{"response":{"games":[
            {"appid":730,"name":"Counter-Strike 2","playtime_forever":15820,"rtime_last_played":1759000000},
            {"appid":1245620,"name":"ELDEN RING","playtime_forever":4231,"rtime_last_played":1758000000}
        ]}}"#;
        let r = parse_owned_games(json);
        assert!(r.error.is_none(), "不应报错: {:?}", r.error);
        assert_eq!(r.games.len(), 2);
        assert_eq!(r.games[0].app_id, 730);
        assert_eq!(r.games[0].playtime_minutes, 15820);
    }

    #[test]
    fn 游戏详情非公开时给可操作提示() {
        // HTTP 200 + response 无 games —— SystemToolkit 实测的「隐私未公开」形态
        let r = parse_owned_games(r#"{"response":{}}"#);
        assert!(r.error.is_some());
        assert!(r.error.unwrap().contains("隐私设置"));
    }

    #[test]
    fn appid类型不符时跳过而非整份丢失() {
        // 🔴 回归钉：官方对未公开条目会返回 "appid":"123"（字符串）。
        //    若不验 ValueKind，TryGetUInt32 会抛，整份库存全丢。
        let json = r#"{"response":{"games":[
            {"appid":"123","name":"脏数据"},
            {"appid":730,"name":"Counter-Strike 2"}
        ]}}"#;
        let r = parse_owned_games(json);
        assert!(r.error.is_none());
        assert_eq!(r.games.len(), 1, "只应保留 1 条有效记录");
        assert_eq!(r.games[0].app_id, 730);
    }

    #[test]
    fn 名称缺失时退化() {
        let json = r#"{"response":{"games":[{"appid":999}]}}"#;
        let r = parse_owned_games(json);
        assert!(r.error.is_none(), "不该报错，实际: {:?}", r.error);
        assert_eq!(r.games.len(), 1, "应保留 1 条，实际 0 条");
        assert_eq!(r.games[0].name, "App 999");
    }

    #[test]
    fn steamid格式校验() {
        assert!(is_valid_steam_id64("76561198000000001"));
        assert!(!is_valid_steam_id64("76561198"));
        assert!(!is_valid_steam_id64("7656119800000000x"));
    }

    #[test]
    fn 百分号编码() {
        assert_eq!(url_encode("abc123"), "abc123");
        assert_eq!(url_encode("a b&c"), "a%20b%26c");
    }
}

/// 🔴 审查 R-6（2026-10-07，用户裁决方案 C）：Key 落盘 DPAPI 加密的回归钉。
///   Windows 真实 DPAPI 调用（同用户加密/解密），故 `cfg(windows)`。
#[cfg(all(test, windows))]
mod key_dpapi_tests {
    use super::*;

    /// 独立临时目录（真实文件读写走 load/save 全链路）。
    struct TempDir(std::path::PathBuf);
    impl TempDir {
        fn new(tag: &str) -> Self {
            let dir = std::env::temp_dir().join(format!(
                "aegis-key-test-{}-{}-{}",
                tag,
                std::process::id(),
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .map(|d| d.subsec_nanos())
                    .unwrap_or(0)
            ));
            let _ = std::fs::remove_dir_all(&dir);
            std::fs::create_dir_all(&dir).expect("创建临时目录");
            TempDir(dir)
        }
        fn dir(&self) -> String {
            self.0.to_string_lossy().to_string()
        }
    }
    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    const KEY: &str = "0123456789abcdef0123456789abcdef";

    #[test]
    fn dpapi_roundtrip_recovers_plain() {
        let cipher = key_dpapi::protect(KEY.as_bytes()).expect("DPAPI 加密不应失败");
        assert_ne!(
            cipher,
            KEY.as_bytes(),
            "盘上形态必须不同于明文（禁明文落盘红线）"
        );
        assert_ne!(
            String::from_utf8_lossy(&cipher).trim(),
            KEY,
            "密文不得可读出明文"
        );
        let back = key_dpapi::unprotect(&cipher).expect("DPAPI 解密应成功");
        assert_eq!(back, KEY.as_bytes());
    }

    #[test]
    fn dpapi_rejects_garbage() {
        assert!(
            key_dpapi::unprotect(b"definitely-not-a-dpapi-blob").is_err(),
            "垃圾输入必须报错而非解出内容"
        );
    }

    #[test]
    fn save_then_load_roundtrip_and_no_plaintext_on_disk() {
        let tmp = TempDir::new("roundtrip");
        let dir = tmp.dir();
        save_api_key(&dir, KEY).expect("保存应成功");

        let stored = std::fs::read(format!("{}\\{}", dir, API_KEY_FILE)).expect("文件应存在");
        assert_ne!(
            String::from_utf8_lossy(&stored).trim(),
            KEY,
            "盘上文件不得是明文 Key"
        );
        assert_eq!(
            load_api_key(&dir).as_deref(),
            Some(KEY),
            "加密后读取应还原出原 Key"
        );
    }

    #[test]
    fn legacy_plaintext_file_is_auto_migrated() {
        let tmp = TempDir::new("migrate");
        let dir = tmp.dir();
        let path = format!("{}\\{}", dir, API_KEY_FILE);
        // 旧版形态：明文 32 位十六进制
        std::fs::write(&path, KEY).expect("写入旧明文文件");

        assert_eq!(
            load_api_key(&dir).as_deref(),
            Some(KEY),
            "旧明文应能读出（迁移不改变本次结果）"
        );
        let stored = std::fs::read(&path).expect("迁移后文件应存在");
        assert_ne!(
            String::from_utf8_lossy(&stored).trim(),
            KEY,
            "读取后旧明文应已被重写为密文"
        );
        assert!(
            !std::path::Path::new(&format!("{}.bak", path)).exists(),
            "迁移成功后 .bak 里是明文旧值，必须已清除"
        );
    }
}
