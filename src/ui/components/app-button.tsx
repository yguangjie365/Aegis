// §三 组件最小集的壳层子集：button 四变体（primary/secondary/ghost/danger）。
// W1-05 基础组件库的完整交付（表格/表单/向导/抽屉等）仍在途，此处仅外壳所需。

import styled from "@emotion/styled";

type ButtonVariant = "primary" | "secondary" | "ghost" | "danger";

/* 🔴 2026-10-04 复审修复 + CDP 实测发现：本组件是 **styled.button，不走 Chakra style config**
 * ⇒ theme.ts 的 `components.Button.sizes` 覆盖对它**完全无效**（实测：按钮高确实是 28，
 * 但那是本组件自己写的 height；`position` 与 `::after` 命中区一个都没落地）。
 * 主题层那份只对用 Chakra `<Button>` 的 50 处生效。
 * ⇒ 三档高度 / 命中区必须在**本组件内**也写一份，否则「统一控件规范」在这 29 处引用上是空的。
 * 长期出路仍是控件规范 §八 的 C4「AppButton 与 Chakra Button 收敛为单一实现」。 */
const SIZE_H = { sm: "28px", md: "36px", lg: "44px" } as const;
const SIZE_PX = { sm: "12px", md: "16px", lg: "20px" } as const;

export const AppButton = styled.button<{ variant: ButtonVariant; size?: "md" | "sm" | "lg" }>`
  display: inline-flex;
  align-items: center;
  gap: var(--aegis-space-xs);
  height: ${(p) => SIZE_H[p.size ?? "md"]};
  padding: 0 ${(p) => SIZE_PX[p.size ?? "md"]};
  border-radius: var(--aegis-radius-sm);
  /* 🔴 sm 档命中区补偿的定位宿主（控件规范 §5.1：视觉 28 / 命中 ≥32）。
     Chakra 的 Button baseStyle 不提供 position:relative，styled.button 更没有 ——
     缺了它，::after 会相对 BODY 定位，一块透明伪元素盖住整页并拦截点击。 */
  position: relative;
  ${(p) =>
    p.size === "sm" &&
    `
    &::after {
      content: "";
      position: absolute;
      top: -2px;
      bottom: -2px;
      left: 0;
      right: 0;
    }
  `}
  font: inherit;
  font-size: 14px;
  font-weight: 700;
  white-space: nowrap;
  cursor: pointer;
  border: 1px solid transparent;
  transition:
    background-color var(--aegis-motion-duration-fast) var(--aegis-motion-easing-standard),
    border-color var(--aegis-motion-duration-fast) var(--aegis-motion-easing-standard),
    color var(--aegis-motion-duration-fast) var(--aegis-motion-easing-standard);
  ${(p) =>
    p.variant === "primary" &&
    `
    background: var(--aegis-accent);
    color: var(--aegis-accent-on);
    &:hover { background: color-mix(in srgb, var(--aegis-accent) 90%, var(--aegis-ink)); }
    &:active { background: var(--aegis-accent-pressed); }
  `}
  ${(p) =>
    p.variant === "secondary" &&
    `
    background: transparent;
    /* 偏离①已批准（2026-10-02）：浅底 accent 仅 2.41:1 不达文字门禁，用 accent-strong；规范 §三 已同步 */
    border-color: var(--aegis-accent-strong);
    color: var(--aegis-accent-strong);
    /* 🔴 6% 淡绿底由令牌派生（2026-10-04 审查一般 7）：此前写死 rgba(118,185,0,0.06)，
       换主题时不会跟着变。同文件 :30 的 hover 已是 color-mix 写法，此处漏网。 */
    &:hover { background: color-mix(in srgb, var(--aegis-accent) 6%, transparent); }
  `}
  ${(p) =>
    p.variant === "ghost" &&
    `
    background: transparent;
    color: var(--aegis-mute);
    &:hover { background: var(--aegis-surface-soft); }
  `}
  ${(p) =>
    p.variant === "danger" &&
    `
    /* 🔴 底色用 error-fill 而非 error（2026-10-06 UI 审查批次②）：单令牌在数学上无解 ——
       error 必须留在「错误文字色」岗位（#ff5c5c 是为可读性从 #e52020 提亮来的），
       而白字压在 #ff5c5c 上实测仅 3.03:1 不达 4.5。证明见 tokens.ts COLOR_TOKEN_KEYS。 */
    background: var(--aegis-error-fill);
    color: var(--aegis-on-dark);
    &:hover { background: color-mix(in srgb, var(--aegis-error-fill) 88%, var(--aegis-ink)); }
    /* 🔴 按下态由底色令牌压深派生（2026-10-04 审查一般 7）：此前写死 #650b0b，
       与上方 background 是两套色值，改主题必漂。 */
    &:active { background: color-mix(in srgb, var(--aegis-error-fill) 62%, var(--aegis-ink)); }
  `}
  &:disabled {
    background: var(--aegis-surface-soft);
    color: var(--aegis-disabled);
    border-color: transparent;
    cursor: not-allowed;
  }
`;
