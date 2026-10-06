//! 每日定时能力边界 —— 🟡 **本批不实现**（TASKS K-003 未拍板）。
//!
//! 规格依据：F-FB-41~47（注册 / 注销 / 状态回显 / 无界面执行 / 错过补做 / 同日不重复）、
//! R-15（时间合法性口径统一）、R-16（只认成功的记账）、R-17（任务登记规则）。
//!
//! 🔴 **为什么纯逻辑部分照做、实现部分留空**：
//! K-003 待决的是**形态**（`schtasks` 外部命令 / Task Scheduler COM / 独立 worker 进程），
//! 而 R-15 的「时间合法性判据」是**纯逻辑**、与形态无关。规格明确要求
//! 「注册入口与到期判定**共用同一合法性判据**」—— 所以本模块把判据实现出来并测透，
//! 等形态拍板后只需补 `TaskRegistrar` 的实现类。
//!
//! **R-15 的踩坑背景（母本实测）**：格式对但越界（例如把小时填成 `25`）会导致
//! **永不到点且无提示**。所以合法性判定必须是「格式 + 范围」两关，且注册与判定
//! 调用**同一个**函数 —— 绝不允许两处各写一套。

use super::dto::ScheduleCapability;
use super::models::BackupRule;

/// 时间字段是否合法（R-15：`HH:mm` 且落在 00:00–23:59）。
///
/// 🔴 R-15 踩坑背景：格式对但越界（如小时填 `25`）会**永不到点且无提示**。
///    所以「格式合法」与「取值合法」是**两关**，任一不过即拒。
///
/// 🟠 接受 `H:mm` / `HH:mm` 两种形态（与 Windows `schtasks /ST` 的解析一致）：
///    用户在 UI 里很容易只输一位小时；把它判成非法会得到一句莫名其妙的报错。
///    🔴 但**只放宽前导零**，`9:5`（分钟未补零）仍拒 —— 那更像是笔误而非格式变体。
#[must_use]
pub fn parse_daily_time(raw: &str) -> Option<(u32, u32)> {
    let text = raw.trim();
    let (hour_text, minute_text) = text.split_once(':')?;
    // 小时 1–2 位、分钟**必须**2 位
    if !(1..=2).contains(&hour_text.len()) || minute_text.len() != 2 {
        return None;
    }
    // 🔴 必须逐字符判 ASCII 数字：`char::is_numeric` 会放过全角数字等 Unicode 数字，
    // 而 `scheduled_at` 只认 ASCII，导致「界面看着是数字、实际永远不执行」。
    if !hour_text.bytes().all(|b| b.is_ascii_digit())
        || !minute_text.bytes().all(|b| b.is_ascii_digit())
    {
        return None;
    }
    let hour: u32 = hour_text.parse().ok()?;
    let minute: u32 = minute_text.parse().ok()?;
    if hour > 23 || minute > 59 {
        return None;
    }
    Some((hour, minute))
}

/// 时间是否合法（`parse_daily_time` 的布尔包装，供 UI 提交前校验用）。
#[must_use]
pub fn is_valid_daily_time(raw: &str) -> bool {
    parse_daily_time(raw).is_some()
}

/// 到期判定：此刻是否应当执行这条规则的定时（F-FB-44 / R-15）。
///
/// 六道门，**任一不过即不执行**（规格原文「规则不存在或此刻不应执行则直接退出，
/// 不重复执行」）：
/// 1. 规则已启用（F-FB-07：停用规则不参与定时）
/// 2. 配置了每日定时
/// 3. 时间字段合法
/// 4. 此刻已到点
/// 5. 今日未记账（同日不重复 R-16 / F-FB-46）
/// 6. （`should_skip_for_bootstrap` 另行处理错过补做）
#[must_use]
pub fn should_run_now(rule: &BackupRule, now_unix_ms: i64, today: &str) -> bool {
    if !rule.enabled {
        return false;
    }
    let Some(raw) = rule.daily_time.as_deref() else {
        return false;
    };
    let Some((hour, minute)) = parse_daily_time(raw) else {
        return false;
    };
    if rule.last_scheduled_date.as_deref() == Some(today) {
        return false; // 同日不重复
    }
    let now_minutes = minute_of_day(now_unix_ms);
    now_minutes >= i64::from(hour) * 60 + i64::from(minute)
}

/// 本地时刻的「一天内第几分钟」（固定 UTC+8 偏移，理由同 [`crate::backup::retention`]）。
#[must_use]
pub fn minute_of_day(unix_ms: i64) -> i64 {
    let local = unix_ms.saturating_add(8 * 3_600 * 1_000);
    (local.rem_euclid(86_400)) / 60
}

/// 启动补做判定（F-FB-45：错过补做）。
///
/// 🔴 **只在「主程序被打开」时发生**（规格 §七·10：计算机整天没开机就一直不补）——
/// 这不是 bug，是本机任务计划模型下的固有限制，UI 须如实说明。
///
/// 判据：启用定时 + 时间合法 + **已过点** + 今日未记账。
#[must_use]
pub fn should_skip_for_bootstrap(rule: &BackupRule, now_unix_ms: i64, today: &str) -> bool {
    !should_run_now(rule, now_unix_ms, today)
}

/// 记账：只有「成功且未被取消」才记（R-16 / F-FB-45）。
///
/// 🔴 失败与取消**不记账** —— 留待下次机会重试。这条不能反：记了就等于
/// 「今天不用再试了」，而用户恰恰希望失败后还能补上。
#[must_use]
pub fn should_mark_executed(succeeded: bool, cancelled: bool) -> bool {
    succeeded && !cancelled
}

/// 今日日期串（`YYYY-MM-DD`，固定 UTC+8）。
#[must_use]
pub fn today_str(unix_ms: i64) -> String {
    let (year, month, day) = super::retention::civil_from_unix_ms(unix_ms);
    format!("{year:04}-{month:02}-{day:02}")
}

/// 每日任务的系统任务名（R-17：按规则标识生成；重复注册为**覆盖式**即幂等）。
#[must_use]
pub fn task_name(rule_id: &str) -> String {
    format!("Aegis\\FileBackup\\{rule_id}")
}

/// 系统任务调度器。
///
/// 🔴 契约同 [`super::vss::ShadowCopyProvider`]：**不得**依赖 Tauri 宿主类型（AD-4）——
/// 无界面执行形态由 `crates/worker` 复用本trait。
pub trait TaskRegistrar: Send + Sync {
    /// 探测注册状态（F-FB-43：🔴 查询不可用时**按"未注册"显示，不影响其他功能**）。
    fn query(&self, rule_id: &str) -> ScheduleCapability;

    /// 注册每日任务（覆盖式 ⇒ 幂等，R-17）。
    ///
    /// 🔴 时间非法时**必须**拒绝且**不注册**（R-15：格式对但越界会永不到点且无提示）。
    ///
    /// # Errors
    /// 权限不足 / 任务计划服务不可用 / 同名冲突 ⇒ 须带退出码或原因（🔴 绝不假装成功）。
    fn register(&self, rule: &BackupRule) -> Result<(), ScheduleError>;

    /// 注销（任务本就不存在视为成功并留痕，F-FB-42）。
    ///
    /// # Errors
    /// 权限不足 / 服务不可用。
    fn unregister(&self, rule_id: &str) -> Result<(), ScheduleError>;
}

/// 定时失败原因。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ScheduleError {
    /// 时间字段非法（🔴 **不注册**，因为注册了也永不到点）。
    InvalidTime { raw: String },
    /// 权限不足。
    PermissionDenied { code: i32 },
    /// 任务计划服务不可用。
    ServiceUnavailable { code: i32 },
    /// 其它失败（带退出码；🔴 退出码须过翻译表再呈现）。
    Failed { code: i32, detail: String },
}

impl std::fmt::Display for ScheduleError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidTime { raw } => {
                write!(f, "定时时刻非法（{raw}），需为 HH:mm 且 00:00–23:59")
            }
            Self::PermissionDenied { code } => write!(f, "权限不足（退出码 {code}）"),
            Self::ServiceUnavailable { code } => write!(f, "任务计划服务不可用（退出码 {code}）"),
            Self::Failed { code, detail } => write!(f, "注册失败（退出码 {code}）：{detail}"),
        }
    }
}

impl std::error::Error for ScheduleError {}

/// 🟡 本批唯一的实现：一律「形态待定」，注册/注销直接报形态待定。
///
/// 这是**如实上报**而非桩实现：UI 据此把开关显示为禁用 + 就地说明原因（F-FB-43），
/// 而不是显示「未注册」让用户以为可以点。
#[derive(Debug, Clone, Copy, Default)]
pub struct UndecidedTaskRegistrar;

impl TaskRegistrar for UndecidedTaskRegistrar {
    fn query(&self, _rule_id: &str) -> ScheduleCapability {
        ScheduleCapability::Undecided
    }

    fn register(&self, rule: &BackupRule) -> Result<(), ScheduleError> {
        // 🔴 先校时间：即便形态未定，**非法时间也必须被拒**（R-15 与形态无关）。
        // 顺序反了会让「永远不执行」的原因被误报成「形态待定」。
        let Some(raw) = rule.daily_time.as_deref() else {
            return Err(ScheduleError::InvalidTime { raw: String::new() });
        };
        if !is_valid_daily_time(raw) {
            return Err(ScheduleError::InvalidTime {
                raw: raw.to_owned(),
            });
        }
        Err(ScheduleError::Failed {
            code: -1,
            detail: "定时任务形态待定（TASKS K-003 未拍板）".to_owned(),
        })
    }

    fn unregister(&self, _rule_id: &str) -> Result<(), ScheduleError> {
        Err(ScheduleError::Failed {
            code: -1,
            detail: "定时任务形态待定（TASKS K-003 未拍板）".to_owned(),
        })
    }
}

#[cfg(test)]
mod tests;
