// invoke 包装器（01 专题 §2.3：前端不触引擎——业务代码只允许经此调用后端）。
// 每个 command 参数包络统一 CmdEnvelope { correlationId, payload }（契约在 W0-07 生成物落地）。
// 🔴 浏览器预览模式下调用一律显式抛错并留日志，禁止假数据静默兜底 —— 空态/占位页不得伪装成后端返回。
// （原注释写"src-tauri 尚未创建（W0-02 需 Rust 工具链，本机未装）"是**过时**的：W0-02早已完成，
//  `src-tauri/` 有 workspace 根+ 5 crates 且已产出 release exe。此处真实原因是"未运行在 Tauri 运行时内"。2026-10-04 更正）

import { invoke as tauriInvoke } from "@tauri-apps/api/core";
import { log, newCorrelationId, type LogModule } from "@/lib/logging";

export function isTauriRuntime(): boolean {
  return "__TAURI_INTERNALS__" in window;
}

/**
 * 浏览器预览模式（非 Tauri 运行时）下抛出。
 *
 * 🔴 这是**永久性错误**而非瞬时故障：进程根本没跑在 Tauri 里，重试多少次都不会成功。
 * 故消费方（如 TanStack Query 的 `retry`）应把它判为"不重试"，否则用户要盯好几秒骨架屏
 * 才等到一个必然失败的错误。
 *
 * 🔴 `command` 必须留成**独立字段**而不是只嵌在 message 里（2026-10-06 UI 审查 D7）：
 * 界面要把这句拆成「主线人话 + 详情折叠」，靠正则回捞自己写下的命令名既不体面也会随
 * 文案改动而失效。message 保持原样，因为它同时是 JSONL 日志里的 exception 文本。
 */
export class BackendUnavailableError extends Error {
  constructor(readonly command: string) {
    super(`后端不可用（当前不在 Tauri 运行时内，浏览器预览模式）：无法调用 ${command}`);
    this.name = "BackendUnavailableError";
  }
}

/**
 * `as*Error` 归一化后用来标记「后端没在跑」的错误码。
 *
 * 🔴 为什么需要它：`asBackupError` / `asTransferError` 会把任何非规范形状折成
 * `{ code: "UNKNOWN", message }`，**Error 实例身份在途中丢失**。界面若只拿到归一化结果，
 * 就再也分不清「后端未运行」与「后端回了一个未知错误码」—— 而这两句话在界面上必须不同
 * （前者是环境事实，后者是故障）。故归一化时把这一类单独标出来。
 */
export const BACKEND_OFFLINE_CODE = "BACKEND_OFFLINE";

/** 该错误是否「后端进程没在跑」（认 Error 实例，也认归一化后的错误码）。 */
export function isBackendOffline(error: unknown): boolean {
  if (error instanceof BackendUnavailableError) return true;
  return (
    typeof error === "object" &&
    error !== null &&
    (error as { code?: unknown }).code === BACKEND_OFFLINE_CODE
  );
}

/** 取回后端命令标识（只在这一类错误里有；界面无需它时返回 null）。 */
export function backendOfflineCommand(error: unknown): string | null {
  if (error instanceof BackendUnavailableError) return error.command;
  return null;
}

export async function invoke<T>(
  command: string,
  module: LogModule,
  action: string,
  payload?: unknown,
): Promise<T> {
  const correlationId = newCorrelationId();
  const started = performance.now();
  if (!isTauriRuntime()) {
    log({
      level: "warn",
      module,
      action,
      result: "Failed",
      exception: "BackendUnavailableError: browser preview mode",
      correlationId,
    });
    throw new BackendUnavailableError(command);
  }
  try {
    // 顶层展开 payload 键：兼容 `envelope + 平铺参数（platform/songId…）` 形状的 command；
    // 只收 envelope 的 command 会忽略多余键，行为不变。
    //
    // 🔴 `extra` 展开在**后**（2026-10-04 审查提示 10）：若某个 payload 里带了 `envelope` 键，
    // 展开顺序会让它**覆盖**真正的包络 → `correlationId` 丢失 → 后端 `check_envelope` 直接
    // 拒绝，且报错信息完全指不到真因。故剔除该保留键（而不是调换展开顺序：
    // 调换会让所有平铺参数失效）。
    // 🔴🔴 必须是**副本**上删 —— `payload as Record` 与 `extra` 是同一个对象引用，
    // 直接 `delete extra.envelope` 会把调用方传进来的对象改掉（store 里的 state 就地变异）。
    const extra: Record<string, unknown> = {};
    if (payload && typeof payload === "object" && !Array.isArray(payload)) {
      for (const [key, value] of Object.entries(payload as Record<string, unknown>)) {
        if (key !== "envelope") extra[key] = value;
      }
    }
    const result = await tauriInvoke<T>(command, {
      envelope: { correlationId, payload },
      ...extra,
    });
    log({
      level: "info",
      module,
      action,
      result: "Success",
      duration: Math.round(performance.now() - started),
      correlationId,
    });
    return result;
  } catch (e) {
    log({
      level: "error",
      module,
      action,
      result: "Failed",
      duration: Math.round(performance.now() - started),
      exception: e instanceof Error ? `${e.name}: ${e.message}` : String(e),
      correlationId,
    });
    throw e;
  }
}
