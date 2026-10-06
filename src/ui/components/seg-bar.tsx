import styled from "@emotion/styled";

// seg-bar：F-01 卡内的分段占用条（母本 SystemToolkit OverviewView 的进度条语系）。
// 🔴 高度恒定：无数据（`percent === null`）时保留轨道、只清空填充——
// 这是 L-08「传感器全灭时卡高不变」在这一个组件上的落点。

const Track = styled.div`
  height: 6px;
  border-radius: var(--aegis-radius-full);
  background: var(--aegis-surface-inset);
  overflow: hidden;
`;

// 🔴 三态配色与母本逐值对齐（`StatCardVm.SegLevel`）：Normal 走主色绿、
// Warning=母本 `Brush_SegmentWarn` #EF9100、Critical 走 error 红。
// 分级值由 Rust 判完下发，本组件只按值取色。
const LEVEL_COLOR = {
  normal: "var(--aegis-accent)",
  warning: "var(--aegis-warning-bright)",
  critical: "var(--aegis-error)",
} as const;

const Fill = styled.div<{ width: number; $level: keyof typeof LEVEL_COLOR }>`
  height: 100%;
  width: ${(p) => p.width}%;
  border-radius: var(--aegis-radius-full);
  background: ${(p) => LEVEL_COLOR[p.$level]};
  /* 节拍 2 秒，过渡 slow 档(400ms)：肉眼能看出在动，又不至于永远追不上。
     颜色一并过渡：跨档时（如 59% → 61%）色带平滑推过去，不闪一下。
     🔴 2026-10-04 起取令牌（第六维 motion），不再裸写 400ms / ease。 */
  transition:
    width var(--aegis-motion-duration-slow) var(--aegis-motion-easing-standard),
    background-color var(--aegis-motion-duration-slow) var(--aegis-motion-easing-standard);
`;

export type SegLevel = "normal" | "warning" | "critical";

/**
 * 占用条。`label` **只服务无障碍**（`aria-label` 与无值时的 `aria-valuetext`）。
 *
 * 🔴 以前这里还会把 label 再渲染成一行小字（Caption），2026-10-07 真数据审查 T7 删掉：
 * 调用方（stat-card）在上一行的 `UsageLabel` 已经说过同一个词 ——
 * 一屏四张卡就是八处重复（「使用率 / 内存使用 / 存储已用」各写两遍）。
 * 用户方向 #2：同一事实只说一次。母本 `OverviewView.xaml` 的占用条本身也没有说明行。
 */
export function SegBar({
  percent,
  label,
  level = "normal",
}: {
  percent: number | null;
  label: string;
  /** 🔴 由后端 `segLevel` 下发（`SegLevel::from_percent`）；缺省仅供无后端的预览场景 */
  level?: SegLevel;
}) {
  const width = percent === null ? 0 : Math.min(100, Math.max(0, percent));
  return (
    <Track
      role="progressbar"
      aria-label={label}
      aria-valuenow={percent ?? undefined}
      aria-valuemin={0}
      aria-valuemax={100}
      // 🔴 无障碍：三态不只靠颜色区分，读屏需能读到档位
      aria-valuetext={percent === null ? label : `${percent}%`}
    >
      <Fill width={width} $level={level} />
    </Track>
  );
}
