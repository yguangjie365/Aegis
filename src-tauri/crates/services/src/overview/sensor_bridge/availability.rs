//! 采集可用性五态分类（DIMM 温度，2026-10-07 用户裁决）。
//!
//! 🔴 **为什么需要这个模块**：用户裁决「DIMM 温度走按需提权、不得常驻高权限」，
//!   但同一句话里还有一条更重要的约束——**不伪造状态**。此前 `has_memory_temperature`
//!   是个 `bool`，只有「有」与「没有」两态，于是界面只能把「权限不足」「硬件不支持」
//!   「暂时读不到」三件事**一律显示成「未检测」**。这在只有一个传感器时无伤大雅，
//!   但一旦引入按需提权通道，它就变成有害的：用户看到一个笼统的「未检测」，
//!   无从判断「提权能不能解决」，从而永远不知道该做什么。
//!
//! 🔴 **本模块的铁律：`PermissionDenied` 只在有正向证据时才成立。**
//!   「读不到 DIMM 读数」这一现象**不足以**推出权限不足——它同样可能意味着
//!   内存控制器/固件根本不上报 TSOD。若把现象直接标成权限不足，就等于凭空
//!   给用户一个「提权即可解决」的错误承诺（用户裁决规则 4：不保证必然成功，
//!   也不伪造状态）。故本模块要求调用方从 sidecar 的诊断输出里取到
//!   **明确的权限失败字样**才允许给出 `PermissionDenied`，其余情况一律按
//!   可观测事实降级为 `Unsupported` / `Unavailable`。
//!
//! 🔴 **另一条同样重要的顺序纪律：`Unsupported` 优先于 `PermissionDenied`。**
//!   即便诊断里出现过权限字样，只要回包里**一个内存族节点都没有**，就必须报
//!   `Unsupported` —— 因为此时提权也造不出内存节点，报「权限不足」会让用户
//!   反复提权却永远读不到。这一点在写第一版时判断反了，由集成测试
//!   `permission_denied_only_appears_when_sidecar_said_so` 的第三条断言钉住。
//!
//!
//! 与 `badges.rs` 的关系：那边判「温度多少度算高」（R-09 分级），这边判
//! 「这个数字**能不能拿到**」。两者正交，不可互相替代。

/// 单项指标的采集可用性。
///
/// 🔴 序列化标签全小写（`permissionDenied` 等），前端 `src/types/overview.ts`
///   的 `MetricAvailability` 必须逐字对齐 —— 手写镜像类型，两端漂移无编译期防护。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MetricAvailability {
    /// 成功读到读数。
    Ok,
    /// 权限不足。**仅在有正向证据时给出**（见模块头注）。
    PermissionDenied,
    /// 硬件 / 固件 / 数据源不上报该项（不是权限问题，提权无用）。
    Unsupported,
    /// 本次读不到，但数据源本身存在该类传感器 —— 可能是瞬时失败、控制器
    /// 未就绪、或权限不足但未取到证据。
    Unavailable,
}

impl MetricAvailability {
    /// 是否拿到了真实读数（只有 `Ok` 算）。
    #[must_use]
    pub fn is_ok(self) -> bool {
        matches!(self, Self::Ok)
    }

    /// 兜底文案（**数据而非翻译**，故留在 Rust 侧；前端 i18n 只出各态的固定短语）。
    ///
    /// 🔴 措辞纪律：不得暗示「提权一定能解决」（用户裁决规则 4）。
    #[must_use]
    pub fn reason(self) -> &'static str {
        match self {
            Self::Ok => "已读取",
            // 「可能需要管理员权限」而非「需要管理员权限」—— 我们没有验证过提权
            // 一定能成功（AIDA64 能读到不等于本仓提权后能读到）。
            Self::PermissionDenied => "读取被拒绝，可能需要管理员权限",
            Self::Unsupported => "当前硬件/固件未提供该传感器",
            Self::Unavailable => "本次未能读取，稍后自动重试",
        }
    }
}

/// DIMM 采集的证据包。
///
/// 🔴 `permission_denied_hint` **只能来自 sidecar 自己的诊断输出**，不得由
///   「读不到读数」推导。留空即表示无证据，分类会落到 `Unsupported`/`Unavailable`。
#[derive(Debug, Clone, Copy, Default)]
pub struct DimmEvidence {
    /// sidecar 诊断输出里出现过明确的权限失败字样。
    pub permission_denied_hint: bool,
    /// 回包里存在 `hardwareType == "Memory"` 的**任意**传感器节点（不限温度）。
    ///
    /// 用来区分两种「没有内存温度」：连内存节点都没有 ⇒ 硬件/固件不上报；
    /// 有内存节点但没有 DIMM 温度读数 ⇒ 传感器存在但这一项读不到。
    pub has_memory_node: bool,
    /// 是否读到了合格的 DIMM 温度读数（`is_dimm_reading` 且非阈值型）。
    pub has_dimm_reading: bool,
}

impl DimmEvidence {
    /// 由聚合层从回包与诊断输出构造证据。
    #[must_use]
    pub fn collect(has_dimm_reading: bool, has_memory_node: bool, diagnostics: &[String]) -> Self {
        Self {
            permission_denied_hint: diagnostics.iter().any(|line| mentions_access_denied(line)),
            has_memory_node,
            has_dimm_reading,
        }
    }
}

/// 诊断文本里出现这些字样，才允许判定为「权限被拒」。
///
/// 🔴 这是**单向的充分条件**：命中即判 `PermissionDenied`，未命中**绝不**反推
///   「权限没问题」—— 未命中只意味着「无证据」，分类会退到更保守的态。
///
/// 覆盖中英文与 .NET 常见措辞：sidecar 是 C# 进程，异常消息走
/// `System.UnauthorizedAccessException` / `Win32Exception(5)` / 本地化文本。
#[must_use]
pub fn mentions_access_denied(line: &str) -> bool {
    const MARKS: [&str; 8] = [
        "unauthorizedaccess",
        "access is denied",
        "access denied",
        "拒绝访问",
        "权限不足",
        "需要管理员",
        "requires administrative",
        "requires admin",
    ];
    let lower = line.to_ascii_lowercase();
    MARKS.iter().any(|mark| lower.contains(mark))
}

/// 按证据分类 DIMM 采集状态。
///
/// 🔴 判定顺序即优先级，且 **`Unsupported` 必须排在 `PermissionDenied` 之前**
///   （2026-10-07 实测修正）：若先判权限，则「本机压根没有内存节点 + 诊断里恰好
///   出现过权限字样」会报成 `PermissionDenied` —— 而此时**提权也造不出内存节点**，
///   用户会反复提权却永远读不到。`Unsupported` 描述的是更根本的**硬件层缺失**，
///   它一旦成立就否决了「权限是原因」这一假设。
///
/// 顺序：`Ok` → `Unsupported`（硬件层缺失）→ `PermissionDenied`（唯一有证据的）→ `Unavailable`。
#[must_use]
pub fn classify_dimm(evidence: DimmEvidence) -> MetricAvailability {
    if evidence.has_dimm_reading {
        return MetricAvailability::Ok;
    }
    // 硬件层缺失优先于权限：有权限字样也改变不了「没有内存传感器」这个事实。
    if !evidence.has_memory_node {
        return MetricAvailability::Unsupported;
    }
    if evidence.permission_denied_hint {
        return MetricAvailability::PermissionDenied;
    }
    MetricAvailability::Unavailable
}

// 🔴🔴 **`Stale` 态与 `apply_staleness` 已于 2026-10-07 整体删除，请勿再加回来。**
///
/// 删除理由（真机截图 + 用户裁决「不要显示，容易造成误解，几秒后就恢复」）：
///
/// 1. **阈值无法可靠估计。** 我当初按「3 倍 2 秒节拍」拍了个 6 秒，但传感器
///    的实际更新周期**不由前端节拍决定，而由 sidecar 读耗时支配**：
///    `FIRST_READ_BUDGET = OPEN_BUDGET(15s) + READ_TIMEOUT(8s) = 23s`
///    （`bridge.rs:14-19`）。管理员下 LHM 要枚举 205 条传感器（真机实测），
///    首次读轻易十几秒 ⇒ `taken_at.elapsed()` **必然 > 6 秒** ⇒ 每次都误报。
/// 2. **调大阈值只是把误报推后**，不是修好：读耗时随机波动，任何固定值都会
///    在某些机器上误报。而误报的代价是「用户以为出了故障」—— 比不显示更糟。
/// 3. **短暂不可用不该用「过期」这个词描述。** 通道重试（`COOLDOWN` 30s /
///    连败 5 次禁用）本来就有自己的状态表达，不需要再叠一层时间维度。
///
/// 保留 [`MetricAvailability::Unavailable`]：它表示「本次读不到、稍后重试」，
/// 是**如实**描述而非推测。
#[cfg(test)]
mod tests {
    use super::*;

    fn diag(lines: &[&str]) -> Vec<String> {
        lines.iter().map(|s| (*s).to_owned()).collect()
    }

    #[test]
    fn permission_denied_requires_positive_evidence_not_just_missing_reading() {
        // 🔴 核心回归：**「读不到」不等于「权限不足」**。无证据时不得给出
        // PermissionDenied，否则等于凭空承诺「提权就能解决」。
        let silent = DimmEvidence {
            permission_denied_hint: false,
            has_memory_node: true,
            has_dimm_reading: false,
        };
        assert_eq!(classify_dimm(silent), MetricAvailability::Unavailable);

        // 有明确证据时才成立
        let proven = DimmEvidence {
            permission_denied_hint: true,
            has_memory_node: true,
            has_dimm_reading: false,
        };
        assert_eq!(classify_dimm(proven), MetricAvailability::PermissionDenied);
    }

    #[test]
    fn no_memory_node_means_unsupported_not_permission_problem() {
        // 连内存节点都没有 ⇒ 提权无用，必须说 unsupported 而不是 permission_denied，
        // 否则用户会反复提权却永远读不到。
        let evidence = DimmEvidence {
            permission_denied_hint: false,
            has_memory_node: false,
            has_dimm_reading: false,
        };
        assert_eq!(classify_dimm(evidence), MetricAvailability::Unsupported);

        // 🔴 关键顺序纪律：**即使有权限证据**，硬件层缺失也优先 ——
        //   「没有内存节点」是更根本的事实，提权不可能凭空造出内存传感器。
        //   （第一版把 PermissionDenied 排在前面，被这条用例抓出。）
        let with_evidence = DimmEvidence {
            permission_denied_hint: true,
            has_memory_node: false,
            has_dimm_reading: false,
        };
        assert_eq!(
            classify_dimm(with_evidence),
            MetricAvailability::Unsupported,
            "Unsupported 必须优先于 PermissionDenied，否则用户会反复提权却永远读不到"
        );
    }

    #[test]
    fn reading_beats_every_other_state() {
        // 有读数时，即便同时存在权限类诊断文本也不该降级 —— 读到就是拿到了。
        let evidence = DimmEvidence {
            permission_denied_hint: true,
            has_memory_node: true,
            has_dimm_reading: true,
        };
        assert_eq!(classify_dimm(evidence), MetricAvailability::Ok);
    }

    #[test]
    fn access_denied_markers_cover_dotnet_and_localized_wordings() {
        for hit in [
            "System.UnauthorizedAccessException: denied",
            "Access is denied. (Exception from HRESULT: 0x80070005)",
            "拒绝访问。",
            "读取传感器失败：权限不足",
            "This operation requires administrative privileges",
        ] {
            assert!(mentions_access_denied(hit), "应识别为权限失败证据：{hit}");
        }
        // 反向：普通诊断不得被误判成权限问题（否则所有失败都会被引导去提权）
        for miss in [
            "[monitor] 传感器回包解析失败（第 16210 列附近）",
            "timeout waiting for response",
            "未找到 AegisMonitor.exe",
        ] {
            assert!(
                !mentions_access_denied(miss),
                "不该被误判为权限证据：{miss}"
            );
        }
    }

    #[test]
    fn evidence_collector_reads_diagnostics() {
        let evidence = DimmEvidence::collect(
            false,
            true,
            &diag(&["[monitor] ok", "[monitor] Access is denied"]),
        );
        assert!(evidence.permission_denied_hint);
        assert!(evidence.has_memory_node);
        assert!(!evidence.has_dimm_reading);
    }

    #[test]
    fn reason_never_promises_that_elevation_will_fix_it() {
        // 用户裁决规则 4：不保证提权后必然成功 ⇒ 文案不得断言。
        assert!(
            MetricAvailability::PermissionDenied
                .reason()
                .contains("可能"),
            "权限态文案必须留有余地：{}",
            MetricAvailability::PermissionDenied.reason()
        );
        assert!(!MetricAvailability::Ok.reason().contains("权限"));
        // 🔴 四态文案必须两两不同：降级原因直接上屏，重复文案会让用户
        //   以为是同一件事换了说法。`Stale` 已删除，故这里校验余下四态。
        let mut reasons = vec![
            MetricAvailability::Ok.reason(),
            MetricAvailability::PermissionDenied.reason(),
            MetricAvailability::Unsupported.reason(),
            MetricAvailability::Unavailable.reason(),
        ];
        reasons.sort_unstable();
        let before = reasons.len();
        reasons.dedup();
        assert_eq!(before, reasons.len(), "降级文案须两两可区分：{reasons:?}");
    }
}
