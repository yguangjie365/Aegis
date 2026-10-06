//! `vss` 单测：K-002 未拍板 ⇒ 能力**必须**恒为不可用，且不得被静默忽略。

use super::*;

#[test]
fn unavailable_provider_reports_client_unavailable_not_component_missing() {
    // 🔴 报「组件缺失」会暗示"装个 VSS 功能包就能用"，让用户白花钱。
    //    K-002 未决时的正确表述是「客户端不可用」。
    let provider = UnavailableShadowCopy;
    let probed = provider.probe();
    assert_eq!(probed, ShadowSupport::ClientUnavailable);
    assert_eq!(probed.to_capability(), ShadowCapability::ClientUnavailable);
    assert!(!probed.is_usable());
}

#[test]
fn create_views_always_fails_so_callers_fall_back_visibly() {
    // 🔴 R-4：任一卷建立失败 ⇒ **整体回退普通复制**并警告"被占用文件可能失败"，
    //    绝不静默降级。前提是 create_views 真的失败。
    let provider = UnavailableShadowCopy;
    let err = provider
        .create_views(&["C:".to_owned()])
        .expect_err("本批必须失败");
    assert_eq!(err, ShadowError::ComponentMissing);
    assert!(!err.to_string().is_empty(), "错误须有可读说明");
}

#[test]
fn support_maps_ready_to_configured_pending() {
    // Ready 与 ConfiguredPendingElevation 都映射为"可用"色（成功色），
    // 但只有 Ready 才 is_usable —— 前者尚未真正可用（待首次提权）。
    let ready = ShadowSupport::Ready {
        volumes: vec!["C:".to_owned()],
    };
    assert!(ready.is_usable());
    assert_eq!(
        ready.to_capability(),
        ShadowCapability::ConfiguredPendingElevation
    );

    let pending = ShadowSupport::ConfiguredPendingElevation;
    assert!(!pending.is_usable(), "待提权 ≠ 已就绪");
    assert_eq!(
        pending.to_capability(),
        ShadowCapability::ConfiguredPendingElevation
    );

    assert!(!ShadowSupport::ComponentMissing.is_usable());
}
