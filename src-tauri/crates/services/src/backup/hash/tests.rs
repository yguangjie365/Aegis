//! `hash` 单测：SHA-256 正确性 + **单遍拷贝与读回一致**（备份的核心断言）。

use super::*;
use std::path::PathBuf;

/// 🔴 规格测试用真实临时文件，**禁 mock 掉 IO 后声称"已验证"**（04 分册 §九·3）。
fn temp_dir(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("aegis_backup_hash_{tag}"));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("建临时目录");
    dir
}

#[test]
fn sha256_matches_the_known_vector() {
    // 🔴 用 NIST 官方向量钉住算法接线：空串的 SHA-256 必须是这个值。
    //    写错算法（如用了 CRC/xxhash）时这条会立刻红。
    let got = hash_reader(std::io::Cursor::new(b"")).expect("哈希空串");
    assert_eq!(
        got,
        "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
    );
    let got = hash_reader(std::io::Cursor::new(b"abc")).expect("哈希 abc");
    assert_eq!(
        got,
        "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
    );
}

#[test]
fn copy_and_hash_matches_separate_hash() {
    // 🔴 单遍拷贝算出的哈希必须与"单独读一遍算"完全一致 ——
    //    不一致意味着边读边算的实现有偏差，快照的完整性校验会全线失真。
    let dir = temp_dir("parity");
    let src = dir.join("src.bin");
    let dst = dir.join("nested").join("dst.bin");
    let payload: Vec<u8> = (0..=255u8).cycle().take(300_000).collect();
    std::fs::write(&src, &payload).expect("写源");

    let copied = copy_and_hash(&src, &dst).expect("拷贝");
    let separate = sha256_file(&src).expect("单独哈希");
    assert_eq!(copied, separate, "单遍哈希与独立哈希不一致");
    assert_eq!(
        std::fs::read(&dst).expect("读目标"),
        payload,
        "内容须逐字节一致"
    );

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn copy_creates_missing_parent_directories() {
    // 快照内的相对路径必然带子目录（`来源名/子/文件`）⇒ 不建目录会整批失败
    let dir = temp_dir("mkdir");
    let src = dir.join("a.txt");
    let dst = dir.join("x").join("y").join("z").join("a.txt");
    std::fs::write(&src, b"hi").expect("写源");
    copy_and_hash(&src, &dst).expect("应自动建父目录");
    assert!(dst.exists(), "目标文件应存在");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn missing_source_returns_error_not_empty_hash() {
    // 🔴 读不到源文件必须报错，**绝不能**返回"空内容的哈希"——
    //    那会让一个失败的文件以"合法的空文件"身份进清单。
    let dir = temp_dir("missing");
    let err = sha256_file(&dir.join("nope.bin")).expect_err("应报错");
    assert_eq!(err.kind(), std::io::ErrorKind::NotFound);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn hex_is_lowercase() {
    // 🔴 清单里存小写；比对侧用 `eq_ignore_ascii_case`。
    //    两处口径必须一致，否则会出现"明明一样却不一致"的假告警。
    let hex = to_hex(&[0xde, 0xad, 0xBE, 0xEF]);
    assert_eq!(hex, "deadbeef");
    assert!(hex.chars().all(|c| !c.is_ascii_uppercase()));
}
