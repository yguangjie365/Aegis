//! 域内唯一的「短随机串」生成器。
//!
//! 🔴 **为什么三处都必须用这一份**：`commands`（规则 id）、`manifest`（快照目录后缀）、
//!   `restore`（改名让位的兜底名）原本各自写了一份，其中**两份有同一个缺陷**，
//!   而只有一份在实测里暴露出来。同款算法留三份 ⇒ 修一处漏两处。
//!
//! # 本模块存在的唯一理由：朴素取模写法会「耗尽种子」
//!
//! 朴素的「取模 + 除法」写法长这样：
//!
//! ```text
//! for _ in 0..len {
//!     out.push(ALPHABET[state % BASE]);
//     state /= BASE;              // ← 每轮消耗 log2(BASE) 位
//!     if state == 0 { state = 兜底值; }
//! }
//! ```
//!
//! `state` 是 `u64`（64 位）。基数 32 时每轮消耗 5 位 ⇒ **第 13 轮起种子归零**，
//! 之后每一位都由那个**恒定**的兜底值产生。
//!
//! 实测症状（2026-10-06）：规则 id 长成 `r0fe7pvaietmh111111111111` ——
//! 后 12 位全是 `1`（兜底值 `0 | 1 = 1`，`1 % 32 = 1` 且 `1 / 32 = 0` ⇒ 自我循环）。
//!
//! 危害不只是「难看」：注释声称有 `32^24 ≈ 1.2e36` 的空间，实际只有 `32^12`。
//! **注释与实现不符本身就是缺陷** —— 后来者会据此误判碰撞风险。
//!
//! # 修法：每轮重新做强混合（`SplitMix64` 单步）
//!
//! 不做「除法推进」，而是每轮对状态做一次满雪崩混合后再取一位。
//! 好处是**每一位都依赖整个 64 位状态**，不会出现恒定尾部。
//!
//! # 强度说明（刻意不引入 `rand`）
//!
//! 本生成器**不是密码学安全的**，也不需要：用途是文件名去重与规则寻址。
//! 唯一性来源 = 系统时间（纳秒）^ 进程内单调计数器，保证：
//! ① 同一次调用内 24 位互不相同（混合）；② 同一进程内绝不重复（计数器）；
//! ③ 跨进程重启不重复（时间戳主导，且计数器只做扰动）。
//! `services` 虽已依赖 `rand`（`NexBox` 音乐域引入），但为 24 个字符引入全局随机源
//! 不划算，且会让「同一次调用可复现」变得不可能（测试更难写）。

use std::sync::atomic::{AtomicU64, Ordering};

/// 进程内单调计数器（同刻两次调用不撞）。
static COUNTER: AtomicU64 = AtomicU64::new(0);

/// base32 字母表（`0-9a-v`）。
///
/// 🔴 **规则 id 必须用它**：`rule_dir_name` 会过 `infra::safe::sanitize_file_name`，
///   而非法字符会被**替换** ⇒ 两个不同 id 可能 sanitize 成同一个目录名
///   （那只是把 bug 换个位置）。本表每个字符都会被 `sanitize_file_name` 原样保留。
pub const BASE32: &[u8] = b"0123456789abcdefghijklmnopqrstuv";

/// 小写字母 + 数字（快照目录后缀用）。
pub const ALNUM_LOWER: &[u8] = b"abcdefghijklmnopqrstuvwxyz0123456789";

/// 十六进制小写（恢复侧兜底名用）。
pub const HEX_LOWER: &[u8] = b"0123456789abcdef";

/// 规则 id 的随机段长度。
pub const RULE_ID_LEN: usize = 24;

/// `SplitMix64` 单步混合（满雪崩）。
///
/// 这三个常量是 `SplitMix64` 的固定参数，取自 Vigna 的原始实现；
/// 换掉它们会削弱雪崩性，**不要改**。
#[must_use]
fn mix(state: u64) -> u64 {
    let z = state.wrapping_add(0x9E37_79B9_7F4A_7C15);
    let z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    let z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
}

/// 本次调用的种子：纳秒时间戳 ^ 计数器扰动。
///
/// 🔴 时间戳用 `as_secs × 1e9 + subsec_nanos` 组装，**不用 `as u64`**
///   （`as_nanos()` 返 `u128`，`as` 是静默截断；这里显式组装等价且无歧义）。
#[must_use]
fn seed(counter: u64) -> u64 {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| {
            d.as_secs()
                .wrapping_mul(1_000_000_000)
                .wrapping_add(u64::from(d.subsec_nanos()))
        });
    nanos ^ counter.wrapping_mul(0x9E37_79B9_7F4A_7C15)
}

/// 从 `alphabet` 中取字符，生成 `len` 位的随机串。
///
/// `alphabet` 为空或 `len` 为 0 ⇒ 返回空串（不 panic；调用方传的都是编译期常量表）。
#[must_use]
pub fn random_token(len: usize, alphabet: &[u8]) -> String {
    if alphabet.is_empty() || len == 0 {
        return String::new();
    }
    let base = alphabet.len() as u64;
    let mut state = seed(COUNTER.fetch_add(1, Ordering::Relaxed));
    let mut out = String::with_capacity(len);
    for _ in 0..len {
        // 🔴 每轮先混合再取位：这是「不耗尽种子」的关键。
        //   若写成 `state /= base`，第 13 轮起就会输出恒定字符。
        state = mix(state);
        let index = usize::try_from(state % base).unwrap_or(0);
        out.push(alphabet[index] as char);
    }
    out
}

/// 新规则的稳定唯一标识（`r` + 24 位 base32）。
///
/// 🔴 **id 必须由后端生成**（F-FB-05：规则要有稳定标识才能寻址既有快照）。
///   前端 `emptyRule()` 的 id 是空串（它不知道也不该知道 id 格式）；
///   若原样落盘，`rule_dir_name("")` 会 sanitize 失败退回常量 `"rule"`
///   ⇒ 所有空 id 规则的快照挤进同一目录，且前端 `enabled: ruleId !== ""` 恒 false
///   ⇒ 快照列表永久骨架屏。
#[must_use]
pub fn rule_id() -> String {
    let mut id = String::with_capacity(RULE_ID_LEN + 1);
    id.push('r');
    id.push_str(&random_token(RULE_ID_LEN, BASE32));
    id
}

#[cfg(test)]
mod tests;
