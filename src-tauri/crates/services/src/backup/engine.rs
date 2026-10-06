//! 备份执行引擎：分阶段流水线（**F-FB-18 阶段与进度反馈** · R-13/14/19）。
//!
//! 复用结论（DP-1）：③ 思路复用 ← `Core/Backup/Services/BackupService.cs`（`BackupAsync`）。
//! 阶段划分沿用母本顺序：**扫描 → 空间预检 → 复制+算指纹 → 写清单 → 读回校验 → 裁剪**。
//!
//! 🔴 **四条不可让步的语义**（都在下面的流水线里以代码固定，不是注释里的口号）：
//! 1. **R-13 部分成功不是成功**：只要存在失败项，快照状态即为 `Failed`
//!    （含义 = 清单不完整，不能作为可信基准），但数据仍保留可用。
//! 2. **R-14 取消语义**：取消清理半成品目录；快照清单**已写完**则保留该快照
//!    （备份其实已完成）；取消**不记失败**，**不记**定时已执行。
//! 3. **R-6 空间三态**：不足 ⇒ 中止；查不到 ⇒ 放行但**必须**留下 warn。
//! 4. **R-7 重叠在扫描之前**：否则大目录要白扫一遍才报错。
//!
//! 🔴 **并发与取消**（01 册 §3.1）：本函数是**同步阻塞**实现，由调用方
//! （`commands.rs` 的 `spawn_blocking` 或 `worker` 进程）放到后台线程；
//! 🔴 **不得**在 tokio 异步上下文里直接调用（守卫 `no_sync_over_async`）。

use super::admission::{check_disk_space, check_overlap, DiskVerdict};
use super::dto::{BackupOutcome, BackupStage, FailureView, ProgressView};
use super::manifest;
use super::models::{
    BackupRule, BackupSettings, FileEntry, IntegrityStatus, SnapshotInfo, SnapshotStatus,
};
use super::retention::{plan_retention, RetentionCandidate};
use super::scan::{drop_contained_sources, scan_source, ExclusionMatcher, ScanResult};
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

/// 一次备份的可取消句柄（🔴 复制令牌：前端取消 → 这里置位，流水线在阶段边界检查）。
#[derive(Debug, Clone, Default)]
pub struct CancelFlag(Arc<AtomicBool>);

impl CancelFlag {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// 请求取消（幂等）。
    pub fn cancel(&self) {
        self.0.store(true, Ordering::Relaxed);
    }

    #[must_use]
    pub fn is_cancelled(&self) -> bool {
        self.0.load(Ordering::Relaxed)
    }
}

/// 进度回调（🔴 调用方负责节流 ≥100ms，引擎不自己做节流）。
pub type ProgressSink = Box<dyn Fn(ProgressView) + Send + Sync>;

/// 引擎需要的外部能力（🔴 用依赖注入而非直接 new，便于测试与替换）。
pub struct EngineDeps<'a> {
    /// 全局备份根（已由调用方按 R-19 解析）。
    pub backup_root: &'a Path,
    /// 规则。
    pub rule: &'a BackupRule,
    /// 全局设置。
    pub settings: &'a BackupSettings,
    /// 取消句柄。
    pub cancel: CancelFlag,
    /// 进度回调。
    pub progress: ProgressSink,
    /// 关联 ID（CT-5：一次用户操作一个 ID贯穿始终）。
    pub correlation_id: String,
    /// 批量备份时的标签（`[i/N] 规则名`，F-FB-17）。
    pub label: String,
}

impl EngineDeps<'_> {
    fn emit(&self, stage: BackupStage, processed: u64, total: u64) {
        // 🔴 不允许除零后把 NaN 传给前端：total==0 时报0%
        let percent = if total == 0 {
            0
        } else {
            let raw = u128::from(processed.saturating_mul(100)) / u128::from(total);
            u8::try_from(raw.min(100)).unwrap_or(0)
        };
        (self.progress)(ProgressView {
            rule_id: self.rule.id.clone(),
            label: self.label.clone(),
            stage,
            processed,
            total,
            percent,
            correlation_id: self.correlation_id.clone(),
        });
    }
}

/// 执行一次备份。
///
/// # Errors
/// 只有**闸门类**硬失败才返回 `Err`（重叠、空间不足、备份根不可用）；
/// 单文件失败、部分成功、取消**都不走 `Err`**，而是体现在返回值的
/// [`BackupOutcome`] 里（R-13 语义：失败要可见，不是要抛异常）。
/// 前置闸门：卷影可用性 + 重叠 + 扫描 + 空间预检。
///
/// 🔴 独立成函数是为了让[`run`] 只剩「编排」语义；同时把R-7「重叠检查必须在扫描**之前**」
///   这条顺序约束固化在类型上：闸门返回的 [`GateResult`] 里带着已扫好的结果，
///   物理上不可能出现"先扫后检"的调用路径。
fn pass_gates(deps: &EngineDeps<'_>) -> Result<(ScanResult, std::path::PathBuf), HardFailure> {
    // ── 阶段 0：卷影时点视图（🟡 K-002 未拍板 ⇒ 本批恒跳过，规则开关被禁用）──
    deps.emit(BackupStage::PreparingShadowView, 0, 1);
    // 🔴 规则里勾了卷影也不走时点视图：能力恒不可用。**必须如实告知**而不是静默忽略
    //   （否则用户勾了开关却得到"普通复制"的结果而无任何提示）。
    if deps.rule.use_shadow_copy {
        return Err(HardFailure::ShadowUnavailable);
    }

    // ── 闸门 1：来源与落点重叠（R-7，🔴 必须在扫描之前）──
    let root = deps.backup_root.to_path_buf();
    let source_paths: Vec<String> = deps.rule.sources.iter().map(|s| s.path.clone()).collect();
    let overlap = check_overlap(&source_paths, &root);
    if overlap.blocks() {
        return Err(HardFailure::Overlap(overlap.reason_code()));
    }

    // ── 阶段 1：扫描（R-8/R-9）──
    deps.emit(BackupStage::Scanning, 0, 0);
    let scan = scan_all(deps.rule, deps);
    deps.emit(
        BackupStage::Scanning,
        scan.files.len() as u64,
        scan.files.len() as u64,
    );

    // 🔴 全部来源都不存在 ⇒ 没有任何可备份内容，这不是"成功"
    if scan.files.is_empty() && scan.empty_dirs.is_empty() {
        return Err(HardFailure::NothingToBackup(scan.skipped_sources));
    }

    // ── 闸门 2：磁盘空间三态（R-6）──
    deps.emit(BackupStage::CheckingSpace, 0, 1);
    let verdict = check_disk_space(&root, scan.total_bytes);
    if verdict.blocks() {
        let (code, _) = verdict.reason();
        return Err(HardFailure::InsufficientSpace {
            code,
            detail: format!("需要 {} 字节，可用空间不足", required_of(verdict)),
        });
    }
    // 🔴 查不到 ⇒ 放行但**必须留痕**（R-6「禁止把未知当充足」）。
    if verdict.needs_warning() {
        let (code, text) = verdict.reason();
        log_warn(code, text, &deps.correlation_id);
    }
    Ok((scan, root))
}

/// 复制阶段：逐文件「拷贝 + 单遍算指纹」，逐条检查取消。
///
/// # Errors
/// 目前不返回 `Err`（单文件失败计入 `failures`，R-13「部分成功不是成功」）；
/// 签名保留 `Result` 是为将来需要整体中止（如磁盘写满）时不必改调用点。
fn copy_phase(
    deps: &EngineDeps<'_>,
    scan: &ScanResult,
    data_dir: &std::path::Path,
) -> (Vec<FileEntry>, Vec<FailureView>, bool) {
    let mut entries: Vec<FileEntry> = Vec::with_capacity(scan.files.len());
    let mut failures: Vec<FailureView> = Vec::new();
    let total = u64::try_from(scan.files.len()).unwrap_or(u64::MAX);
    let mut cancelled = false;

    for (index, file) in scan.files.iter().enumerate() {
        // 🔴 R-14：每个文件边界检查取消点，让取消能在细粒度任务里及时响应
        if deps.cancel.is_cancelled() {
            cancelled = true;
            break;
        }
        // 🔴 目标路径必须经守卫拼接（02 专题 §5.1「一切外部字符串 + 根目录拼接」）
        let Ok(target) = infra::safe::join_under_root(data_dir, &file.relative) else {
            // 路径越界是**源端或前缀生成**的 bug，不是用户可修的：如实报失败
            failures.push(FailureView {
                code: super::reasons::FILE_WRITE_FAILED.to_owned(),
                text: "相对路径未通过越界守卫".to_owned(),
                path: file.relative.clone(),
            });
            continue;
        };
        match super::hash::copy_and_hash(&file.absolute, &target) {
            Ok(sha256) => entries.push(manifest::file_entry(
                &file.relative,
                &file.absolute,
                file.size,
                file.modified_unix_ms,
                sha256,
            )),
            // 🔴 R-4：单个文件失败**不终止整体**，其余照常完成（R-13）
            Err(error) => failures.push(FailureView {
                code: super::reasons::FILE_READ_FAILED.to_owned(),
                text: error.to_string(),
                path: file.absolute.display().to_string(),
            }),
        }
        deps.emit(
            BackupStage::Copying,
            u64::try_from(index + 1).unwrap_or(u64::MAX),
            total,
        );
    }
    (entries, failures, cancelled)
}

/// 执行一次备份。
///
/// # Errors
/// 只有**闸门类**硬失败才返回 `Err`（重叠、空间不足、备份根不可用）；
/// 单文件失败、部分成功、取消**都不走 `Err`**，而是体现在返回值的
/// [`BackupOutcome`] 里（R-13 语义：失败要可见，不是要抛异常）。
pub fn run(deps: &EngineDeps<'_>) -> Result<BackupOutcome, HardFailure> {
    if deps.cancel.is_cancelled() {
        return Ok(cancelled_outcome());
    }
    let (scan, root) = pass_gates(deps)?;

    // ── 建快照目录（母本 TOCTOU 的修法：随机后缀 + 独占创建）──
    let created = now_unix_ms();
    let snapshot_dir = manifest::create_snapshot_dir(&root, &deps.rule.id, created)
        .map_err(|e| HardFailure::SnapshotDirFailed(e.to_string()))?;
    let data_dir = manifest::data_dir_of(&snapshot_dir);
    std::fs::create_dir_all(&data_dir)
        .map_err(|e| HardFailure::SnapshotDirFailed(e.to_string()))?;

    // ── 阶段 2：复制 + 单遍算指纹（R-4备份侧）──
    let (entries, failures, cancelled) = copy_phase(deps, &scan, &data_dir);
    if cancelled {
        return Ok(finish_cancelled(
            &snapshot_dir,
            &data_dir,
            entries,
            failures,
            deps,
        ));
    }
    let source_paths: Vec<String> = deps.rule.sources.iter().map(|s| s.path.clone()).collect();

    // ── 阶段 3：写清单（R-18）──
    deps.emit(BackupStage::WritingManifest, 0, 1);
    let has_failures = !failures.is_empty();
    // 🔴 R-11 结论推导：有失败项 ⇒ Failed。**在写清单前就要定好**，
    //   否则清单与元信息会给出比实际更乐观的结论。
    let status = if has_failures {
        SnapshotStatus::Failed
    } else {
        SnapshotStatus::Success
    };
    let m = manifest::build_manifest(manifest::ManifestInput {
        rule_id: deps.rule.id.clone(),
        rule_name: deps.rule.name.clone(),
        snapshot_id: snapshot_dir
            .file_name()
            .map_or_else(String::new, |n| n.to_string_lossy().into_owned()),
        created_unix_ms: created,
        sources: source_paths.clone(),
        entries: entries.clone(),
        empty_dirs: scan.empty_dirs.clone(),
        status,
        // 初始为「未做完整校验」；下面校验阶段会改写
        integrity: IntegrityStatus::NotFullyVerified,
        reused_count: 0,
    });
    if let Err(error) = manifest::write_manifest(&snapshot_dir, &m) {
        // 🔴 R-18：**保留数据目录**，报「已保留待人工处理」，绝不静默删除
        log_error(
            super::reasons::MANIFEST_WRITE_FAILED,
            &error.to_string(),
            &deps.correlation_id,
        );
        return Err(HardFailure::ManifestWriteFailed {
            detail: error.to_string(),
            snapshot_dir: snapshot_dir.display().to_string(),
        });
    }

    // ── 阶段 4：读回校验（F-FB-29 / R-11）──
    deps.emit(BackupStage::Verifying, 0, 1);
    let mode = deps.settings.verify_mode();
    // 🔴 本批复用恒为 false ⇒ has_reused = false；待 K-002 后这里接真实统计
    let has_reused = false;
    let report = super::verify::verify_by_mode(&m, &data_dir, mode, has_reused);
    // 🔴 R-11 末句 / F-FB-29：**校验自身失败不推翻备份成功**。
    //   校验结论只改 integrity，不改 status。
    let mut final_manifest = m.clone();
    final_manifest.integrity = report.integrity;

    // ── 阶段 5：按保留策略裁剪（R-5）──
    deps.emit(BackupStage::Pruning, 0, 1);
    let pruned = prune(&root, deps.rule, created, deps);

    // 🔴 判据顺序即语义：**有失败 ⇒ Failed**，一个失败都不该标成功（R-13）。
    //    写成 `failures.is_empty()` 的正向形，是为了让「成功」成为默认臂——
    //    `if_not_else` 的本意就是提醒别把否定写成主条件。
    let final_status = if failures.is_empty() {
        SnapshotStatus::Success
    } else {
        SnapshotStatus::Failed
    };
    final_manifest.status = final_status;
    //🔴 清单二次落盘（原子写）：把校验结论与最终状态固化
    if let Err(error) = manifest::write_manifest(&snapshot_dir, &final_manifest) {
        log_error(
            super::reasons::MANIFEST_WRITE_FAILED,
            &error.to_string(),
            &deps.correlation_id,
        );
    }

    let info = SnapshotInfo {
        rule_id: deps.rule.id.clone(),
        rule_name: deps.rule.name.clone(),
        created_unix_ms: created,
        file_count: final_manifest.file_count,
        total_bytes: final_manifest.total_bytes,
        status: final_status,
        integrity: report.integrity,
        reused_count: 0,
        failed_count: failures.len() as u64,
        display_path: manifest::display_path_of(&snapshot_dir),
    };
    let _ = manifest::write_snapshot_info(&snapshot_dir, &info);

    deps.emit(BackupStage::Done, 1, 1);
    Ok(BackupOutcome {
        snapshot: Some((&info).into()),
        file_count: final_manifest.file_count,
        total_bytes: final_manifest.total_bytes,
        status: final_status,
        integrity: report.integrity,
        failure_count: failures.len() as u64,
        // 🔴 R-13：失败明细最多前 10 条，**并注明总数**
        failures: failures.into_iter().take(10).collect(),
        pruned,
        cancelled: false,
    })
}

/// 扫描全部来源（含R-8③ 内侧来源剔除）。
fn scan_all(rule: &BackupRule, deps: &EngineDeps<'_>) -> ScanResult {
    let sources = drop_contained_sources(&rule.sources);
    let matcher = ExclusionMatcher::new(&rule.exclusions);
    let mut out = ScanResult::default();
    for source in &sources {
        let (files, empty_dirs, skipped) = scan_source(source, &matcher);
        if files.is_empty() && empty_dirs.is_empty() {
            out.skipped_sources.push(source.path.clone());
            continue;
        }
        // 🔴 多来源合并时**必须**加来源前缀（R-9：否则两个来源的同名文件互相覆盖）
        let prefix = super::scan::source_display_name(&source.path);
        for mut file in files {
            file.relative = format!("{prefix}/{}", file.relative);
            out.total_bytes = out.total_bytes.saturating_add(file.size);
            out.files.push(file);
        }
        for dir in empty_dirs {
            out.empty_dirs.push(format!("{prefix}/{dir}"));
        }
        out.skipped_links.extend(skipped);
    }
    let _ = deps;
    out
}

/// 保留策略裁剪（F-FB-31 / R-5）。
///
/// 🔴 三条硬约束在 [`plan_retention`] 里；此处只负责**执行**与**逐条留痕**。
fn prune(root: &Path, rule: &BackupRule, newest_unix_ms: i64, deps: &EngineDeps<'_>) -> Vec<i64> {
    let (dirs, _unparsed) = manifest::list_snapshot_dirs(root, &rule.id);
    let candidates: Vec<RetentionCandidate> = dirs
        .iter()
        .map(|d| RetentionCandidate {
            created_unix_ms: Some(d.created_unix_ms),
            is_newest: d.created_unix_ms == newest_unix_ms,
        })
        .collect();
    let decision = plan_retention(&candidates, rule.retention);
    for (index, reason) in &decision.skipped {
        if let Some(dir) = dirs.get(*index) {
            log_info(
                "保留策略",
                &format!(
                    "跳过清理 {}：{reason}",
                    manifest::display_dir_name(&dir.dir_name)
                ),
                &deps.correlation_id,
            );
        }
    }
    let mut pruned = Vec::new();
    for index in decision.delete {
        let Some(dir) = dirs.get(index) else { continue };
        // 🔴 R-5：删除失败即**停止**后续删除（避免连锁误删）
        match std::fs::remove_dir_all(&dir.dir) {
            Ok(()) => {
                pruned.push(dir.created_unix_ms);
                log_info(
                    "保留策略",
                    &format!(
                        "已自动清理最旧快照：{}",
                        manifest::display_dir_name(&dir.dir_name)
                    ),
                    &deps.correlation_id,
                );
            }
            Err(error) => {
                log_error(
                    super::reasons::RETENTION_CLEANUP_FAILED,
                    &format!("{error}"),
                    &deps.correlation_id,
                );
                break;
            }
        }
    }
    pruned
}

/// 取消时的收尾（R-14）。
fn finish_cancelled(
    snapshot_dir: &Path,
    data_dir: &Path,
    entries: Vec<FileEntry>,
    failures: Vec<FailureView>,
    deps: &EngineDeps<'_>,
) -> BackupOutcome {
    // 🔴 R-14：清单已写完 ⇒ 备份其实已完成，保留该快照；否则清理半成品目录。
    //   本函数只在"写清单之前"被调用 ⇒ 一律清理。
    let _ = std::fs::remove_dir_all(data_dir);
    let _ = std::fs::remove_dir_all(snapshot_dir);
    log_info("备份", "已取消（半成品目录已清理）", &deps.correlation_id);
    let _ = (entries, failures);
    cancelled_outcome()
}

fn cancelled_outcome() -> BackupOutcome {
    BackupOutcome {
        snapshot: None,
        file_count: 0,
        total_bytes: 0,
        status: SnapshotStatus::Failed,
        //🔴 取消不是失败：完整性维度不表态（没有快照可校验）
        integrity: IntegrityStatus::NotVerified,
        failures: Vec::new(),
        failure_count: 0,
        pruned: Vec::new(),
        cancelled: true,
    }
}

/// 硬失败（闸门类；**与"部分成功"严格区分**，R-13）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HardFailure {
    /// 🔴 R-7 重叠 / 无法归一化。
    Overlap(&'static str),
    /// 🔴 K-002 未拍板却勾了卷影。
    ShadowUnavailable,
    /// 🔴 R-6 空间不足。
    InsufficientSpace { code: &'static str, detail: String },
    /// 没有任何可备份内容。
    NothingToBackup(Vec<String>),
    /// 快照目录创建失败。
    SnapshotDirFailed(String),
    /// 🔴 R-18 清单写入失败，**数据目录已保留**。
    ManifestWriteFailed {
        detail: String,
        snapshot_dir: String,
    },
}

impl std::fmt::Display for HardFailure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Overlap(code) => write!(
                f,
                "{}（可能导致递归自我备份，请把备份目录设到来源之外）",
                super::reasons::describe(code).unwrap_or("来源与落点重叠")
            ),
            Self::ShadowUnavailable => {
                f.write_str("卷影能力不可用（形态待定），请取消勾选后再备份")
            }
            Self::InsufficientSpace { code, detail } => write!(
                f,
                "{}：{detail}",
                super::reasons::describe(code).unwrap_or("磁盘空间不足")
            ),
            Self::NothingToBackup(sources) => {
                write!(
                    f,
                    "没有可备份的内容（来源不存在或全被排除）：{}",
                    sources.join("；")
                )
            }
            Self::SnapshotDirFailed(detail) => write!(f, "快照目录创建失败：{detail}"),
            Self::ManifestWriteFailed {
                detail,
                snapshot_dir,
            } => write!(
                f,
                "快照清单写入失败：{detail}；已保留数据目录待人工处理（{snapshot_dir}）"
            ),
        }
    }
}

impl std::error::Error for HardFailure {}

fn required_of(verdict: DiskVerdict) -> u64 {
    match verdict {
        DiskVerdict::Enough { required, .. }
        | DiskVerdict::Insufficient { required, .. }
        | DiskVerdict::Unknown { required } => required,
    }
}

fn now_unix_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| i64::try_from(d.as_millis()).unwrap_or(i64::MAX))
}

/// 🔴 **结构化日志占位**：本仓 tracing subscriber 尚未安装（W0-10，M-05 已登记同一欠账），
/// 故 `tracing::*!` 目前空转 ⇒ 这里改用 `log` crate（R-6/R-11 要求的
/// 「显式警告留痕」在后端日志侧暂不可验证，**已登记为欠账**，不擅自安装 subscriber）。
fn log_warn(code: &str, text: &str, correlation_id: &str) {
    log::warn!("[backup][{correlation_id}][{code}] {text}");
}

fn log_error(code: &str, detail: &str, correlation_id: &str) {
    log::error!("[backup][{correlation_id}][{code}] {detail}");
}

fn log_info(stage: &str, text: &str, correlation_id: &str) {
    log::info!("[backup][{correlation_id}][{stage}] {text}");
}
