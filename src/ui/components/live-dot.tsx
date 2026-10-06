import { useTranslation } from "react-i18next";
import styled from "@emotion/styled";

// live-dot：F-08 刷新生命周期徽章（LIVE · 每 2 秒 / 已暂停）。
// 用户靠直接看这颗点验证 R-19「切页/失焦立即停表」，所以状态必须一眼可辨：
// 活跃 = 实心绿 + 脉动光环；暂停 = 静默灰点 + 文案改「已暂停」。

const Wrap = styled.span`
  display: inline-flex;
  align-items: center;
  gap: var(--aegis-space-xs);
  /* 🔴 24 = 品牌区**非按钮项**档高（规范 §三 brand-bar 行：按钮 h=28 / 徽章与状态点 h=24）。
     原值 18 是 12px 文字自然行高，与同行的 24 徽章、28 按钮三种高度并存 ⇒ 同一排三种基线。
     只撑行盒高，点（8px）与字号都不动。 */
  height: 24px;
  font-size: 12px;
  color: var(--aegis-mute);
  white-space: nowrap;
`;

const Dot = styled.span<{ live: boolean }>`
  position: relative;
  width: 8px;
  height: 8px;
  flex: none;
  border-radius: var(--aegis-radius-full);
  background: ${(p) => (p.live ? "var(--aegis-success)" : "var(--aegis-disabled)")};
  &::after {
    content: "";
    position: absolute;
    inset: 0;
    border-radius: inherit;
    background: ${(p) => (p.live ? "var(--aegis-success)" : "transparent")};
    /* 周期 2s 属「循环速率」，按 motion 契约不入 duration 刻度；缓动取令牌 */
    animation: ${(p) =>
      p.live ? "aegis-live-pulse 2s var(--aegis-motion-easing-decelerate) infinite" : "none"};
  }
  @keyframes aegis-live-pulse {
    from {
      transform: scale(1);
      opacity: 0.55;
    }
    to {
      transform: scale(2.6);
      opacity: 0;
    }
  }
  @media (prefers-reduced-motion: reduce) {
    &::after {
      animation: none;
    }
  }
`;

export function LiveDot({ live }: { live: boolean }) {
  const { t } = useTranslation("overview");
  return (
    <Wrap role="status" aria-live="polite">
      <Dot live={live} aria-hidden="true" />
      {live ? t("live.on") : t("live.paused")}
    </Wrap>
  );
}
