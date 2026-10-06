//! 备份域的持久化模型（③ 思路复用 ← `Core/Backup/Models/`）。
//!
//! 🔴 **落盘字段名必须与母本一致**：母本 `BackupConfigService` / `RuleManager` / `SnapshotManager`
//! 序列化时统一用 `JsonNamingPolicy.SnakeCaseLower`（C#侧实测），故本模块所有落盘结构一律
//! `#[serde(rename_all = "snake_case")]`。改名会让既有用户数据读不出来——这正是R-18
//! 要求「记录损坏时先另存留痕而不静默重建」要防的损失。
//!
//! **时间统一 Unix 毫秒（i64）**：与 `infra::fs::set_modified_time(unix_ms)` 同口径，
//! 避免在域内出现第二套时间表示（跨层时间转换是 bug 温床）。

use serde::{Deserialize, Serialize};

/// 冲突策略（F-FB-34；`恢复`与`互传`各有同名概念但语义不同，🔴 不得互相推导，见 01 册 §2.2）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ConflictPolicy {
    /// 覆盖目标。
    Overwrite,
    /// 保留两者：自动追加 `" (n)"`（母本 `RestoreService` 同款）。
    RenameKeepBoth,
    /// 跳过。
    Skip,
    /// 每次询问。**数据层支持、界面未开放**（规格 §七·3 已知限制）。
    AskEach,
}

impl Default for ConflictPolicy {
    /// R-19：设置的默认冲突策略为非法值时回落 `RenameKeepBoth`（最不易毁数据的策略）。
    /// 🔴 不是`Overwrite` —— 默认覆盖是最危险的选择。
    fn default() -> Self {
        Self::RenameKeepBoth
    }
}

impl ConflictPolicy {
    /// 解析设置里的字符串；**非法值回落默认而非报错**（R-19「回落最不易毁数据的策略」）。
    #[must_use]
    pub fn from_setting(raw: &str) -> Self {
        match raw.trim().to_ascii_lowercase().as_str() {
            "overwrite" => Self::Overwrite,
            "skip" => Self::Skip,
            "ask_each" | "ask" => Self::AskEach,
            "rename" | "rename_keep_both" => Self::RenameKeepBoth,
            _ => Self::default(),
        }
    }

    /// 落盘用的稳定字符串（🔴 不得随界面文案改动）。
    #[must_use]
    pub fn as_setting(self) -> &'static str {
        match self {
            Self::Overwrite => "overwrite",
            Self::RenameKeepBoth => "rename_keep_both",
            Self::Skip => "skip",
            Self::AskEach => "ask_each",
        }
    }

    /// 「每次询问」在没有交互通道（无界面执行）时应降级为何种（R-34 末句）。
    #[must_use]
    pub fn degrade_without_prompt(self) -> Self {
        match self {
            // 母本同款降级：无闸道时降级为改名，绝不默认覆盖
            Self::AskEach => Self::RenameKeepBoth,
            other => other,
        }
    }
}

/// 保留策略的时间深度配额（F-FB-12 / R-5）。
///
/// 🟡 **当前无界面编辑入口**（规格 §七·7 已知限制）：只能通过记录字段生效。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub struct GfsRetention {
    /// 保留最近 N 天的每日快照。
    pub daily_keep: u32,
    /// 保留最近 N 周的周快照。
    pub weekly_keep: u32,
    /// 保留最近 N 月的月快照。
    pub monthly_keep: u32,
}

impl GfsRetention {
    /// R-5 硬上限：单项钳制到 100，防止被设成极大值导致无限累积。
    pub const PER_BUCKET_MAX: u32 = 100;

    /// 三项全 0 时语义是「只保最新一份」（R-5）。
    #[must_use]
    pub fn is_keep_latest_only(self) -> bool {
        self.daily_keep == 0 && self.weekly_keep == 0 && self.monthly_keep == 0
    }

    /// 钳制到合法区间（读入旧记录时调用，R-5「数值一律钳制在合法区间」）。
    #[must_use]
    pub fn clamped(self) -> Self {
        Self {
            daily_keep: self.daily_keep.min(Self::PER_BUCKET_MAX),
            weekly_keep: self.weekly_keep.min(Self::PER_BUCKET_MAX),
            monthly_keep: self.monthly_keep.min(Self::PER_BUCKET_MAX),
        }
    }
}

/// 保留策略二选一（F-FB-31/ R-5）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RetentionPolicy {
    /// 固定条数（1–100；留空取全局默认 7）。
    Count(u32),
    /// 时间深度配额。**一旦配置即优先于固定条数**。
    Gfs(GfsRetention),
}

impl RetentionPolicy {
    /// 固定条数硬上限（R-5「硬上限钳制，防止被设成极大值」）。
    pub const COUNT_MAX: u32 = 100;
    /// 固定条数下限（0 非法，最少 1）。
    pub const COUNT_MIN: u32 = 1;

    /// 全局默认保留份数（F-FB-11 规格默认值7）。
    pub const DEFAULT_COUNT: u32 = 7;

    /// 读入时钳制。`Count(0)` 钳到 1（规格 R-20：非法即就地提示并阻止保存，
    /// 这里是数据层的第二道防线 —— 第一道在 UI）。
    #[must_use]
    pub fn clamped(self) -> Self {
        match self {
            Self::Count(n) => Self::Count(n.clamp(Self::COUNT_MIN, Self::COUNT_MAX)),
            Self::Gfs(g) => Self::Gfs(g.clamped()),
        }
    }
}

impl Default for RetentionPolicy {
    fn default() -> Self {
        Self::Count(Self::DEFAULT_COUNT)
    }
}

/// 备份落点方式（F-FB-10 / R-19）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "mode", rename_all = "snake_case")]
pub enum BackupTarget {
    /// 写入全局备份根（`%LOCALAPPDATA%\Aegis\backups`，R-19：取剩余空间最大的本地固定磁盘）。
    GlobalRoot,
    /// 写入规则专属根。
    CustomRoot { root: String },
}

/// 来源类型（F-FB-02：文件夹与单个文件可混用）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SourceType {
    /// 一个文件夹（递归扫描）。
    Directory,
    /// 单个文件。
    File,
}

/// 一条来源路径（F-FB-02/ 03）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BackupSource {
    /// 规范化后的路径（去引号、去首尾空白）。
    pub path: String,
    /// 目录或文件。
    pub kind: SourceType,
}

impl BackupSource {
    /// 构造并**规范化**一条来源（F-FB-02：自动去引号、去首尾空白）。
    #[must_use]
    pub fn new(raw: &str, kind: SourceType) -> Self {
        Self {
            path: normalize_source_input(raw),
            kind,
        }
    }
}

/// 来源去重键（大小写不敏感 —— Windows 语义，F-FB-02「去重（大小写不敏感）」）。
#[must_use]
pub fn source_dedup_key(path: &str) -> String {
    path.trim().trim_matches('"').to_ascii_lowercase()
}

/// 规范化用户输入的路径：去首尾空白、去成对引号、合并重复分隔符。
///
/// 🔴 只做**无损**清洗：不做盘符补全、不做 `..` 解析、不猜相对路径的当前目录 ——
/// 那些是「猜」，而备份的信任属性高于便利（R-6「禁止把未知当充足」的同源要求）。
#[must_use]
pub fn normalize_source_input(raw: &str) -> String {
    let trimmed = raw.trim();
    let unquoted = if trimmed.len() >= 2 && trimmed.starts_with('"') && trimmed.ends_with('"') {
        &trimmed[1..trimmed.len() - 1]
    } else {
        trimmed
    };
    // 合并连续分隔符（`a\\b` → `a\b`）；保留 UNC 前导的 `\\`
    let mut out = String::with_capacity(unquoted.len());
    let mut prev_sep = false;
    for ch in unquoted.chars() {
        let is_sep = ch == '\\' || ch == '/';
        if is_sep && prev_sep && !out.is_empty() {
            continue;
        }
        if is_sep && out.is_empty() {
            out.push('\\');
            prev_sep = true;
            continue;
        }
        prev_sep = is_sep;
        out.push(ch);
    }
    // 去掉结尾分隔符（根目录 `D:\` 除外）
    let trimmed_end = out.trim_end_matches(['\\', '/']);
    if trimmed_end.len() >= 3 && trimmed_end.as_bytes()[1] == b':' {
        out = trimmed_end.to_owned();
    }
    out
}

/// 一条备份规则（F-FB-02~13）。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BackupRule {
    /// 规则标识（快照按它寻址，F-FB-05「改备份根后旧落点快照仍被识别」）。
    pub id: String,
    /// 规则名。
    pub name: String,
    /// 备注（F-FB-09：参与列表信息展示）。
    #[serde(default)]
    pub note: String,
    /// 是否启用（F-FB-07：停用规则不参与「备份全部」与定时）。
    pub enabled: bool,
    /// 来源列表（1..N，文件夹与单文件可混用）。
    pub sources: Vec<BackupSource>,
    /// 备份落点。
    pub target: BackupTarget,
    /// 保留策略。
    pub retention: RetentionPolicy,
    /// 排除式（F-FB-13：命中的文件与目录不参与备份；大小写不敏感）。
    #[serde(default)]
    pub exclusions: Vec<String>,
    /// 每日定时时刻（`HH:mm`）。`None` = 未启用定时。
    ///
    /// 🟡 K-003 未拍板 ⇒ 本批**不注册系统任务**；此字段仅做**纯逻辑**的到期判定
    /// （R-15「注册入口与到期判定共用同一合法性判据」的两侧共用同一函数）。
    #[serde(default)]
    pub daily_time: Option<String>,
    /// 最近一次定时执行日期（`YYYY-MM-DD`），用于同日不重复（R-16 / F-FB-46）。
    ///
    /// 🔴 只认「成功且未取消」的记账（R-16）；手工备份**不**写此字段。
    #[serde(default)]
    pub last_scheduled_date: Option<String>,
    /// 是否勾选使用卷影（F-FB-20）。
    ///
    /// 🟡 K-002 未拍板 ⇒ 本批**能力恒为不可用**，此开关恒禁用 + 就地说明原因（F-FB-21）。
    #[serde(default)]
    pub use_shadow_copy: bool,
    /// 规则排序位（F-FB-08：拖拽排序，持久化失败时界面须回退）。
    #[serde(default)]
    pub order: u32,
}

impl BackupRule {
    /// 规则是否处于「草稿」态（F-FB-04：来源路径全不存在时保存为草稿并显式警告）。
    #[must_use]
    pub fn is_draft(&self) -> bool {
        !self.sources.is_empty()
            && self
                .sources
                .iter()
                .all(|s| !std::path::Path::new(&s.path).exists())
    }

    /// 展开后的实际备份根（全局根由调用方注入 —— 它是全局设置，不在规则里）。
    ///
    /// R-19：未指定备份根时由调用方挑「剩余空间最大的本地固定磁盘」，本函数只处理
    /// 「规则显式指定了自定义根」这一路。
    #[must_use]
    pub fn custom_root(&self) -> Option<&str> {
        match &self.target {
            BackupTarget::CustomRoot { root } if !root.trim().is_empty() => Some(root.trim()),
            _ => None,
        }
    }
}

/// 完整性校验结论（F-FB-29 / **R-11 核心不变量**）。
///
/// 🔴 **三态 + 未校验，四值缺一不可**。规格 §六把「诚实性」列为数据安全红线：
/// 「禁止把'没查'写成'通过'」。若压成 bool，会把「抽样 200 条都过」显示成「通过」——
/// 那正是备份工具最不可接受的失真。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum IntegrityStatus {
    /// 逐文件读回比对**全部一致**，且本轮未使用复用条目 ⇒ 唯一可称「通过」的情形。
    Passed,
    /// 抽样比对通过，或本轮存在复用条目 —— 只能说「没发现坏，但不能证明全好」。
    NotFullyVerified,
    /// 存在不一致 / 缺失，或存在失败文件。
    Failed,
    /// 未做校验（校验档位为关闭）。
    NotVerified,
}

impl IntegrityStatus {
    /// 由「是否有失败项 / 是否全量比对 / 是否有复用条目」推导结论。
    ///
    /// 🔴 优先级 `failed > not_fully_verified > passed`（母本 `BackupService.cs:245-252` 同款
    /// 嵌套三元：`hasFailed ? Failed : (!allCompared || hasReused ? NotFullyVerified : Passed)`）。
    /// 顺序不可交换—— 失败文件存在时绝不能因为"其余都过了"而显示通过（R-13）。
    #[must_use]
    pub fn derive(has_failed: bool, all_compared: bool, has_reused: bool) -> Self {
        if has_failed {
            Self::Failed
        } else if !all_compared || has_reused {
            Self::NotFullyVerified
        } else {
            Self::Passed
        }
    }

    /// 是否可作为可信基准（R-11：`通过` 才可；`NotFullyVerified` 不足以支撑「可恢复」结论）。
    #[must_use]
    pub fn is_trustworthy(self) -> bool {
        self == Self::Passed
    }
}

/// 快照状态（F-FB-26 / R-13）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SnapshotStatus {
    /// 成功且清单完整。
    Success,
    /// **失败 = 清单不完整**（🔴 不是"部分成功即成功"；R-13「部分成功不是成功」）。
    /// 数据仍保留可用，但不能作为可信基准。
    Failed,
}

/// 清单里的一条文件记录（F-FB-28校验 / F-FB-36 恢复的最小单位）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FileEntry {
    /// 相对路径（🔴 恢复时**必须**经 `infra::safe::validate_relative` 复验，R-1）。
    pub relative_path: String,
    /// 原始绝对位置（仅供展示与追溯；🔴 恢复不得直接信任它落盘）。
    #[serde(default)]
    pub original_path: String,
    /// 字节数。
    pub size: u64,
    /// 修改时间（Unix 毫秒）。
    pub modified_unix_ms: i64,
    /// 内容指纹（SHA-256 小写十六进制）。
    pub sha256: String,
}

/// 快照的轻量元信息（F-FB-26：列表**只读轻量元数据**，不整份载入文件清单）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SnapshotInfo {
    /// 所属规则标识。
    pub rule_id: String,
    /// 所属规则名（冗余存一份，避免列表逐行回查规则）。
    pub rule_name: String,
    /// 快照时间（Unix 毫秒）。
    pub created_unix_ms: i64,
    /// 文件数。
    pub file_count: u64,
    /// 总体量（字节）。
    pub total_bytes: u64,
    /// 快照状态。
    pub status: SnapshotStatus,
    /// 完整性结论。
    pub integrity: IntegrityStatus,
    /// 本轮复用的条目数（0 = 本特性未启用；>0 时完整性必为 `NotFullyVerified`，R-12）。
    #[serde(default)]
    pub reused_count: u64,
    /// 失败项数（>0 ⇒ `status` 必为 `Failed`，R-13）。
    #[serde(default)]
    pub failed_count: u64,
    /// 快照目录**展示路径**（F-FB-26：展示时隐藏并发去重后缀，用户只看到可读时间）。
    pub display_path: String,
}

/// 完整快照清单（快照目录内的 `manifest.json`，F-FB-26「快照清单（每份快照的数据内容）」）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SnapshotManifest {
    /// 所属规则标识。
    pub rule_id: String,
    /// 所属规则名。
    pub rule_name: String,
    /// 快照标识（目录名去扩展名）。
    pub snapshot_id: String,
    /// 快照时间（Unix 毫秒）。
    pub created_unix_ms: i64,
    /// 来源路径集合（R-2③白名单回退时用；🔴 属**弱信任**，日志须标注）。
    pub sources: Vec<String>,
    /// 文件条目。
    pub entries: Vec<FileEntry>,
    /// 空目录列表（R-10：作为独立信息记入并在恢复时重建）。
    #[serde(default)]
    pub empty_dirs: Vec<String>,
    /// 文件数。
    pub file_count: u64,
    /// 总体量。
    pub total_bytes: u64,
    /// 状态。
    pub status: SnapshotStatus,
    /// 完整性结论。
    pub integrity: IntegrityStatus,
    /// 复用条数。
    #[serde(default)]
    pub reused_count: u64,
    /// 格式版本（🔴 便于将来字段演进时识别旧清单，而非静默按新语义读）。
    #[serde(default = "manifest_format_version")]
    pub format_version: u32,
}

/// 清单格式版本（🔴 便于将来字段演进时识别旧清单，而非静默按新语义读）。
pub const MANIFEST_FORMAT_VERSION: u32 = 1;

#[must_use]
pub fn manifest_format_version() -> u32 {
    MANIFEST_FORMAT_VERSION
}

impl SnapshotManifest {
    /// R-2③ 白名单取不到规则当前来源时，回退到快照自带的来源记录。
    ///
    /// 🔴 属**弱信任**：调用方必须落日志标注「已回退到快照自带来源」。
    #[must_use]
    pub fn source_whitelist(&self) -> &[String] {
        &self.sources
    }
}

/// 全局设置（F-FB-24/29/34 的设置项；`config/backup.json`）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BackupSettings {
    /// 全局备份根（空 = 由 R-19 挑剩余空间最大的本地固定磁盘）。
    #[serde(default)]
    pub global_root: String,
    /// 默认保留份数。
    #[serde(default = "default_keep_count")]
    pub default_keep_count: u32,
    /// 时间深度配额（可选；`None` = 只按固定条数）。
    #[serde(default)]
    pub gfs: Option<GfsRetention>,
    /// 并发工作数（1–8，默认 4）。
    #[serde(default = "default_concurrency")]
    pub concurrency: u32,
    /// 备份后读回校验档位：`-1` 关闭 / `0` 全量 / `>0` 抽样条数（默认 200）。
    #[serde(default = "default_verify_sample")]
    pub verify_sample: i64,
    /// 未变文件复用开关（**默认关**，R-12：时间戳可被人为改回会造成静默漏备）。
    #[serde(default)]
    pub reuse_unchanged: bool,
    /// 默认冲突策略（读入非法值时回落，R-19）。
    #[serde(default)]
    pub default_conflict_policy: ConflictPolicy,
}

const fn default_keep_count() -> u32 {
    7
}
const fn default_concurrency() -> u32 {
    4
}
const fn default_verify_sample() -> i64 {
    200
}

impl Default for BackupSettings {
    /// 🟡 K-002 未拍板 ⇒「使用卷影」不在设置里（本批能力恒不可用）。
    fn default() -> Self {
        Self {
            global_root: String::new(),
            default_keep_count: default_keep_count(),
            gfs: None,
            concurrency: default_concurrency(),
            verify_sample: default_verify_sample(),
            reuse_unchanged: false,
            default_conflict_policy: ConflictPolicy::default(),
        }
    }
}

impl BackupSettings {
    /// 并发数钳制到 1–8（规格 §五「并发工作数（1–8）」）。
    pub const CONCURRENCY_MAX: u32 = 8;

    /// 读入后钳制（R-19「保留份数、并发数等数值一律钳制在合法区间」）。
    #[must_use]
    pub fn clamped(self) -> Self {
        Self {
            default_keep_count: RetentionPolicy::Count(self.default_keep_count)
                .clamped()
                .into_count(),
            concurrency: self.concurrency.clamp(1, Self::CONCURRENCY_MAX),
            gfs: self.gfs.map(GfsRetention::clamped),
            ..self
        }
    }

    /// 校验档位归一：负值一律视为「关闭」（`-1` 是约定的关闭档）。
    ///
    /// 🔴 按**引用**取（不消耗 `self`）：这是纯查询，调用方往往还要用 `settings` 的别的字段。
    #[must_use]
    pub fn verify_mode(&self) -> VerifyMode {
        match self.verify_sample {
            m if m < 0 => VerifyMode::Off,
            0 => VerifyMode::Full,
            // 🔴 负值已在上一臂拦下，此处 try_from 不会失败；用 unwrap_or(0)
            //    而非 as：若将来臂序被调换，as 会静默绕出天文数字的抽样条数。
            n => VerifyMode::Sample(u64::try_from(n).unwrap_or(0)),
        }
    }
}

impl RetentionPolicy {
    /// 取固定条数（GFS 时返回 0 —— 调用方须先判 `is_gfs`）。
    #[must_use]
    pub fn into_count(self) -> u32 {
        match self {
            Self::Count(n) => n,
            Self::Gfs(_) => 0,
        }
    }
}

/// 读回校验档位（F-FB-29）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VerifyMode {
    /// 关闭 ⇒ 结论记`NotVerified`，**不得**记 `Passed`（R-11）。
    Off,
    /// 全量逐文件比对 ⇒ 只有这条路可能给出 `Passed`。
    Full,
    /// 抽样 N 条 ⇒ 结论最多到 `NotFullyVerified`。
    Sample(u64),
}

/// 校验结论明细（F-FB-28：报告总数 / 一致 / 不一致 / 缺失 / 实际比对数）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VerifyReport {
    /// 清单里的条目总数。
    pub total: u64,
    /// 实际参与比对的数量（🔴 抽样时**小于** total，这个数必须如实展示）。
    pub compared: u64,
    /// 指纹一致。
    pub matched: u64,
    /// 指纹不一致（逐条列明，F-FB-28）。
    pub mismatched: Vec<String>,
    /// 清单里有、实际读不到（文件被手动删）。
    pub missing: Vec<String>,
    /// 结论。
    pub integrity: IntegrityStatus,
}

impl VerifyReport {
    /// 由比对结果推导结论。
    ///
    /// 🔴 `all_compared` 由 `compared == total` 得出，抽样时必然 false ⇒ 自动降级为
    /// `NotFullyVerified`，与 R-11 一致（**抽样通过只能说"没发现坏"，不能说"全好"**）。
    ///
    /// `mismatched` / `missing` 直接收**具体条目**（F-FB-28 要求逐条列明），
    /// 计数由`len()` 得到，不另设字段——避免两处真相不一致。
    #[must_use]
    pub fn derive(
        total: u64,
        compared: u64,
        mismatched: Vec<String>,
        missing: Vec<String>,
    ) -> Self {
        let bad = (mismatched.len() + missing.len()) as u64;
        Self {
            total,
            compared,
            matched: compared.saturating_sub(bad),
            integrity: IntegrityStatus::derive(bad > 0, compared >= total, false),
            mismatched,
            missing,
        }
    }
}

#[cfg(test)]
mod tests;
