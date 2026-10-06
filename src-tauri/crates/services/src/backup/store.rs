//! 规则与设置的持久化（`config/backup.json` 等；F-FB-01/07/08 · **R-18**）。
//!
//! 复用结论（DP-1）：③ 思路复用 ← `Core/Backup/Services/BackupConfigService.cs`
//! 与 `RuleManager.cs`。**母本的损坏处理必须原样继承**（规格 R-18）：
//! 「规则记录损坏时先把损坏文件另存留痕再重建，**不静默丢弃**」。
//!
//! 🔴 落盘一律经 `infra::fs::atomic_write`（02 册 §5.2 强制；守卫
//! `atomic_write_and_path_guard` 会扫「裸 `File::create` 覆盖目标」）。
//!
//! 🔴 **读到的非法值回落默认并留日志**（04 分册 §六：禁「猜半个值写回」）——
//! 回落只影响**内存中的运行值**，**不回写文件**，避免把用户的原始记录改成我们的猜测。

use super::models::{BackupRule, BackupSettings};
use std::path::{Path, PathBuf};

/// 规则记录文件名。
pub const RULES_FILE: &str = "backup.json";
/// 设置记录文件名。
pub const SETTINGS_FILE: &str = "backup-settings.json";
/// 损坏记录另存时的后缀（R-18：留痕可追溯）。
pub const CORRUPT_SUFFIX: &str = ".corrupt";

/// 一次读记录的产出：值 + 是否损坏 + 留痕路径。
#[derive(Debug, Clone, PartialEq)]
pub struct LoadOutcome<T> {
    /// 读到的值（损坏时为 `None`）。
    pub value: Option<T>,
    /// 🔴 损坏文件的另存路径（`None` = 未损坏或文件不存在）。
    pub quarantined: Option<PathBuf>,
    /// 面向用户的警告（前端须展示，R-18「显式警告」）。
    pub warning: Option<String>,
}

/// 读规则列表。
///
/// 🔴 损坏时**不返回空列表了事**（那会让用户以为规则全丢了）：另存留痕 + 返回警告，
/// 调用方据此在页头显示警告并**继续按空列表工作**（F-FB-01「页面不空白」）。
#[must_use]
pub fn load_rules(config_dir: &Path) -> LoadOutcome<Vec<BackupRule>> {
    let path = config_dir.join(RULES_FILE);
    load_json(&path)
}

/// 读设置。
#[must_use]
pub fn load_settings(config_dir: &Path) -> LoadOutcome<BackupSettings> {
    let path = config_dir.join(SETTINGS_FILE);
    let outcome: LoadOutcome<BackupSettings> = load_json(&path);
    LoadOutcome {
        // 🔴 读到的数值钳制到合法区间（R-19）
        value: outcome.value.map(BackupSettings::clamped),
        quarantined: outcome.quarantined,
        warning: outcome.warning,
    }
}

/// 通用读：不存在 ⇒ `None` 无警告；损坏 ⇒ 另存 + 警告。
fn load_json<T: serde::de::DeserializeOwned>(path: &Path) -> LoadOutcome<T> {
    let text = match std::fs::read_to_string(path) {
        Ok(text) => text,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return LoadOutcome {
                value: None,
                quarantined: None,
                warning: None,
            };
        }
        Err(error) => {
            // 🔴 读得到但打不开（如权限）也要显式警告，不能当"没有记录"
            return LoadOutcome {
                value: None,
                quarantined: None,
                warning: Some(format!("读取失败（{}）：{error}", path.display())),
            };
        }
    };
    match serde_json::from_str::<T>(&text) {
        Ok(value) => LoadOutcome {
            value: Some(value),
            quarantined: None,
            warning: None,
        },
        Err(error) => {
            // 🔴 损坏：另存留痕（R-18），**不删原文件**
            let quarantined = quarantine(path, &text);
            let shown = quarantined
                .as_ref()
                .map_or_else(|| "另存失败".to_owned(), |p| p.display().to_string());
            LoadOutcome {
                value: None,
                quarantined,
                warning: Some(format!(
                    "备份记录已损坏，已另存为 {shown} 并按空记录继续（原因：{error}）"
                )),
            }
        }
    }
}

/// 另存损坏文件（R-18）。同名已存在时加序号，避免覆盖上一次的留痕。
fn quarantine(path: &Path, text: &str) -> Option<PathBuf> {
    // `backup.json` → `backup.corrupt` / `backup.corrupt.1` / …
    let base = format!("{}.{CORRUPT_SUFFIX}", path.display());
    let mut target = PathBuf::from(&base);
    let mut n = 1;
    while target.exists() {
        if n > 99 {
            return None;
        }
        target = PathBuf::from(format!("{base}.{n}"));
        n += 1;
    }
    // 🔴 另存本身失败时**不删原文件**（那才是真正的数据丢失）
    std::fs::write(&target, text).ok()?;
    Some(target)
}

/// 写规则列表（原子写；R-3）。
///
/// # Errors
/// 序列化或落盘失败 ⇒ `io::Error`。**失败不得留半截文件**（原子写保证）。
pub fn save_rules(config_dir: &Path, rules: &[BackupRule]) -> std::io::Result<()> {
    save_json(config_dir, RULES_FILE, rules)
}

/// 写设置（原子写）。
///
/// # Errors
/// 同 [`save_rules`]。
pub fn save_settings(config_dir: &Path, settings: &BackupSettings) -> std::io::Result<()> {
    save_json(config_dir, SETTINGS_FILE, settings)
}

fn save_json<T: serde::Serialize + ?Sized>(
    config_dir: &Path,
    name: &str,
    value: &T,
) -> std::io::Result<()> {
    let bytes = serde_json::to_vec_pretty(value)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
    infra::fs::atomic_write(&config_dir.join(name), &bytes)
}

/// 解析默认备份根（R-19）。
///
/// 规格原文：「未指定备份根时，取'剩余空间最大的本地固定磁盘'根下的专用目录；
/// 枚举不到可用磁盘时兜底到用户文档目录」。
///
/// 🔴 本工程尚无磁盘枚举能力（`overview::hardware::storage` 有采集，但属概览域，
/// 跨域直读违反 AD-15表归属）⇒ 本批**兜底到用户文档目录**并留警告，
/// 待 `settings` 域提供全局「剩余空间最大磁盘」查询后接入。
#[must_use]
pub fn resolve_global_root(settings: &BackupSettings) -> (PathBuf, Option<String>) {
    let configured = settings.global_root.trim();
    if !configured.is_empty() {
        return (PathBuf::from(configured), None);
    }
    let docs = dirs_data_dir().join("Aegis").join("backups");
    (
        docs,
        Some("未指定全局备份根，暂用文档目录下的 Aegis\\backups（磁盘枚举能力待接入）".to_owned()),
    )
}

/// 兜底数据目录（`%LOCALAPPDATA%`）。取不到时退回当前目录的相对路径。
#[must_use]
pub fn dirs_data_dir() -> PathBuf {
    std::env::var_os("LOCALAPPDATA").map_or_else(|| PathBuf::from("."), PathBuf::from)
}

#[cfg(test)]
mod tests;
