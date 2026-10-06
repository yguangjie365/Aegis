// backup 域上行类型 —— Rust 生成物的临时手写形状（沿 overview.ts / music.ts / transfer.ts 先例）。
// 🔴 字段口径唯一事实源在 crates/services/src/backup/dto.rs（serde 全部 camelCase）；
// 改后端字段必须同批改本文件，Q-17 ts-rs 生成器接入后由本文件被生成物替换。
//
// 🔴 设计纪律：「要不要显示 / 显示成什么」的口径在 Rust 侧做完
//（原因码、三态结论、能力可用性），前端只按字段渲染，**不得二次猜测**。
//   反例：本文件不提供 `isIntegrityOk(integrity)` 这类判定函数 ——
//   `notFullyVerified`（抽样/有复用条目）绝不能被前端渲染成「通过」。

/** 失败载荷：只回原因码 + 可读文案，不回显用户输入。 */
export interface BackupError {
  code: string;
  message: string;
}

/** 校验三态（R-11「状态诚实」：抽样或有复用条目**必须**降级为 notFullyVerified）。 */
export type IntegrityStatus = "passed" | "notFullyVerified" | "failed";

/** 快照终态。 */
export type SnapshotStatus = "success" | "failed";

/** 卷影能力三态（F-FB-21）。🟡 本批恒为 `clientUnavailable`（K-002 未拍板）。 */
export type ShadowCapability =
  "componentMissing" | "configuredPendingElevation" | "clientUnavailable";

/** 定时能力状态（🟡 K-003 未拍板 ⇒ 只有 `undecided` 会被返回）。 */
export type ScheduleCapability = "undecided" | "registered" | "notRegistered";

/** 备份规则（线协议形状；落盘形状是 snake_case，两者在 Rust 侧分离）。 */
export interface RuleView {
  id: string;
  name: string;
  note: string;
  enabled: boolean;
  /** 来源路径列表（文件夹与单文件混用）。 */
  sources: string[];
  /** 备份落点（空串 = 用全局备份根）。 */
  customRoot: string;
  /** 保留份数（GFS 生效时为 0）。 */
  keepCount: number;
  useGfs: boolean;
  /** 排除模式（单段通配 / 多段相对路径前缀）。 */
  exclusions: string[];
  /** 每日定时时刻 `HH:mm`（空 = 未启用）。🟡 K-003 未拍板 ⇒ 不会真的注册。 */
  dailyTime: string;
  lastScheduledDate: string;
  /** 🟡 恒 false（K-002 未拍板，勾了会被后端拒绝）。 */
  useShadowCopy: boolean;
  /** 来源全不存在 ⇒ 草稿态（F-FB-04）。 */
  isDraft: boolean;
  shadowCapability: ShadowCapability;
  scheduleCapability: ScheduleCapability;
  order: number;
  /**
   * 该规则现有快照份数（**逐规则**口径；页头那个是总数）。
   *
   * 🔴 派生值，由后端扫目录回填 —— 判断「哪条规则该管」靠它，
   *   不能靠总数除以条数。
   */
  snapshotCount: number;
  /** 最近一次备份时间（Unix 毫秒；`0` = 从未备份过）。同样是派生值。 */
  latestSnapshotAt: number;
}

/** 规则列表响应（页头汇总 F-FB-01）。 */
export interface RuleListView {
  rules: RuleView[];
  ruleCount: number;
  snapshotCount: number;
  /**
   * 🔴 规则记录损坏时的显式提示（R-18）。
   * 非空数组 ⇒ 页面**必须**显示警告条，不能当空列表处理——
   * 用户会以为自己的规则凭空消失了。
   */
  warnings: string[];
}

/** 快照行（只读轻量元数据，F-FB-26）。 */
export interface SnapshotView {
  ruleId: string;
  ruleName: string;
  /** Unix 毫秒（前端负责格式化）。 */
  createdUnixMs: number;
  fileCount: number;
  totalBytes: number;
  status: SnapshotStatus;
  integrity: IntegrityStatus;
  reusedCount: number;
  failedCount: number;
  /** 展示路径（已隐藏并发去重后缀，F-FB-32）——🔴 不是可直接打开的绝对路径。 */
  displayPath: string;
}

/** 一条失败明细（最多 10 条 + 总数，R-13）。 */
export interface FailureView {
  /** 原因码（`reasons` 表内；未登记码原样透传）。 */
  code: string;
  text: string;
  path: string;
}

/** 一次备份的产出结论。 */
export interface BackupOutcome {
  /** 生成的快照（取消或失败时为 `null`）。 */
  snapshot: SnapshotView | null;
  fileCount: number;
  totalBytes: number;
  status: SnapshotStatus;
  integrity: IntegrityStatus;
  failures: FailureView[];
  failureCount: number;
  /** 被裁剪的旧快照时间戳（F-FB-31「逐条留痕」）。 */
  pruned: number[];
  /** 🔴 用户取消（cancel）—— **不是失败**，界面文案必须区分。 */
  cancelled: boolean;
}

/** 备份阶段（F-FB-18：文案必须与真实阶段一致）。 */
export type BackupStage =
  | "preparingShadowView"
  | "scanning"
  | "checkingSpace"
  | "copying"
  | "writingManifest"
  | "verifying"
  | "pruning"
  | "done";

/**
 * 进度上报（F-FB-18）。
 *
 * 🟠 **本批不会被调用**：`commands.rs` 的 `progress` 回调是空实现，
 * 因为事件名必须走 `contracts` 注册表而 W0-07 生成链未落地。
 * ⇒ 页面的进度区按「阶段未知」处理（见 hooks/use-backup.ts 的说明），
 * 接线后本类型即可直接消费。
 */
export interface ProgressView {
  ruleId: string;
  /** 批量备份时为 `[i/N] 规则名`（F-FB-17）。 */
  label: string;
  stage: BackupStage;
  processed: number;
  total: number;
  /** 0–100；`total === 0` 时为 0（🔴 不允许除零后展示 NaN）。 */
  percent: number;
  correlationId: string;
}

/** 校验报告（手动校验 F-FB-28 / 自动校验回显）。 */
export interface VerifyReportView {
  total: number;
  /** 🔴 实际比对数（抽样时**小于** total，必须如实展示，F-FB-29）。 */
  compared: number;
  matched: number;
  /** 内容不一致的相对路径。 */
  mismatched: string[];
  /** 快照里缺失的相对路径。 */
  missing: string[];
  integrity: IntegrityStatus;
}

/** 恢复预演（F-FB-35：与执行共用同一 `plan`）。 */
export interface RestorePlanView {
  total: number;
  /** 目标已存在（冲突数）。 */
  conflicts: number;
  /** 会被安全守卫拒绝（R-2）。 */
  guardRejected: number;
  /** 全新写入。 */
  freshWrites: number;
  /** 生效的落点（展示用）。 */
  target: string;
  policy: ConflictPolicy;
}

/** 冲突策略（与后端枚举小写名逐字一致）。 */
export type ConflictPolicy = "rename" | "overwrite" | "skip";

/** 恢复结果（F-FB-36）。 */
export interface RestoreOutcome {
  succeeded: number;
  failed: number;
  /** 写入成功但读回比对不一致。 */
  verifyMismatched: number;
  skipped: number;
  failures: FailureView[];
  skipReasons: FailureView[];
  /** 重建的空目录数（R-10）。 */
  emptyDirsRebuilt: number;
}

/** 导出结果（F-FB-14：成功须提示条数与路径）。 */
export interface ExportView {
  count: number;
  path: string;
}

/** 导入结果（F-FB-15：如实报成功数 + 逐条问题明细）。 */
export interface ImportView {
  updated: number;
  appended: number;
  renamed: number;
  /** 逐条问题明细（改名、空名等；**禁止静默**）。 */
  issues: string[];
}

/** 全局设置。 */
export interface SettingsView {
  /** 全局备份根（空 = 尚未配置，界面应提示去设置）。 */
  globalRoot: string;
  defaultKeepCount: number;
  concurrency: number;
  /** -1 关闭 / 0 全量 / >0 抽样条数（F-FB-29）。 */
  verifySample: number;
  /** 🟡 恒 false（依赖 K-002）。 */
  reuseUnchanged: boolean;
  defaultConflictPolicy: ConflictPolicy;
}

/** 新建规则的空模板（`id` 空串 = 让后端分配）。 */
export function emptyRule(): RuleView {
  return {
    id: "",
    name: "",
    note: "",
    enabled: true,
    sources: [],
    customRoot: "",
    keepCount: 7,
    useGfs: false,
    exclusions: [],
    dailyTime: "",
    lastScheduledDate: "",
    useShadowCopy: false,
    isDraft: false,
    shadowCapability: "clientUnavailable",
    scheduleCapability: "undecided",
    order: 0,
    // 派生值：新建规则还没有任何快照 —— 0 是**真实值**而非占位。
    snapshotCount: 0,
    latestSnapshotAt: 0,
  };
}
