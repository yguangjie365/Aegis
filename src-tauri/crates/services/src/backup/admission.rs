//! 备份准入闸门：磁盘空间三态判档+ 来源/落点重叠守卫（F-FB-22/23 / **R-6 / R-7**）。
//!
//! 复用结论（DP-1）：
//! - **③ 思路复用** ← `Core/Backup/Services/BackupService.cs:637-682`
//!   （`ValidateNoOverlap` + 空间预检）。**母本 2026-09-27 的两处纠正必须原样继承**：
//!   ① 「路径无法归一化」从**放行**改为**阻断**（放行 = 递归自我备份导致无限膨胀）；
//!   ② 饱和加法防溢出（`checked_add` 溢出后回退 int.MaxValue，母本实测踩过）。
//! - **① 直接消费本工程既有**：空间取数走 `infra::fs::available_bytes`（FFI 只能在 infra，02 册 KN-04），
//!   路径比较走 `infra::safe::is_lexically_under`。**本模块不新�� FFI、不另写路径比较。**

use std::path::Path;

/// 空间预检结论（**三态**，R-6）。
///
/// 🔴 为什么必须三态：把「查不到」当成「充足」就是在赌用户的磁盘。
/// 规格原文：「无法确认（网络路径 / 驱动器未就绪）⇒ 不阻断但必须显式警告存在写爆风险，
/// **禁止把'未知'当'充足'**」。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiskVerdict {
    /// 剩余空间 ≥ 需求（满足上浮后）。
    Enough { required: u64, available: u64 },
    /// 剩余空间 < 需求 ⇒ 立即中止。
    Insufficient { required: u64, available: u64 },
    /// **无法确认** ⇒ 不阻断，但调用方必须落warn 日志并在 UI 上如实告知风险。
    Unknown { required: u64 },
}

impl DiskVerdict {
    /// 是否阻断执行。
    #[must_use]
    pub fn blocks(self) -> bool {
        matches!(self, Self::Insufficient { .. })
    }

    /// 是否需要在 UI 上显式警告（🔴 `Unknown` 恒需警告，不能静默放行）。
    #[must_use]
    pub fn needs_warning(self) -> bool {
        matches!(self, Self::Unknown { .. })
    }

    /// 原因码 + 中文说明（供日志与失败明细）。
    ///
    /// 🔴 `Enough` **不产生失败码**（返回空串）—— 够用不是失败，套上"空间不足"的码
    /// 会让日志面板出现自相矛盾的记录。
    #[must_use]
    pub fn reason(self) -> (&'static str, &'static str) {
        match self {
            Self::Enough { .. } => ("", "目标磁盘空间充足"),
            Self::Insufficient { .. } => (super::reasons::INSUFFICIENT_DISK, "目标磁盘空间不足"),
            Self::Unknown { .. } => (
                super::reasons::DISK_SPACE_UNKNOWN,
                "无法确认目标磁盘剩余空间",
            ),
        }
    }
}

/// 空间预检的安全余量倍数（R-6：按预估体量**上浮 20%**）。
///
/// 🔴 20% 覆盖的是「清单、目录项、临时件」的开销 —— 母本用 128 MiB 绝对值，
///   但绝对值对「备份几百 MB 小文件」的盘不够（目录项本身就很占），对「备份 2 TB 单文件」
///   又过剩。取两者的**相对**口径：小体量再加绝对下限。
///
/// 🔴 **以分数表示而非 `f64`**：见 [`required_with_headroom`] 的说明——浮点会丢精度。
///   分母 5 ⇒ 上浮 1/5 = 20%。
pub const SPACE_HEADROOM_DEN: u64 = 5;

/// 小体量的绝对余量下限（覆盖目录项与清单开销）。
pub const SPACE_HEADROOM_MIN: u64 = 64 * 1024 * 1024;

/// 计算「上浮后所需空间」（饱和加法，母本 2026-09-27 实测教训）。
///
/// 🔴🔴 **全程整数运算，不用 `f64`**：`raw_bytes as f64 * 0.20` 在 `raw_bytes > 2^53`
///   （约 9 PB）时会**静默丢精度**，算出的余量偏小；而这里正是「宁可少算也不能算错」
///   的安全判据（少算 = 空间不够时没拦住 = 写爆磁盘）。
///   改用 `x / 5`（= 20% 整除，天然饱和）后精度问题消失。
#[must_use]
pub fn required_with_headroom(raw_bytes: u64) -> u64 {
    // 🔴 饱和加法：`raw / 5` 天然不会溢出；三段加法各自 saturating
    let ratio_part = raw_bytes / SPACE_HEADROOM_DEN;
    raw_bytes
        .saturating_add(ratio_part)
        .saturating_add(SPACE_HEADROOM_MIN)
}

/// 空间预检。
///
/// # Errors
/// 不返回 `Err`：取数失败映射为 [`DiskVerdict::Unknown`]（🔴 这不是错误，是「不知道」）。
#[must_use]
pub fn check_disk_space(target_dir: &Path, raw_bytes: u64) -> DiskVerdict {
    let required = required_with_headroom(raw_bytes);
    match infra::fs::available_bytes(target_dir) {
        Ok(available) => {
            if available >= required {
                DiskVerdict::Enough {
                    required,
                    available,
                }
            } else {
                DiskVerdict::Insufficient {
                    required,
                    available,
                }
            }
        }
        // 🔴 查不到 ≠ 0，也 ≠ 充足
        Err(_) => DiskVerdict::Unknown { required },
    }
}

/// 重叠检查结论（R-7）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OverlapVerdict {
    /// 无重叠，可执行。
    Clear,
    /// 互为包含 ⇒ 必须拒绝（防递归自我备份）。
    Overlapping,
    /// 🔴 路径无法归一化比较 ⇒ **同样拒绝**（母本 2026-09-27 的纠正）。
    NotNormalizable,
}

impl OverlapVerdict {
    /// 是否阻断。
    #[must_use]
    pub fn blocks(self) -> bool {
        !matches!(self, Self::Clear)
    }

    /// 对应原因码。
    #[must_use]
    pub fn reason_code(self) -> &'static str {
        match self {
            Self::Clear => "",
            Self::Overlapping => super::reasons::SOURCE_TARGET_OVERLAP,
            Self::NotNormalizable => super::reasons::PATH_NOT_NORMALIZABLE,
        }
    }
}

/// 来源与落点重叠守卫（F-FB-23 / R-7）。
///
/// 🔴 **本检查必须在扫描之前**完成 —— 否则一个大目录要白扫一遍才报错（规格原文）。
///
/// 任一来源包含备份根、或备份根包含任一来源 ⇒ 拒绝。
#[must_use]
pub fn check_overlap(sources: &[String], backup_root: &Path) -> OverlapVerdict {
    // 备份根自身必须可归一化，否则无法做任何比较 ⇒ 按「无法归一化」拒绝
    let Some(root) = normalize_for_compare(backup_root) else {
        return OverlapVerdict::NotNormalizable;
    };

    for raw in sources {
        let path = Path::new(raw.trim().trim_matches('"'));
        if path.as_os_str().is_empty() {
            // 空来源在规则校验期就该被拒；走到这里说明记录损坏 ⇒ 拒绝而非跳过
            return OverlapVerdict::NotNormalizable;
        }
        let Some(source) = normalize_for_compare(path) else {
            return OverlapVerdict::NotNormalizable;
        };
        // 源包含根，或根包含源 —— 任一方向都算重叠
        if is_under(&root, &source) || is_under(&source, &root) {
            return OverlapVerdict::Overlapping;
        }
    }
    OverlapVerdict::Clear
}

/// 🔴 归一化到「可比较」形态：取绝对路径 + 去掉 `\\?\` 前缀 + 去掉结尾分隔符。
///
/// 之所以要归一化后再比：`D:\a` 与 `d:\A\`、`D:\a\` 指向同一目录，
/// 直接比字符串会把它们当成两个不同目录而**漏掉重叠**。
#[must_use]
pub fn normalize_for_compare(path: &Path) -> Option<std::path::PathBuf> {
    let absolute = if path.is_absolute() {
        path.to_path_buf()
    } else {
        // 🔴 这里的 join 是「相对路径 → 绝对路径」的 CWD 补全，**不是**
        //「外部路径拼到允许根之下」—— 后者才是 `join_under_root` 要管的事。
        // 无「允许根」参与，不产生可越界的落点；已登记进 `guard::ALLOWED`。
        std::env::current_dir().ok()?.join(path)
    };
    let stripped = infra::safe::strip_verbatim_prefix(&absolute);
    let text = stripped.to_string_lossy();
    // 去结尾分隔符：`D:\a\` 与 `D:\a` 同指；`D:\` 会被削成 `D:`（仍是合法的盘符根）
    let trimmed = text.trim_end_matches(['\\', '/']);
    if trimmed.is_empty() {
        None
    } else {
        Some(std::path::PathBuf::from(trimmed))
    }
}

/// `child` 是否在 `root` 之内或相等（大小写不敏感，Windows 语义）。
fn is_under(root: &Path, child: &Path) -> bool {
    infra::safe::is_lexically_under(root, child)
}

#[cfg(test)]
mod tests;
