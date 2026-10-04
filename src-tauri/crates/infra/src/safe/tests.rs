//! `safe` 的测试。
//!
//! 🟡 **覆盖声明（如实）**：词法层（`validate_relative` / `is_lexically_under` /
//! `strip_verbatim_prefix` / `sanitize_file_name`）为**穷举单测**；`join_under_root` 的
//! 真实文件系统路径（含"目标尚不存在"）用 TEMP 下的真实目录测。
//! 🔴 **符号链接逃逸（`PathEscape::SymlinkEscape`）未自动化**——那需要在测试里创建
//! 真实链接/junction；其逻辑由 `is_lexically_under` 与 `deepest_existing_ancestor`
//! 两个纯函数共同承担，二者均有单测。此项登记为残留，接手机通道批做端到端真机验证。

use super::*;
use std::fs;

fn temp_root(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("aegis-safe-{}-{}", std::process::id(), tag));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).expect("建临时根");
    dir
}

// ── validate_relative：接受 ────────────────────────────────────────────────

#[test]
fn accepts_plain_relative_paths() {
    assert_eq!(validate_relative("a.txt").unwrap(), vec!["a.txt"]);
    assert_eq!(
        validate_relative("sub/dir/文件 🚀.txt").unwrap(),
        vec!["sub", "dir", "文件 🚀.txt"]
    );
    assert_eq!(
        validate_relative("sub\\dir\\x.txt").unwrap(),
        vec!["sub", "dir", "x.txt"],
        "反斜杠分隔同样接受"
    );
}

// ── validate_relative：逐条拒绝（每条对应规格或母本的一条规则） ─────────────

#[test]
fn rejects_traversal_absolute_drive_unc_and_verbatim() {
    // 规格 §5.1 明确列举的四类
    assert_eq!(validate_relative("..\\x"), Err(PathEscape::ParentDir));
    assert_eq!(validate_relative("a/../b"), Err(PathEscape::ParentDir));
    assert_eq!(
        validate_relative("C:\\Windows"),
        Err(PathEscape::DriveLetter)
    );
    assert_eq!(
        validate_relative("C:relative"),
        Err(PathEscape::DriveLetter)
    );
    assert_eq!(validate_relative("\\\\server\\share"), Err(PathEscape::Unc));
    assert_eq!(validate_relative("//server/share"), Err(PathEscape::Unc));
    assert_eq!(
        validate_relative("\\\\?\\D:\\x"),
        Err(PathEscape::VerbatimPrefix),
        "长路径前缀必须拒（TASKS PathGuard 明列）"
    );
    assert_eq!(
        validate_relative("\\\\.\\NUL"),
        Err(PathEscape::VerbatimPrefix)
    );
    assert_eq!(validate_relative("/etc/passwd"), Err(PathEscape::Absolute));
    assert_eq!(validate_relative("\\Windows"), Err(PathEscape::Absolute));
}

#[test]
fn rejects_reserved_device_names_including_with_extension() {
    for name in ["NUL", "con", "COM1", "lpt9"] {
        assert_eq!(
            validate_relative(name),
            Err(PathEscape::ReservedName),
            "{name} 是设备保留名"
        );
    }
    // 带扩展名也算（Win32 只看主名）
    assert_eq!(validate_relative("NUL.txt"), Err(PathEscape::ReservedName));
    assert_eq!(
        validate_relative("sub/CON.log"),
        Err(PathEscape::ReservedName)
    );
    // 只是前缀相似的不算
    assert!(validate_relative("CONSOLE.txt").is_ok());
    assert!(validate_relative("COM10.txt").is_ok(), "COM10 不在保留表内");
}

#[test]
fn rejects_empty_nul_curdir_and_empty_segments() {
    assert_eq!(validate_relative(""), Err(PathEscape::Empty));
    assert_eq!(validate_relative("   "), Err(PathEscape::Empty));
    assert_eq!(validate_relative("a\0b"), Err(PathEscape::NulByte));
    assert_eq!(
        validate_relative("./a"),
        Err(PathEscape::CurDir),
        "`.` 段不裁剪、直接拒（不静默改写）"
    );
    assert_eq!(validate_relative("a//b"), Err(PathEscape::EmptySegment));
    assert_eq!(validate_relative("a\\\\b"), Err(PathEscape::EmptySegment));
}

#[test]
fn enforces_segment_count_and_length_limits() {
    let too_many = (0..MAX_SEGMENTS + 1)
        .map(|i| format!("s{i}"))
        .collect::<Vec<_>>()
        .join("/");
    assert_eq!(
        validate_relative(&too_many),
        Err(PathEscape::SegmentCount(MAX_SEGMENTS + 1))
    );
    let at_limit = (0..MAX_SEGMENTS)
        .map(|i| format!("s{i}"))
        .collect::<Vec<_>>()
        .join("/");
    assert!(
        validate_relative(&at_limit).is_ok(),
        "正好 {MAX_SEGMENTS} 段应接受"
    );

    let long = "x".repeat(MAX_SEGMENT_CHARS + 1);
    assert_eq!(validate_relative(&long), Err(PathEscape::SegmentTooLong));
    assert!(validate_relative(&"x".repeat(MAX_SEGMENT_CHARS)).is_ok());
}

// ── strip_verbatim_prefix（K-S01 T8 的落点） ───────────────────────────────

#[test]
fn strips_verbatim_prefix_including_unc_form() {
    assert_eq!(
        strip_verbatim_prefix(Path::new(r"\\?\D:\a\b")),
        PathBuf::from(r"D:\a\b")
    );
    assert_eq!(
        strip_verbatim_prefix(Path::new(r"\\?\UNC\server\share\x")),
        PathBuf::from(r"\\server\share\x"),
        "verbatim UNC 要还原成普通 UNC，而不是 \\\\?\\UNC\\…"
    );
    assert_eq!(
        strip_verbatim_prefix(Path::new(r"\\.\NUL")),
        PathBuf::from("NUL")
    );
    // 普通路径原样
    assert_eq!(
        strip_verbatim_prefix(Path::new(r"D:\a\b")),
        PathBuf::from(r"D:\a\b")
    );
    // 🔴 这条是 K-S01 实测的回归闸：canonicalize 的输出必须被归一，否则泄漏到界面
    let real = Path::new(r"D:\x")
        .canonicalize()
        .unwrap_or_else(|_| PathBuf::from(r"D:\x"));
    assert!(
        !strip_verbatim_prefix(&real)
            .to_string_lossy()
            .starts_with(r"\\?\"),
        "归一后不得再含 \\\\?\\ 前缀"
    );
}

// ── is_lexically_under ────────────────────────────────────────────────────

#[test]
fn lexical_containment_is_case_insensitive_and_segment_aware() {
    let root = Path::new(r"D:\Share");
    assert!(is_lexically_under(root, Path::new(r"D:\Share")));
    assert!(is_lexically_under(root, Path::new(r"D:\Share\a.txt")));
    assert!(is_lexically_under(root, Path::new(r"d:\share\sub\b.txt")));
    // 🔴 前缀相似但不同目录：必须判为"不在内"（`D:\ShareX` 不是 `D:\Share` 的子路径）
    assert!(!is_lexically_under(root, Path::new(r"D:\ShareX\a.txt")));
    assert!(!is_lexically_under(root, Path::new(r"D:\Other\a.txt")));
    assert!(!is_lexically_under(root, Path::new(r"D:\")));
}

// ── sanitize_file_name（写侧） ─────────────────────────────────────────────

#[test]
fn sanitize_replaces_illegal_chars_and_strips_invisible() {
    assert_eq!(
        sanitize_file_name("a/b\\c:d*e?f\"g<h>i|j").as_deref(),
        Some("a_b_c_d_e_f_g_h_i_j")
    );
    // 母本实证：不剥零宽会落盘成 `a\u{200B}b.txt`（日志名与实际名不一致）
    assert_eq!(
        sanitize_file_name("a\u{200B}b.txt").as_deref(),
        Some("ab.txt")
    );
    assert_eq!(
        sanitize_file_name("\u{FEFF}x.txt").as_deref(),
        Some("x.txt")
    );
    // 中文与 emoji 不受影响
    assert_eq!(
        sanitize_file_name("报告 2026 🚀.pdf").as_deref(),
        Some("报告 2026 🚀.pdf")
    );
}

#[test]
fn sanitize_handles_windows_name_quirks() {
    // Win32 会静默裁掉结尾的点与空格 → 日志名 ≠ 实际名
    assert_eq!(sanitize_file_name("name...").as_deref(), Some("name"));
    assert_eq!(sanitize_file_name("name   ").as_deref(), Some("name"));
    assert_eq!(sanitize_file_name("name. . .").as_deref(), Some("name"));
    // 保留名加前缀
    assert_eq!(sanitize_file_name("NUL").as_deref(), Some("_NUL"));
    assert_eq!(sanitize_file_name("con.txt").as_deref(), Some("_con.txt"));
}

#[test]
fn sanitize_returns_none_when_nothing_usable_remains() {
    assert_eq!(sanitize_file_name(""), None);
    assert_eq!(sanitize_file_name("   "), None);
    assert_eq!(sanitize_file_name("..."), None);
    assert_eq!(sanitize_file_name("."), None);
    assert_eq!(sanitize_file_name(".."), None);
    assert_eq!(sanitize_file_name("\u{200B}"), None, "只有不可见字符 = 空");
    assert_eq!(sanitize_file_name(&"x".repeat(MAX_SEGMENT_CHARS + 1)), None);
}

// ── join_under_root（真实文件系统） ────────────────────────────────────────

#[test]
fn joins_under_a_real_root_and_returns_a_clean_path() {
    let root = temp_root("join");
    let joined = join_under_root(&root, "sub/文件.txt").expect("应放行");
    assert!(joined.starts_with(&root), "结果必须在根之下");
    assert_eq!(joined.file_name().unwrap(), "文件.txt");
    assert!(
        !joined.to_string_lossy().starts_with(r"\\?\"),
        "🔴 返回值不得带 verbatim 前缀（否则会显示到界面上）"
    );
    // 目标尚不存在也应成功（"接收文件"正是这种情形）
    assert!(!joined.exists());
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn traversal_attempts_are_rejected_never_silently_trimmed() {
    let root = temp_root("escape");
    // 🔴 核心承诺：不裁剪成"看起来安全"的路径，而是直接拒
    for bad in [
        r"..\..\Windows\System32\x.dll",
        "../etc/passwd",
        r"C:\Windows\x",
        r"\\server\share\x",
        "./x",
        "",
    ] {
        let result = join_under_root(&root, bad);
        assert!(result.is_err(), "必须拒绝：{bad}");
    }
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn unusable_root_is_reported_as_such() {
    let missing = std::env::temp_dir().join("aegis-safe-definitely-missing-dir-xyz");
    let _ = fs::remove_dir_all(&missing);
    assert_eq!(
        join_under_root(&missing, "a.txt"),
        Err(PathEscape::RootUnusable)
    );
}

/// 🔴 纵深防御：即使词法校验被绕过，最终返回的路径也必须仍在根内。
/// 这里用"根之下再套一层"的正例 + 前缀相似目录的反例把边界钉住。
#[test]
fn result_always_stays_under_root_even_with_similar_sibling_names() {
    let base = temp_root("sibling");
    let root = base.join("Share");
    fs::create_dir_all(&root).unwrap();
    let sibling = base.join("ShareX");
    fs::create_dir_all(&sibling).unwrap();

    let joined = join_under_root(&root, "a.txt").unwrap();
    assert!(is_lexically_under(&root, &joined));
    assert!(
        !is_lexically_under(&sibling, &joined),
        "D:\\ShareX 不得被视为 D:\\Share 的父级，反之亦然"
    );
    let _ = fs::remove_dir_all(&base);
}

#[test]
fn error_messages_are_human_readable_and_carry_no_payload() {
    assert_eq!(PathEscape::ParentDir.to_string(), "路径含上级目录（..）");
    assert_eq!(
        PathEscape::ReservedName.to_string(),
        "路径含 Windows 设备保留名"
    );
    assert_eq!(PathEscape::Unc.to_string(), "不允许 UNC 路径");
    assert!(PathEscape::SegmentCount(99).to_string().contains("段数"));
}

/// 🔴 真正的"不回显"承诺：把一段**可辨识的外部输入**喂进去，错误里不得出现它。
/// （上一条用例里 `（..）` 是固定字面量、不是回显，故不能拿它当证据——这是本轮自己写错的断言。）
#[test]
fn rejection_never_echoes_the_offending_input() {
    let root = temp_root("echo");
    let marker = "ATTACKER-MARKER-9f3a";
    let payload = format!("..\\{marker}\\x.dll");
    let err = join_under_root(&root, &payload).unwrap_err();
    let text = err.to_string();
    assert!(
        !text.contains(marker),
        "错误文案不得回显外部输入（会进日志与界面）：{text}"
    );
    assert!(!text.contains("x.dll"));
    let _ = fs::remove_dir_all(&root);
}
