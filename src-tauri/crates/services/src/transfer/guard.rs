//! 架构守卫：**`transfer` 域禁止直接 `Path::join` 消费外部输入**。
//!
//! 规格依据（`Docs/开发文档/02-专题设计/02-安全与特权通道.md` §5.1）：
//! *"各域禁止 `Path::join` 直接消费外部输入"*；外部字符串 + 根目录的拼接**只能**经
//! `infra::safe::join_under_root` 这一个入口。
//!
//! 🔴 **为什么用"源码扫描断言"而不是自定义 clippy lint**：规格原文是
//! "自定义 clippy lint **/** 依赖审计断言"（二者取一）。本仓**尚无 lint 基建**
//! （W0-11「架构守卫」未落地，全仓只有 `eslint.config.mjs`），而自定义 clippy lint 需要
//! dylint/clippy-driver 一整套工具链，属独立工程。故本批走**审计断言**路线，
//! 在违规会真正发生的地方（`transfer` 域）就地拦下。
//! 🟠 待 W0-11 落地后，本守卫应迁移为该基建下的一条规则（形式可变，口径不变）。
//!
//! 🔴 **例外须登记**：确需在域内直接 join 时，把文件与理由加进 [`ALLOWED`]，
//! 并在变更记录里写明"为什么 `join_under_root` 不适用"。**不得静默放宽本守卫。**

use std::path::{Path, PathBuf};

/// 允许在域内直接 `Path::join` 的文件（文件名，非路径）与理由。
///
/// 🔴 当前为**空**：本域尚无任何一处需要直接 join。新增条目必须附理由。
const ALLOWED: &[(&str, &str)] = &[];

/// 守卫自身（本文件）**不参与扫描**：它必然含被禁模式的字面量（合成违规样本、禁用函数名），
/// 否则无法自证"抓得到违规"。这与 [`ALLOWED`] 不是一回事——后者是**真的允许 join**。
const SCANNER_FILE: &str = "guard.rs";

/// 测试模块文件名（本仓惯例：`#[cfg(test)] mod tests;` 指向同目录的 `tests.rs`）**不参与扫描**。
///
/// 理由：被测对象的**期望路径**得由测试自己拼（`temp_root().join("a.txt.aegisft-tok.part")`），
/// 那是构造字面量、不是消费外部输入；强行要求走 `join_under_root` 只会让测试变绕而安全性不变。
/// 🔴 这**不是**放松口径：本仓的 `tests.rs` 只以 `#[cfg(test)]` 编入，生产构建里不存在，
/// 故它不构成"域代码绕过守卫"的通路。
const TEST_MODULE_FILE: &str = "tests.rs";

/// 禁止在本域出现的同义函数名（防"再写一份路径守卫"）。
const FORBIDDEN_FN_PATTERNS: [&str; 3] = [
    "fn join_under_root",
    "fn is_safe_under_root",
    "fn sanitize_relative",
];

fn file_name_of(path: &Path) -> String {
    path.file_name()
        .map_or_else(String::new, |n| n.to_string_lossy().into_owned())
}

/// 递归收集 `dir` 下的 `.rs` 文件。
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

/// 判断一行的 `.join(...)` 是否违规。
///
/// 口径（比"出现即违规"更准，且仍然抓得住真风险）：**只看参数是不是"安全字面量"**。
/// * 参数是字符串字面量、且字面量里**不含分隔符与 `..`** → **不违规**
///   （`.join("Received")` 无法越界；这正是 `settings.rs` 拼 `%LOCALAPPDATA%` 子目录的情形）
/// * 其余一律违规：参数是变量/表达式（真正的"外部输入"风险），
///   或字面量里带 `..` / `/` / `\`
///
/// 🔴 为什么不做"接收者类型推断"：文本扫描做不到。但**参数**是字面量还是变量，
/// 文本上可判且恰好就是那条安全边界——外部输入必然是变量或表达式。
/// 对常量拼接（如 `.join(SETTINGS_FILE_NAME)`）本守卫仍会报，届时请就地写成字面量
/// 并用测试钉住它与常量的同值关系（见 `settings::tests::default_paths_follow_the_house_convention`）。
fn offending_lines(source: &str) -> Vec<(usize, String)> {
    source
        .lines()
        .enumerate()
        .filter(|(_, line)| {
            let trimmed = line.trim_start();
            if trimmed.starts_with("//") || trimmed.starts_with("///") {
                return false;
            }
            has_unsafe_join(line)
        })
        .map(|(index, line)| (index + 1, line.trim().to_owned()))
        .collect()
}

/// 该行是否存在"参数不是安全字面量"的 `.join(`。
fn has_unsafe_join(line: &str) -> bool {
    let marker = ".join(";
    let mut rest = line;
    while let Some(index) = rest.find(marker) {
        let after = &rest[index + marker.len()..];
        // 🔴 **零参数的 `.join()` 不是路径拼接**：`Path::join` 必须带一个参数，
        // 而 `JoinHandle::join()` 恰恰零参数。线程收尾处到处是 `handle.join()`，
        // 把它们当违规会让守卫从"有用"退化成噪声（本批实测踩到）。
        if after.trim_start().starts_with(')') {
            rest = after;
            continue;
        }
        if !is_safe_literal_argument(after) {
            return true;
        }
        rest = after;
    }
    false
}

/// 参数是否是"无法越界"的字符串字面量（不含分隔符、不含 `..`）。
fn is_safe_literal_argument(after: &str) -> bool {
    let trimmed = after.trim_start();
    let inner = trimmed
        .strip_prefix("r#\"")
        .or_else(|| trimmed.strip_prefix("r\""))
        .or_else(|| trimmed.strip_prefix('"'));
    let Some(inner) = inner else {
        return false; // 变量 / 表达式 / 函数调用 —— 一律按违规处理
    };
    let Some(end) = inner.find('"') else {
        return false;
    };
    let value = &inner[..end];
    !value.contains("..") && !value.contains('/') && !value.contains('\\')
}

/// 需要扫描的域内源文件（排除守卫自身、测试模块与 [`ALLOWED`] 条目）。
///
/// 🔴 **两种测试写法都要豁免**（本仓两种都在用）：
/// 1. 独立文件 `tests.rs`（`#[cfg(test)] mod tests;` 指向它）—— 按文件名豁免
/// 2. **内联** `#[cfg(test)] mod tests { … }` —— 按**截断**豁免：正文里 `#[cfg(test)]`
///    之后的内容一律不扫
///
/// 理由同 [`TEST_MODULE_FILE`]：测试要自己拼期望路径（构造字面量，不是消费外部输入），
/// 且这些代码只以 `#[cfg(test)]` 编入、生产构建里不存在。
fn domain_sources() -> Vec<(String, String)> {
    let domain = Path::new(env!("CARGO_MANIFEST_DIR")).join("src/transfer");
    assert!(
        domain.is_dir(),
        "找不到 transfer 域目录：{}",
        domain.display()
    );

    let mut files = Vec::new();
    collect_rs(&domain, &mut files);
    assert!(
        !files.is_empty(),
        "transfer 域下未扫到任何 .rs —— 守卫会形同虚设"
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
            // 内联测试模块之后的部分不扫
            let scanned = match source.find("#[cfg(test)]") {
                Some(index) => source[..index].to_owned(),
                None => source,
            };
            Some((file.display().to_string(), scanned))
        })
        .collect()
}

#[test]
fn transfer_domain_must_not_join_paths_directly() {
    let mut violations = Vec::new();
    for (path, source) in domain_sources() {
        for (line, text) in offending_lines(&source) {
            violations.push(format!("{path}:{line}  {text}"));
        }
    }
    assert!(
        violations.is_empty(),
        "🔴 transfer 域出现直接路径 join（外部输入必须经 infra::safe::join_under_root）：\n  {}\n\
         确需例外时把文件加入 ALLOWED 并在变更记录写明理由。",
        violations.join("\n  ")
    );
}

/// 守卫自身的活性检查：**它必须能抓到违规**，否则"全绿"是假象。
///
/// 同时钉住判据的**两侧边界**：变量参数必须红、安全字面量必须放过。
#[test]
fn guard_detects_variable_joins_and_allows_safe_literals() {
    // ① 变量参数（真正的外部输入风险）→ 必须红
    let bad = "
fn f(root: &Path, external: &str) -> PathBuf {
    root.join(external)
}
";
    let hits = offending_lines(bad);
    assert_eq!(hits.len(), 1, "变量参数必须被抓到");

    // ② 安全字面量 → 必须放过（否则 settings.rs 拼 %LOCALAPPDATA% 子目录会被误报）
    let fine = r#"
fn g() -> PathBuf {
    root.join("aegis").join("transfer").join("settings.json")
}
"#;
    assert!(
        offending_lines(fine).is_empty(),
        "不含分隔符与 .. 的字面量不该误报"
    );

    // ③ 带穿越意图的字面量 → 仍然必须红
    let sneaky = r#"
fn h() -> PathBuf {
    root.join("../escape")
}
"#;
    assert_eq!(offending_lines(sneaky).len(), 1, "带 .. 的字面量必须红");
    let absolute = r#"
fn i() -> PathBuf {
    root.join("C:/Windows")
}
"#;
    assert_eq!(offending_lines(absolute).len(), 1, "带分隔符的字面量必须红");

    // ④ 注释里的示例不得误报
    let commented = "
// 说明：不要这样写 —— root.join(external)
/// 文档里的示例同样不算
fn j() {}
";
    assert_eq!(offending_lines(commented), [], "注释里的示例不得误报");

    // ⑤ 零参数的 `.join()` 是 JoinHandle 收尾，不是路径拼接（本批实测的误报类）
    let thread_join = "
fn k() {
    for handle in threads {
        let _ = handle.join();
    }
}
";
    assert_eq!(
        offending_lines(thread_join),
        [],
        "JoinHandle::join() 零参数，不得当成路径拼接"
    );
}

/// 🔴 依赖方向守卫：`infra` 不得反向依赖 `services`（02 架构分册 AD-*）。
/// 本批把守卫落进 `infra` 正是因为 transfer 要消费它——方向必须是 services → infra。
#[test]
fn transfer_consumes_the_guard_instead_of_reimplementing_it() {
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
    }
}
