// 前端日志总线门面（01 专题 §2.1 八字段契约的前端侧投影）。
// 🔴 单一事实源在 crates/contracts（W0-07 落地后由 ts-rs 生成物替换本文件的手工定义）；
// 浏览器预览模式下只落 console，JSONL 落盘是 W0-10 的 Rust Sink 职责。

export type LogLevel = "trace" | "debug" | "info" | "warn" | "error";
export type LogResult = "Success" | "Failed" | "Cancelled" | "Rejected";

// LOG_MODULES：八模块 + 四个非 feature 模块（01 专题 §2.1 Module 行）
export const LOG_MODULES = [
  "Overview",
  "Software",
  "Driver",
  "Network",
  "Transfer",
  "Backup",
  "Music",
  "Steam",
  "ElevatedHelper",
  "Worker",
  "Shell",
  "Security",
] as const;
export type LogModule = (typeof LOG_MODULES)[number];

export interface LogEntry {
  timestamp: string;
  level: LogLevel;
  module: LogModule;
  action: string;
  result: LogResult;
  duration?: number;
  exception?: string;
  correlationId: string;
}

// 8 字符短码：UUID 前 8 字节 Base32（01 专题 §2.2；生成器与 Rust 侧实现对齐前，
// 此处用 Crockford Base32 字母表从 CSPRNG 取 8 位，形态一致）
const BASE32 = "0123456789ABCDEFGHJKMNPQRSTVWXYZ";
export function newCorrelationId(): string {
  const bytes = new Uint8Array(8);
  crypto.getRandomValues(bytes);
  // 5bit/字符 → 8 字节恰产 13 字符，取前 8
  let bits = "";
  for (const b of bytes) bits += b.toString(2).padStart(8, "0");
  let out = "";
  for (let i = 0; i + 5 <= bits.length && out.length < 8; i += 5) {
    out += BASE32[parseInt(bits.slice(i, i + 5), 2)] ?? "0";
  }
  return out;
}

export function log(
  partial: Pick<LogEntry, "level" | "module" | "action" | "result"> &
    Partial<Pick<LogEntry, "duration" | "exception" | "correlationId">>,
): void {
  const entry: LogEntry = {
    timestamp: new Date().toISOString(),
    correlationId: partial.correlationId ?? "-",
    ...partial,
  };
  const line = `[aegis] ${JSON.stringify(entry)}`;
  switch (entry.level) {
    case "trace":
    case "debug":
      console.debug(line);
      break;
    case "info":
      console.info(line);
      break;
    case "warn":
      console.warn(line);
      break;
    case "error":
      console.error(line);
      break;
  }
}
