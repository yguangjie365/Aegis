use super::*;
use std::collections::HashSet;

/// 单个串内**最长连续相同字符**的长度。
///
/// # 🔴 这是本模块的核心判据，不能换成「两次生成是否相同」
///
/// 朴素取模写法（`state /= base` + 种子耗尽兜底）的产物特征是
/// **尾部一段同一字符的重复**，而不是「所有样本在某位上取值恒定」——
/// 因为兜底值 `n.wrapping_add(1) | 1` 随计数器 `n` 变化，
/// 所以逐位置看去仍然「在变」，唯一性测试**完全抓不到它**。
///
/// 真正的机制（以基数 32 为例）：
/// ```text
/// state=1 → '1' ; state /= 32 → 0 ; 命中兜底 → state = n|1
/// 若 n=0: 兜底=1 ⇒ 下一轮又输出 '1'，且 state 又归 0 ⇒ 无限循环
/// ```
/// 故尾部必然是**同一字符的长重复串**。
fn longest_run(s: &str) -> usize {
    let mut best = 0;
    let mut cur = 0;
    let mut prev: Option<char> = None;
    for c in s.chars() {
        if Some(c) == prev {
            cur += 1;
        } else {
            cur = 1;
            prev = Some(c);
        }
        best = best.max(cur);
    }
    best
}

/// 允许的最长连续重复。
///
/// 24 位、32 字母表下，出现 5 连同字符的概率约 `20 × 32^-4 ≈ 2e-5`；
/// 200 个样本里期望出现 0.004 次 ⇒ 几乎不可能误报。
const MAX_RUN: usize = 4;

#[test]
fn run_detector_flags_the_real_observed_bad_id() {
    // 🔴 **阳性对照**（记忆纪律：自写检查器先跑阳性对照）：
    //   下面这个串是 2026-10-06 从真机落盘的
    //   `%LOCALAPPDATA%\Aegis\config\backup.json` 里**读出来的**，不是构造的。
    //   没有这一条，「新实现无长重复」这个结论就没有仪器背书。
    const REAL_BAD: &str = "r0fe7pvaietmh111111111111";
    assert_eq!(
        longest_run(REAL_BAD),
        12,
        "判据必须能量出真机坏样本的尾部重复长度"
    );
    assert!(
        longest_run(REAL_BAD) > MAX_RUN,
        "阈值必须把真机坏样本判为坏，否则整个判据形同虚设"
    );
}

#[test]
fn generated_ids_have_no_long_character_run() {
    // 旧实现 24 位里必然出现 ≥12 连长重复（种子耗尽）⇒ 这条会立刻红。
    for _ in 0..200 {
        let id = rule_id();
        let run = longest_run(&id);
        assert!(
            run <= MAX_RUN,
            "id 出现 {run} 连重复字符（阈值 {MAX_RUN}），疑似种子耗尽：{id}"
        );
    }
}

#[test]
fn token_characters_are_not_fixed_per_position() {
    // 补充判据（比上一条弱，但能抓「某位恒定」这类未来回归）。
    let samples: Vec<String> = (0..200)
        .map(|_| random_token(RULE_ID_LEN, BASE32))
        .collect();
    for pos in 0..RULE_ID_LEN {
        let seen: HashSet<char> = samples.iter().filter_map(|s| s.chars().nth(pos)).collect();
        assert!(
            seen.len() >= 2,
            "第 {pos} 位在所有样本里取值恒定（{seen:?}）—— 随机性不足"
        );
    }
}

#[test]
fn rule_id_has_expected_shape() {
    let id = rule_id();
    assert_eq!(id.chars().count(), RULE_ID_LEN + 1, "id 总长应为 1 + 24");
    assert!(id.starts_with('r'), "id 必须以 r 开头");
    for c in id.chars().skip(1) {
        assert!(BASE32.contains(&(c as u8)), "id 出现非 base32 字符：{c}");
    }
}

#[test]
fn rule_id_survives_filename_sanitize_unchanged() {
    // 🔴 这是本表选 base32 的**唯一理由**：若 sanitize 会改写字符，
    //   两个不同 id 可能落进同一目录名 ⇒ 快照互相覆盖。
    for _ in 0..50 {
        let id = rule_id();
        let sanitized = infra::safe::sanitize_file_name(&id)
            .expect("id 每个字符都是 base32，sanitize 不该失败");
        assert_eq!(sanitized, id, "id 经 sanitize 后必须原样保留");
    }
}

#[test]
fn ids_are_unique_within_process() {
    let ids: HashSet<String> = (0..1000).map(|_| rule_id()).collect();
    assert_eq!(ids.len(), 1000, "进程内 1000 次生成必须两两不同");
}

#[test]
fn tokens_with_mixed_alphabets_are_unique_within_process() {
    // 三处调用点用的是**不同字母表**（快照后缀 alnum / 恢复兜底 hex），
    // 共用同一实现后必须仍然唯一。
    let a: HashSet<String> = (0..500).map(|_| random_token(6, ALNUM_LOWER)).collect();
    let b: HashSet<String> = (0..500).map(|_| random_token(12, HEX_LOWER)).collect();
    assert_eq!(a.len(), 500);
    assert_eq!(b.len(), 500);
}

#[test]
fn degenerate_inputs_do_not_panic() {
    // 边界：空表 / 零长。调用方传的都是编译期常量表，但契约上不该 panic。
    assert_eq!(random_token(0, BASE32), "");
    assert_eq!(random_token(8, b""), "");
}

#[test]
fn every_character_in_alphabet_is_reachable() {
    // 若混合函数退化（例如恒返同一值），长样本里只会出现少数几个字符。
    let samples: String = (0..400).map(|_| random_token(24, BASE32)).collect();
    let seen: HashSet<char> = samples.chars().collect();
    assert!(
        seen.len() >= 30,
        "32 个字母里只出现 {} 种，混合函数疑似退化",
        seen.len()
    );
}
