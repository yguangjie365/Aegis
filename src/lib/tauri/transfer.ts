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

import { invoke } from "./invoke";
import type { LogModule } from "@/lib/logging";
import type {
  SendFilePayload,
  SendResult,
  SettingsLoaded,
  StopReport,
  TransferError,
  TransferPeer,
  TransferSettings,
  TransferStatus,
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
  return { code: "UNKNOWN", message: error instanceof Error ? error.message : String(error) };
}
