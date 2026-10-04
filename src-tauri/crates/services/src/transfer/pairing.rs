//! 一次性配对码 —— **两条通道共用的唯一信任根**（R-1 / R-2 / R-3 / R-5；F-FT-13 / F-FT-37 / F-FT-38）。
//!
//! ## 为什么这个模块必须先于"服务能收文件"落地
//!
//! R-1 原文：*"设备发现、已知设备白名单、IP 匹配都只是**便利过滤**，不得作为授权依据使用；
//! 任何跨机写入前必须存在有效配对凭据"*。所以**先起一个能收文件的服务、配对以后再说**是不可接受的：
//! 那期间任何局域网内的人都能往用户机器里写文件。本模块落地后，接收侧才有资格打开闸门。
//!
//! ## R-2 的六条契约（逐条对应，且**两条通道必须共用本实现**）
//!
//! | 契约 | 落点 |
//! |---|---|
//! | **6 位** | [`CODE_LENGTH`] |
//! | 字符集排除易混字形（不含 `0`/`O`/`1`/`I`） | [`ALPHABET`]（**实测 31 字符**：8 个数字 + 23 个字母，`L` 也被排除） |
//! | **大小写不敏感** | [`PairingGuard::authorize`] 先 `to_ascii_uppercase` |
//! | **10 分钟惰性轮换**（到点后下次需要时才换，旧码同时作废） | [`PairingGuard::current_code`] |
//! | **一次性消费**（用过即废，重放必失败） | [`PairingGuard::authorize`] 成功即清空 |
//! | **比较不泄露时序** | [`constant_time_eq`] + 定长零填充 |
//!
//! ## R-3 与 R-5
//!
//! * **R-3 免码范围严格限定**：记住的是"该对端**地址** + **本次服务运行期**"。
//!   本模块只存 IP（不含端口——客户端端口是临时分配的），且**实例即运行期**：
//!   服务重启 = 新实例 = 免码全部失效（无任何持久化）✓。
//! * **R-5 配对节流**：同一设备连续 **5 次**失败锁定 **60 秒**；锁定是"可理解的等待提示"，
//!   不是永久拉黑——锁定到期后自动放行。
//!
//! ## 🔴 与"直连主机名"有关的实现差异（有意偏离母本）
//!
//! 母本用 `ipPort.LastIndexOf(':')` 裁掉端口来取 IP。那对 **IPv6 会切错**
//! （`[fe80::1]:5000` 的最后一个冒号在地址内部）。本工程直接用 [`SocketAddr::ip`]，
//! 从根上没有这个缺陷。
//!
//! 复用结论（DP-1）：③思路复用 ← 母本 `PairingService.cs`（字符集、6 位、惰性轮换、
//! 一次性消费、常量时间比较、32 字节零填充）与 `FileTransferService.cs` 的
//! `_pairedIps` / `IsPairedIp`（按 IP 记住、服务启停时 `Clear()`）。
//! 🔴 **一处简化**：母本手写"拒绝采样"以消除取模偏置；本工程改用 `rand` 的
//! `gen_range`（内部即无偏采样），少一段容易写错的位运算。

use std::collections::{HashMap, HashSet};
use std::net::IpAddr;
use std::time::Duration;

use rand::Rng;

/// 配对码字母表（R-2：31 字符 = 8 个数字 + 23 个字母，排除易混字形 `0`/`O`/`1`/`I`/`L`）。
///
/// 🔴 **逐字与母本 `PairingService.CodeAlphabet` 相同**——R-2 要求两条通道"同一份规则"，
/// 改动本常量等于改动契约。
///
/// 🔴 **长度必须是 31（不是 32）**：本批第一版按"32 字符便于无偏采样"的**假设**写了文档与用例，
/// 被自己的字母表契约用例抓出来。31 与母本一致（母本注释里那个 `256 % 31 = 8` 说的就是它）。
/// 因为不是 2 的幂，**必须**用无偏采样（[`generate_code`] 走 `rand::gen_range`）。
pub const ALPHABET: &[u8] = b"23456789ABCDEFGHJKMNPQRSTUVWXYZ";

/// 配对码长度（R-2：6 位）。
pub const CODE_LENGTH: usize = 6;

/// 配对码有效期（R-2：10 分钟惰性轮换）。
pub const CODE_LIFETIME: Duration = Duration::from_secs(10 * 60);

/// 同一设备的连续失败上限（R-5：5 次）。
pub const MAX_CONSECUTIVE_FAILURES: u32 = 5;

/// 触顶后的锁定时长（R-5：60 秒）。
pub const LOCKOUT: Duration = Duration::from_secs(60);

/// 常量时间比较用的定长缓冲（R-2：比较不泄露时序）。
///
/// 取 32 是因为"长度不同的输入也要走同一条路径"——先把两边都零填充到定长，
/// 长度差异就不参与比较（配对码长度本身是公开的 6，不构成秘密）。
const COMPARE_BUFFER: usize = 32;

/// 配对裁决。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PairingVerdict {
    /// 用户显式关闭了配对要求（R-1：需用户显式改动并承担后果）
    NotRequired,
    /// 该地址在**本次运行期**内已配对过 → 免码（F-FT-13 / R-3）
    AlreadyAuthorized,
    /// 配对码正确，**已消费**（同一码再用必失败）
    Consumed,
    /// 码缺失 / 错误 / 已消费 / 已过期
    Invalid,
    /// 该地址正被节流锁定（R-5），`retry_after_ms` 后自动放行
    Throttled { retry_after_ms: i64 },
}

impl PairingVerdict {
    /// 是否放行。
    #[must_use]
    pub fn is_allowed(&self) -> bool {
        matches!(
            self,
            Self::NotRequired | Self::AlreadyAuthorized | Self::Consumed
        )
    }
}

/// 某地址的连续失败记录。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Failures {
    consecutive: u32,
    locked_until_ms: i64,
}

/// 一次性配对码守卫（**实例即"一次服务运行期"**）。
#[derive(Debug)]
pub struct PairingGuard {
    require_pairing: bool,
    lifetime_ms: i64,
    code: String,
    expires_at_ms: i64,
    authorized_ips: HashSet<IpAddr>,
    failures: HashMap<IpAddr, Failures>,
}

impl PairingGuard {
    /// 新建一次"运行期"。`require_pairing` 取设置里的开关（默认 [`true`]）。
    ///
    /// 🔴 服务重启 = 新建实例 = 免码与轮换状态全部重置（R-3：免码只对本次运行期有效）。
    #[must_use]
    pub fn new(require_pairing: bool) -> Self {
        Self::with_lifetime(require_pairing, CODE_LIFETIME)
    }

    /// 指定有效期（测试用；生产用 [`CODE_LIFETIME`]）。
    #[must_use]
    pub fn with_lifetime(require_pairing: bool, lifetime: Duration) -> Self {
        Self {
            require_pairing,
            lifetime_ms: i64::try_from(lifetime.as_millis()).unwrap_or(i64::MAX),
            code: String::new(),
            expires_at_ms: i64::MIN,
            authorized_ips: HashSet::new(),
            failures: HashMap::new(),
        }
    }

    /// 当前有效配对码（**惰性生成与轮换**：到期后下一次读取才换新码，旧码同时作废）。
    pub fn current_code(&mut self, now_ms: i64) -> &str {
        self.ensure_fresh_code(now_ms);
        &self.code
    }

    /// 距轮换还剩多少毫秒（界面秒级倒计时用；已是新码则为整个有效期）。
    pub fn expires_in_ms(&mut self, now_ms: i64) -> i64 {
        self.ensure_fresh_code(now_ms);
        self.expires_at_ms.saturating_sub(now_ms).max(0)
    }

    /// 本次运行期已授权的地址数（诊断/测试用）。
    #[must_use]
    pub fn authorized_count(&self) -> usize {
        self.authorized_ips.len()
    }

    /// 某地址当前是否免码。
    #[must_use]
    pub fn is_authorized(&self, peer: IpAddr) -> bool {
        self.authorized_ips.contains(&peer)
    }

    /// 裁决一次配对尝试（R-13 的配对闸门就调它）。
    ///
    /// 判定顺序：**先节流** → 再"本次运行期已免码" → 再验码。
    /// 先查节流是为了让锁定期的行为可预期（锁定期内即便码正确也不放行，
    /// 否则"锁定"形同虚设）。
    ///
    /// 🔴 无论 `provided` 是 `None` 还是错值，都会走同一条常量时间比较路径。
    pub fn authorize(
        &mut self,
        peer: IpAddr,
        provided: Option<&str>,
        now_ms: i64,
    ) -> PairingVerdict {
        if !self.require_pairing {
            return PairingVerdict::NotRequired;
        }

        // ① 节流（R-5）：锁定期内一律不放行，且**不**继续累加失败
        if let Some(record) = self.failures.get(&peer) {
            if now_ms < record.locked_until_ms {
                return PairingVerdict::Throttled {
                    retry_after_ms: record.locked_until_ms.saturating_sub(now_ms),
                };
            }
        }

        // ② 本次运行期已配对过（F-FT-13 / R-3）
        if self.authorized_ips.contains(&peer) {
            return PairingVerdict::AlreadyAuthorized;
        }

        // ③ 验码
        self.ensure_fresh_code(now_ms);
        let expired = now_ms >= self.expires_at_ms;
        let candidate = provided.unwrap_or("").trim().to_ascii_uppercase();
        let matched = !expired
            && candidate.chars().count() == CODE_LENGTH
            && constant_time_eq(&self.code, &candidate);

        if matched {
            // 🔴 一次性：成功即失效，重放同一码必失败（R-2）
            self.code.clear();
            self.expires_at_ms = i64::MIN;
            self.authorized_ips.insert(peer);
            self.failures.remove(&peer);
            return PairingVerdict::Consumed;
        }

        // ④ 失败计数与锁定（R-5）
        let record = self.failures.entry(peer).or_insert(Failures {
            consecutive: 0,
            locked_until_ms: i64::MIN,
        });
        record.consecutive = record.consecutive.saturating_add(1);
        if record.consecutive >= MAX_CONSECUTIVE_FAILURES {
            record.locked_until_ms =
                now_ms.saturating_add(i64::try_from(LOCKOUT.as_millis()).unwrap_or(i64::MAX));
            record.consecutive = 0; // 锁定到期后重新计数（R-5：不是永久拉黑）
        }
        PairingVerdict::Invalid
    }

    /// 惰性生成/轮换（调用方不必持有锁：本结构按 `&mut self` 独占）。
    fn ensure_fresh_code(&mut self, now_ms: i64) {
        if !self.code.is_empty() && now_ms < self.expires_at_ms {
            return;
        }
        self.code = generate_code();
        self.expires_at_ms = now_ms.saturating_add(self.lifetime_ms);
    }
}

/// 生成一个 6 位配对码。
///
/// 复用 `rand` 的 `gen_range`（内部无偏），不手写取模或拒绝采样。
#[must_use]
pub fn generate_code() -> String {
    let mut rng = rand::thread_rng();
    (0..CODE_LENGTH)
        .map(|_| ALPHABET[rng.gen_range(0..ALPHABET.len())] as char)
        .collect()
}

/// 常量时间比较（先各自零填充到 [`COMPARE_BUFFER`]，故长度差异不参与比较）。
///
/// 🔴 只用于**等长**缓冲之间的比较；调用方负责填充。
#[must_use]
fn constant_time_eq(expected: &str, candidate: &str) -> bool {
    let mut left = [0_u8; COMPARE_BUFFER];
    let mut right = [0_u8; COMPARE_BUFFER];
    for (slot, byte) in left.iter_mut().zip(expected.as_bytes()) {
        *slot = *byte;
    }
    for (slot, byte) in right.iter_mut().zip(candidate.as_bytes()) {
        *slot = *byte;
    }
    let mut diff = 0_u8;
    for (a, b) in left.iter().zip(right.iter()) {
        diff |= a ^ b;
    }
    diff == 0
}

#[cfg(test)]
mod tests;
