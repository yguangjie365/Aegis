// transfer 域前端门面（M-05 片1）：七个命令。
// 🔴 业务代码不得裸调 tauriInvoke —— 一律经本文件（01 专题「前端不触引擎」白名单侧）。
//
// 命令面契约（crates/services/src/transfer/commands.rs）：
// * 入参一律 `CmdEnvelope`（由 invoke.ts 自动包）；需要载荷的命令这里传对象
// * 失败**reject**（带 `{ code, message }` 的 `TransferError`），成功 resolve
// * 🔴 **例外**：`transfer_peers_list` 未启动时返回**空数组**而非 reject——
//   那是"暂时没有设备"这一事实，不是错误，界面应渲染空态而不是失败态。
//
// `transfer_service_status` 每轮都会读设置文件（未启动时要回端口/目录），
// 故界面轮询它时不要把间隔设得太小（页面用 2 秒节拍，与概览页口径一致）。

import { BACKEND_OFFLINE_CODE, invoke, isBackendOffline } from "./invoke";
import type { LogModule } from "@/lib/logging";
import type {
  PendingConfirm,
  PerTransferPolicy,
  SendFilePayload,
  SendResult,
  SettingsLoaded,
  StopReport,
  TransferError,
  TransferHistoryEntry,
  TransferPeer,
  TransferSettings,
  TransferStatus,
  TransferTask,
  TextOutcome,
  ExportReport,
  PauseOutcome,
} from "@/types/transfer";

const TRANSFER: LogModule = "Transfer";

/** 服务状态快照（界面轮询；无副作用）。 */
export async function fetchTransferStatus(): Promise<TransferStatus> {
  return invoke<TransferStatus>("transfer_service_status", TRANSFER, "transferServiceStatus", null);
}

/** 启动传输与设备发现服务。失败按段归因（见 `TransferError.code`）。 */
export async function startTransferService(): Promise<TransferStatus> {
  return invoke<TransferStatus>("transfer_service_start", TRANSFER, "transferServiceStart", null);
}

/** 停止服务。🔴 调用方**必须**检查返回的 `allStopped`（R-28）。 */
export async function stopTransferService(): Promise<StopReport> {
  return invoke<StopReport>("transfer_service_stop", TRANSFER, "transferServiceStop", null);
}

/** 读取设置（含"文件缺失/损坏/修了什么"）。 */
export async function loadTransferSettings(): Promise<SettingsLoaded> {
  return invoke<SettingsLoaded>("transfer_settings_load", TRANSFER, "transferSettingsLoad", null);
}

/** 保存设置（后端原子写）。 */
export async function saveTransferSettings(settings: TransferSettings): Promise<void> {
  return invoke<void>("transfer_settings_save", TRANSFER, "transferSettingsSave", settings);
}

/** 在线设备列表（未启动时为空数组，不是错误）。 */
export async function listTransferPeers(): Promise<TransferPeer[]> {
  return invoke<TransferPeer[]>("transfer_peers_list", TRANSFER, "transferPeersList", null);
}

/** 发送一个文件到指定设备。 */
export async function sendTransferFile(payload: SendFilePayload): Promise<SendResult> {
  return invoke<SendResult>("transfer_send_file", TRANSFER, "transferSendFile", payload);
}

/** 把后端 reject 出来的东西收敛成 `TransferError`（`invoke` 可能抛出任意形状）。 */
export function asTransferError(error: unknown): TransferError {
  if (
    typeof error === "object" &&
    error !== null &&
    "code" in error &&
    "message" in error &&
    typeof (error as TransferError).code === "string" &&
    typeof (error as TransferError).message === "string"
  ) {
    return error as TransferError;
  }
  // 🔴 「后端没在跑」单独成码，否则归一化后与未知错误码无法区分（UI 审查 D7）。
  if (isBackendOffline(error)) {
    return {
      code: BACKEND_OFFLINE_CODE,
      message: error instanceof Error ? error.message : String(error),
    };
  }
  return { code: "UNKNOWN", message: error instanceof Error ? error.message : String(error) };
}

/**
 * 列出待用户裁决的接收确认（F-FT-14）。
 *
 * 🔴 服务未运行时返回**空表**而不是抛错：「没有待决」与「服务没跑」对界面都是"没有弹窗"，
 * 不必让调用方处理两种空。
 */
export async function listPendingConfirms(): Promise<PendingConfirm[]> {
  return invoke<PendingConfirm[]>(
    "transfer_pending_confirms",
    TRANSFER,
    "transferPendingConfirms",
    null,
  );
}

/**
 * 答复一条待决确认（`accept=false` 即拒绝）。
 *
 * `conflictPolicy` 是**本次传输**的冲突策略覆盖（`rename` / `overwrite` / `skip`）；
 * 省略即沿用设置。🔴 它**不写设置**——用户在弹窗里选「覆盖」，
 * 不该把他下次的默认行为一并改掉。
 *
 * 🔴 失败**不是**可以忽略的：`CONFIRM_GONE` 表示"你点慢了，那条请求已按拒绝处理"，
 * 界面必须如实说，而不是显示"已接收"。
 */
export async function confirmReceive(
  id: string,
  accept: boolean,
  conflictPolicy?: PerTransferPolicy,
): Promise<void> {
  await invoke<void>("transfer_confirm_receive", TRANSFER, "transferConfirmReceive", {
    id,
    accept,
    conflictPolicy: conflictPolicy ?? null,
  });
}

/**
 * 传输任务台账快照（F-FT-25/30）。
 *
 * 🔴 只读：任务由后端收发线程登记，界面不得改写（R-16 终态不可回退由后端独占保证）。
 * 服务未运行时返回**空表**而不是报错（与设备列表同口径）。
 */
export async function listTransferTasks(): Promise<TransferTask[]> {
  return invoke<TransferTask[]>("transfer_tasks_list", TRANSFER, "transferTasksList", null);
}

/**
 * 取消一个进行中的任务（F-FT-26）。
 *
 * 🔴 返回 `false` **不是失败**，是"该任务已收尾或不存在"——
 * 调用方应据此**刷新列表**，而不是弹失败提示（真实情况往往是任务刚跑完）。
 */
export async function cancelTransferTask(id: string): Promise<boolean> {
  const result = await invoke<{ cancelled: boolean }>(
    "transfer_cancel_task",
    TRANSFER,
    "transferCancelTask",
    { id },
  );
  return result.cancelled;
}

/**
 * 传输历史（F-FT-54）。
 *
 * 🔴 服务未运行时返回**空表**（不是错误，与设备列表同口径）。
 */
export async function listTransferHistory(): Promise<TransferHistoryEntry[]> {
  return invoke<TransferHistoryEntry[]>(
    "transfer_history_list",
    TRANSFER,
    "transferHistoryList",
    null,
  );
}

/**
 * 导出历史为 CSV 到 `path`（F-FT-56「所见即所得」）。
 *
 * 🔴 `ids` 传**当前筛选后那些行**的 id（`null` = 全量）。导出与屏幕所见必须一致——
 * 界面上 3 条、导出的文件里 7 条，用户只会觉得被糊弄了。
 *
 * 🔴 **写盘由后端完成**（原子写）：这样不必为导出引入前端 fs 插件。
 * 返回值里的 `count` 是**实际写出的条数**，界面据此说"已导出 {N} 条"。
 */
export async function exportTransferHistory(
  path: string,
  ids: string[] | null,
): Promise<ExportReport> {
  return invoke<ExportReport>("transfer_history_export", TRANSFER, "transferHistoryExport", {
    path,
    ids,
  });
}

/**
 * 发送一条文本到指定对端（F-FT-33）。
 *
 * 🔴 返回值的 `delivered` **不是**"发出去了"，而是"**对方剪贴板写入成功**"（F-FT-34）。
 * 界面文案必须照此措辞——写成"已发送"就是在骗人。
 */
export async function sendText(peer: string, text: string): Promise<TextOutcome> {
  return invoke<TextOutcome>("transfer_send_text", TRANSFER, "transferSendText", { peer, text });
}

/**
 * 从系统剪贴板读入文本（F-FT-33）。
 *
 * 🔴 三档失败分开报（`CLIPBOARD_BUSY` / `CLIPBOARD_EMPTY` / `CLIPBOARD_FAILED`）——
 * 「被别的程序占用」与「里面没有文本」对用户是两件事，能做的处理也不同。
 */
export async function readClipboard(): Promise<string> {
  return invoke<string>("transfer_read_clipboard", TRANSFER, "transferReadClipboard", null);
}

/**
 * 暂停一个任务（F-FT-26）。
 *
 * 🔴 返回 `accepted: false` 时**不是失败**：那是"任务当前状态不接受这个动作"，
 * 调用方应把 `state` 原样带进"未被受理：任务当前状态为 …"（R-14）。
 */
export async function pauseTransferTask(id: string): Promise<PauseOutcome> {
  return invoke<PauseOutcome>("transfer_pause_task", TRANSFER, "transferPauseTask", { id });
}

/** 恢复一个任务（F-FT-26）。语义同 `pauseTransferTask`。 */
export async function resumeTransferTask(id: string): Promise<PauseOutcome> {
  return invoke<PauseOutcome>("transfer_resume_task", TRANSFER, "transferResumeTask", { id });
}
