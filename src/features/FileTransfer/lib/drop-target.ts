// 拖放目标的命中测试与去重（纯函数，可单测）。
//
// 🔴 两个坑都来自 K-S01 的**实测**（`spikes/K-S01-drag-drop.md` §四）：
//
// 1. **坐标系不同**：Tauri 给的 `position` 是**物理像素**，而 DOM 的
//    `getBoundingClientRect()` 是 **CSS 像素**。在缩放 125% 的机器上两者差 1.25 倍——
//    直接比较会让"拖到第 2 行"命中第 3 行，而且**只在非 100% 缩放的机器上复现**。
// 2. **一次拖放投递两条事件**：`emit_to_webview` 与 `emit_to(EventTarget::labeled)` 都匹配
//    `listen()` 的默认目标，故同一 drop 会到两次（官方 `onDragDropEvent` 也不规避）。
//    不去重 => 一次拖放**入队两个传输任务**。

/** 设备行在 DOM 中的位置（CSS 像素，取自 `getBoundingClientRect()`）。 */
export interface DropRowRect {
  /** 设备标识（命中的返回值） */
  id: string;
  left: number;
  top: number;
  right: number;
  bottom: number;
}

/** 一个点（单位由调用方语境决定：本模块只在边界处换算一次）。 */
export interface DropPoint {
  x: number;
  y: number;
}

/** drop 去重窗口（K-S01 建议 ~100ms 抑制窗）。 */
export const DROP_DEDUP_WINDOW_MS = 100;

/**
 * 物理像素 → CSS 像素。
 *
 * 🔴 `devicePixelRatio` 取不到/为 0/为负时一律按 **1** 处理：宁可坐标系退化到"与 1x 一致"，
 * 也不能让整个命中测试算出 `Infinity`/`NaN` 而**每次拖放都落空**（那看起来像"拖放功能坏了"）。
 */
export function toCssPoint(point: DropPoint, devicePixelRatio: number): DropPoint {
  const ratio = Number.isFinite(devicePixelRatio) && devicePixelRatio > 0 ? devicePixelRatio : 1;
  return { x: point.x / ratio, y: point.y / ratio };
}

/**
 * 命中的设备行 id；未命中返回 `null`。
 *
 * 🔴 **未命中就是未命中**：R-7 明令「目标选择不许猜测…不允许用当前选中行兜底」。
 * 故本函数没有"回退到选中行"的分支——调用方拿 `null` 就只能提示用户重新拖到行上。
 *
 * 重叠时取**数组里第一个命中的行**（调用方按 DOM 顺序传入，与视觉上的上层一致）。
 */
export function hitTestDropRow(
  rows: readonly DropRowRect[],
  point: DropPoint,
  devicePixelRatio: number,
): string | null {
  const css = toCssPoint(point, devicePixelRatio);
  if (!Number.isFinite(css.x) || !Number.isFinite(css.y)) return null;
  for (const row of rows) {
    if (css.x >= row.left && css.x <= row.right && css.y >= row.top && css.y <= row.bottom) {
      return row.id;
    }
  }
  return null;
}

/** drop 的幂等键：路径集合 + 落点（K-S01 建议的判据）。 */
export function dropKey(paths: readonly string[], point: DropPoint): string {
  // 用 `\u0000` 分隔路径：它不可能出现在合法路径里，故拼接无歧义
  return `${paths.join("\u0000")}|${point.x}|${point.y}`;
}

/**
 * 这次 drop 是否应当**丢弃**（重复投递的第二次）。
 *
 * 判据：键相同 **且** 距上次处理在 [`DROP_DEDUP_WINDOW_MS`] 之内。
 * 只比键不比时间会把"用户隔一会儿又拖了同一批文件到同一位置"也误杀。
 */
export function isDuplicateDrop(
  lastKey: string | null,
  lastHandledAtMs: number,
  key: string,
  nowMs: number,
  windowMs: number = DROP_DEDUP_WINDOW_MS,
): boolean {
  if (lastKey === null) return false;
  if (lastKey !== key) return false;
  const elapsed = nowMs - lastHandledAtMs;
  // 时间倒流（时钟被改）时按"不重复"处理：宁可多处理一次，也不要永远吞掉后续 drop
  if (!Number.isFinite(elapsed) || elapsed < 0) return false;
  return elapsed <= windowMs;
}
