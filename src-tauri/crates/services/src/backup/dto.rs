//! Tauri command 的出入参 DTO（薄边界；`serde` 形状与 `models` 分离）。
//!
//! 🔴 **为什么不直接把 `models` 暴露给 command**：`models` 是落盘格式（`snake_case`，
//! 与母本 `SnakeCaseLower` 对齐），DTO 是线协议形状。前端拿到的 JSON 应是camelCase
//! （`src/types/generated` 的惯例），两者分开才能各自演进而不互相污染。
//!
//! 本批🟡 **走手写 DTO 而非 `ts-rs` 生成**：`contracts` 的生成链随 W0-07 落地
//! （CT-1/CT-2）。手写投影已登记欠账，生成链就位后由 `src/types/backup.ts` 替换。

use super::models::{
    BackupRule, BackupSettings, ConflictPolicy, IntegrityStatus, RetentionPolicy, SnapshotInfo,
    SnapshotStatus, VerifyReport,
};
use serde::{Deserialize, Serialize};

/// 规则视图（camelCase 线协议形状）。
// 🔴 `struct_excessive_bools` 豁免：`RuleView` 是**线协议 DTO**，字段名就是 UI 契约。
//   把 5 个开关折成 bitflags 会让前端读到的 JSON 变成不透明整数 ——
//   「界面显示的是不是用户勾的东西」这条链就断了，而它正是 R-11「状态诚实」的核心。
//   同口径豁免见 `models.rs` 的 `BackupRule`。
#[allow(clippy::struct_excessive_bools)]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RuleView {
    pub id: String,
    pub name: String,
    pub note: String,
    pub enabled: bool,
    /// 来源路径列表（已去引号/去重，F-FB-02）。
    pub sources: Vec<String>,
    /// 备份落点（空 = 全局根）。
    pub custom_root: String,
    /// 保留份数（GFS 生效时为 0）。
    pub keep_count: u32,
    /// 是否按时间深度配额（GFS）保留。
    pub use_gfs: bool,
    pub exclusions: Vec<String>,
    /// 每日定时时刻（`HH:mm`）；空 = 未启用。
    pub daily_time: String,
    pub last_scheduled_date: String,
    /// 是否勾选卷影（🟡 K-002 未拍板 ⇒ 恒为 false 且开关禁用）。
    pub use_shadow_copy: bool,
    /// 是否为草稿态（来源全不存在，F-FB-04）。
    pub is_draft: bool,
    /// **卷影能力可用状态**（F-FB-21 三态；🟡 本批恒为 `Unavailable`）。
    pub shadow_capability: ShadowCapability,
    /// **定时能力可用状态**（🟡 K-003 未拍板 ⇒ 恒为 `Undecided`）。
    pub schedule_capability: ScheduleCapability,
    pub order: u32,
    /// 该规则现有快照份数（**逐规则**口径）。
    ///
    /// 🔴 与页头汇总的 `RuleListView::snapshot_count` 是两回事：页头给总数，
    ///   本字段给「这条规则自己攒了几份」—— 用户判断「哪条规则该管」靠它，
    ///   不能靠总数除以条数。**派生值，不落盘**（落盘的目录才是事实源）。
    pub snapshot_count: u64,
    /// 最近一次备份时间（Unix 毫秒；`0` = 从未备份过）。
    ///
    /// 🔴 同样**派生值**。取所有快照目录名里 `created_unix_ms` 的最大者，
    ///   不读 `info.json` —— 列表页不该为了一行摘要去读 N 份元信息。
    pub latest_snapshot_at: i64,
}

/// 卷影能力三态（F-FB-21）。
///
/// 🔴 三态而非 bool：规格要求「不以'配置存在'就报可用」，且能力缺失时
/// 用户要能从状态理解「为什么勾了仍备不到被占用文件」。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ShadowCapability {
    /// 组件缺失（警告色）。
    ComponentMissing,
    /// 已配置待首次提权（成功色）。
    ConfiguredPendingElevation,
    /// 客户端不可用（危险色）。
    ClientUnavailable,
}

/// 定时能力状态（🟡 K-003 未拍板 ⇒ 只有 `Undecided` 一态会被返回）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ScheduleCapability {
    /// 形态待定（TASKS K-003 未拍板）。
    Undecided,
    /// 已登记每日任务。
    Registered,
    /// 未登记。
    NotRegistered,
}

/// 快照行视图（F-FB-26：只读轻量元数据）。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SnapshotView {
    pub rule_id: String,
    pub rule_name: String,
    /// 可读展示时间（Unix 毫秒；前端负责格式化）。
    pub created_unix_ms: i64,
    pub file_count: u64,
    pub total_bytes: u64,
    pub status: SnapshotStatus,
    pub integrity: IntegrityStatus,
    pub reused_count: u64,
    pub failed_count: u64,
    /// 展示路径（已隐藏并发去重后缀，F-FB-32）。
    pub display_path: String,
}

/// 备份阶段（F-FB-18：阶段文案必须与真实阶段一致）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum BackupStage {
    /// 建立时点视图（卷影；🟡 本批恒不走这条）。
    PreparingShadowView,
    /// 扫描来源。
    Scanning,
    /// 磁盘空间预检。
    CheckingSpace,
    /// 复制并计算内容指纹。
    Copying,
    /// 写快照清单。
    WritingManifest,
    /// 读回校验。
    Verifying,
    /// 按保留策略裁剪旧快照。
    Pruning,
    /// 已完成。
    Done,
}

/// 进度上报（F-FB-18：已处理 / 总数 + 百分比；🔴 由上层节流 ≥100ms）。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProgressView {
    pub rule_id: String,
    /// 批量备份时为 `[i/N] 规则名`（F-FB-17）。
    pub label: String,
    pub stage: BackupStage,
    pub processed: u64,
    pub total: u64,
    /// 0–100；`total == 0` 时为 0（🔴 不允许除零后展示 NaN）。
    pub percent: u8,
    /// 关联 ID（CT-5：由封装层注入，事件必带）。
    pub correlation_id: String,
}

/// 备份结果（一份快照的产出结论）。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BackupOutcome {
    /// 生成的快照（取消或失败时为 `None`）。
    pub snapshot: Option<SnapshotView>,
    pub file_count: u64,
    pub total_bytes: u64,
    pub status: SnapshotStatus,
    pub integrity: IntegrityStatus,
    /// 🔴 失败明细（最多前 10 条 + 总数，R-13）。
    pub failures: Vec<FailureView>,
    pub failure_count: u64,
    /// 清理掉的旧快照时间戳（F-FB-31「逐条留痕」）。
    pub pruned: Vec<i64>,
    /// 是否被用户取消（R-14：取消**不是**失败）。
    pub cancelled: bool,
}

/// 一条失败明细。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FailureView {
    /// 原因码（`reasons` 表内；🔴 未登记码原样透传）。
    pub code: String,
    /// 中文说明。
    pub text: String,
    /// 相关路径（可能为空）。
    pub path: String,
}

/// 恢复预演结果（F-FB-35：四项计数，预演与执行同判据）。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RestorePlanView {
    pub total: u64,
    /// 目标已存在（冲突数）。
    pub conflicts: u64,
    /// 会被安全守卫拒绝（R-2）。
    pub guard_rejected: u64,
    /// 全新写入。
    pub fresh_writes: u64,
    /// 目标位置（展示用）。
    pub target: String,
    /// 生效的冲突策略。
    pub policy: ConflictPolicy,
}

/// 恢复结果（F-FB-36：四项计数 + 明细）。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RestoreOutcome {
    pub succeeded: u64,
    pub failed: u64,
    /// 校验不一致（写入成功但读回比对失败）。
    pub verify_mismatched: u64,
    /// 跳过（含原因明细，F-FB-36）。
    pub skipped: u64,
    pub failures: Vec<FailureView>,
    pub skip_reasons: Vec<FailureView>,
    /// 重建的空目录数（R-10）。
    pub empty_dirs_rebuilt: u64,
}

/// 规则列表响应（页头汇总 F-FB-01「规则 N 条 · 快照 M 份」）。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RuleListView {
    pub rules: Vec<RuleView>,
    pub rule_count: u64,
    pub snapshot_count: u64,
    /// ⚠️ 规则记录损坏时的显式提示（R-18：先留痕再按可读部分继续，页面不空白）。
    pub warnings: Vec<String>,
}

/// 设置视图。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SettingsView {
    pub global_root: String,
    pub default_keep_count: u32,
    pub concurrency: u32,
    /// -1 关闭 / 0 全量 / >0 抽样条数（F-FB-29）。
    pub verify_sample: i64,
    /// 未变文件复用（🟡 本批恒 false：依赖 K-002，且母本自身也默认关闭）。
    pub reuse_unchanged: bool,
    pub default_conflict_policy: ConflictPolicy,
}

/// 导出结果（F-FB-14：成功须提示**条数与路径**）。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportView {
    /// 导出的规则条数。
    pub count: u64,
    /// 实际写入的绝对路径。
    pub path: String,
}

/// 导入结果（F-FB-15：如实报成功数 + 逐条问题明细）。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportView {
    /// 原地覆盖更新的条数。
    pub updated: u64,
    /// 新增追加的条数。
    pub appended: u64,
    /// 因来源集合与既有规则完全相同而改名的条数（⊆ `appended`）。
    pub renamed: u64,
    /// 逐条问题明细（改名、空名等；**禁止静默**）。
    pub issues: Vec<String>,
}

/// 实现 → DTO 的转换（集中在一处，避免字段漏改）。
impl From<&BackupRule> for RuleView {
    fn from(rule: &BackupRule) -> Self {
        Self {
            id: rule.id.clone(),
            name: rule.name.clone(),
            note: rule.note.clone(),
            enabled: rule.enabled,
            sources: rule.sources.iter().map(|s| s.path.clone()).collect(),
            custom_root: rule.custom_root().unwrap_or_default().to_owned(),
            keep_count: rule.retention.into_count(),
            use_gfs: matches!(rule.retention, RetentionPolicy::Gfs(_)),
            exclusions: rule.exclusions.clone(),
            daily_time: rule.daily_time.clone().unwrap_or_default(),
            last_scheduled_date: rule.last_scheduled_date.clone().unwrap_or_default(),
            use_shadow_copy: rule.use_shadow_copy,
            is_draft: rule.is_draft(),
            // 🔴 本批恒不可用：K-002 未拍板，不做任何"能力探测"（探测本身需要 VSS 客户端）
            shadow_capability: ShadowCapability::ClientUnavailable,
            // 🔴 K-003 未拍板 ⇒ 形态待定，如实上报
            schedule_capability: ScheduleCapability::Undecided,
            order: rule.order,
            // 🔴 这两个是**派生值**，只能由 `backup_list_rules` 扫目录后回填
            //   （`From` 这一层拿不到 IO 上下文）。此处给 0 = 「未知」，
            //   前端据此显示占位而不是伪装成「0 份 / 从未备份」。
            snapshot_count: 0,
            latest_snapshot_at: 0,
        }
    }
}

impl From<&SnapshotInfo> for SnapshotView {
    fn from(info: &SnapshotInfo) -> Self {
        Self {
            rule_id: info.rule_id.clone(),
            rule_name: info.rule_name.clone(),
            created_unix_ms: info.created_unix_ms,
            file_count: info.file_count,
            total_bytes: info.total_bytes,
            status: info.status,
            integrity: info.integrity,
            reused_count: info.reused_count,
            failed_count: info.failed_count,
            display_path: info.display_path.clone(),
        }
    }
}

impl From<&BackupSettings> for SettingsView {
    fn from(s: &BackupSettings) -> Self {
        Self {
            global_root: s.global_root.clone(),
            default_keep_count: s.default_keep_count,
            concurrency: s.concurrency,
            verify_sample: s.verify_sample,
            reuse_unchanged: s.reuse_unchanged,
            default_conflict_policy: s.default_conflict_policy,
        }
    }
}

/// 校验报告视图（手动校验 F-FB-28 / 自动校验回显）。
///
/// 🔴 **刻意不为 `VerifyReport` 单独实现 `From`**：一旦实现就会被误用成跨层直传，
/// 而本域的纪律是「线协议形状（`camelCase`）与落盘形状（`snake_case`）分离」。
/// 走这条转换的唯一入口是下面的 `From<&VerifyReport> for VerifyReportView`。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VerifyReportView {
    pub total: u64,
    /// 🔴 实际比对数（抽样时小于 total，**必须如实展示**，F-FB-29）。
    pub compared: u64,
    pub matched: u64,
    pub mismatched: Vec<String>,
    pub missing: Vec<String>,
    pub integrity: IntegrityStatus,
}

impl From<&VerifyReport> for VerifyReportView {
    fn from(r: &VerifyReport) -> Self {
        Self {
            total: r.total,
            compared: r.compared,
            matched: r.matched,
            mismatched: r.mismatched.clone(),
            missing: r.missing.clone(),
            integrity: r.integrity,
        }
    }
}
