// 概览页展示层纯函数（M-01 F-12）。
//
// 之所以把"运行时长"格式化放前端：Rust 侧 `uptime_secs` 是裸秒数（dto.rs 头注：
// 单位文案归前端 i18n，避免后端长出一份中文串）。本文件不碰 React、不碰后端，
// 因此 vitest 可以直接钉住分段口径（片1-e 的首批用例之一）。

export interface UptimeUnits {
  day: string;
  hour: string;
  minute: string;
  belowMinute: string;
}

/**
 * 秒 → 「N 天 M 小时」/「N 小时 M 分」/「N 分钟」/「不足 1 分钟」。
 *
 * 口径：最多给两段（天+时 或 时+分），尾段为 0 时省略——
 * 与 F-12 预期结果"任一组成项缺失时该段自然省略，不出现空分隔符"一致。
 */
export function formatUptime(totalSecs: number, units: UptimeUnits): string {
  if (!Number.isFinite(totalSecs) || totalSecs < 0) return units.belowMinute;
  const days = Math.floor(totalSecs / 86400);
  const hours = Math.floor((totalSecs % 86400) / 3600);
  const minutes = Math.floor((totalSecs % 3600) / 60);
  if (days > 0)
    return hours === 0 ? `${days}${units.day}` : `${days}${units.day} ${hours}${units.hour}`;
  if (hours > 0)
    return minutes === 0
      ? `${hours}${units.hour}`
      : `${hours}${units.hour} ${minutes}${units.minute}`;
  return minutes > 0 ? `${minutes}${units.minute}` : units.belowMinute;
}

/** F-12 页头摘要：`DESKTOP-AEGIS · Windows 11 23H2 · 运行 3 天 4 小时`；任一段为空则省略分隔符 */
export function joinHeader(parts: readonly (string | null | undefined)[]): string {
  return parts
    .filter((part): part is string => typeof part === "string" && part.trim() !== "")
    .join(" · ");
}
