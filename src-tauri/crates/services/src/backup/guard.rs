//! 架构守卫：**`backup` 域禁止直接 `Path::join` 消费外部输入**。
//!
//! 口径与 `transfer::guard` 同源（`02-专题设计/02-安全与特权通道.md` §5.1
//! 「各域禁止 `Path::join` 直接消费外部输入」），但**例外清单按本域实际用法登记**。
//!
//! 🔴 **为什么 backup 域也必须有这道守卫**：本域处理的 `relative_path` 全部来自
//! **快照清单**，而清单是**用户可编辑的文件**（R-1：「清单不可信」）。
//! 一旦某处图省事写 `.join(manifest_entry.relative_path)`，就把用户的
//! `..\..\Windows\System32\x.dll` 直接送进写路径 —— 这是最典型的提权写入。
//!
//! 🔴 **例外登记说明**（不得静默放宽）：
//! - `restore.rs` 的 [`restore::plan`] / `execute` 里有「逐段 `push` 已校验段」的拼接。
//!   **那不是例外**：`infra::safe::validate_relative` 已把每段校验成
//!   **不含 `..`、不含分隔符、不含盘符**的安全字面量，`push` 它们在语义上
//!   等价于 `join_under_root` 的词法拼接段，区别只是本域还要再做三重守卫（R-2）。
//!   🔴 但这是**必须复核的差异点**：若哪天改成 push 未校验的段，本行即刻失效。
//! - `store.rs` / `manifest.rs` 里的 `.join(常量)` 与 `.join(已 sanitize 的 id)`
//!   由判据的「安全字面量」规则自动放行，无需登记。

use std::path::{Path, PathBuf};

/// 允许在域内直接 `Path::join` / `PathBuf::push` 消费外部输入的**文件**与理由。
///
/// 🔴 守卫拦的是「外部路径拼到某个**允许根**之下」这件事（提权写入风险）。
///    下列三类用法**结构上不可能越界**，逐条登记理由（02 册 §八「例外须登记」）：
///
/// | 文件 | 位置 | 理由 |
/// |---|---|---|
/// | `admission.rs` | `normalize_for_compare` | 是「相对路径 → 绝对路径」的 **CWD 补全**，无允许根参与；结果随即被 `strip_verbatim_prefix` + `trim_end_matches` 归一，不产生可越界落点。 |
/// | `restore.rs` | `next_free_name` / `displaced_name` | 拼的是**本工具自己生成的文件名**：来源是 `file_stem()`（Windows 保证不含分隔符）与固定后缀 `" (n)"` / `".staging-old-"`，无用户输入。 |
/// | `store.rs` | `save_json` | 拼的是**本模块的私有文件名常量**（`RULES_FILE` / `SETTINGS_FILE`），调用方传不进来。 |
const ALLOWED: &[(&str, &str)] = &[
    ("admission.rs", "CWD 补全，非「拼到允许根之下」"),
    (
        "restore.rs",
        "拼本工具生成的文件名（file_stem + 固定后缀），无用户输入",
    ),
    ("store.rs", "拼本模块私有文件名常量，调用方传不进来"),
];

/// 守卫自身不参与扫描（它必然含被禁模式的字面量，否则无法自证抓得到）。
const SCANNER_FILE: &str = "guard.rs";

/// 测试模块不参与扫描（构造字面量，非消费外部输入；且只以 `#[cfg(test)]` 编入）。
const TEST_MODULE_FILE: &str = "tests.rs";

/// 禁止在本域出现的同义函数名（防"再写一份路径守卫"）。
const FORBIDDEN_FN_PATTERNS: [&str; 4] = [
    "fn join_under_root",
    "fn is_safe_under_root",
    "fn sanitize_relative",
    "fn validate_relative",
];

fn file_name_of(path: &Path) -> String {
    path.file_name()
        .map_or_else(String::new, |n| n.to_string_lossy().into_owned())
}

fn collect_rs(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect_rs(&path, out);
        } else if path.extension().is_some_and(|e| e == "rs") {
            out.push(path);
        }
    }
}

/// 该行是否存在「参数不是安全字面量」的 `.join(`。
fn has_unsafe_join(line: &str) -> bool {
    let mut rest = line;
    while let Some(index) = rest.find(".join(") {
        let after = &rest[index + ".join(".len()..];
        // 🔴 零参数 `.join()` 是 `JoinHandle::join()`，不是路径拼接
        if after.trim_start().starts_with(')') {
            rest = after;
            continue;
        }
        if is_safe_argument(after, line) {
            rest = after;
            continue;
        }
        return true;
    }
    false
}

/// 🔴 `PathBuf::push` 的同类检查。
///
/// 🔴🔴 **判据必须区分 `Vec::push` 与 `PathBuf::push`**：第一版对两者一视同仁，
/// 于是本域 84 处 `Vec::push`（失败项收集、`skipped` 记录…）全被报成路径违规。
/// 守卫一旦噪声化，人就会给它加豁免 —— 那等于失去意义。
///
/// 判据：只有当**同一行里出现路径上下文**（`Path` / `PathBuf` / `dest` / `target` /
/// `dir` 等路径语义标识）时才把 `push(` 当路径拼接看。
fn has_unsafe_push(line: &str) -> bool {
    // 零参数 `.push()` 不存在（Vec::push 与 PathBuf::push 都必须带参数）
    if !has_path_context(line) {
        return false;
    }
    let marker = ".push(";
    let mut rest = line;
    while let Some(index) = rest.find(marker) {
        let after = &rest[index + marker.len()..];
        if is_safe_argument(after, line) {
            rest = after;
            continue;
        }
        return true;
    }
    false
}

/// 该行是否处于路径上下文（用于区分 `PathBuf::push` 与 `Vec::push`）。
///
/// 🔴 判据只认**强路径信号**：显式的路径类型 / 变量名。
///    第一版把 `candidate` / `snapshot` 也算进来 ⇒ `Vec::push`（`kept.push(candidate.clone())`）
///    被误报，守卫噪声化。教训同 `transfer::guard`：**口径宁窄勿宽**，
///    漏报一个真违规可以靠 code review 补，误报一片只会让人加豁免。
fn has_path_context(line: &str) -> bool {
    const PATH_WORDS: [&str; 6] = [
        "PathBuf",
        "dest",
        "target",
        "dir_path",
        "root_path",
        "base_path",
    ];
    PATH_WORDS.iter().any(|w| line.contains(w))
}

/// 参数是否**来源安全**。
///
/// 🔴 判据不止「字面量」一种。本域有两类**经过校验的标识**同样安全：
/// - `rule_dir_name(rule_id)` —— 内部走 `infra::safe::sanitize_file_name`（去分隔符/保留名）
/// - `snapshot_dir_name(unix_ms)` / `format_snapshot_stamp(..)` / `data_dir_of(..)` / 常量
///   —— 由**纯数字与固定字面量**拼成，结构上不可能含 `..`
///
/// 第一版只放行字面量 ⇒ 把上述全部误报成违规（实测 17 处），
/// 守卫一旦噪声化就会被人加豁免，等于失去意义。
fn is_safe_argument(after: &str, whole_line: &str) -> bool {
    // ① 字面量：不含分隔符与 `..`
    if let Some(value) = literal_after(after) {
        return !value.contains("..") && !value.contains('/') && !value.contains('\\');
    }
    // ② 已 sanitize 的规则目录名
    if whole_line.contains("rule_dir_name(") {
        return true;
    }
    // ③ 由纯数字/固定常量拼出的快照内部路径
    for safe_fn in [
        "snapshot_dir_name(",
        "format_snapshot_stamp(",
        "data_dir_of(",
        "display_dir_name(",
    ] {
        if whole_line.contains(safe_fn) {
            return true;
        }
    }
    // ④ 路径**常量**（已登记在本模块的常量表里）
    for const_name in [
        "MANIFEST_FILE",
        "INFO_FILE",
        "DATA_DIR",
        "RULES_FILE",
        "SETTINGS_FILE",
        "CORRUPT_SUFFIX",
    ] {
        if whole_line.contains(const_name) {
            return true;
        }
    }
    // ⑤ `push` 已被 `has_path_context` 限定到强路径信号；
    //    `out.push(char)` 是 hex 缓冲区、`dest.push(segment)` 的段来自
    //    `validate_relative` 的返回值，两者结构上都无 `..`。
    if whole_line.contains("out.push(") || whole_line.contains("dest.push(segment)") {
        return true;
    }
    false
}

fn offending_lines(source: &str) -> Vec<(usize, String)> {
    source
        .lines()
        .enumerate()
        .filter(|(_, line)| {
            let trimmed = line.trim_start();
            if trimmed.starts_with("//") || trimmed.starts_with("///") {
                return false;
            }
            has_unsafe_join(line) || has_unsafe_push(line)
        })
        .map(|(index, line)| (index + 1, line.trim().to_owned()))
        .collect()
}

/// 取出 `.join(`/`.push(` 之后的字符串字面量内容（不是字面量则`None`）。
fn literal_after(after: &str) -> Option<&str> {
    let trimmed = after.trim_start();
    let inner = trimmed
        .strip_prefix("r#\"")
        .or_else(|| trimmed.strip_prefix("r\""))
        .or_else(|| trimmed.strip_prefix('"'))?;
    let end = inner.find('"')?;
    Some(&inner[..end])
}

/// 需要扫描的域内源文件。
fn domain_sources() -> Vec<(String, String)> {
    let domain = Path::new(env!("CARGO_MANIFEST_DIR")).join("src/backup");
    assert!(
        domain.is_dir(),
        "找不到 backup 域目录：{}",
        domain.display()
    );

    let mut files = Vec::new();
    collect_rs(&domain, &mut files);
    assert!(
        !files.is_empty(),
        "backup 域下未扫到任何 .rs —— 守卫会形同虚设"
    );

    files
        .into_iter()
        .filter_map(|file| {
            let name = file_name_of(&file);
            if name == SCANNER_FILE
                || name == TEST_MODULE_FILE
                || ALLOWED.iter().any(|(allowed, _)| *allowed == name)
            {
                return None;
            }
            let source = std::fs::read_to_string(&file).expect("读源文件");
            let scanned = match source.find("#[cfg(test)]") {
                Some(index) => source[..index].to_owned(),
                None => source,
            };
            Some((file.display().to_string(), scanned))
        })
        .collect()
}

#[test]
fn backup_domain_must_not_join_paths_directly() {
    let mut violations = Vec::new();
    for (path, source) in domain_sources() {
        for (line, text) in offending_lines(&source) {
            violations.push(format!("{path}:{line}  {text}"));
        }
    }
    assert!(
        violations.is_empty(),
        "🔴 backup 域出现直接路径 join/push（外部输入必须经 infra::safe）：\n  {}\n\
         确需例外时把文件加入 ALLOWED 并在变更记录写明理由。",
        violations.join("\n  ")
    );
}

/// 守卫自身的活性检查：**它必须能抓到违规**，否则「全绿」是假象。
#[test]
fn guard_detects_escapes_and_allows_safe_literals() {
    // ① 变量参数的 join → 必须红
    let bad = "
fn f(root: &Path, external: &str) -> PathBuf {
    root.join(external)
}
";
    assert_eq!(offending_lines(bad).len(), 1, "变量参数必须被抓到");

    // ①' 变量参数的 push → 同样必须红
    let bad_push = "
fn f(dest: &mut PathBuf, external: &str) {
    dest.push(external);
}
";
    assert_eq!(
        offending_lines(bad_push).len(),
        1,
        "push 的变量参数同样必须被抓到（只禁 join 等于留后门）"
    );

    // ② 安全字面量 → 必须放过
    let fine = r#"
fn g() -> PathBuf {
    root.join("aegis").join("backup").join("settings.json")
}
"#;
    assert!(
        offending_lines(fine).is_empty(),
        "不含分隔符与 .. 的字面量不该误报"
    );

    // ②' 已 sanitize / 已 validate 的标识 → 同样必须放过
    //     （第一版只放行字面量 ⇒ 实测误报 17 处，守卫一旦噪声化就等于失去意义）
    let safe_helpers = "
fn h(root: &Path, rule_id: &str, ms: i64) -> PathBuf {
    let base = root.join(rule_dir_name(rule_id));
    let dir = base.join(snapshot_dir_name(ms));
    dir.join(DATA_DIR)
}
";
    assert!(
        offending_lines(safe_helpers).is_empty(),
        "已sanitize 的标识与纯数字拼出的路径不该误报"
    );

    // ③ 带穿越意图的字面量 → join 与 push 都必须红
    let sneaky = r#"
fn h() {
    root.join("../escape");
    dest.push("..\\Windows");
}
"#;
    assert_eq!(
        offending_lines(sneaky).len(),
        2,
        "带 .. 或分隔符的字面量必须红（join 与 push 同罪）"
    );

    // ④ 注释里的示例不得误报
    let commented = "
// 不要这样写 —— root.join(external)
/// 文档示例同样不算
fn j() {}
";
    assert_eq!(offending_lines(commented), [], "注释里的示例不得误报");

    // ⑤ 零参数 `.join()` 是线程收尾
    let thread_join = "
fn k() {
    for handle in threads { let _ = handle.join(); }
}
";
    assert_eq!(
        offending_lines(thread_join),
        [],
        "JoinHandle::join() 零参数，不得当成路径拼接"
    );

    // ⑥ `push` 变量形态（本域 restore 的已校验段用法）必须放行
    let push_segment = "
fn m(dest: &mut PathBuf, segments: &[String]) {
    for segment in segments { dest.push(segment); }
}
";
    assert_eq!(
        offending_lines(push_segment),
        [],
        "已校验段的 push 属本域有意用法"
    );
}

/// 🔴 依赖方向守卫：本域**消费** `infra::safe` 而不是重实现。
#[test]
fn backup_consumes_the_guard_instead_of_reimplementing_it() {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR")).join("Cargo.toml");
    let text = std::fs::read_to_string(&manifest).expect("读 services/Cargo.toml");
    assert!(
        text.contains("infra = { path = \"../infra\" }"),
        "services 必须依赖 infra（路径守卫在其 safe 模块）"
    );

    for (path, source) in domain_sources() {
        for pattern in FORBIDDEN_FN_PATTERNS {
            assert!(
                !source.contains(pattern),
                "🔴 {path} 内不得重实现路径守卫（{pattern}）——唯一入口在 infra::safe"
            );
        }
        // 🔴 本域必须**真的调用**了守卫，否则说明绕过了它
        if path.ends_with("verify.rs")
            || path.ends_with("engine.rs")
            || path.ends_with("restore.rs")
        {
            assert!(
                source.contains("infra::safe::"),
                "🔴 {path} 消费外部相对路径却未调用 infra::safe —— 清单不可信（R-1）"
            );
        }
    }
}
