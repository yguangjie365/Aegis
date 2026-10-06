// 展示格式化（纯函数，可单测 —— 与 Overview/lib/format.ts 同款分层）。
//
// 🔴 为什么不放在组件里：字节单位、时间口径、校验三态的**措辞**都是
//   「显示什么」的判定，抽出来才能单测；留在组件里就只能靠眼看。

import type { IntegrityStatus, SnapshotStatus } from "@/types/backup";

const UNITS = ["B", "KB", "MB", "GB", "TB", "PB"] as const;

/**
 * 字节数 → 人类可读（`2.3 GB`）。
 *
 * 🔴 用 1024 进制（与 Windows 资源管理器一致），**不用** 1000 ——
 *   用户在资源管理器里看到的是 1024 进制，两边不一致会被当成「备份少了一块」。
 * 🔴 `bytes === 0` 返回 `"0 B"` 而非 `"0 B"`+ 小数噪声。
 */
export function formatBytes(bytes: number): string {
  if (!Number.isFinite(bytes) || bytes <= 0) return "0 B";
  let value = bytes;
  let unit = 0;
  while (value >= 1024 && unit < UNITS.length - 1) {
    value /= 1024;
    unit += 1;
  }
  // 🔴 小于 10 的单位保留一位小数、大于等于 10 的取整：
  //   "1.5 GB" 有信息量，"2.34 GB" 是噪声（备份体量不需要精确到 MB）。
  const decimals = unit === 0 ? 0 : value < 10 ? 1 : 0;
  return `${value.toFixed(decimals)} ${UNITS[unit]}`;
}

/**
 * Unix 毫秒 → 本地时间串（`2026-10-05 09:12`）。
 *
 * 🔴 不用 `toISOString`：那给的是 UTC，与快照目录名的**本地时间**口径不一致，
 *   用户会看到「备份于昨天 17:12」这种差 8 小时的错值（正是后端
 *   `parse_snapshot_time` 踩过的同一个坑的另一面）。
 * 🔴 非法时间（0 / NaN）返回 `"————"` 而不是 `Invalid Date`。
 */
export function formatSnapshotTime(unixMs: number): string {
  if (!Number.isFinite(unixMs) || unixMs <= 0) return "————";
  const d = new Date(unixMs);
  if (Number.isNaN(d.getTime())) return "————";
  const pad = (n: number): string => String(n).padStart(2, "0");
  return (
    `${d.getFullYear()}-${pad(d.getMonth() + 1)}-${pad(d.getDate())} ` +
    `${pad(d.getHours())}:${pad(d.getMinutes())}`
  );
}

/**
 * 文件数 → 千分位（`1,204`）。
 *
 * 🔴 用 `toLocaleString("zh-CN")` 而非手写正则：手写的会漏负数/超大数边界。
 */
export function formatCount(count: number): string {
  if (!Number.isFinite(count)) return "—";
  return count.toLocaleString("zh-CN");
}

/**
 * Unix 毫秒 → 本地时刻串（`09:12:04`），供日志行前缀用。
 *
 * 🔴 与 [`formatSnapshotTime`] 同口径取**本地**时间，不用 `toISOString`（那是 UTC）——
 *   否则日志里「刚刚发生的事」会显示成 8 小时前的时刻。
 * 🔴 非法时间返回 `"--:--:--"`，不返回 `Invalid Date`。
 */
export function formatClock(unixMs: number): string {
  if (!Number.isFinite(unixMs) || unixMs <= 0) return "--:--:--";
  const d = new Date(unixMs);
  if (Number.isNaN(d.getTime())) return "--:--:--";
  const pad = (n: number): string => String(n).padStart(2, "0");
  return `${pad(d.getHours())}:${pad(d.getMinutes())}:${pad(d.getSeconds())}`;
}

/** 快照终态 → i18n key。 */
export function statusKey(status: SnapshotStatus): string {
  return status === "success" ? "status.success" : "status.failed";
}

/** 校验三态 → i18n key（🔴 三态各有独立措辞，**不可**折叠成「通过/不通过」）。 */
export function integrityKey(integrity: IntegrityStatus): string {
  return `status.integrity.${integrity}`;
}

/**
 * 校验三态 → 语义色（供徽章取 var()）。
 *
 * 🔴 返回**语义名**而非色值：颜色一律由 CSS 侧 `var(--aegis-*)` 解析，
 *   这里给字面色值就等于绕过令牌体系（换主题必漂）。
 */
export function integrityTone(integrity: IntegrityStatus): "ok" | "warn" | "bad" {
  if (integrity === "passed") return "ok";
  if (integrity === "notFullyVerified") return "warn";
  return "bad";
}

/** 快照终态 → 语义色。 */
export function statusTone(status: SnapshotStatus): "ok" | "bad" {
  return status === "success" ? "ok" : "bad";
}
