//! 失败原因码：机器可读码 + 中文说明成对携带。
//!
//! 复用结论（DP-1）：③ 思路复用 ← `Core/Backup/Services/` 各处的失败原因表达
//! （母本无集中码表，用 `IOException` 消息直出；本工程按 01 册「外部命令退出码必须过翻译表」
//! 与规格 §六「失败可读」的要求收成**码 + 中文**的成对结构，形态对齐 `transfer::reasons`）。
//!
//! 🔴 码是**跨端契约**：历史条目、操作日志、界面「原因」列共用同一套码。
//! **改动任一码的拼写即破坏兼容**（历史记录里存的是字符串），故本模块带码表稳定性测试。
//!
//! 🔴 **未知码原样显示、不吞、不硬翻**（对齐 `transfer::reasons` 的同款纪律）：
//! 母本 `Describe()` 对未知码原样返回码本身；本域的 [`describe`] 返回 `None`，
//! 调用方应把原始码直接展示，而不是替换成"未知错误"。

/// 一个失败原因码。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Reason {
    /// 机器可读码（全大写下划线；🔴 不得改写）。
    pub code: &'static str,
    /// 面向用户的中文说明（🔴 照规格措辞，不得由实现者改写）。
    pub text: &'static str,
}

// ── 码常量（供代码内引用，避免调用点散落字面量） ──────────────────────────

/// 来源路径不存在（F-FB-04草稿规则的执行结果）。
pub const SOURCE_NOT_FOUND: &str = "SOURCE_NOT_FOUND";
/// 备份根不可写或不存在（R-19 默认落点挑选失败）。
pub const BACKUP_ROOT_UNUSABLE: &str = "BACKUP_ROOT_UNUSABLE";
/// 目标磁盘空间不足（R-6「上浮 20% 后仍不足」）。
pub const INSUFFICIENT_DISK: &str = "INSUFFICIENT_DISK";
/// 磁盘空间无法确认（R-6：网络路径 / 驱动器未就绪—— **不阻断但必须显式警告**）。
pub const DISK_SPACE_UNKNOWN: &str = "DISK_SPACE_UNKNOWN";
/// 来源与落点重叠（R-7：互为包含 ⇒ 拒绝执行，防递归自我备份）。
pub const SOURCE_TARGET_OVERLAP: &str = "SOURCE_TARGET_OVERLAP";
/// 路径无法归一化比较（R-7 末句：**同样拒绝**，母本 2026-09-27 的纠正必须继承）。
pub const PATH_NOT_NORMALIZABLE: &str = "PATH_NOT_NORMALIZABLE";
/// 文件读取失败（最常见：被占用；R-4 备份侧默认行为）。
pub const FILE_READ_FAILED: &str = "FILE_READ_FAILED";
/// 文件写入失败。
pub const FILE_WRITE_FAILED: &str = "FILE_WRITE_FAILED";
/// 内容指纹不一致（F-FB-28 校验不通过）。
pub const HASH_MISMATCH: &str = "HASH_MISMATCH";
/// 清单写入失败（🔴 R-18：此时**保留数据目录**并报「已保留待人工处理」）。
pub const MANIFEST_WRITE_FAILED: &str = "MANIFEST_WRITE_FAILED";
/// 规则记录损坏（R-18：先另存留痕再按可读部分继续，页面不空白）。
pub const RULE_STORE_CORRUPTED: &str = "RULE_STORE_CORRUPTED";
/// 快照清单缺失或损坏（规格 §七·2：报告「清单读取失败」）。
pub const MANIFEST_READ_FAILED: &str = "MANIFEST_READ_FAILED";
/// 用户主动取消（R-14：🔴 记「已取消」而**非**「失败」，且不记定时已执行）。
pub const USER_CANCELLED: &str = "USER_CANCELLED";
/// 保留策略裁剪失败（R-5：出现即**停止**本轮后续删除，避免连锁误删）。
pub const RETENTION_CLEANUP_FAILED: &str = "RETENTION_CLEANUP_FAILED";
/// 目标快照不存在（F-FB-27/30：目录已被手动删除 — 母本同款场景）。
pub const SNAPSHOT_NOT_FOUND: &str = "SNAPSHOT_NOT_FOUND";
/// 打开快照目录失败（F-FB-27：无权限 / 系统文件管理器不可用）。
pub const OPEN_DIR_FAILED: &str = "OPEN_DIR_FAILED";
/// 恢复目标被安全守卫拒绝（R-2：疑似快照数据被篡改）。
pub const GUARD_REJECTED: &str = "GUARD_REJECTED";
/// 恢复写入失败。
pub const RESTORE_WRITE_FAILED: &str = "RESTORE_WRITE_FAILED";
/// 卷影能力不可用（🟡 K-002 未拍板 ⇒ 本批恒为不可用；F-FB-21 三态显示）。
pub const SHADOW_COPY_UNAVAILABLE: &str = "SHADOW_COPY_UNAVAILABLE";
/// 定时形态待定（🟡 K-003 未拍板 ⇒ 本批不注册系统任务；F-FB-43）。
pub const SCHEDULE_FORM_UNDECIDED: &str = "SCHEDULE_FORM_UNDECIDED";

/// 码表全集（🔴 与 `ALL` 一一对应，改一处必须改另一处——有稳定性测试钉住）。
pub const ALL: &[Reason] = &[
    Reason {
        code: SOURCE_NOT_FOUND,
        text: "来源路径不存在",
    },
    Reason {
        code: BACKUP_ROOT_UNUSABLE,
        text: "备份根不可用",
    },
    Reason {
        code: INSUFFICIENT_DISK,
        text: "目标磁盘空间不足",
    },
    Reason {
        code: DISK_SPACE_UNKNOWN,
        text: "无法确认目标磁盘剩余空间",
    },
    Reason {
        code: SOURCE_TARGET_OVERLAP,
        text: "来源与备份落点存在重叠",
    },
    Reason {
        code: PATH_NOT_NORMALIZABLE,
        text: "路径无法归一化比较",
    },
    Reason {
        code: FILE_READ_FAILED,
        text: "文件读取失败",
    },
    Reason {
        code: FILE_WRITE_FAILED,
        text: "文件写入失败",
    },
    Reason {
        code: HASH_MISMATCH,
        text: "内容指纹不一致",
    },
    Reason {
        code: MANIFEST_WRITE_FAILED,
        text: "快照清单写入失败",
    },
    Reason {
        code: RULE_STORE_CORRUPTED,
        text: "规则记录损坏",
    },
    Reason {
        code: MANIFEST_READ_FAILED,
        text: "快照清单读取失败",
    },
    Reason {
        code: USER_CANCELLED,
        text: "已取消",
    },
    Reason {
        code: RETENTION_CLEANUP_FAILED,
        text: "保留策略清理失败",
    },
    Reason {
        code: SNAPSHOT_NOT_FOUND,
        text: "快照不存在或已被删除",
    },
    Reason {
        code: OPEN_DIR_FAILED,
        text: "打开目录失败",
    },
    Reason {
        code: GUARD_REJECTED,
        text: "被安全守卫拒绝",
    },
    Reason {
        code: RESTORE_WRITE_FAILED,
        text: "恢复写入失败",
    },
    Reason {
        code: SHADOW_COPY_UNAVAILABLE,
        text: "卷影能力不可用",
    },
    Reason {
        code: SCHEDULE_FORM_UNDECIDED,
        text: "定时任务形态待定",
    },
];

/// 查中文说明。**未知码返回 `None`** —— 调用方须原样展示码本身，不得替换为"未知错误"。
#[must_use]
pub fn describe(code: &str) -> Option<&'static str> {
    ALL.iter().find(|r| r.code == code).map(|r| r.text)
}

/// 原因码的线协议形态：`(code, text)` 成对，供 DTO 直接携带。
#[must_use]
pub fn pair(code: &'static str) -> (&'static str, &'static str) {
    match ALL.iter().find(|r| r.code == code) {
        Some(r) => (r.code, r.text),
        // 🔴 传了未登记的码：返回码本身当说明（对齐母本 `Describe()` 的不吞语义），
        // 而不是 panic 或回一个无意义的"未知错误"。
        None => (code, code),
    }
}

#[cfg(test)]
mod tests;
