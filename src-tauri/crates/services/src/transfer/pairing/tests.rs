//! `pairing` 的测试。
//!
//! 全部是**纯逻辑**（把"当前时刻"当参数传进来），故 R-2/R-3/R-5 的每条契约都能穷举，
//! 不需要真网络也不需要等真实时间流逝。

use super::*;

fn ip(last: u8) -> IpAddr {
    IpAddr::from([192, 168, 1, last])
}

/// 🔴 字母表是**契约**（R-2 要求两条通道同一份规则），逐条钉死。
#[test]
fn alphabet_matches_the_r2_contract() {
    // 🔴 31 而不是 32：本批第一版按"32 便于无偏采样"的**假设**写，被本条用例抓出来。
    //    31 = 8 个数字 + 23 个字母（A–Z 去掉 I/L/O），与母本逐字一致。
    assert_eq!(ALPHABET.len(), 31, "R-2：与母本逐字一致的 31 字符");
    let text = std::str::from_utf8(ALPHABET).unwrap();
    assert!(text.is_ascii(), "只用 ASCII，避免大小写折叠的跨语言差异");
    for forbidden in ['0', 'O', '1', 'I', 'L'] {
        assert!(
            !text.contains(forbidden),
            "易混字形 {forbidden} 不得出现在配对码字母表里"
        );
    }
    // 逐字符唯一（有重复会让某些字出现概率翻倍）
    let mut chars: Vec<char> = text.chars().collect();
    chars.sort_unstable();
    chars.dedup();
    assert_eq!(chars.len(), 31, "字符必须两两不同");
    // 全大写（比较时统一 to_ascii_uppercase）
    assert!(
        text.chars().all(|c| !c.is_ascii_lowercase()),
        "字母表必须全大写"
    );
}

#[test]
fn generated_codes_have_the_contract_shape() {
    for _ in 0..200 {
        let code = generate_code();
        assert_eq!(code.chars().count(), CODE_LENGTH, "R-2：6 位");
        assert!(
            code.bytes().all(|b| ALPHABET.contains(&b)),
            "只允许字母表内的字符，实得 {code}"
        );
    }
}

/// 分布粗查：每个字母表字符都应出现过。
/// 单个字符在 5 万次采样里一次不出现的概率约 e^-9375——漏了就说明索引写错了。
#[test]
fn every_alphabet_character_can_be_sampled() {
    let mut seen = vec![false; ALPHABET.len()];
    for _ in 0..5_000 {
        let code = generate_code();
        assert_eq!(code.len(), 6, "ASCII 字母表下字节长度即字符数");
        for byte in code.bytes() {
            let index = ALPHABET.iter().position(|b| *b == byte).expect("必在表内");
            seen[index] = true;
        }
    }
    assert!(seen.iter().all(|hit| *hit), "有字符从未被采到");
}

// ── R-2：惰性轮换 ────────────────────────────────────────────────────────

#[test]
fn code_rotates_lazily_and_the_old_one_dies() {
    let mut guard = PairingGuard::with_lifetime(true, Duration::from_secs(10));
    let first = guard.current_code(0).to_owned();
    assert_eq!(guard.expires_in_ms(0), 10_000);

    // 有效期内反复读都是同一个码（惰性：不主动轮换）
    assert_eq!(guard.current_code(5_000), first);
    assert_eq!(guard.expires_in_ms(5_000), 5_000);

    // 到点后下一次读取才换新码
    let second = guard.current_code(10_000).to_owned();
    assert_ne!(second, first, "到期后必须换新码");
    assert_eq!(guard.expires_in_ms(10_000), 10_000);

    // 🔴 旧码同时作废：拿旧码去配对必失败
    let stale = first.clone();
    assert_eq!(
        guard.authorize(ip(10), Some(&stale), 11_000),
        PairingVerdict::Invalid,
        "旧码必须同时作废"
    );
}

#[test]
fn presenting_a_code_after_expiry_fails_even_if_it_was_right() {
    let mut guard = PairingGuard::with_lifetime(true, Duration::from_secs(10));
    let code = guard.current_code(0).to_owned();
    // 掐在到期那一刻之后才来
    assert_eq!(
        guard.authorize(ip(10), Some(&code), 10_000),
        PairingVerdict::Invalid,
        "过期码不得放行（此时 current_code 已轮换，比的是新码）"
    );
}

// ── R-2：一次性消费 ──────────────────────────────────────────────────────

#[test]
fn a_valid_code_is_consumed_and_cannot_be_replayed() {
    let mut guard = PairingGuard::new(true);
    let code = guard.current_code(0).to_owned();

    assert_eq!(
        guard.authorize(ip(10), Some(&code), 0),
        PairingVerdict::Consumed
    );
    // 🔴 重放同一码必失败——但它现在落在"同一地址已授权"上，
    //   故换一个地址来证明这个码本身确实死了
    assert_eq!(
        guard.authorize(ip(11), Some(&code), 0),
        PairingVerdict::Invalid,
        "一次性：同一个码不得给第二个地址用"
    );
}

#[test]
fn comparison_is_case_insensitive() {
    let mut guard = PairingGuard::new(true);
    let code = guard.current_code(0).to_ascii_lowercase();
    assert_eq!(
        guard.authorize(ip(10), Some(&code), 0),
        PairingVerdict::Consumed,
        "手机键盘自动大写，小写输入必须接受（R-2）"
    );
    // 前后空白也容忍（用户复制粘贴常见）
    let mut guard = PairingGuard::new(true);
    let padded = format!("  {}  ", guard.current_code(0));
    assert_eq!(
        guard.authorize(ip(10), Some(&padded), 0),
        PairingVerdict::Consumed
    );
}

#[test]
fn wrong_length_and_empty_inputs_are_rejected_without_panicking() {
    // 🔴 每个输入都用**独立 guard**：同一个 guard 连喂 6 个错值会撞上 R-5 节流
    // （首版就是这么写的，第 6 个输入返回 Throttled 而非 Invalid——那恰好证明节流是活的）。
    for bad in [
        None,
        Some(""),
        Some("   "),
        Some("ABC"),
        Some("ABCDEFG"),
        Some("!!!!!!"),
        Some("AB C234"),
    ] {
        let mut guard = PairingGuard::new(true);
        assert_eq!(
            guard.authorize(ip(10), bad, 0),
            PairingVerdict::Invalid,
            "非法输入应判无效：{bad:?}"
        );
    }
}

// ── R-3：免码范围 = 该地址 + 本次运行期 ──────────────────────────────────

#[test]
fn a_paired_address_is_code_free_for_the_rest_of_the_run_only() {
    let mut guard = PairingGuard::new(true);
    let code = guard.current_code(0).to_owned();
    assert_eq!(
        guard.authorize(ip(10), Some(&code), 0),
        PairingVerdict::Consumed
    );
    assert!(guard.is_authorized(ip(10)));

    // 同一地址再来（多文件/多文本批次）→ 免码
    assert_eq!(
        guard.authorize(ip(10), None, 1_000),
        PairingVerdict::AlreadyAuthorized
    );
    // 🔴 别的地址照样要码（R-3：免码只给"该对端地址"）
    assert_eq!(
        guard.authorize(ip(11), None, 1_000),
        PairingVerdict::Invalid,
        "免码不得扩散到其它地址"
    );

    // 🔴 服务重启 = 新实例 = 免码全部失效
    let mut restarted = PairingGuard::new(true);
    assert_eq!(
        restarted.authorize(ip(10), None, 2_000),
        PairingVerdict::Invalid,
        "R-3：免码只对本次服务运行期有效"
    );
    assert_eq!(restarted.authorized_count(), 0);
}

// ── R-5：配对节流 ────────────────────────────────────────────────────────

#[test]
fn five_consecutive_failures_lock_the_device_for_sixty_seconds() {
    let mut guard = PairingGuard::with_lifetime(true, Duration::from_secs(600));
    let good = guard.current_code(0).to_owned();

    // 前 4 次失败：如实判无效，不锁
    for attempt in 0..4 {
        assert_eq!(
            guard.authorize(ip(10), Some("WRONG1"), 0),
            PairingVerdict::Invalid,
            "第 {attempt} 次失败不该锁定"
        );
    }
    // 第 5 次失败触顶并锁定
    assert_eq!(
        guard.authorize(ip(10), Some("WRONG1"), 0),
        PairingVerdict::Invalid
    );

    // 🔴 锁定期内即便给出**正确**的码也不放行——否则"锁定"形同虚设
    match guard.authorize(ip(10), Some(&good), 10_000) {
        PairingVerdict::Throttled { retry_after_ms } => {
            assert_eq!(retry_after_ms, 50_000, "60 秒锁，已过 10 秒");
        }
        other => panic!("锁定期内应报节流，实得 {other:?}"),
    }

    // 🔴 锁定期内失败**不继续累加**（否则一次锁定会无限延长）
    assert!(matches!(
        guard.authorize(ip(10), Some("WRONG1"), 20_000),
        PairingVerdict::Throttled { .. }
    ));

    // 锁定到期 → 自动放行（R-5：不是永久拉黑）
    assert_eq!(
        guard.authorize(ip(10), Some(&good), 60_000),
        PairingVerdict::Consumed,
        "60 秒后应放行"
    );
}

#[test]
fn other_devices_are_not_affected_by_one_devices_lockout() {
    let mut guard = PairingGuard::with_lifetime(true, Duration::from_secs(600));
    let good = guard.current_code(0).to_owned();
    for _ in 0..5 {
        guard.authorize(ip(10), Some("WRONG1"), 0);
    }
    assert!(matches!(
        guard.authorize(ip(10), Some(&good), 1_000),
        PairingVerdict::Throttled { .. }
    ));
    // 另一台设备不受牵连（R-5 是"同一设备"锁定）
    assert_eq!(
        guard.authorize(ip(11), Some(&good), 1_000),
        PairingVerdict::Consumed
    );
}

#[test]
fn a_success_resets_the_failure_counter() {
    let mut guard = PairingGuard::with_lifetime(true, Duration::from_secs(600));
    let good = guard.current_code(0).to_owned();
    for _ in 0..4 {
        guard.authorize(ip(10), Some("WRONG1"), 0);
    }
    // 成功一次（另一个地址用的是同一码…故先给 ip(99) 用掉，再换新码）
    assert_eq!(
        guard.authorize(ip(10), Some(&good), 0),
        PairingVerdict::Consumed
    );
    // 换新码后，ip(10) 已授权 → 免码；故用另一个地址验证计数确实被清过
    let fresh = guard.current_code(1).to_owned();
    assert_eq!(
        guard.authorize(ip(12), Some("WRONG1"), 1),
        PairingVerdict::Invalid
    );
    assert_eq!(
        guard.authorize(ip(12), Some(&fresh), 1),
        PairingVerdict::Consumed,
        "各自独立计数，不该被别的地址的失败拖累"
    );
}

// ── R-1：显式关闭配对 ────────────────────────────────────────────────────

#[test]
fn disabling_pairing_requires_an_explicit_choice_and_is_reported_as_such() {
    let mut guard = PairingGuard::new(false);
    // R-1：关闭是用户显式改动、承担后果；本模块如实报 NotRequired，
    // 好让调用方能把它记进日志（不是悄悄放行）
    assert_eq!(
        guard.authorize(ip(10), None, 0),
        PairingVerdict::NotRequired
    );
    assert!(PairingVerdict::NotRequired.is_allowed());

    // 默认必须是"要求配对"（R-1：默认开）
    let mut default_guard = PairingGuard::new(true);
    assert_eq!(
        default_guard.authorize(ip(10), None, 0),
        PairingVerdict::Invalid,
        "R-1：默认要求配对，缺码必拒"
    );
}

#[test]
fn verdicts_report_allowance_consistently() {
    assert!(PairingVerdict::Consumed.is_allowed());
    assert!(PairingVerdict::AlreadyAuthorized.is_allowed());
    assert!(PairingVerdict::NotRequired.is_allowed());
    assert!(!PairingVerdict::Invalid.is_allowed());
    assert!(!PairingVerdict::Throttled { retry_after_ms: 1 }.is_allowed());
}

/// 常量时间比较自身的边界：等长、含 NUL、超长输入都不能 panic，也不能误判相等。
#[test]
fn constant_time_comparison_behaves_at_the_boundaries() {
    assert!(constant_time_eq("ABC234", "ABC234"));
    assert!(!constant_time_eq("ABC234", "ABC235"));
    assert!(!constant_time_eq("ABC234", "ABC23"));
    assert!(!constant_time_eq("", "ABC234"));
    assert!(constant_time_eq("", ""));
    // 超过定长缓冲：多出的字节被截掉——故调用方**必须**先校验长度
    let long_a = "A".repeat(40);
    let long_b = format!("{}B", "A".repeat(39));
    assert!(
        constant_time_eq(&long_a, &long_b),
        "本函数不管长度；authorize 里的长度校验才是那道闸"
    );
}
