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

/// 找出含 `.join(` 且**不在注释里**的行。
///
/// 口径是**保守**的"出现即违规"：文本扫描做不了接收者类型推断，
/// 宁可要求一律走统一入口，也不放行一个"看起来是 Vec 的 join"。
fn offending_lines(source: &str) -> Vec<(usize, String)> {
    source
        .lines()
        .enumerate()
        .filter(|(_, line)| {
            let trimmed = line.trim_start();
            !trimmed.starts_with("//") && !trimmed.starts_with("///") && line.contains(".join(")
        })
        .map(|(index, line)| (index + 1, line.trim().to_owned()))
        .collect()
}

/// 需要扫描的域内源文件（排除守卫自身与 [`ALLOWED`] 条目）。
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
            if name == SCANNER_FILE || ALLOWED.iter().any(|(allowed, _)| *allowed == name) {
                return None;
            }
            Some((
                file.display().to_string(),
                std::fs::read_to_string(&file).expect("读源文件"),
            ))
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
#[test]
fn guard_detects_a_synthetic_violation() {
    let bad = "
fn f(root: &Path, external: &str) -> PathBuf {
    root.join(external)
}
";
    let hits = offending_lines(bad);
    assert_eq!(hits.len(), 1, "守卫必须能识别 join 调用");
    assert!(hits[0].1.contains(".join("));

    let commented = "
// 说明：不要这样写 —— root.join(external)
/// 文档里的示例同样不算
fn g() {}
";
    assert!(
        offending_lines(commented).is_empty(),
        "注释里的示例不得误报"
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
