// §三 组件最小集的壳层子集：button 四变体（primary/secondary/ghost/danger）。
// W1-05 基础组件库的完整交付（表格/表单/向导/抽屉等）仍在途，此处仅外壳所需。

import styled from "@emotion/styled";

type ButtonVariant = "primary" | "secondary" | "ghost" | "danger";

export const AppButton = styled.button<{ variant: ButtonVariant; size?: "md" | "sm" }>`
  display: inline-flex;
  align-items: center;
  gap: var(--aegis-space-xs);
  height: ${(p) => (p.size === "sm" ? "28px" : "36px")};
  padding: 0 var(--aegis-space-lg);
  border-radius: var(--aegis-rounded-sm);
  font: inherit;
  font-size: 14px;
  font-weight: 700;
  white-space: nowrap;
  cursor: pointer;
  border: 1px solid transparent;
  transition:
    background-color 120ms ease,
    border-color 120ms ease,
    color 120ms ease;
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
    &:hover { background: rgba(118, 185, 0, 0.06); }
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
    background: var(--aegis-error);
    color: var(--aegis-on-dark);
    &:hover { background: color-mix(in srgb, var(--aegis-error) 88%, var(--aegis-ink)); }
    &:active { background: #650b0b; }
  `}
  &:disabled {
    background: var(--aegis-surface-soft);
    color: var(--aegis-disabled);
    border-color: transparent;
    cursor: not-allowed;
  }
`;
