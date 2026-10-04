// 配对码倒计时的纯格式化（F-FT-38）。
//
// 单独成纯函数而不是写在组件里：倒计时的边界（跨分钟、0、负值、刚好整分）
// 用单测覆盖比在界面上肉眼试可靠得多——这正是 `text-metrics.ts` 的先例。

/** 接近轮换的阈值（F-FT-38：接近时显示「即将轮换」）。 */
export const EXPIRING_SOON_MS = 60_000;

/**
 * 把剩余毫秒格式化成 `MM:SS`（F-FT-38 的「MM:SS 后轮换」）。
 *
 * 🔴 向上取整（`ceil`）而不是向下：剩余 900ms 时向下取整会显示 `00:00`，
 * 而配对码其实还有效——那会误导用户以为已经不能用了。
 * 负值/`NaN` 一律折成 `00:00`（已过期就是这么显示）。
 */
export function formatCountdown(ms: number): string {
  if (!Number.isFinite(ms) || ms <= 0) return "00:00";
  const totalSeconds = Math.ceil(ms / 1000);
  const minutes = Math.floor(totalSeconds / 60);
  const seconds = totalSeconds % 60;
  return `${String(minutes).padStart(2, "0")}:${String(seconds).padStart(2, "0")}`;
}

/** 是否"即将轮换"（F-FT-38；已过期不算"即将"，那是已经发生的事）。 */
export function isExpiringSoon(ms: number): boolean {
  return Number.isFinite(ms) && ms > 0 && ms <= EXPIRING_SOON_MS;
}
