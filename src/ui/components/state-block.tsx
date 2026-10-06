import { useTranslation } from "react-i18next";
import styled from "@emotion/styled";
import { AppButton } from "./app-button";
import { FailureDetail } from "./failure-detail";

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

/* 🔴 2026-10-06 批次⑤：字号一律取令牌，不再写裸值。
   Title 原 15px 不在 12/13/14/16/17/20/24/32 阶梯上（唯一一处），现归 h3；
   Body 原 12px 数值本就等于 caption，改的是"由令牌定义"而非"由组件私自决定"。 */
const Title = styled.h2`
  font-size: var(--aegis-type-h3-size);
  font-weight: var(--aegis-type-h3-weight);
  line-height: var(--aegis-type-h3-leading);
`;

const Body = styled.p`
  font-size: var(--aegis-type-caption-size);
  line-height: var(--aegis-type-caption-leading);
  color: var(--aegis-mute-on-soft);
  max-width: 560px;
`;

const SkelRow = styled.div<{ w: string; $busy: boolean }>`
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
     （混进去会让人以为改 slow 档会连带改骨架速度）；缓动则取令牌。
     🔴 $busy=false 时**不扫光**：同一套轮廓也用来在失败态顶住版面（UI 审查 m2/m3），
     那时它表达的是「这里本该有内容」，不是「正在加载」—— 会动的条子等于撒这个谎。 */
  animation: ${(p) => (p.$busy ? "aegis-sk 1.4s infinite var(--aegis-motion-easing-linear)" : "none")};
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
  actionVariant = "primary",
}: {
  icon?: React.ReactNode;
  titleKey: string;
  bodyKey: string;
  ns?: string;
  actionLabel?: string;
  onAction?: () => void;
  /**
   * 🔴 2026-10-06 UI 审查 D8：§三 要求空态自带**主 CTA**，而 §一 #1 限定同屏实心绿 ≤ 1。
   * 两者在「品牌区已经放了实心主操作」的页面上直接撞车（`/file-backup` 实测就是：
   * 品牌区「新建规则」实心 + 空态「新建规则」实心 = 同屏两枚）。
   * 与 `ErrorStateBlock.retryVariant` 同设计：语义不变、仍是可点的动作，只是让位给工具栏。
   * 默认 `primary`（空态是本页唯一主操作时正确，如全屏空态）；
   * 品牌区已有实心主操作的**分区内**空态传 `secondary`。
   */
  actionVariant?: "primary" | "secondary";
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
        <AppButton variant={actionVariant} type="button" onClick={onAction}>
          {actionLabel}
        </AppButton>
      ) : null}
    </Frame>
  );
}

export function ErrorStateBlock({
  title,
  body,
  detail,
  onRetryLabel,
  onRetry,
  retryVariant = "primary",
}: {
  title: string;
  body: React.ReactNode;
  /**
   * 🔴 技术标识（后端命令名等）收进「详情」折叠，不进主线（2026-10-06 UI 审查 D7）。
   * 传 `failureDetail(error)` 的结果；为 `null` 时不出现折叠，与不传完全一样。
   */
  detail?: string | null;
  onRetryLabel?: string;
  onRetry?: () => void;
  /**
   * 🔴 2026-10-06 批次⑤：重试按钮的视觉权重由调用方定。
   * 默认 `primary`（失败态是本页唯一主操作时正确）；但当工具栏**已有主刷新**时，
   * 失败态里的「重试」再给 primary 就出现两个同权重主 CTA 争抢（Overview 实测）。
   * 那种场景传 `secondary`：语义不变、仍是可点的重试，视觉让位给工具栏。
   */
  retryVariant?: "primary" | "secondary";
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
      <FailureDetail detail={detail ?? null} />
      {onRetry && onRetryLabel ? (
        <AppButton variant={retryVariant} type="button" onClick={onRetry}>
          {onRetryLabel}
        </AppButton>
      ) : null}
    </Frame>
  );
}

/**
 * 骨架行组。
 *
 * `busy` 默认 true（加载态：扫光 + `aria-busy`）。
 * 🔴 传 false 得到**静止**的同一套轮廓 —— 失败态用它顶住版面而不谎称在加载
 * （2026-10-06 UI 审查 m2/m3：整栏/整页在失败时塌成一块，剩下的空白比错误信息更抢眼）。
 */
export function LoadingSkeleton({ rows = 4, busy = true }: { rows?: number; busy?: boolean }) {
  const widths = ["38%", "86%", "64%", "90%", "52%", "78%"];
  return (
    <Frame aria-busy={busy}>
      {Array.from({ length: rows }, (_, i) => (
        <SkelRow key={i} w={widths[i % widths.length] ?? "80%"} $busy={busy} />
      ))}
    </Frame>
  );
}
