//! Tauri command 薄壳（🔴 纯转发：域逻辑全在 `services` 子模块，可被 worker 复用）。
//!
//! 🔴 **禁持 Tauri 宿主类型**（02 册 AD-4：不得 `use tauri::{Window, WebviewWindow, Menu, Tray}`），
//! 只允许 command 宏与 `State`。
//!
//! 🔴 **禁 `unwrap()` / `expect()`**（01 册 §1.2）—— 每个 command 的失败都映射成
//! [`AegisErrorPayload`] 形状的错误字符串返回。
//!
//! 命令命名（04 分册 §三）：`backup_<动词>[_<对象>]`，域词 `backup` 已在 01 册 §2.3 锁定。

// 🔴 Tauri 命令**必须按值接收参数**（`generate_handler!` 生成的反序列化要靠它），
// 故 `state: State<'_, BackupState>` / `args: XxxArgs` 是宏契约要求的形状，
// 不是可以改成 `&` 的选择。同款豁免与理由见 `transfer/commands.rs:28`。
//
// 🔴 **本条曾被踩反（2026-10-05 记）**：上一轮为消 `clippy::needless_pass_by_value`
// 的 12 处告警，把所有参数改成了 `&`。而 `cargo clippy -p services` 是**逐 crate** 跑的，
// `generate_handler!` 的宏展开发生在主 crate `aegis` —— 于是 clippy 绿、`cargo test` 绿、
// `cargo build -p services` 全绿，直到本轮把命令**真正注册进 `src-tauri/src/lib.rs`**
// 才炸出 24 个 `CommandArg` 未实现错误。
// ⇒ 教训：**逐 crate 的门禁覆盖不到跨 crate 的宏展开**。凡「注册进 generate_handler!
//   之后才会编译」的东西，必须跑 `cargo check`（主 crate）而不是只跑 `-p services`。
#![allow(clippy::needless_pass_by_value)]

use super::dto::{
    ExportView, ImportView, RestoreOutcome, RestorePlanView, RuleListView, RuleView, SettingsView,
    SnapshotView, VerifyReportView,
};
use super::engine::{self, CancelFlag, EngineDeps};
use super::models::{BackupRule, BackupSettings, ConflictPolicy, VerifyMode};
use super::restore::{self, RestoreTarget};
use super::store;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::sync::Mutex;

/// 运行期状态：配置目录 + 批量任务的取消句柄。
///
/// 🔴 用 `Mutex` 而非 `RwLock`：写入极少（创建/取消），读取是主路径。
#[derive(Default)]
pub struct BackupState {
    /// 配置目录（`%LOCALAPPDATA%\Aegis\config`）。
    pub config_dir: Mutex<Option<PathBuf>>,
    /// 当前运行中的备份的取消句柄（F-FB-19：取消按钮只在运行期可点）。
    pub cancel: Mutex<Option<CancelFlag>>,
}

impl BackupState {
    /// 取配置目录；未初始化时用 `%LOCALAPPDATA%\Aegis\config` 兜底。
    fn config_dir(&self) -> PathBuf {
        if let Ok(guard) = self.config_dir.lock() {
            if let Some(dir) = guard.as_ref() {
                return dir.clone();
            }
        }
        store::dirs_data_dir().join("Aegis").join("config")
    }
}

// ── 入参 ────────────────────────────────────────────────────────────────
//
// 🔴 **入参形状纪律（本域曾踩坑，务必先读）**
//
//   Tauri 的 `#[tauri::command]` **按参数名**从 invoke 载荷里取值，而前端
//   `src/lib/tauri/invoke.ts` 发的是 `{ envelope: {...}, ...payload顶层键 }`
//   —— 它把 payload 的**顶层键平铺**到 envelope 之外，**不会**发一个叫 `args` 的键。
//
//   ⇒ 本域所有命令**必须**形如 `fn x(envelope: CmdEnvelope, <字段>: T, ...)`：
//     一个 `envelope` 参数 + 每个字段各占一个**平铺**参数。
//     若图省事写成 `fn x(args: XxxArgs)`，`args` 在运行时**永远是 null**
//     （Tauri 找不到同名键），且**编译与测试全绿** —— 症状是界面上「点了没反应」。
//     可运行的同款口径见 `overview/commands.rs:204`（`envelope` + `payload` + `cards`）。
//
//   下方各 `*Args` 结构体因此**只作为「字段清单的文档」与解析后的载体**保留：
//   实际取值走平铺参数，解析失败统一映射 `BAD_PAYLOAD`。

use contracts::CmdEnvelope;

/// 关联 ID 校验（CT-5：一次用户操作一个 ID，贯穿前端 → 日志 → helper）。
///
/// # Errors
/// `correlationId` 缺失或全空白 ⇒ `BAD_PAYLOAD`。
fn check_envelope(envelope: &CmdEnvelope) -> Result<String, CommandError> {
    let id = envelope.correlation_id.trim();
    if id.is_empty() {
        return Err(err(
            "BAD_PAYLOAD",
            "缺少关联 ID（correlationId）".to_owned(),
            &envelope.correlation_id,
        ));
    }
    Ok(id.to_owned())
}

/// 列出全部规则 + 页头汇总（F-FB-01）。
#[derive(Debug, Clone, Deserialize)]
pub struct ListRulesArgs {
    /// 批量任务的关联 ID（CT-5：一次用户操作一个 ID）。
    #[serde(rename = "correlationId")]
    pub correlation_id: String,
}

/// 保存规则（F-FB-02/04/05；R-20非法值就地拒绝由 UI 负责，后端二次校验）。
#[derive(Debug, Clone, Deserialize)]
pub struct SaveRuleArgs {
    #[serde(rename = "correlationId")]
    pub correlation_id: String,
    /// 完整规则对象（前端传后端形状，🔴 后端不信任任何字段）。
    pub rule: RuleView,
    /// 草稿确认：来源全不存在时 UI 必须先弹确认（F-FB-04）。
    #[serde(default)]
    pub confirmed_draft: bool,
}

/// 立即备份（F-FB-16）。
///
/// # Errors
/// 返回 [`CommandError`]，code 为 reasons 表内的原因码、message 为可直接展示的中文。
/// 本 command 不返回 panic：所有失败路径都映射成 Err(CommandError)
///（01 册 §1.2「禁 `unwrap()` 出现在 command 路径」）。
#[derive(Debug, Clone, Deserialize)]
pub struct StartBackupArgs {
    #[serde(rename = "correlationId")]
    pub correlation_id: String,
    pub rule_id: String,
    /// 批量备份时的标签（`[i/N] 规则名`，F-FB-17）。
    #[serde(default)]
    pub label: String,
}

/// 规则快照列表（F-FB-26）。
#[derive(Debug, Clone, Deserialize)]
pub struct ListSnapshotsArgs {
    #[serde(rename = "correlationId")]
    pub correlation_id: String,
    pub rule_id: String,
}

/// 校验快照（F-FB-28）。
#[derive(Debug, Clone, Deserialize)]
pub struct VerifySnapshotArgs {
    #[serde(rename = "correlationId")]
    pub correlation_id: String,
    pub rule_id: String,
    /// 快照目录名（**展示名**，不含去重后缀）。
    pub snapshot_dir: String,
}

/// 恢复预演（F-FB-33/34/35）。
#[derive(Debug, Clone, Deserialize)]
pub struct PlanRestoreArgs {
    #[serde(rename = "correlationId")]
    pub correlation_id: String,
    pub rule_id: String,
    pub snapshot_dir: String,
    /// 空 = 恢复到原始位置。
    #[serde(default)]
    pub custom_target: String,
    /// 空 = 取全局默认（非法值回落，R-19）。
    #[serde(default)]
    pub policy: String,
}

/// 执行恢复（F-FB-36）。
///
/// # Errors
/// 返回 [`CommandError`]，code 为 reasons 表内的原因码、message 为可直接展示的中文。
/// 本 command 不返回 panic：所有失败路径都映射成 Err(CommandError)
///（01 册 §1.2「禁 `unwrap()` 出现在 command 路径」）。
#[derive(Debug, Clone, Deserialize)]
pub struct ExecuteRestoreArgs {
    #[serde(rename = "correlationId")]
    pub correlation_id: String,
    pub rule_id: String,
    pub snapshot_dir: String,
    #[serde(default)]
    pub custom_target: String,
    #[serde(default)]
    pub policy: String,
}

/// 统一错误包络（🔴 字段口径以 `02-专题设计/01-日志与错误处理.md` §3.2 为准）。
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CommandError {
    /// 原因码（`reasons` 表内；🔴 稳定串）。
    pub code: String,
    /// 面向用户的中文说明。
    pub message: String,
    /// 关联 ID（CT-5）。
    pub correlation_id: String,
}

fn err(code: &str, message: String, correlation_id: &str) -> CommandError {
    CommandError {
        code: code.to_owned(),
        message,
        correlation_id: correlation_id.to_owned(),
    }
}

type CommandResult<T> = Result<T, CommandError>;

// ── 规则 ────────────────────────────────────────────────────────────────

/// 列出全部规则（F-FB-01）。
///
/// # Errors
/// 返回 [`CommandError`]，code 为 reasons 表内的原因码、message 为可直接展示的中文。
/// 本 command 不返回 panic：所有失败路径都映射成 Err(CommandError)
///（01 册 §1.2「禁 `unwrap()` 出现在 command 路径」）。
#[tauri::command]
pub fn backup_list_rules(
    state: tauri::State<'_, BackupState>,
    envelope: CmdEnvelope,
) -> CommandResult<RuleListView> {
    let correlation_id = check_envelope(&envelope)?;
    let config_dir = state.config_dir();
    let outcome = store::load_rules(&config_dir);
    let rules = outcome.value.unwrap_or_default();
    let settings = store::load_settings(&config_dir).value.unwrap_or_default();
    let (root, _) = store::resolve_global_root(&settings);

    let mut warnings = Vec::new();
    if let Some(warning) = outcome.warning {
        // 🔴 R-18：记录损坏必须既另存留痕、又对用户可见（页面不空白但要有警告条）
        log::warn!("[backup][{correlation_id}] {warning}");
        warnings.push(warning);
    }

    // 快照总数 = 逐规则数（页头汇总 F-FB-01「规则 N 条 · 快照 M 份」）
    //
    // 🔴 2026-10-06 修一个既存失真：此前**统一用全局根**扫目录，忽略了规则的
    //   `custom_root` ⇒ 自定义根的规则快照数恒为 0，页头汇总对它完全失真
    //   （而 `resolve_snapshot` 走的是 `backup_root_of`，是认自定义根的
    //   —— 两处口径分叉的症状正是「列表看得到、统计是 0」）。
    //   现统一为「自定义根优先」，与 `resolve_snapshot` 同源。
    //
    // 🔴 顺带把**逐规则**的份数与最近时间在这里一并算出：**零额外 IO**
    //   （目录本来就为了总数要扫一遍），供卡片显示（F-FB-01 的逐规则口径）。
    let mut per_rule: Vec<(u64, i64)> = Vec::with_capacity(rules.len());
    let mut snapshot_count = 0_u64;
    for rule in &rules {
        let rule_root = rule
            .custom_root()
            .map_or_else(|| root.clone(), PathBuf::from);
        let (dirs, unparsed) = super::manifest::list_snapshot_dirs(&rule_root, &rule.id);
        snapshot_count = snapshot_count.saturating_add(dirs.len() as u64);
        // 🔴 取最大而非 `[0]`：`list_snapshot_dirs` **不保证有序**（它按 read_dir
        //   的原始顺序返回），早前就因为「展示时隐藏后缀」踩过顺序假设的坑。
        let latest = dirs.iter().map(|d| d.created_unix_ms).max().unwrap_or(0);
        per_rule.push((u64::try_from(dirs.len()).unwrap_or(0), latest));
        if !unparsed.is_empty() {
            warnings.push(format!(
                "规则「{}」有 {} 个快照目录名无法解析时间，已跳过裁剪",
                rule.name,
                unparsed.len()
            ));
        }
    }

    Ok(RuleListView {
        rule_count: rules.len() as u64,
        snapshot_count,
        rules: rules
            .iter()
            .zip(per_rule.iter())
            .map(|(rule, (count, latest))| {
                let mut view = RuleView::from(rule);
                view.snapshot_count = *count;
                view.latest_snapshot_at = *latest;
                view
            })
            .collect(),
        warnings,
    })
}

/// 保存规则（F-FB-02/04/05/07/08/10~13）。
///
/// # Errors
/// 返回 [`CommandError`]，code 为 reasons 表内的原因码、message 为可直接展示的中文。
/// 本 command 不返回 panic：所有失败路径都映射成 Err(CommandError)
///（01 册 §1.2「禁 `unwrap()` 出现在 command 路径」）。
///
/// 🔴 R-20：非法值**不静默沿用旧值** —— 一律拒绝并回读错误，交 UI 就地提示。
#[tauri::command]
pub fn backup_save_rule(
    state: tauri::State<'_, BackupState>,
    envelope: CmdEnvelope,
    rule: RuleView,
    confirmed_draft: bool,
) -> CommandResult<RuleView> {
    let correlation_id = check_envelope(&envelope)?;
    let config_dir = state.config_dir();
    let outcome = store::load_rules(&config_dir);
    let mut rules = outcome.value.unwrap_or_default();

    let view = rule;
    // 🔴 校验与转换**分成两个函数**：校验是「拒绝或放行」，转换是「映射」，
    //    混在一个函数里时「新增一条校验」很容易顺手改到转换分支上。
    if let Err(reason) = validate_rule_view(&view, confirmed_draft) {
        return Err(err(reason.0, reason.1, &correlation_id));
    }
    let rule = rule_from_view(view);

    match rules.iter_mut().find(|r| r.id == rule.id) {
        Some(existing) => *existing = rule.clone(),
        // 🔴 F-FB-05：既有快照按标识寻址 ⇒ 新规则也要有稳定标识
        None => rules.push(rule.clone()),
    }
    store::save_rules(&config_dir, &rules).map_err(|e| {
        err(
            "RULE_SAVE_FAILED",
            format!("保存规则失败：{e}"),
            &correlation_id,
        )
    })?;
    Ok(RuleView::from(&rule))
}

/// 删除规则（F-FB-06：两步确认由 UI 完成；🔴 快照**不连带删除**，如实说明）。
///
/// # Errors
/// 返回 [`CommandError`]，code 为 reasons 表内的原因码、message 为可直接展示的中文。
/// 本 command 不返回 panic：所有失败路径都映射成 Err(CommandError)
///（01 册 §1.2「禁 `unwrap()` 出现在 command 路径」）。
#[tauri::command]
pub fn backup_delete_rule(
    state: tauri::State<'_, BackupState>,
    envelope: CmdEnvelope,
    rule: RuleView,
) -> CommandResult<u64> {
    let correlation_id = check_envelope(&envelope)?;
    let config_dir = state.config_dir();
    let mut rules = store::load_rules(&config_dir).value.unwrap_or_default();
    let before = rules.len() as u64;
    rules.retain(|r| r.id != rule.id);
    if rules.len() as u64 == before {
        return Err(err("RULE_NOT_FOUND", "规则不存在".into(), &correlation_id));
    }
    store::save_rules(&config_dir, &rules).map_err(|e| {
        err(
            "RULE_SAVE_FAILED",
            format!("保存规则失败：{e}"),
            &correlation_id,
        )
    })?;
    Ok(before - rules.len() as u64)
}

// ── 备份执行 ────────────────────────────────────────────────────────────

/// 立即备份（F-FB-16）。
///
/// 🔴 同步引擎在 `spawn_blocking` 里跑（01 册 §3.1：禁阻塞 tokio 运行时）。
///
/// # Errors
/// 规则不存在 / 备份根不可用 / 来源与落点重叠 / 磁盘空间不足 ⇒ `Err(CommandError)`。
/// 🔴 **单文件失败、部分成功、用户取消都不走 `Err`**（R-13/R-14）——
///    它们体现在返回的 [`BackupOutcome`] 里，因为「备份了 1200 个、失败 4 个」
///    是**成功完成了一次带部分失败的备份**，报成异常会让用户以为白跑了。
#[tauri::command]
pub async fn backup_start(
    state: tauri::State<'_, BackupState>,
    envelope: CmdEnvelope,
    rule_id: String,
    label: String,
) -> CommandResult<super::dto::BackupOutcome> {
    let correlation_id = check_envelope(&envelope)?;
    let config_dir = state.config_dir();
    let rules = store::load_rules(&config_dir).value.unwrap_or_default();
    let Some(rule) = rules.iter().find(|r| r.id == rule_id).cloned() else {
        return Err(err("RULE_NOT_FOUND", "规则不存在".into(), &correlation_id));
    };
    let settings = store::load_settings(&config_dir).value.unwrap_or_default();
    let (root, root_warning) = store::resolve_global_root(&settings);
    let backup_root = match rule.custom_root() {
        Some(custom) => PathBuf::from(custom),
        None => root,
    };

    let cancel = CancelFlag::new();
    if let Ok(mut slot) = state.cancel.lock() {
        *slot = Some(cancel.clone());
    }

    // 🔴 `move` 闭包会**移走** correlation_id / label，而 await 之后的错误分支
    //   还要用它们来填 `CommandError.correlationId` ⇒ 必须给闭包一份**副本**，
    //   外层保留原值。（这不是"多余的一行clone"：删掉它编译不过。）
    let engine_cid = correlation_id.clone();
    let engine_label = label.clone();
    let result = tauri::async_runtime::spawn_blocking(move || {
        let deps = EngineDeps {
            backup_root: &backup_root,
            rule: &rule,
            settings: &settings,
            cancel: cancel.clone(),
            // 🔴 本批无 Tauri 事件通道接线（W0-07 生成链未落地）⇒ 进度落到日志，
            //   前端按「阶段未知」处理。接线时把这里换成 emit。
            progress: Box::new(|_view| {}),
            correlation_id: engine_cid,
            label: engine_label,
        };
        engine::run(&deps)
    })
    .await;

    if let Ok(mut slot) = state.cancel.lock() {
        *slot = None;
    }

    let outcome = match result {
        Ok(Ok(outcome)) => outcome,
        Ok(Err(failure)) => {
            if let Some(warning) = root_warning {
                log::warn!("[backup][{correlation_id}] {warning}");
            }
            return Err(err("BACKUP_FAILED", failure.to_string(), &correlation_id));
        }
        Err(join_error) => {
            return Err(err(
                "BACKUP_PANICKED",
                format!("备份任务异常终止：{join_error}"),
                &correlation_id,
            ));
        }
    };
    Ok(outcome)
}

/// 取消备份（F-FB-19：🔴 取消按钮只在运行期可点，由前端按状态控制）。
///
/// # Errors
/// 返回 [`CommandError`]，code 为 reasons 表内的原因码、message 为可直接展示的中文。
/// 本 command 不返回 panic：所有失败路径都映射成 Err(CommandError)
///（01 册 §1.2「禁 `unwrap()` 出现在 command 路径」）。
#[tauri::command]
pub fn backup_cancel(
    state: tauri::State<'_, BackupState>,
    envelope: CmdEnvelope,
) -> CommandResult<()> {
    let correlation_id = check_envelope(&envelope)?;
    // 🔴 `CancelFlag::cancel` 是**副作用**函数（置位标志，返回 ()），
    //    所以不能塞进 `map_or` / `is_some_and`（它们要求闭包**产出值**）——
    //    那会把「置位」变成「取值」而永不生效。故显式 match 三个态。
    let cancelled = match state.cancel.lock() {
        Ok(slot) => match slot.as_ref() {
            Some(flag) => {
                flag.cancel();
                true
            }
            None => false,
        },
        //锁坏了（中毒/死锁残留）⇒ 判「没有在跑」，不假装取消成功
        Err(_) => false,
    };
    if !cancelled {
        return Err(err(
            "NOT_RUNNING",
            "当前没有运行中的备份".into(),
            &correlation_id,
        ));
    }
    Ok(())
}

// ── 快照 ────────────────────────────────────────────────────────────────

/// 快照列表（F-FB-26）。
///
/// # Errors
/// 返回 [`CommandError`]，code 为 reasons 表内的原因码、message 为可直接展示的中文。
/// 本 command 不返回 panic：所有失败路径都映射成 Err(CommandError)
///（01 册 §1.2「禁 `unwrap()` 出现在 command 路径」）。
#[tauri::command]
pub fn backup_list_snapshots(
    state: tauri::State<'_, BackupState>,
    envelope: CmdEnvelope,
    rule_id: String,
) -> CommandResult<Vec<SnapshotView>> {
    // 🔴 校验仍要做（CT-5：空关联 ID 不该被放行），但本命令无失败分支
    //   ⇒ 只需副作用、不必留返回值。
    check_envelope(&envelope)?;
    let config_dir = state.config_dir();
    let rules = store::load_rules(&config_dir).value.unwrap_or_default();
    let settings = store::load_settings(&config_dir).value.unwrap_or_default();
    let (root, _) = store::resolve_global_root(&settings);
    let backup_root = rules
        .iter()
        .find(|r| r.id == rule_id)
        .and_then(|r| r.custom_root())
        .map_or_else(|| root.clone(), PathBuf::from);

    let (dirs, _unparsed) = super::manifest::list_snapshot_dirs(&backup_root, &rule_id);
    let rule_name = rules
        .iter()
        .find(|r| r.id == rule_id)
        .map_or_else(String::new, |r| r.name.clone());
    Ok(dirs
        .iter()
        .map(|d| SnapshotView::from(&d.load_info(&rule_id, &rule_name)))
        .collect())
}

/// 校验快照（F-FB-28：手动全量读回）。
///
/// # Errors
/// 返回 [`CommandError`]，code 为 reasons 表内的原因码、message 为可直接展示的中文。
/// 本 command 不返回 panic：所有失败路径都映射成 Err(CommandError)
///（01 册 §1.2「禁 `unwrap()` 出现在 command 路径」）。
#[tauri::command]
pub fn backup_verify_snapshot(
    state: tauri::State<'_, BackupState>,
    envelope: CmdEnvelope,
    rule_id: String,
    snapshot_dir: String,
) -> CommandResult<VerifyReportView> {
    let correlation_id = check_envelope(&envelope)?;
    let (snapshot_dir, manifest, data_dir) = resolve_snapshot(&state, &rule_id, &snapshot_dir)
        .map_err(|message| err(super::reasons::SNAPSHOT_NOT_FOUND, message, &correlation_id))?;
    let report = super::verify::verify_manifest(&manifest, &data_dir);
    let _ = snapshot_dir;
    Ok(VerifyReportView::from(&report))
}

/// 删除快照（F-FB-30：二次确认由 UI 完成）。
///
/// # Errors
/// 返回 [`CommandError`]，code 为 reasons 表内的原因码、message 为可直接展示的中文。
/// 本 command 不返回 panic：所有失败路径都映射成 Err(CommandError)
///（01 册 §1.2「禁 `unwrap()` 出现在 command 路径」）。
#[tauri::command]
pub fn backup_delete_snapshot(
    state: tauri::State<'_, BackupState>,
    envelope: CmdEnvelope,
    rule_id: String,
    snapshot_dir: String,
) -> CommandResult<()> {
    let correlation_id = check_envelope(&envelope)?;
    let (snapshot_dir, _manifest, _data_dir) = resolve_snapshot(&state, &rule_id, &snapshot_dir)
        .map_err(|message| err(super::reasons::SNAPSHOT_NOT_FOUND, message, &correlation_id))?;
    // 🔴 目录已不存在时**如实报失败**，不显示成"已删除"（F-FB-30）
    std::fs::remove_dir_all(&snapshot_dir).map_err(|error| {
        err(
            "SNAPSHOT_DELETE_FAILED",
            format!("删除快照失败（{}）：{error}", snapshot_dir.display()),
            &correlation_id,
        )
    })
}

/// 用系统文件管理器打开快照目录（F-FB-27）。
///
/// 🔴 **只允许打开「本规则快照目录」范围内的路径**（R-1 末句）。目录来自由
///   [`resolve_snapshot`] 的**枚举匹配**（不是把用户输入拼进路径），此处再显式复验一次
///   落点（纵深防御）—— 防将来有人把这里改成「前端直接传绝对路径」而丢掉守卫。
///   三种失败分别如实提示（F-FB-27）：越界 → `GUARD_REJECTED`；
///   不存在 → `SNAPSHOT_NOT_FOUND`；无权限/开不了 → `OPEN_DIR_FAILED`。
///
/// # Errors
/// 返回 [`CommandError`]，code 为 reasons 表内的原因码、message 为可直接展示的中文。
/// 本 command 不返回 panic：所有失败路径都映射成 Err(CommandError)
///（01 册 §1.2「禁 `unwrap()` 出现在 command 路径」）。
#[tauri::command]
pub fn backup_open_snapshot_dir(
    state: tauri::State<'_, BackupState>,
    envelope: CmdEnvelope,
    rule_id: String,
    snapshot_dir: String,
) -> CommandResult<()> {
    let correlation_id = check_envelope(&envelope)?;
    let (dir, _manifest, _data_dir) = resolve_snapshot(&state, &rule_id, &snapshot_dir)
        .map_err(|message| err(super::reasons::SNAPSHOT_NOT_FOUND, message, &correlation_id))?;

    // 🔴 注意参数序：(root, child)。
    let rule_root = backup_root_of(&state, &rule_id).join(super::manifest::rule_dir_name(&rule_id));
    if !infra::safe::is_lexically_under(&rule_root, &dir) {
        return Err(err(
            super::reasons::GUARD_REJECTED,
            format!("拒绝打开该规则快照目录之外的路径：{}", dir.display()),
            &correlation_id,
        ));
    }
    if !dir.is_dir() {
        return Err(err(
            super::reasons::SNAPSHOT_NOT_FOUND,
            format!("快照目录不存在（可能已被手动删除）：{}", dir.display()),
            &correlation_id,
        ));
    }
    open_in_file_manager(&dir)
        .map_err(|message| err(super::reasons::OPEN_DIR_FAILED, message, &correlation_id))
}

/// 导出规则到指定文件（F-FB-14）。
///
/// 🔴 落盘一律经 `infra::fs::atomic_write`（R-3）；**扩展名不合规直接拒绝并说明原因**
///   （规格明文）—— 不静默改后缀，因为用户可能把 `.json` 存成了 `.txt` 却以为导出成功。
///
/// # Errors
/// 返回 [`CommandError`]，message 为可直接展示的中文（扩展名不合规 / 读不到规则 / 落盘失败）。
/// 本 command 不返回 panic：所有失败路径都映射成 Err(CommandError)（01 册 §1.2）。
#[tauri::command]
pub fn backup_export_rules(
    state: tauri::State<'_, BackupState>,
    envelope: CmdEnvelope,
    target_path: String,
) -> CommandResult<ExportView> {
    let correlation_id = check_envelope(&envelope)?;
    let config_dir = state.config_dir();
    let rules = store::load_rules(&config_dir).value.unwrap_or_default();
    let path = PathBuf::from(target_path.trim());
    let count = super::portability::export_rules(&rules, &path)
        .map_err(|message| err("RULE_EXPORT_FAILED", message, &correlation_id))?;
    Ok(ExportView {
        count: u64::try_from(count).unwrap_or(0),
        path: path.display().to_string(),
    })
}

/// 导入规则（**合并式**，F-FB-15）。
///
/// 🔴 不是「整表替换」：换机迁移时那会丢掉本机已建的规则。
///   三条判据（同标识覆盖 / 重复标识换新 id / 来源集合重复改名）落在
///   [`super::portability::merge_import`]，本命令只做 IO 与落盘。
///
/// # Errors
/// 返回 [`CommandError`]：读不到/解析不了文件，或落盘失败。
///   🔴 **只有落盘成功才报成功**；失败返回 Err ⇒ 界面重取后仍是旧列表，
///   绝不出现「界面显示导入成功、磁盘没变」。
#[tauri::command]
pub fn backup_import_rules(
    state: tauri::State<'_, BackupState>,
    envelope: CmdEnvelope,
    source_path: String,
) -> CommandResult<ImportView> {
    let correlation_id = check_envelope(&envelope)?;
    let config_dir = state.config_dir();
    let existing = store::load_rules(&config_dir).value.unwrap_or_default();
    let path = PathBuf::from(source_path.trim());
    let incoming = super::portability::read_import(&path)
        .map_err(|message| err("RULE_IMPORT_FAILED", message, &correlation_id))?;
    let report = super::portability::merge_import(&existing, incoming);
    store::save_rules(&config_dir, &report.merged).map_err(|error| {
        err(
            "RULE_IMPORT_FAILED",
            format!("规则记录保存失败：{error}"),
            &correlation_id,
        )
    })?;
    Ok(ImportView {
        updated: u64::try_from(report.updated).unwrap_or(0),
        appended: u64::try_from(report.appended).unwrap_or(0),
        renamed: u64::try_from(report.renamed).unwrap_or(0),
        issues: report.issues,
    })
}

/// 重排规则顺序（F-FB-08：拖到末尾也必须能落下）。
///
/// 🔴 `ordered_ids` 与现有规则**集合不等即拒绝**（不静默忽略缺失/多余条目）——
///   否则「拖动失败」会表现成「悄悄丢了一条规则」。
///   落盘失败返回 Err ⇒ 界面重取后顺序**自动回退**，绝不出现「界面变了、磁盘没变」。
///
/// # Errors
/// 返回 [`CommandError`]：排序清单与现有规则不一致，或落盘失败。
#[tauri::command]
pub fn backup_reorder_rules(
    state: tauri::State<'_, BackupState>,
    envelope: CmdEnvelope,
    ordered_ids: Vec<String>,
) -> CommandResult<()> {
    let correlation_id = check_envelope(&envelope)?;
    let config_dir = state.config_dir();
    let mut rules = store::load_rules(&config_dir).value.unwrap_or_default();
    super::portability::reorder(&mut rules, &ordered_ids)
        .map_err(|message| err("RULE_REORDER_FAILED", message, &correlation_id))?;
    store::save_rules(&config_dir, &rules).map_err(|error| {
        err(
            "RULE_REORDER_FAILED",
            format!("规则记录保存失败：{error}"),
            &correlation_id,
        )
    })
}

/// 单键切换规则的启用 / 停用（F-FB-07）。
///
/// 🔴 **刻意不复用 `backup_save_rule`**：那条路径要过完整表单校验（含「来源全不存在 ⇒
///   草稿确认」），于是「停用一条来源暂时不在的规则」会被守卫拦下 —— 而用户只是想停用它。
///   本命令只碰 `enabled` 一个字段，其余字段原样保留。
///
/// # Errors
/// 返回 [`CommandError`]，code 为 reasons 表内的原因码、message 为可直接展示的中文。
/// 本 command 不返回 panic：所有失败路径都映射成 Err(CommandError)
///（01 册 §1.2「禁 `unwrap()` 出现在 command 路径」）。
#[tauri::command]
pub fn backup_set_rule_enabled(
    state: tauri::State<'_, BackupState>,
    envelope: CmdEnvelope,
    rule_id: String,
    enabled: bool,
) -> CommandResult<()> {
    let correlation_id = check_envelope(&envelope)?;
    let config_dir = state.config_dir();
    let mut rules = store::load_rules(&config_dir).value.unwrap_or_default();
    let Some(rule) = rules.iter_mut().find(|r| r.id == rule_id) else {
        return Err(err(
            "RULE_NOT_FOUND",
            format!("找不到规则：{rule_id}"),
            &correlation_id,
        ));
    };
    rule.enabled = enabled;
    // 🔴 落盘失败即返回 Err —— 界面**不落地显示**（F-FB-07：绝不出现「界面变了、磁盘没变」）。
    store::save_rules(&config_dir, &rules).map_err(|error| {
        err(
            super::reasons::RULE_STORE_CORRUPTED,
            format!("规则记录保存失败：{error}"),
            &correlation_id,
        )
    })
}

/// 用系统文件管理器打开一个**已通过守卫**的目录。
///
/// 🔴 跨域零互引（AD-2）：`steam` 域已有同款（`steam::open_game_folder`），
///   但两域不得互引，故本域自持一份。**若出现第三个调用方，应下沉到 `infra`**。
///
/// 🔴 本函数**不做路径守卫** —— 守卫是调用方（`backup_open_snapshot_dir`）的职责，
///   这样「谁能开什么目录」只有一个判定点。
#[cfg(windows)]
fn open_in_file_manager(dir: &Path) -> Result<(), String> {
    use std::os::windows::process::CommandExt;
    // 不弹控制台黑框。
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    std::process::Command::new("explorer")
        .arg(dir)
        .creation_flags(CREATE_NO_WINDOW)
        .spawn()
        .map(|_| ())
        .map_err(|error| format!("调用系统文件管理器失败：{error}"))
}

#[cfg(not(windows))]
fn open_in_file_manager(dir: &Path) -> Result<(), String> {
    let _ = dir;
    Err("当前平台暂不支持打开目录".to_owned())
}

// ── 恢复 ────────────────────────────────────────────────────────────────

/// 恢复预演（F-FB-35：🔴 与执行共用 `restore::plan`）。
///
/// # Errors
/// 返回 [`CommandError`]，code 为 reasons 表内的原因码、message 为可直接展示的中文。
/// 本 command 不返回 panic：所有失败路径都映射成 Err(CommandError)
///（01 册 §1.2「禁 `unwrap()` 出现在 command 路径」）。
#[tauri::command]
pub fn backup_plan_restore(
    state: tauri::State<'_, BackupState>,
    envelope: CmdEnvelope,
    rule_id: String,
    snapshot_dir: String,
    custom_target: String,
    policy: String,
) -> CommandResult<RestorePlanView> {
    let correlation_id = check_envelope(&envelope)?;
    let config_dir = state.config_dir();
    let rules = store::load_rules(&config_dir).value.unwrap_or_default();
    let rule = rules.iter().find(|r| r.id == rule_id).cloned();
    let (_dir, manifest, _data) = resolve_snapshot(&state, &rule_id, &snapshot_dir)
        .map_err(|message| err(super::reasons::SNAPSHOT_NOT_FOUND, message, &correlation_id))?;
    let target = build_target(&custom_target);
    let policy = build_policy(&config_dir, &policy);
    let plan = restore::plan(&manifest, rule.as_ref(), &target, policy);
    Ok(plan.to_view(custom_target.clone()))
}

/// 执行恢复（F-FB-36）。
///
/// # Errors
/// 仅「快照定位失败」返回 `Err`（`SNAPSHOT_NOT_FOUND`，含「已被手动删除」的情形）。
/// 🔴 **单文件恢复失败不走 `Err`** —— 它计入 [`RestoreOutcome`]的 `failed` /
///    `verifyMismatched` / `skipped` 并带明细，因为「1200 项里 3 项失败」
///    是一次**已执行完的恢复**，报成异常会让用户以为要重跑。
#[tauri::command]
pub fn backup_execute_restore(
    state: tauri::State<'_, BackupState>,
    envelope: CmdEnvelope,
    rule_id: String,
    snapshot_dir: String,
    custom_target: String,
    policy: String,
) -> CommandResult<RestoreOutcome> {
    let correlation_id = check_envelope(&envelope)?;
    let config_dir = state.config_dir();
    let rules = store::load_rules(&config_dir).value.unwrap_or_default();
    let rule = rules.iter().find(|r| r.id == rule_id).cloned();
    let (_dir, manifest, data_dir) = resolve_snapshot(&state, &rule_id, &snapshot_dir)
        .map_err(|message| err(super::reasons::SNAPSHOT_NOT_FOUND, message, &correlation_id))?;
    let target = build_target(&custom_target);
    let policy = build_policy(&config_dir, &policy);
    // 🔴 预演与执行共用同一 plan —— 这里**不重新实现**判据
    let plan = restore::plan(&manifest, rule.as_ref(), &target, policy);
    Ok(restore::execute(&manifest, &plan, &data_dir))
}

// ── 设置 ────────────────────────────────────────────────────────────────

/// 读设置（F-FB-24/29/34）。
///
/// # Errors
/// 返回 [`CommandError`]，code 为 reasons 表内的原因码、message 为可直接展示的中文。
/// 本 command 不返回 panic：所有失败路径都映射成 Err(CommandError)
///（01 册 §1.2「禁 `unwrap()` 出现在 command 路径」）。
#[tauri::command]
pub fn backup_get_settings(
    state: tauri::State<'_, BackupState>,
    envelope: CmdEnvelope,
) -> CommandResult<SettingsView> {
    let correlation_id = check_envelope(&envelope)?;
    let _ = &correlation_id;
    let settings = store::load_settings(&state.config_dir())
        .value
        .unwrap_or_default();
    Ok(SettingsView::from(&settings))
}

/// 写设置。
#[derive(Debug, Clone, Deserialize)]
pub struct SaveSettingsArgs {
    #[serde(rename = "correlationId")]
    pub correlation_id: String,
    pub settings: SettingsView,
}

/// 保存设置。
///
/// # Errors
/// 返回 [`CommandError`]，code 为 reasons 表内的原因码、message 为可直接展示的中文。
/// 本 command 不返回 panic：所有失败路径都映射成 Err(CommandError)
///（01 册 §1.2「禁 `unwrap()` 出现在 command 路径」）。
#[tauri::command]
pub fn backup_save_settings(
    state: tauri::State<'_, BackupState>,
    envelope: CmdEnvelope,
    settings: SettingsView,
) -> CommandResult<()> {
    let correlation_id = check_envelope(&envelope)?;
    let settings = BackupSettings {
        global_root: settings.global_root.clone(),
        default_keep_count: settings.default_keep_count,
        gfs: None,
        concurrency: settings.concurrency,
        verify_sample: settings.verify_sample,
        // 🔴 本批复用恒关闭（依赖 K-002，且母本自身也默认关闭 R-12）
        reuse_unchanged: false,
        default_conflict_policy: settings.default_conflict_policy,
    }
    .clamped();
    store::save_settings(&state.config_dir(), &settings).map_err(|error| {
        err(
            "SETTINGS_SAVE_FAILED",
            format!("保存设置失败：{error}"),
            &correlation_id,
        )
    })
}

// ── 内部工具 ────────────────────────────────────────────────────────────

/// 规则校验（R-20：**就地拒绝，不静默回落旧值**）。
///
/// 🔴 返回 `(原因码, 中文文案)` 而非 `CommandError` —— 关联 ID 由调用方补，
///    这样本函数是**纯函数**，可被将来的 worker 批复用（不依赖 Tauri 状态）。
///
/// # Errors
/// 任一校验不通过即返回原因码；调用方**必须**把它映射成 `Err`，不得降级为警告。
fn validate_rule_view(
    view: &RuleView,
    confirmed_draft: bool,
) -> Result<(), (&'static str, String)> {
    if view.name.trim().is_empty() {
        return Err(("RULE_NAME_EMPTY", "规则名不能为空".to_owned()));
    }
    if view.sources.is_empty() {
        return Err((
            "SOURCE_EMPTY",
            "至少需要一个来源（文件夹或文件）".to_owned(),
        ));
    }
    if !view.use_gfs && !(1..=super::models::RetentionPolicy::COUNT_MAX).contains(&view.keep_count)
    {
        return Err((
            "KEEP_COUNT_INVALID",
            format!(
                "保留份数需在 1–{} 之间（收到 {}）",
                super::models::RetentionPolicy::COUNT_MAX,
                view.keep_count
            ),
        ));
    }
    // 🔴 F-FB-04：来源全不存在 ⇒ 必须显式确认后才存为草稿
    if !confirmed_draft {
        let missing: Vec<&String> = view
            .sources
            .iter()
            .filter(|p| !Path::new(p).exists())
            .collect();
        if missing.len() == view.sources.len() {
            let listed: Vec<&str> = missing.iter().map(|p| p.as_str()).collect();
            return Err((
                "DRAFT_CONFIRM_REQUIRED",
                format!("以下来源不存在，请确认保存为草稿：{}", listed.join("；")),
            ));
        }
    }
    // 🔴 K-002 未拍板 ⇒ 勾了卷影就明说不可用，不静默忽略
    if view.use_shadow_copy {
        return Err((
            super::reasons::SHADOW_COPY_UNAVAILABLE,
            "卷影能力形态待定，暂不支持启用".to_owned(),
        ));
    }
    // 🔴 K-003：定时时间非法 ⇒ 就地拒绝（R-15）
    let daily = view.daily_time.trim();
    if !daily.is_empty() && !super::schedule::is_valid_daily_time(daily) {
        return Err((
            super::reasons::SCHEDULE_FORM_UNDECIDED,
            format!("定时时刻「{daily}」非法，需为 HH:mm 且 00:00–23:59"),
        ));
    }
    Ok(())
}

/// 线协议 → 落盘模型（🔴 **不信任任何字段**：来源类型按磁盘实况判定，
/// 不采信前端送来的 `kind`，否则用户改了 JSON 就能让目录当文件复制）。
fn rule_from_view(view: RuleView) -> BackupRule {
    let custom_root = view.custom_root.trim().to_owned();
    let daily_time = view.daily_time.trim().to_owned();
    let last_scheduled = view.last_scheduled_date.trim().to_owned();
    // 🔴 **id 为空 ⇒ 后端生成**（F-FB-05：规则必须有稳定标识才能寻址既有快照）。
    //   前端 `emptyRule()` 的 id 是空串（它不知道也不该知道id 格式），
    //   若原样落盘：`rule_dir_name("")` 会 sanitize 失败退回常量`"rule"`
    //   ⇒ **所有空 id 规则的快照全挤进同一个目录**，
    //   且前端 `useSnapshots` 的 `enabled: ruleId !== ""` 恒false
    //   ⇒ 快照列表**永久骨架屏**、永远加载不出来。
    //   症状（本轮实测）：页头「快照 N 份」正常（按 id 数目录）但列表空转。
    let id = if view.id.trim().is_empty() {
        super::ids::rule_id()
    } else {
        view.id
    };
    BackupRule {
        id,
        name: view.name.trim().to_owned(),
        note: view.note,
        enabled: view.enabled,
        sources: view
            .sources
            .iter()
            .map(|raw| {
                let kind = if Path::new(raw).is_dir() {
                    super::models::SourceType::Directory
                } else {
                    super::models::SourceType::File
                };
                super::models::BackupSource::new(raw, kind)
            })
            .collect(),
        target: if custom_root.is_empty() {
            super::models::BackupTarget::GlobalRoot
        } else {
            super::models::BackupTarget::CustomRoot { root: custom_root }
        },
        retention: if view.use_gfs {
            super::models::RetentionPolicy::Count(1)
        } else {
            super::models::RetentionPolicy::Count(view.keep_count)
        },
        exclusions: view.exclusions,
        daily_time: (!daily_time.is_empty()).then_some(daily_time),
        last_scheduled_date: (!last_scheduled.is_empty()).then_some(last_scheduled),
        // 🔴 已被 validate 拒绝，此处置 false
        use_shadow_copy: false,
        order: view.order,
    }
}

// 🔴 规则 id 的生成实现在 [`super::ids`]（域内唯一一份）。
//   本文件**不再自带**生成器：曾经的那份用了「取模 + 除法」推进，
//   24 位需 120 位而 `u64` 只有 64 位 ⇒ 种子耗尽后退化成恒定尾部，
//   实测产出 `r0fe7pvaietmh111111111111`（后 12 位全是 `1`）。
//   修法与判据见 `ids.rs` 与 `ids/tests.rs`。

/// 定位快照目录 + 读清单 + 算数据目录。
///
/// 🔴 展示名 → 真实目录名需要补后缀（F-FB-32：展示时隐藏了去重后缀），
/// 故这里做**前缀匹配**而不是直接拼名字。
fn resolve_snapshot(
    state: &tauri::State<'_, BackupState>,
    rule_id: &str,
    display_name: &str,
) -> Result<(PathBuf, super::models::SnapshotManifest, PathBuf), String> {
    // 🔴 取根走 [`backup_root_of`]：与本域「打开目录」共用同一段逻辑（见该函数注释）。
    let backup_root = backup_root_of(state, rule_id);

    let (dirs, _unparsed) = super::manifest::list_snapshot_dirs(&backup_root, rule_id);
    let Some(entry) = dirs
        .iter()
        .find(|d| super::manifest::display_dir_name(&d.dir_name) == display_name)
    else {
        return Err(format!("找不到快照「{display_name}」（可能已被手动删除）"));
    };
    let manifest = super::manifest::read_manifest(&entry.dir).map_err(|e| e.to_string())?;
    let data_dir = super::manifest::data_dir_of(&entry.dir);
    Ok((entry.dir.clone(), manifest, data_dir))
}

/// 该规则的备份根：自定义根优先，否则全局根（F-FB-10）。
///
/// 🔴 抽成函数是为了让 [`resolve_snapshot`] 与 `backup_open_snapshot_dir`
///   共用**同一段**取根逻辑 —— 两处各算一次就会出现「一处用自定义根、一处用全局根」
///   的分叉，而那种分叉的症状是「快照列表看得到、打开目录却说找不到」。
fn backup_root_of(state: &tauri::State<'_, BackupState>, rule_id: &str) -> PathBuf {
    let config_dir = state.config_dir();
    let rules = store::load_rules(&config_dir).value.unwrap_or_default();
    let settings = store::load_settings(&config_dir).value.unwrap_or_default();
    let (root, _) = store::resolve_global_root(&settings);
    rules
        .iter()
        .find(|r| r.id == rule_id)
        .and_then(|r| r.custom_root())
        .map_or_else(|| root.clone(), PathBuf::from)
}

fn build_target(custom: &str) -> RestoreTarget {
    if custom.trim().is_empty() {
        RestoreTarget::Original
    } else {
        RestoreTarget::Custom(PathBuf::from(custom.trim()))
    }
}

fn build_policy(config_dir: &Path, raw: &str) -> ConflictPolicy {
    if raw.trim().is_empty() {
        return store::load_settings(config_dir)
            .value
            .unwrap_or_default()
            .default_conflict_policy;
    }
    // 🔴 非法值回落（R-19：最不易毁数据的策略）
    ConflictPolicy::from_setting(raw)
}

/// 校验档位转换（供设置读入后使用；🟡 UI 编辑入口本批未开放）。
#[must_use]
pub fn verify_mode_of(settings: &BackupSettings) -> VerifyMode {
    settings.verify_mode()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::backup::dto::RuleView;
    use crate::backup::manifest::rule_dir_name;

    /// 空 id 的最小可用规则（其余字段填合法值，让校验层不干扰）。
    fn view_with_id(id: &str) -> RuleView {
        RuleView {
            id: id.to_owned(),
            name: "规则".to_owned(),
            note: String::new(),
            enabled: true,
            sources: vec!["D:\\工作".to_owned()],
            custom_root: String::new(),
            keep_count: 7,
            use_gfs: false,
            exclusions: Vec::new(),
            daily_time: String::new(),
            last_scheduled_date: String::new(),
            use_shadow_copy: false,
            is_draft: false,
            shadow_capability: super::super::dto::ShadowCapability::ClientUnavailable,
            schedule_capability: super::super::dto::ScheduleCapability::Undecided,
            order: 0,
            // 派生字段：保存路径不写它们，这里给中性值即可。
            snapshot_count: 0,
            latest_snapshot_at: 0,
        }
    }

    #[test]
    fn empty_id_gets_a_generated_one() {
        // 🔴 回归测试（本轮真缺陷）：前端 `emptyRule()` 的 id 是空串，
        //    原样落盘会让**所有**空 id 规则的快照挤进同一目录，
        //    且前端 `enabled: ruleId !== ""` 恒 false ⇒ 列表永久骨架屏。
        let rule = rule_from_view(view_with_id(""));
        assert!(!rule.id.is_empty(), "空 id 必须由后端生成");
        assert!(
            rule.id.starts_with('r'),
            "生成的 id 应带可识别前缀：{}",
            rule.id
        );
    }

    #[test]
    fn blank_id_also_counts_as_empty() {
        // 🔴 只判 `is_empty()` 会漏掉 "   "（全空格）—— 那种 id 同样 sanitize 失败
        let rule = rule_from_view(view_with_id("   "));
        assert!(!rule.id.trim().is_empty(), "全空格 id 也必须生成");
    }

    #[test]
    fn provided_id_is_kept_verbatim() {
        // 🔴 编辑既有规则时**不能**换 id，否则快照目录会指向新的空目录（快照「消失」）
        let rule = rule_from_view(view_with_id("rabc123"));
        assert_eq!(rule.id, "rabc123");
    }

    #[test]
    fn generated_ids_are_unique_and_filename_safe() {
        // 🔴 两个约束同时成立才算修好：① 互不相同（不共享目录）
        //    ② sanitize 后**不变**（否则不同 id 可能映射到同一目录名）
        let a = rule_from_view(view_with_id("")).id;
        let b = rule_from_view(view_with_id("")).id;
        assert_ne!(a, b, "两次生成不得相同");
        assert_eq!(
            rule_dir_name(&a),
            format!("r{}", &a[1..]),
            "sanitize 不得改动生成 id"
        );
        assert_eq!(rule_dir_name(&b), format!("r{}", &b[1..]));
    }
}
