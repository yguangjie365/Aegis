//! 传输历史（F-FT-54 ~ F-FT-58）。
//!
//! 复用结论（DP-1）：③思路复用 ←
//! `Source/SystemToolkit/src/SystemToolkit.Core/FileTransfer/` 的历史记录
//! （GPL-3.0 同源，ADR-003 五步登记见 `Aegis/DEPENDENCIES.md §三`）。
//! 方向/状态枚举复用本仓 [`super::tasks`]（同一套状态语义，不另造一份）。
//!
//! ## 三条硬约束
//!
//! 1. **上限 200 条**（F-FT-54）：超出丢**最旧**的。历史是"最近发生了什么"，
//!    不是审计日志——审计另有八字段日志那条线。
//! 2. **快照不可变**：条目写进去就不再被改写（与 [`super::tasks`] 的 R-16 同精神）。
//!    文件名/对端这些字段是**当时的**事实，不该随后续状态变化而变。
//! 3. **原因码原样保存**：历史里存的是**字符串码**（线协议契约，见 [`super::reasons`] 模块头），
//!    未知码也照存照显，不硬翻成"未知错误"（F-FT-31 末句）。
//!
//! ## CSV 的四个细节（都是"导出到 Excel 能直接看"的必要条件）
//!
//! * **UTF-8 BOM**：不加的话 Excel 打开中文列会乱码。
//! * **CRLF 行尾**：RFC 4180 规定，Excel 也认。
//! * **RFC 4180 转义**：含逗号/引号/换行的字段要整体加引号、内部引号翻倍。
//! * **表头固定**：`时间,方向,文件名,对端,大小(字节),状态,原因码,原因说明`（F-FT-58）。

use std::collections::VecDeque;
use std::path::Path;

use serde::{Deserialize, Serialize};

use super::reasons;
use super::tasks::{Direction, TaskState};

/// 历史容量上限（F-FT-54：200 条）。
pub const HISTORY_CAPACITY: usize = 200;

/// 历史文件名（与 `settings.json` 同目录）。
pub const HISTORY_FILE_NAME: &str = "history.json";

/// CSV 表头（F-FT-58 规定的列与顺序）。
pub const CSV_HEADER: &str = "时间,方向,文件名,对端,大小(字节),状态,原因码,原因说明";

/// 一条历史是**文件**还是**文本**（F-FT-35 的类型列）。
///
/// 🔴 规格里的 CSV 表头（F-FT-58）**没有「类型」这一列**，所以它**不进 CSV** ✓；
/// 它只用于界面上的「类型」列。不要顺手往 CSV 里加列——那会破坏已冻结的表头。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum HistoryKind {
    /// 文件传输（**也是缺省值**，见下方 `kind` 字段的 `serde(default)`）
    #[default]
    File,
    /// 文本消息（历史里只留 120 字预览，正文不落盘）
    Text,
}

/// 一条历史记录（**不可变快照**）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HistoryEntry {
    pub id: String,
    pub direction: Direction,
    /// 🔴 文件还是文本。
    ///
    /// `#[serde(default)]` 不是可选的：文本通道落地**之前**写下的历史文件里
    /// **没有这个字段**，而缺字段会让整份 JSON 解析失败 ⇒ 服务会退化成"空历史" ⇒
    /// **用户的旧历史被静默丢掉**。默认成 [`HistoryKind::File`] 正好符合事实
    /// （那些记录确实都是文件）。
    #[serde(default)]
    pub kind: HistoryKind,
    pub peer_label: String,
    pub file_name: String,
    pub file_size: u64,
    pub state: TaskState,
    /// 原因码（`None` = 成功/无原因）
    pub reason_code: Option<String>,
    /// 完成时刻（Unix 毫秒）
    pub finished_at_ms: i64,
}

impl HistoryEntry {
    /// 原因说明（走 [`reasons::display_text`]，**未知码原样返回**）。
    #[must_use]
    pub fn reason_text(&self) -> String {
        match &self.reason_code {
            Some(code) => reasons::display_text(code),
            None => String::new(),
        }
    }

    /// 方向的中文名（界面与 CSV 共用）。
    #[must_use]
    pub fn direction_label(&self) -> &'static str {
        match self.direction {
            Direction::Send => "发送",
            Direction::Receive => "接收",
        }
    }

    /// 状态的中文名（界面与 CSV 共用）。
    #[must_use]
    pub fn state_label(&self) -> &'static str {
        match self.state {
            TaskState::Active => "进行中",
            TaskState::Completed => "完成",
            TaskState::Skipped => "已跳过",
            TaskState::Failed => "失败",
            TaskState::Cancelled => "已取消",
        }
    }
}

/// 时间戳 → `YYYY-MM-DD HH:MM:SS`（本地时区）。
///
/// 🔴 取值失败（越界的时间戳）时返回**原始毫秒串**而不是 `""`：
/// 空白会让人以为"这条没有时间"，而原始数字至少能查。
#[must_use]
pub fn format_time(ms: i64) -> String {
    chrono::DateTime::from_timestamp_millis(ms).map_or_else(
        || ms.to_string(),
        |utc| {
            utc.with_timezone(&chrono::Local)
                .format("%Y-%m-%d %H:%M:%S")
                .to_string()
        },
    )
}

/// 按 RFC 4180 转义一个 CSV 字段。
///
/// 🔴 含 `,` / `"` / 换行 / 回车的字段必须整体加引号并把内部 `"` 翻倍——
/// 否则文件名里的逗号会把一行撑成两列，导出的表就废了。
#[must_use]
pub fn csv_escape(field: &str) -> String {
    if field.contains(',') || field.contains('"') || field.contains('\n') || field.contains('\r') {
        format!("\"{}\"", field.replace('"', "\"\""))
    } else {
        field.to_owned()
    }
}

/// 历史表（运行期 + 落盘）。
#[derive(Debug, Clone, Default)]
pub struct History {
    entries: VecDeque<HistoryEntry>,
}

impl History {
    #[must_use]
    pub fn new() -> Self {
        Self {
            entries: VecDeque::new(),
        }
    }

    /// 从落盘内容恢复；超容量的**只保留最新的 200 条**（截尾，不报错）。
    #[must_use]
    pub fn from_entries(entries: impl IntoIterator<Item = HistoryEntry>) -> Self {
        let mut list: VecDeque<HistoryEntry> = entries.into_iter().collect();
        while list.len() > HISTORY_CAPACITY {
            list.pop_front();
        }
        Self { entries: list }
    }

    /// 追加一条；超容量时丢**最旧**的（F-FT-54）。
    ///
    /// 🔴 同一 `id` 重复追加时**覆盖**（不是新增一条）：一次传输只该在历史里占一行，
    /// 重放/重试不该留下两条互相矛盾的记录。
    pub fn push(&mut self, entry: HistoryEntry) {
        if let Some(existing) = self.entries.iter_mut().find(|item| item.id == entry.id) {
            *existing = entry;
            return;
        }
        self.entries.push_back(entry);
        while self.entries.len() > HISTORY_CAPACITY {
            self.entries.pop_front();
        }
    }

    /// 内部存储顺序 = **最旧在前**（追加在尾、超容量丢头，见 [`Self::push`]）。
    ///
    /// 🔴 **对外展示必须用 [`Self::newest_first`]**：F-FT-58 明写"保留最近 200 条（**新的在前**）"。
    /// 落盘用的就是这个内部顺序，**故它也是文件格式的一部分**——不要为了展示去反转存储，
    /// 那会让已经写下的历史文件被读成倒序。
    #[must_use]
    pub fn entries(&self) -> &VecDeque<HistoryEntry> {
        &self.entries
    }

    /// 展示顺序：**新的在前**（F-FT-58）。
    ///
    /// 界面列表与 CSV 都必须走这里——否则用户看到的与规格要求的方向相反。
    pub fn newest_first(&self) -> impl Iterator<Item = &HistoryEntry> {
        self.entries.iter().rev()
    }

    /// 清空全部历史，返回清掉的条数（0 = 本来就没有，界面据此说"暂无可清空记录"）。
    ///
    /// 🔴 只动历史：**进行中的任务在 [`super::tasks`] 的台账里，不受影响**（F-FT-57 原文）。
    /// 调用方负责"二次确认且明示不可恢复"——本函数不做确认，它是不可逆的。
    pub fn clear(&mut self) -> usize {
        let removed = self.entries.len();
        self.entries.clear();
        removed
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// 导出 CSV（含 BOM 与 CRLF，见模块头"CSV 的四个细节"；**新的在前**，F-FT-58）。
    #[must_use]
    pub fn to_csv(&self) -> String {
        let mut out = String::from("\u{feff}");
        out.push_str(CSV_HEADER);
        out.push_str("\r\n");
        for entry in self.newest_first() {
            let cells = [
                format_time(entry.finished_at_ms),
                entry.direction_label().to_owned(),
                entry.file_name.clone(),
                entry.peer_label.clone(),
                entry.file_size.to_string(),
                entry.state_label().to_owned(),
                entry.reason_code.clone().unwrap_or_default(),
                entry.reason_text(),
            ];
            let line: Vec<String> = cells.iter().map(|cell| csv_escape(cell)).collect();
            out.push_str(&line.join(","));
            out.push_str("\r\n");
        }
        out
    }

    /// 读盘。文件不存在 ⇒ 空历史（**不是错误**：首次运行就是这样）。
    ///
    /// # Errors
    /// 文件存在但读不动、或 JSON 解析失败时返回错误文案
    /// （🔴 不静默返回空表——那会让"历史莫名清空"变得无法排查）。
    pub fn load(path: &Path) -> Result<Self, String> {
        if !path.exists() {
            return Ok(Self::new());
        }
        let text = std::fs::read_to_string(path).map_err(|error| error.to_string())?;
        if text.trim().is_empty() {
            return Ok(Self::new());
        }
        let entries: Vec<HistoryEntry> =
            serde_json::from_str(&text).map_err(|error| error.to_string())?;
        Ok(Self::from_entries(entries))
    }

    /// 落盘。🔴 走 [`infra::fs::atomic_write`]（同目录 `.staging-` + fsync + rename），
    /// 与设置同一条路径——半截文件比没有文件更糟。
    ///
    /// # Errors
    /// 序列化或写盘失败时返回错误文案。
    pub fn save(&self, path: &Path) -> Result<(), String> {
        let entries: Vec<&HistoryEntry> = self.entries.iter().collect();
        let text = serde_json::to_string_pretty(&entries).map_err(|error| error.to_string())?;
        infra::fs::atomic_write(path, text.as_bytes()).map_err(|error| error.to_string())
    }
}

/// 历史文件的默认位置：与 `settings.json` **同目录**（`%LOCALAPPDATA%\aegis\transfer\`）。
///
/// 🔴 与设置同目录是刻意的：两者都是"这台机器的互传状态"，
/// 分开存放只会让备份与排查时漏掉一个。
///
/// 🔴 这里**就地写字面量**而不是引用 [`HISTORY_FILE_NAME`]：本域的架构守卫
/// （`transfer::guard`）禁止带非字面量参数的 `.join(`，与 `settings::default_settings_path`
/// 同一处置。两者一致由测试 `default_path_ends_with_the_constant` 钉住。
#[must_use]
pub fn default_history_path() -> std::path::PathBuf {
    // 🔴 数据目录可被 `AEGIS_TRANSFER_DATA_DIR` 覆盖：
    //    ① 用户可以整体搬走互传的数据（设置/历史/令牌）；
    //    ② **测试靠它把历史隔离到临时目录**——不然跑一次测试就会写脏用户的真实历史
    //       （2026-10-04 实际观测到过 `命令层传输.bin`、`拒绝我.bin` 落进真实文件）。
    if let Some(dir) = std::env::var_os("AEGIS_TRANSFER_DATA_DIR") {
        return std::path::PathBuf::from(dir).join("history.json");
    }
    let base = dirs::data_local_dir().unwrap_or_else(std::env::temp_dir);
    base.join("aegis").join("transfer").join("history.json")
}

#[cfg(test)]
mod tests;
