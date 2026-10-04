import { useTranslation } from "react-i18next";
import styled from "@emotion/styled";
import { AppButton } from "./app-button";

// state-block 三态齐备强制件（§三）：空态（图标+一句+主 CTA）/ 失败态（原因+重试）/ 加载态（骨架）。
// 全站唯一一套 skeleton 实现（旧工程 4 份复制教训）。

const Frame = styled.section`
  border: 1px solid var(--aegis-hairline);
  border-radius: var(--aegis-radius-sm);
  background: var(--aegis-canvas);
  padding: var(--aegis-space-xxl) var(--aegis-space-lg);
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: var(--aegis-space-sm);
  text-align: center;
`;

const Title = styled.h2`
  font-size: 15px;
  font-weight: 700;
`;

const Body = styled.p`
  font-size: 12px;
  color: var(--aegis-mute-on-soft);
  line-height: 1.6;
  max-width: 560px;
`;

const SkelRow = styled.div<{ w: string }>`
  height: 12px;
  width: ${(p) => p.w};
  max-width: 480px;
  align-self: flex-start;
  border-radius: var(--aegis-radius-sm);
  background: linear-gradient(
    90deg,
    var(--aegis-surface-inset) 25%,
    var(--aegis-surface-soft) 50%,
    var(--aegis-surface-inset) 75%
  );
  background-size: 200% 100%;
  /* 周期 1.4s 属「循环速率」而非「过渡时长」，按 motion 契约刻意不入 duration 刻度
     （混进去会让人以为改 slow 档会连带改骨架速度）；缓动则取令牌。 */
  animation: aegis-sk 1.4s infinite var(--aegis-motion-easing-linear);
  @keyframes aegis-sk {
    to {
      background-position: -200% 0;
    }
  }
  /* 规范 §8.3 / ADR-006 §三 P1-5：prefers-reduced-motion 下**停在这一帧**，
     而不是把时长压到 0.01ms —— 无限动画 + 0.01ms 会在 60fps 下每帧重放约 1670 次
     （theme.ts 已记载该教训并整段移除了全局兜底，故此处由组件局部实现）。
     🔴 本处曾是本域唯一漏做reduced-motion 的 infinite 动画（2026-10-04 审查「一般 2」）：
     nav-rail 的流光环与 live-dot 的呼吸点都已就位，只有骨架屏漏了。
     停帧后渐变仍在、信息不丢失，只是不再扫光。 */
  @media (prefers-reduced-motion: reduce) {
    animation: none;
  }
`;

export function EmptyStateBlock({
  icon,
  titleKey,
  bodyKey,
  ns = "common",
  actionLabel,
  onAction,
}: {
  icon?: React.ReactNode;
  titleKey: string;
  bodyKey: string;
  ns?: string;
  actionLabel?: string;
  onAction?: () => void;
}) {
  const { t } = useTranslation([ns, "common"]);
  return (
    <Frame aria-live="polite">
      {icon ?? (
        <svg
          width="28"
          height="28"
          aria-hidden="true"
          viewBox="0 0 24 24"
          fill="none"
          stroke="currentColor"
          strokeWidth="1.5"
          style={{ color: "var(--aegis-mute)" }}
        >
          <circle cx="11" cy="11" r="6.5" />
          <path d="M16 16l4.5 4.5" />
        </svg>
      )}
      <Title>{t(titleKey, { ns })}</Title>
      <Body>{t(bodyKey, { ns })}</Body>
      {onAction && actionLabel ? (
        <AppButton variant="primary" type="button" onClick={onAction}>
          {actionLabel}
        </AppButton>
      ) : null}
    </Frame>
  );
}

export function ErrorStateBlock({
  title,
  body,
  onRetryLabel,
  onRetry,
}: {
  title: string;
  body: React.ReactNode;
  onRetryLabel?: string;
  onRetry?: () => void;
}) {
  return (
    <Frame aria-live="assertive">
      <svg
        width="28"
        height="28"
        aria-hidden="true"
        viewBox="0 0 24 24"
        fill="none"
        stroke="currentColor"
        strokeWidth="1.5"
        style={{ color: "var(--aegis-error)" }}
      >
        <path d="M12 4l9 16H3l9-16zM12 10v4M12 17h.01" />
      </svg>
      <Title style={{ color: "var(--aegis-error)" }}>{title}</Title>
      <Body>{body}</Body>
      {onRetry && onRetryLabel ? (
        <AppButton variant="primary" type="button" onClick={onRetry}>
          {onRetryLabel}
        </AppButton>
      ) : null}
    </Frame>
  );
}

export function LoadingSkeleton({ rows = 4 }: { rows?: number }) {
  const widths = ["38%", "86%", "64%", "90%", "52%", "78%"];
  return (
    <Frame aria-busy="true">
      {Array.from({ length: rows }, (_, i) => (
        <SkelRow key={i} w={widths[i % widths.length] ?? "80%"} />
      ))}
    </Frame>
  );
}
