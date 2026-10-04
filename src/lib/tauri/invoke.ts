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
 */
export class BackendUnavailableError extends Error {
  constructor(command: string) {
    super(`后端不可用（当前不在 Tauri 运行时内，浏览器预览模式）：无法调用 ${command}`);
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
