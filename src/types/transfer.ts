// transfer 域上行类型 —— Rust 生成物的临时手写形状（沿 overview.ts / music.ts 先例）。
// 🔴 字段口径唯一事实源在 crates/services/src/transfer/commands.rs（serde 全部 camelCase）；
// 改后端字段必须同批改本文件，Q-17 ts-rs 生成器接入后本文件由生成物替换。
//
// 🔴 设计纪律：「要不要显示」的判定在 Rust 侧做完（原因码、启动期码、`allStopped`），
// 前端只按字段渲染，**不得二次猜测口径**。
//
// 🟠 **已知缺口（登记）**：`require_pairing`（F-FT-13 的"需要配对码"开关）在 Rust 侧是
// `#[serde(skip)]`（不落盘），而 serde 的 `skip` **两个方向都跳** ⇒ 前端看不到也设不了它。
// 要开放该开关需把"磁盘形状"与"命令形状"拆成两个类型（持久化仍不含它）。
// 本轮先不动：页面尚无该开关，且拆类型属独立改动。

/** `transfer_service_status` 的返回（F-FT-06/08）。 */
export interface TransferStatus {
  running: boolean;
  transferPort: number;
  discoveryPort: number;
  receiveDirectory: string;
  /** 未启动时为 `null`（界面显示占位「——————」，F-FT-38） */
  pairingCode: string | null;
  pairingExpiresInMs: number;
  onlinePeerCount: number;
  liveConnections: number;
  /** 本机设备标识（R-6）；未启动时为 `null` */
  deviceId: string | null;
}

/** 在线设备一行（F-FT-08 的被动监听快照）。 */
export interface TransferPeer {
  deviceId: string;
  deviceName: string;
  ip: string;
  port: number;
  /** 本机收到最后一次宣告的时刻（Unix 毫秒） */
  lastSeenMs: number;
}

/** 失败载荷：只回原因码 + 可读文案，不回显对端输入。 */
export interface TransferError {
  code: string;
  message: string;
}

/** 一个已知设备（F-FT-10）。 */
export interface KnownPeer {
  name: string;
  ip: string;
  port: number;
}

/** 设置（磁盘契约；`requirePairing` 不在其中，见文件头缺口说明）。 */
export interface TransferSettings {
  receiveDirectory: string | null;
  requireKnownPeer: boolean;
  requireReceiveConfirmation: boolean;
  /** `"rename" | "ask" | "skip" | "overwrite"`（与后端枚举的小写名逐字一致） */
  conflictPolicy: string;
  pauseTimeoutMinutes: number;
  transferPort: number;
  discoveryPort: number;
  knownPeers: KnownPeer[];
  deviceName: string | null;
  deviceToken: string | null;
}

/** `transfer_settings_load` 的返回（禁止静默失败：把"缺失/损坏/修了什么"带回界面）。 */
export interface SettingsLoaded {
  settings: TransferSettings;
  fileMissing: boolean;
  corrupt: boolean;
  repairs: string[];
}

/** `transfer_service_stop` 的返回（R-28：界面**必须**用 `allStopped`）。 */
export interface StopReport {
  allStopped: boolean;
  threadsJoined: number;
  message: string;
}

/** `transfer_send_file` 的返回。 */
export interface SendResult {
  /** `"Delivered" | "Skipped" | "HashMismatch"` */
  outcome: string;
  reasonCode: string | null;
}

/** `transfer_send_file` 的载荷。 */
export interface SendFilePayload {
  ip: string;
  port: number;
  path: string;
  pairCode?: string;
}

/** 启动期失败码（🔴 **不属** `reasons` 的 16 码冻结表——那张表说的是"一次传输"的失败）。 */
export const START_INVALID_PORT = "START_INVALID_PORT";
export const START_RECEIVE_DIR_FAILED = "START_RECEIVE_DIR_FAILED";
export const START_TRANSFER_FAILED = "START_TRANSFER_FAILED";
export const START_DISCOVERY_FAILED = "START_DISCOVERY_FAILED";
export const SERVICE_NOT_RUNNING = "SERVICE_NOT_RUNNING";
export const SERVICE_ALREADY_RUNNING = "SERVICE_ALREADY_RUNNING";
export const BAD_PAYLOAD = "BAD_PAYLOAD";
export const SETTINGS_IO_FAILED = "SETTINGS_IO_FAILED";

/** 待用户裁决的接收确认（F-FT-14）。 */
export interface PendingConfirm {
  /** 裁决时回传此 id */
  id: string;
  peerLabel: string;
  fileName: string;
  fileSize: number;
  saveDirectory: string;
  /** 磁盘剩余；null = **无法判定**（界面显示"剩余空间无法判定"，不得显示 0） */
  availableBytes: number | null;
  /** 是否已存在同名文件。🔴 为 false 时**不得**显示冲突区（F-FT-14 原文） */
  nameConflict: boolean;
  /** 距超时判拒还剩多少毫秒（R-8：超时按拒绝，故界面必须给出倒计时） */
  expiresInMs: number;
}

/** 答复待决确认时的专属失败码。 */
export const CODE_CONFIRM_GONE = "CONFIRM_GONE";

/**
 * **本次传输**可选的冲突策略（接收确认弹窗的「同名文件」三个选项）。
 *
 * 🔴 定义在 DTO 层而不是组件里：门面（`lib/tauri`）也要用它，
 * 而 `lib` 依赖 `features` 是分层倒挂。
 *
 * 只作用于本次接收，**不写设置**——用户在弹窗里选「覆盖」，
 * 不该把他下次的默认行为一并改掉。
 */
/**
 * **本次传输**可选的冲突策略（接收确认弹窗的「同名文件」三个选项）。
 *
 * 🔴 定义在 DTO 层而不是组件里：门面（`lib/tauri`）也要用它，
 * 而 `lib` 依赖 `features` 是分层倒挂。
 *
 * 只作用于本次接收，**不写设置**——用户在弹窗里选「覆盖」，
 * 不该把他下次的默认行为一并改掉。
 */
export type PerTransferPolicy = "rename" | "ask" | "skip" | "overwrite";

/**
 * 一条传输历史（F-FT-54）。
 *
 * 🔴 `reasonText` 与 `finishedAtText` 由**后端**算好：
 * 界面与 CSV 若各格式化一遍，就会有两处口径漂移。
 */
export interface TransferHistoryEntry {
  id: string;
  direction: "send" | "receive";
  peerLabel: string;
  fileName: string;
  fileSize: number;
  state: TransferTaskState;
  reasonCode: string | null;
  /** 原因说明（🔴 未知码原样回显，不硬翻——F-FT-31 末句） */
  reasonText: string;
  finishedAtMs: number;
  /** 本地时区的可读时间 */
  finishedAtText: string;
}

/**
 * 一条历史在界面上的「类型」列。
 *
 * 🟠 片1 只有文件通道，故恒为「文件」；文本通道（F-FT-32~36）落地后
 * 这里要按条目区分——**现在不假装有类型字段**（规格里 CSV 也没有这一列）。
 */
export type HistoryKind = "file";

/** 任务状态（与后端 `TaskState::as_str` 一字不差）。 */
export type TransferTaskState = "active" | "completed" | "skipped" | "failed" | "cancelled";

/** 一条传输任务的对外快照（F-FT-30 的五态）。 */
export interface TransferTask {
  id: string;
  /** `send` | `receive` */
  direction: "send" | "receive";
  peerLabel: string;
  fileName: string;
  fileSize: number;
  transferred: number;
  /** 0..=100（后端算好，界面不再算一遍——避免两处口径漂移） */
  percent: number;
  state: TransferTaskState;
  /** 终态原因码（取 reasons 的冻结表；进行中为 null） */
  reasonCode: string | null;
  startedAtMs: number;
  finishedAtMs: number | null;
}
