// invoke 包装器（01 专题 §2.3：前端不触引擎——业务代码只允许经此调用后端）。
// 每个 command 参数包络统一 CmdEnvelope { correlationId, payload }（契约在 W0-07 生成物落地）。
// 🔴 src-tauri 尚未创建（W0-02 需 Rust 工具链，本机未装）：浏览器模式下调用一律显式抛错并留日志，
// 禁止假数据静默兜底——空态/占位页不得伪装成后端返回。

import { invoke as tauriInvoke } from "@tauri-apps/api/core";
import { log, newCorrelationId, type LogModule } from "@/lib/logging";

export function isTauriRuntime(): boolean {
  return "__TAURI_INTERNALS__" in window;
}

export class BackendUnavailableError extends Error {
  constructor(command: string) {
    super(`后端未接入（W0-02 src-tauri 未创建）：无法调用 ${command}`);
    this.name = "BackendUnavailableError";
  }
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
    const extra =
      payload && typeof payload === "object" && !Array.isArray(payload)
        ? (payload as Record<string, unknown>)
        : {};
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
