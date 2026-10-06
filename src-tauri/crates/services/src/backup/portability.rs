//! 规则集合级操作：导出 / 导入合并 / 重排（F-FB-08/14/15）。
//!
//! 复用结论（DP-1）：**③ 思路复用** ← `Source/SystemToolkit/src/SystemToolkit.Core/
//! Backup/Services/RuleExportContainer.cs` + `RuleManager.cs`（GPL-3.0 同源）。
//! C# 代码不可移植（02 册 DP-1 §4），取的是**两个设计决定**：
//!
//! 1. **导出带信封而非裸数组**（母本 `RuleExportContainer`）：信封里有
//!    `schemaVersion` / `tool` / `exportedAt`。裸数组在格式演进后**无法判断新旧** ——
//!    读到一份老文件时既不知道它是不是本工具写的、也不知道该按哪版解析。
//! 2. **导入是「合并」而非「替换」**（母本 `RuleManager`）：整表替换会让用户在
//!    换机时丢掉本机已建的规则。
//!
//! 🔴 **导出文件里的 `id` 保持原样**（不重新生成）：规则 id 是快照的寻址凭据
//!   （F-FB-05），换机后用同一份 id 才能让「规则 → 旧快照」的对应关系延续。

use super::models::{BackupRule, BackupSource};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::path::Path;

/// 规则文件的合规扩展名（F-FB-14「扩展名不合规直接拒绝并说明原因」）。
pub const RULE_FILE_EXT: &str = "json";

/// 当前导出信封的格式版本。
///
/// 🔴 **只增不改**：改字段含义时递增，旧文件按 [`parse_import`] 的兜底分支当裸数组处理。
pub const SCHEMA_VERSION: u32 = 1;

/// 导出工具标识（写进文件，便于日后判断来源）。
const TOOL_ID: &str = "AegisFileBackup";

/// 导出信封（③ 思路复用 ← 母本 `RuleExportContainer`，字段名按本工程线协议 camelCase）。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RuleExportEnvelope {
    /// 格式版本（读入时校验，见 [`parse_import`]）。
    pub schema_version: u32,
    /// 导出工具标识。
    pub tool: String,
    /// 导出时刻（本地时间 ISO 文本，如 `2026-10-06T04:30:00`）。
    pub exported_at: String,
    /// 规则列表。
    pub rules: Vec<BackupRule>,
}

/// 导入载荷：**信封**或**裸数组**。
///
/// 🔴 用 `untagged` 兼容两种：母本旧版本、以及用户手写的只有规则数组的文件。
///   顺序很重要 —— 先试信封（它有必填字段），失败再退裸数组。
#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum ImportPayload {
    Envelope(RuleExportEnvelope),
    Bare(Vec<BackupRule>),
}

/// 导入合并的产出。
#[derive(Debug, Clone, PartialEq, Default)]
pub struct MergeReport {
    /// 合并后的完整规则列表（可直接落盘）。
    pub merged: Vec<BackupRule>,
    /// 原地覆盖更新的条数。
    pub updated: usize,
    /// 新增追加的条数。
    pub appended: usize,
    /// 因来源集合与既有规则完全相同而改名的条数（⊆ `appended`）。
    pub renamed: usize,
    /// 逐条问题明细（跳过的、字段非法的等；F-FB-15「逐条问题给明细」）。
    pub issues: Vec<String>,
}

/// 本地时间 ISO 文本（`2026-10-06T04:30:00`）。
///
/// 🔴 复用 [`super::manifest::format_snapshot_stamp`] 的日历算法（本地时区 UTC+8），
///   **不引第三方日期库**（`services` 无 `chrono` 依赖，加一个只为格式化一个字段不划算）。
#[must_use]
pub fn local_iso8601(unix_ms: i64) -> String {
    let stamp = super::manifest::format_snapshot_stamp(unix_ms);
    let (date, time) = stamp.split_once('_').unwrap_or((stamp.as_str(), "000000"));
    if date.len() != 8 || time.len() != 6 {
        return stamp;
    }
    format!(
        "{}-{}-{}T{}:{}:{}",
        &date[0..4],
        &date[4..6],
        &date[6..8],
        &time[0..2],
        &time[2..4],
        &time[4..6]
    )
}

/// 当前 Unix 毫秒（UTC）。
fn now_unix_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| i64::try_from(d.as_millis()).unwrap_or(i64::MAX))
}

/// 把规则写进导出文件（**原子写**，R-3）。
///
/// # Errors
/// - 扩展名不是 `.json` ⇒ 返回可读原因（F-FB-14 明文要求「直接拒绝并说明原因」）；
/// - 序列化或落盘失败 ⇒ 返回 `io::Error` 的可读描述。
///   **失败不留半截文件**（`infra::fs::atomic_write` 保证）。
pub fn export_rules(rules: &[BackupRule], target: &Path) -> Result<usize, String> {
    let ext_ok = target
        .extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| e.eq_ignore_ascii_case(RULE_FILE_EXT));
    if !ext_ok {
        return Err(format!(
            "规则文件扩展名必须是 .{RULE_FILE_EXT}（收到：{}）",
            target.display()
        ));
    }
    let envelope = RuleExportEnvelope {
        schema_version: SCHEMA_VERSION,
        tool: TOOL_ID.to_owned(),
        exported_at: local_iso8601(now_unix_ms()),
        rules: rules.to_vec(),
    };
    let bytes =
        serde_json::to_vec_pretty(&envelope).map_err(|error| format!("规则序列化失败：{error}"))?;
    infra::fs::atomic_write(target, &bytes)
        .map_err(|error| format!("写入规则文件失败（{}）：{error}", target.display()))?;
    Ok(rules.len())
}

/// 解析导入文件（信封或裸数组）。
///
/// # Errors
/// 文件不是合法 JSON、或结构既不是信封也不是规则数组 ⇒ 返回可读原因。
pub fn parse_import(text: &str) -> Result<Vec<BackupRule>, String> {
    match serde_json::from_str::<ImportPayload>(text) {
        Ok(ImportPayload::Envelope(env)) => {
            if env.schema_version > SCHEMA_VERSION {
                return Err(format!(
                    "规则文件版本 {} 高于本程序支持的 {SCHEMA_VERSION}，请升级应用后再导入",
                    env.schema_version
                ));
            }
            Ok(env.rules)
        }
        Ok(ImportPayload::Bare(rules)) => Ok(rules),
        Err(error) => Err(format!("规则文件解析失败：{error}")),
    }
}

/// 读导入文件并解析（薄封装）。
///
/// # Errors
/// 读不到文件或内容不可解析 ⇒ 返回可读原因（**损坏文件另存留痕**，R-18）。
pub fn read_import(path: &Path) -> Result<Vec<BackupRule>, String> {
    let text = std::fs::read_to_string(path)
        .map_err(|error| format!("读取规则文件失败（{}）：{error}", path.display()))?;
    parse_import(&text)
}

/// 两条规则的**来源集合**是否完全相同（大小写与顺序均不敏感）。
///
/// 🔴 规格说的是「集合」⇒ 顺序无关；Windows 路径大小写不敏感（F-FB-02 同款口径），
///   否则 `C:\Data` 与 `c:\data` 会被判成两条不同规则而都留在列表里。
#[must_use]
pub fn same_source_set(a: &[BackupSource], b: &[BackupSource]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    let mut left: Vec<String> = a
        .iter()
        .map(|s| s.path.trim().to_ascii_lowercase())
        .collect();
    let mut right: Vec<String> = b
        .iter()
        .map(|s| s.path.trim().to_ascii_lowercase())
        .collect();
    left.sort_unstable();
    right.sort_unstable();
    left == right
}

/// 导入合并（F-FB-15 的三条判据）。
///
/// 逐条对 `incoming` 应用：
/// 1. **同标识且本批首次命中** ⇒ 原地覆盖更新；
/// 2. **批内重复标识 / 重复命中同一标识** ⇒ 分配新标识后追加；
/// 3. 追加前若**来源集合与合并后已有规则完全相同** ⇒ 名字加 `（导入）` 后缀。
///
/// 🔴 第 3 条只作用于**新增**：覆盖更新不改名（否则用户改过的规则名会被后缀污染）。
#[must_use]
pub fn merge_import(existing: &[BackupRule], incoming: Vec<BackupRule>) -> MergeReport {
    let mut report = MergeReport {
        merged: existing.to_vec(),
        ..MergeReport::default()
    };
    let mut hit_this_batch: HashSet<String> = HashSet::new();

    for mut rule in incoming {
        // ① 同标识且首次命中 ⇒ 覆盖
        if let Some(position) = report.merged.iter().position(|r| r.id == rule.id) {
            if !hit_this_batch.contains(&rule.id) {
                hit_this_batch.insert(rule.id.clone());
                report.merged[position] = rule;
                report.updated += 1;
                continue;
            }
            // ② 重复命中同一标识 ⇒ 换新 id 后走追加
            rule.id = super::ids::rule_id();
        }
        // 规则名空 ⇒ 不静默给个默认名，记问题后照常导入（用户能在列表里看到并改）
        if rule.name.trim().is_empty() {
            report
                .issues
                .push(format!("规则 {} 的名称为空，已按原样导入", rule.id));
        }
        // ③ 来源集合重复 ⇒ 改名区分
        if report
            .merged
            .iter()
            .any(|r| same_source_set(&r.sources, &rule.sources))
        {
            rule.name = format!("{}（导入）", rule.name.trim());
            report.renamed += 1;
            report.issues.push(format!(
                "规则「{}」的来源与既有规则完全相同，已加「（导入）」后缀",
                rule.name
            ));
        }
        report.appended += 1;
        report.merged.push(rule);
    }
    report
}

/// 按给定顺序重排规则（F-FB-08）。
///
/// # Errors
/// `ordered_ids` 与现有规则的 id 集合不一致（有条目缺失或多出）⇒ 返回可读原因。
///   **不静默忽略**：顺序写错会让「拖到末尾」变成「悄悄丢一条」。
///
/// 🔴 判据是**集合相等**而非「顺序一致」—— 调用方给的就是目标顺序。
pub fn reorder(rules: &mut Vec<BackupRule>, ordered_ids: &[String]) -> Result<(), String> {
    let current: HashSet<&str> = rules.iter().map(|r| r.id.as_str()).collect();
    let wanted: HashSet<&str> = ordered_ids.iter().map(String::as_str).collect();
    if current != wanted {
        let missing: Vec<&str> = current.difference(&wanted).copied().collect();
        let extra: Vec<&str> = wanted.difference(&current).copied().collect();
        let mut parts: Vec<String> = Vec::new();
        if !missing.is_empty() {
            parts.push(format!("缺少规则：{}", missing.join("、")));
        }
        if !extra.is_empty() {
            parts.push(format!("多余标识：{}", extra.join("、")));
        }
        return Err(format!("排序清单与现有规则不一致（{}）", parts.join("；")));
    }
    let mut next: Vec<BackupRule> = Vec::with_capacity(rules.len());
    for id in ordered_ids {
        // 上面已断言集合相等 ⇒ 这里必然命中；用 `find` + `if let` 避免 unwrap（01 册 §1.2）
        if let Some(found) = rules.iter().find(|r| &r.id == id) {
            next.push(found.clone());
        }
    }
    // 同步 `order` 字段，使落盘后的顺序与界面一致（列表按 `order` 排序）
    for (index, rule) in next.iter_mut().enumerate() {
        rule.order = u32::try_from(index).unwrap_or(u32::MAX);
    }
    *rules = next;
    Ok(())
}

#[cfg(test)]
mod tests;
