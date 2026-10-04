//! `sink` 的测试。
//!
//! 全部在 TEMP 下的真实目录里跑（落盘器的正确性**只能**用真实文件系统证明）。

use super::*;
use sha2::{Digest, Sha256};

/// 建一个干净的临时根。
fn temp_root(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("aegis-sink-{}-{}", std::process::id(), tag));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).expect("建临时根");
    dir
}

fn sha256_hex(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    hex::encode(hasher.finalize())
}

// ── 命名与令牌 ────────────────────────────────────────────────────────────

#[test]
fn part_name_isolates_peers_and_round_trips_the_token() {
    let name = part_file_name("报告.pdf", "a1b2c3").unwrap();
    assert_eq!(name, "报告.pdf.aegisft-a1b2c3.part");
    assert_eq!(peer_token_of(&name), Some("a1b2c3"));

    // 两个对端对同一文件名给出不同未完成文件 —— 互不干扰是本设计的全部意义
    let other = part_file_name("报告.pdf", "d4e5f6").unwrap();
    assert_ne!(name, other);

    // 令牌里的非法字符被滤掉；滤空则拒绝
    assert_eq!(
        part_file_name("a.txt", "a/b:c").unwrap(),
        "a.txt.aegisft-abc.part"
    );
    assert_eq!(part_file_name("a.txt", "///"), None);
    // 文件名清洗后不可用 → 拒绝（不替它编名字）
    assert_eq!(part_file_name("...", "tok"), None);
    assert_eq!(part_file_name("", "tok"), None);
}

/// 🔴 **数据丢失级缺陷的回归闸**（本轮实测抓出）：
/// 用户合法的 `report.pdf.part`（很多下载器就用这个后缀）**不得**被当成我们的未完成文件，
/// 否则孤儿清理会在 7 天后把它删掉。
#[test]
fn user_owned_part_files_are_not_mistaken_for_ours() {
    assert_eq!(
        peer_token_of("report.pdf.part"),
        None,
        "无 aegisft- 标记 → 不是我们的未完成文件（此前会返回 Some(\"pdf\") 并导致误删）"
    );
    assert_eq!(peer_token_of("backup.deadbeef.part"), None);
    assert_eq!(
        peer_token_of("x.aegisft.part"),
        None,
        "标记后令牌为空 → 不认"
    );
    // 我们自己的才认
    assert_eq!(peer_token_of("report.pdf.aegisft-ab12.part"), Some("ab12"));

    // 非未完成文件一律不认
    assert_eq!(peer_token_of("report.pdf"), None);
    assert_eq!(peer_token_of("report.pdf.part.old"), None);
    assert_eq!(peer_token_of("x.part"), None);
}

// ── mtime 窗口（F-FT-24） ─────────────────────────────────────────────────

#[test]
fn mtime_window_is_1980_to_tomorrow() {
    let now = 1_760_000_000_000; // 2025 年末
    assert!(
        is_acceptable_mtime(MTIME_LOWER_BOUND_MS, now),
        "1980-01-01 本身在窗口内"
    );
    assert!(
        !is_acceptable_mtime(MTIME_LOWER_BOUND_MS - 1, now),
        "早于 1980 一律不还原"
    );
    assert!(is_acceptable_mtime(now, now));
    assert!(
        is_acceptable_mtime(now + 24 * 60 * 60 * 1000, now),
        "明日 +1 天仍在窗口内"
    );
    assert!(
        !is_acceptable_mtime(now + 25 * 60 * 60 * 1000, now),
        "超过今日+1天不还原（防对端伪造 9999 年）"
    );
    assert!(
        !is_acceptable_mtime(253_402_300_799_000, now),
        "9999 年被挡下"
    );
}

// ── 正常落盘 ──────────────────────────────────────────────────────────────

#[test]
fn happy_path_commits_atomically_and_leaves_no_part() {
    let root = temp_root("happy");
    let payload = b"hello world".to_vec();
    let mut sink = ReceiveSink::open(&root, "a.txt", "tok", payload.len() as u64).unwrap();
    assert_eq!(sink.received(), 0);
    sink.append(0, &payload[..5]).unwrap();
    assert_eq!(sink.received(), 5);
    sink.append(5, &payload[5..]).unwrap();

    let outcome = sink
        .finish(
            &sha256_hex(&payload),
            ConflictPolicy::Rename,
            None,
            now_unix_ms(),
        )
        .unwrap();
    match outcome {
        FinishOutcome::Committed { path, renamed } => {
            assert!(!renamed, "无撞名不该改名");
            assert_eq!(fs::read(&path).unwrap(), payload);
            assert_eq!(path.file_name().unwrap(), "a.txt");
        }
        other => panic!("应入位，实得 {other:?}"),
    }
    // 🔴 "要么完整、要么不存在"：目录里只剩成品
    let names: Vec<String> = fs::read_dir(&root)
        .unwrap()
        .flatten()
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .collect();
    assert_eq!(names, vec!["a.txt".to_owned()], "不得残留 .part");
    let _ = fs::remove_dir_all(&root);
}

// ── 摘要不符（F-FT-20 的"必须删掉"） ──────────────────────────────────────

#[test]
fn hash_mismatch_deletes_the_part_and_writes_nothing() {
    let root = temp_root("hashmismatch");
    let payload = b"corrupted".to_vec();
    let mut sink = ReceiveSink::open(&root, "b.txt", "tok", payload.len() as u64).unwrap();
    sink.append(0, &payload).unwrap();

    let wrong = sha256_hex(b"something else");
    let outcome = sink
        .finish(&wrong, ConflictPolicy::Rename, None, now_unix_ms())
        .unwrap();
    assert_eq!(outcome, FinishOutcome::HashMismatch);
    // 🔴 不删的话下次续传会从这个坏文件上继续 → "永远续传一个坏文件"的死循环
    assert!(!root.join("b.txt").exists(), "不得留下成品");
    let leftovers: Vec<_> = fs::read_dir(&root).unwrap().flatten().collect();
    assert!(leftovers.is_empty(), "损坏的未完成文件必须被删除");
    let _ = fs::remove_dir_all(&root);
}

// ── 续传：重灌前缀（模块头 ①，最关键的一条） ──────────────────────────────

#[test]
fn resume_refeeds_the_existing_prefix_into_the_hash() {
    let root = temp_root("resume");
    let payload = b"0123456789ABCDEF".to_vec();
    let full_hash = sha256_hex(&payload);
    let half = payload.len() / 2;

    // 第一段：写到一半就"断线"（drop 掉句柄，模拟进程/连接中断）
    {
        let mut sink = ReceiveSink::open(&root, "c.bin", "tok", payload.len() as u64).unwrap();
        sink.append(0, &payload[..half]).unwrap();
        assert_eq!(sink.received(), half as u64);
        // 不 finish、不 discard —— 直接析构，.part 留在盘上
    }
    let part = root.join("c.bin.aegisft-tok.part");
    assert!(part.exists(), "未完成文件应留在盘上供续传");

    // 第二段：重新打开应"看见"已收长度，并把前缀喂进 hasher
    let mut sink = ReceiveSink::open(&root, "c.bin", "tok", payload.len() as u64).unwrap();
    assert_eq!(sink.received(), half as u64, "必须从已收长度接续");
    sink.append(half as u64, &payload[half..]).unwrap();

    // 🔴 用**全文件**摘要收尾：若前缀没被重灌，这里必然 HASH_MISMATCH
    let outcome = sink
        .finish(&full_hash, ConflictPolicy::Rename, None, now_unix_ms())
        .unwrap();
    match outcome {
        FinishOutcome::Committed { path, .. } => {
            assert_eq!(
                fs::read(&path).unwrap(),
                payload,
                "续传后内容必须完整且正确"
            );
        }
        other => panic!("续传应成功，实得 {other:?}（前缀未被重灌的典型症状）"),
    }
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn untrustworthy_part_is_truncated_and_restarted() {
    let root = temp_root("untrusted");
    // 伪造一个"已收满"的 .part（按 F-FT-21 这不该存在 → 不可信）
    let part = root.join("d.txt.aegisft-tok.part");
    fs::write(&part, b"garbage-that-is-longer-than-declared").unwrap();

    let payload = b"real".to_vec();
    let mut sink = ReceiveSink::open(&root, "d.txt", "tok", payload.len() as u64).unwrap();
    assert_eq!(sink.received(), 0, "不可信的残留必须归零重来，而不是接着写");
    sink.append(0, &payload).unwrap();
    let outcome = sink
        .finish(
            &sha256_hex(&payload),
            ConflictPolicy::Rename,
            None,
            now_unix_ms(),
        )
        .unwrap();
    assert!(matches!(outcome, FinishOutcome::Committed { .. }));
    assert_eq!(fs::read(root.join("d.txt")).unwrap(), payload);
    let _ = fs::remove_dir_all(&root);
}

// ── 冲突策略 ──────────────────────────────────────────────────────────────

#[test]
fn rename_policy_keeps_the_existing_file_and_picks_a_free_candidate() {
    let root = temp_root("rename");
    fs::write(root.join("e.txt"), b"original").unwrap();
    let payload = b"incoming".to_vec();

    let mut sink = ReceiveSink::open(&root, "e.txt", "tok", payload.len() as u64).unwrap();
    sink.append(0, &payload).unwrap();
    let outcome = sink
        .finish(
            &sha256_hex(&payload),
            ConflictPolicy::Rename,
            None,
            now_unix_ms(),
        )
        .unwrap();
    match outcome {
        FinishOutcome::Committed { path, renamed } => {
            assert!(renamed, "撞名必须改名");
            assert_eq!(path.file_name().unwrap(), "e (1).txt");
            assert_eq!(fs::read(&path).unwrap(), payload);
        }
        other => panic!("应改名入位，实得 {other:?}"),
    }
    // 🔴 原文件一字未动
    assert_eq!(fs::read(root.join("e.txt")).unwrap(), b"original");
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn skip_policy_writes_nothing_and_reports_skipped_not_committed() {
    let root = temp_root("skip");
    fs::write(root.join("f.txt"), b"original").unwrap();
    let payload = b"incoming".to_vec();

    let mut sink = ReceiveSink::open(&root, "f.txt", "tok", payload.len() as u64).unwrap();
    sink.append(0, &payload).unwrap();
    let outcome = sink
        .finish(
            &sha256_hex(&payload),
            ConflictPolicy::Skip,
            None,
            now_unix_ms(),
        )
        .unwrap();
    // F-FT-23：终态是「已跳过」而非「已完成」；目录里不留新文件
    assert_eq!(outcome, FinishOutcome::Skipped);
    assert_eq!(fs::read(root.join("f.txt")).unwrap(), b"original");
    let names: Vec<String> = fs::read_dir(&root)
        .unwrap()
        .flatten()
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .collect();
    assert_eq!(names, vec!["f.txt".to_owned()], "跳过时不得留 .part");
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn overwrite_policy_replaces_the_existing_file() {
    let root = temp_root("overwrite");
    fs::write(root.join("g.txt"), b"original").unwrap();
    let payload = b"replacement".to_vec();

    let mut sink = ReceiveSink::open(&root, "g.txt", "tok", payload.len() as u64).unwrap();
    sink.append(0, &payload).unwrap();
    let outcome = sink
        .finish(
            &sha256_hex(&payload),
            ConflictPolicy::Overwrite,
            None,
            now_unix_ms(),
        )
        .unwrap();
    assert!(matches!(
        outcome,
        FinishOutcome::Committed { renamed: false, .. }
    ));
    assert_eq!(fs::read(root.join("g.txt")).unwrap(), payload);
    let _ = fs::remove_dir_all(&root);
}

/// 🔴 **无确认闸道时 Ask 必须降级为改名**（F-FT-23 / R-10）——绝不能落到覆盖。
#[test]
fn ask_policy_at_finish_degrades_to_rename_never_overwrites() {
    let root = temp_root("ask");
    fs::write(root.join("h.txt"), b"original").unwrap();
    let payload = b"incoming".to_vec();

    let mut sink = ReceiveSink::open(&root, "h.txt", "tok", payload.len() as u64).unwrap();
    sink.append(0, &payload).unwrap();
    let outcome = sink
        .finish(
            &sha256_hex(&payload),
            ConflictPolicy::Ask,
            None,
            now_unix_ms(),
        )
        .unwrap();
    match outcome {
        FinishOutcome::Committed { path, renamed } => {
            assert!(renamed, "Ask 在收尾期已无闸道 → 降级改名");
            assert_eq!(path.file_name().unwrap(), "h (1).txt");
        }
        other => panic!("应降级改名，实得 {other:?}"),
    }
    assert_eq!(
        fs::read(root.join("h.txt")).unwrap(),
        b"original",
        "🔴 绝不覆盖"
    );
    let _ = fs::remove_dir_all(&root);
}

// ── 写入纪律 ──────────────────────────────────────────────────────────────

#[test]
fn append_rejects_wrong_offset_and_oversize() {
    let root = temp_root("strict");
    let mut sink = ReceiveSink::open(&root, "i.bin", "tok", 10).unwrap();

    // 起点不符 → 让对端重新查询真实进度（手机通道 409 的电脑通道对应物）
    match sink.append(3, b"xx") {
        Err(SinkError::OffsetMismatch { expected, got }) => {
            assert_eq!((expected, got), (0, 3));
        }
        other => panic!("应报起点不符，实得 {other:?}"),
    }
    // 越界写入 → 拒
    match sink.append(0, &[0_u8; 11]) {
        Err(SinkError::Oversize {
            declared,
            attempted,
        }) => {
            assert_eq!((declared, attempted), (10, 11));
        }
        other => panic!("应报越界，实得 {other:?}"),
    }
    // 合法写入后仍可继续
    sink.append(0, &[1_u8; 10]).unwrap();
    assert_eq!(sink.received(), 10);
    let _ = sink.discard();
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn finish_rejects_an_incomplete_transfer() {
    let root = temp_root("incomplete");
    let mut sink = ReceiveSink::open(&root, "j.bin", "tok", 100).unwrap();
    sink.append(0, &[0_u8; 10]).unwrap();
    match sink.finish("whatever", ConflictPolicy::Rename, None, now_unix_ms()) {
        Err(SinkError::Incomplete { declared, received }) => {
            assert_eq!((declared, received), (100, 10));
        }
        other => panic!("应收尾失败，实得 {other:?}"),
    }
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn discard_removes_the_part_and_is_idempotent() {
    let root = temp_root("discard");
    let mut sink = ReceiveSink::open(&root, "k.bin", "tok", 100).unwrap();
    sink.append(0, b"x").unwrap();
    let part = sink.part_path().to_path_buf();
    sink.discard().unwrap();
    assert!(!part.exists());
    let _ = fs::remove_dir_all(&root);
}

// ── mtime 还原（F-FT-24） ─────────────────────────────────────────────────

#[test]
fn restores_mtime_inside_the_window_and_stays_silent_outside() {
    use std::time::UNIX_EPOCH;
    let root = temp_root("mtime");
    let payload = b"stamp".to_vec();
    let target_ms: i64 = 1_609_459_200_000; // 2021-01-01
    let now = now_unix_ms();

    // 窗口内 → 还原
    let mut sink = ReceiveSink::open(&root, "m1.txt", "tok", payload.len() as u64).unwrap();
    sink.append(0, &payload).unwrap();
    let outcome = sink
        .finish(
            &sha256_hex(&payload),
            ConflictPolicy::Rename,
            Some(target_ms),
            now,
        )
        .unwrap();
    let FinishOutcome::Committed { path, .. } = outcome else {
        panic!("应入位");
    };
    let got_ms = i64::try_from(
        fs::metadata(&path)
            .unwrap()
            .modified()
            .unwrap()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_millis(),
    )
    .unwrap_or(i64::MAX);
    assert!(
        (got_ms - target_ms).abs() < 2000,
        "期望≈{target_ms}，实得 {got_ms}"
    );

    // 窗口外 → 静默保持当前时间（不报错、也不谎称已还原）
    let mut sink = ReceiveSink::open(&root, "m2.txt", "tok", payload.len() as u64).unwrap();
    sink.append(0, &payload).unwrap();
    let outcome = sink
        .finish(
            &sha256_hex(&payload),
            ConflictPolicy::Rename,
            Some(253_402_300_799_000), // 9999 年
            now,
        )
        .unwrap();
    let FinishOutcome::Committed { path, .. } = outcome else {
        panic!("窗口外仍应成功入位");
    };
    let got_ms = i64::try_from(
        fs::metadata(&path)
            .unwrap()
            .modified()
            .unwrap()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_millis(),
    )
    .unwrap_or(i64::MAX);
    assert!(
        got_ms >= now - 60_000,
        "窗口外应保持当前时间，实得 {got_ms}"
    );
    let _ = fs::remove_dir_all(&root);
}

// ── 孤儿清理（F-FT-19 / R-21） ────────────────────────────────────────────

#[test]
fn orphan_cleanup_spares_self_fresh_and_non_part_files() {
    let root = temp_root("orphans");
    let now = now_unix_ms();
    let old = now - 8 * 24 * 60 * 60 * 1000; // 8 天前

    // 其它对端 + 过期 → 应被清
    let stale_other = root.join("old.txt.aegisft-othertok.part");
    fs::write(&stale_other, b"x").unwrap();
    infra::fs::set_modified_time(&stale_other, old).unwrap();

    // 其它对端 + 新鲜 → 保留（可能正在传）
    let fresh_other = root.join("new.txt.aegisft-othertok.part");
    fs::write(&fresh_other, b"x").unwrap();

    // 本对端 + 过期 → 保留（本对端正在续传的就是它）
    let stale_self = root.join("self.txt.aegisft-mytok.part");
    fs::write(&stale_self, b"x").unwrap();
    infra::fs::set_modified_time(&stale_self, old).unwrap();

    // 成品文件（哪怕过期）→ 绝不触碰
    let product = root.join("finished.txt");
    fs::write(&product, b"y").unwrap();
    infra::fs::set_modified_time(&product, old).unwrap();

    // 🔴 用户自己的 `.part` 文件（很多下载器用这个后缀）→ 必须存活（数据丢失级回归闸）
    let user_owned = root.join("download.pdf.part");
    fs::write(&user_owned, b"user's own").unwrap();
    infra::fs::set_modified_time(&user_owned, old).unwrap();

    let removed = cleanup_orphans(&root, "mytok", now, ORPHAN_RETENTION).unwrap();
    assert_eq!(removed, 1, "只应清掉「其它对端 + 过期」那一个");
    assert!(!stale_other.exists());
    assert!(fresh_other.exists(), "新鲜的不清");
    assert!(stale_self.exists(), "本对端的不清");
    assert!(product.exists(), "🔴 成品文件绝不触碰");
    assert!(
        user_owned.exists(),
        "🔴 用户自己的 .part 文件绝不触碰（无 aegisft- 标记即不是我们的）"
    );
    assert_eq!(
        fs::read(&user_owned).unwrap(),
        b"user's own",
        "内容不得被动过"
    );
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn orphan_cleanup_tolerates_a_missing_root() {
    let missing = std::env::temp_dir().join("aegis-sink-missing-root-xyz");
    let _ = fs::remove_dir_all(&missing);
    assert!(cleanup_orphans(&missing, "tok", now_unix_ms(), ORPHAN_RETENTION).is_err());
}

// ── 路径守卫接入（本域的落盘必须经 infra::safe） ──────────────────────────

#[test]
fn traversal_shaped_file_names_never_escape_the_root() {
    let root = temp_root("guard");
    // 文件名里的路径分隔符会被 sanitize 成下划线，故最终仍落在根内
    let mut sink = ReceiveSink::open(&root, "..\\..\\evil.txt", "tok", 3).unwrap();
    let part = sink.part_path().to_path_buf();
    assert!(
        infra::safe::is_lexically_under(
            &root.canonicalize().unwrap(),
            &part.canonicalize().unwrap_or(part.clone())
        ),
        "未完成文件必须仍在根内：{}",
        part.display()
    );
    sink.append(0, b"abc").unwrap();
    let outcome = sink
        .finish(
            &sha256_hex(b"abc"),
            ConflictPolicy::Rename,
            None,
            now_unix_ms(),
        )
        .unwrap();
    match outcome {
        FinishOutcome::Committed { path, .. } => {
            assert!(infra::safe::is_lexically_under(
                &root.canonicalize().unwrap(),
                &path.canonicalize().unwrap()
            ));
        }
        other => panic!("应入位，实得 {other:?}"),
    }
    let _ = fs::remove_dir_all(&root);
}
