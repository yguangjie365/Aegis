import styled from "@emotion/styled";

// seg-bar：F-01 卡内的分段占用条（母本 SystemToolkit OverviewView 的进度条语系）。
// 🔴 高度恒定：无数据（`percent === null`）时保留轨道、只清空填充——
// 这是 L-08「传感器全灭时卡高不变」在这一个组件上的落点。

const Track = styled.div`
  height: 6px;
  border-radius: var(--aegis-rounded-full);
  background: var(--aegis-surface-inset);
  overflow: hidden;
`;

const Fill = styled.div<{ width: number }>`
  height: 100%;
  width: ${(p) => p.width}%;
  border-radius: var(--aegis-rounded-full);
  background: var(--aegis-accent);
  /* 节拍 2 秒，过渡 400ms：肉眼能看出在动，又不至于永远追不上 */
  transition: width 400ms ease;
`;

const Caption = styled.div`
  margin-top: var(--aegis-space-xs);
  font-size: 12px;
  line-height: 1.4;
  color: var(--aegis-mute);
`;

export function SegBar({ percent, label }: { percent: number | null; label: string }) {
  const width = percent === null ? 0 : Math.min(100, Math.max(0, percent));
  return (
    <div>
      <Track
        role="progressbar"
        aria-label={label}
        aria-valuenow={percent ?? undefined}
        aria-valuemin={0}
        aria-valuemax={100}
      >
        <Fill width={width} />
      </Track>
      <Caption>{label}</Caption>
    </div>
  );
}
