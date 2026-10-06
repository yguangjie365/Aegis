//! 卷影复制（VSS）能力边界 —— 🟡 **本批不实现**（TASKS K-002 未拍板）。
//!
//! 规格依据：F-FB-20/21（规则级开关 + 三态能力指示）、R-4（备份侧被占用文件）、
//! 规格 §七·11（需要额外组件与提权同意）。
//!
//! 🔴 **为什么留 trait 而不直接写实现**：VSS 的能力形态本身就是 K-002 的待决内容
//! ——「用 `windows` crate 直接调 COM还是在 `elevated-helper` 里经提权通道」
//! 这一条不定，写任何实现都可能白写。用户 2026-10-05 裁决为「留接口不实现」。
//!
//! **本 trait 的存在意义是让上层现在就写出正确的分支结构**：能力不可用时
//! 走哪条路、能报什么状态、UI 显示什么，全部在调用点显式表达；
//! 待 K-002 拍板后只需补一个实现类，**不改任何调用点**。
//!
//! **当前实现的诚实行为**（规格 F-FB-21：能力缺失时用户能从状态理解
//! 「为什么勾了仍备不到被占用文件」）：[`UnavailableShadowCopy`] 一律返回
//! [`super::dto::ShadowCapability::ClientUnavailable`]，备份照常进行，
//! 被占用文件计入失败项（R-4 备份侧默认行为）。

use super::dto::ShadowCapability;

/// 卷影能力探测结果（F-FB-21 三态）。
///
/// 🔴 不能derive `Copy`（`Ready` 带`Vec`）；调用方若需轻量传递用克隆。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ShadowSupport {
    /// 已就绪：可对给定卷建立时点视图。
    Ready { volumes: Vec<String> },
    /// 组件缺失（警告色）。
    ComponentMissing,
    /// 已配置待首次提权（成功色，但**尚未真正可用**）。
    ConfiguredPendingElevation,
    /// 客户端不可用（危险色）。
    ClientUnavailable,
}

impl ShadowSupport {
    /// 映射为线协议 DTO（UI 三态显示）。
    ///
    /// 🔴 按**引用**取（不消耗 `self`）：调用方常需在同一个值上再问`is_usable`。
    #[must_use]
    pub fn to_capability(&self) -> ShadowCapability {
        match self {
            Self::Ready { .. } | Self::ConfiguredPendingElevation => {
                ShadowCapability::ConfiguredPendingElevation
            }
            Self::ComponentMissing => ShadowCapability::ComponentMissing,
            Self::ClientUnavailable => ShadowCapability::ClientUnavailable,
        }
    }

    /// 是否可建立时点视图。
    ///
    /// 🔴 按**引用**取（不消耗 `self`）：调用方常需在同一个值上再问`to_capability`。
    #[must_use]
    pub fn is_usable(&self) -> bool {
        matches!(self, Self::Ready { .. })
    }
}

/// 一个已建立的时点视图。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ShadowView {
    /// 卷标识（GUID 的字符串形态）。
    pub volume_id: String,
    /// 可读路径（从该视图读取文件时用）。
    /// 🔴 实际取值须经 `infra::safe` 归一（`\\?\` 前缀）。
    pub path: std::path::PathBuf,
}

/// 卷影能力提供者。
///
/// 🔴 契约：本 trait **不得**依赖 Tauri 宿主类型（02 册 AD-4）——
/// worker / elevated-helper 都要复用它。
pub trait ShadowCopyProvider: Send + Sync {
    /// 探测能力（三态；F-FB-21：🔴 **不得**以"配置存在"就报可用）。
    fn probe(&self) -> ShadowSupport;

    /// 对每个涉及的卷建立时点视图。
    ///
    /// # Errors
    /// 任一卷建立失败 ⇒ 整批失败（**不是**部分成功）。
    /// 🔴 调用方**必须**据此整体回退普通复制并显式警告
    /// 「被占用文件可能失败」，**绝不静默降级**（R-4 备份侧）。
    fn create_views(&self, volumes: &[String]) -> Result<Vec<ShadowView>, ShadowError>;
}

/// 卷影失败原因。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ShadowError {
    /// 组件缺失。
    ComponentMissing,
    /// 需要提权但被拒绝（UAC 拒绝 = 安全终止 + 无副作用，02 册 §五）。
    ElevationDenied,
    /// 服务不可用（VSS 服务未运行）。
    ServiceUnavailable,
    /// 卷建立失败。
    VolumeFailed { volume: String },
}

impl std::fmt::Display for ShadowError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ComponentMissing => f.write_str("卷影组件缺失"),
            Self::ElevationDenied => f.write_str("提权被拒绝"),
            Self::ServiceUnavailable => f.write_str("卷影服务不可用"),
            Self::VolumeFailed { volume } => write!(f, "卷 {volume} 的时点视图建立失败"),
        }
    }
}

impl std::error::Error for ShadowError {}

/// 🟡 本批唯一的实现：一律不可用。
///
/// 这是**如实上报**而非占位桩：调用点拿到的 `ClientUnavailable` 会让 UI 显示
/// 「能力不可用」并禁用开关，用户勾不了也备份不了被占用文件 —— 符合
/// 「能力不可用时自动降级并显式警告，不假装成功」（规格 §一·3）。
#[derive(Debug, Clone, Copy, Default)]
pub struct UnavailableShadowCopy;

impl ShadowCopyProvider for UnavailableShadowCopy {
    fn probe(&self) -> ShadowSupport {
        // 🔴 K-002 未拍板 ⇒ 报「客户端不可用」而非「组件缺失」：
        // 后者暗示"装个组件就能用"，会让用户白花钱装 VSS 功能包。
        ShadowSupport::ClientUnavailable
    }

    fn create_views(&self, volumes: &[String]) -> Result<Vec<ShadowView>, ShadowError> {
        // 🔴 无条件失败：调用方据此整体回退普通复制（R-4）
        let _ = volumes;
        Err(ShadowError::ComponentMissing)
    }
}

#[cfg(test)]
mod tests;
