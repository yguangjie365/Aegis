import styled from "@emotion/styled";

/* ═══════════════════════════════════════════════════════════════════════════
   Badge —— 状态徽章的唯一实现（规范 §三 badge 行 + §三·1 非按钮项档 + D6）
   ═══════════════════════════════════════════════════════════════════════════
   来历（2026-10-06 UI 审查 M2/M11）：此前全站有**三份**同构徽章 ——
     ① `ui/components/temp-badge.tsx` 的 Chip（概览温度徽章）
     ② `features/FileBackup/components/status-badge.tsx` 的 Pill（快照/校验/启用/草稿四种）
     ③ `features/FileTransfer/pages/file-transfer-page.tsx` 的 Badge（服务状态，注册进品牌区）
   三份的色配方在 2026-10-06 已被统一成「14% 淡底 + 42% 描边 + 令牌文字色」，
   但仍是三处声明 —— 正是 §三·1 立约缘由点名的漂移温床（「锁在令牌层的会一致、写在各页的会漂移」）。
   本文件把三处合并成一件，三轴各自都有**现存调用方**，不是预留：

   · tone      色彩语义（§一 #1 单强调色：绿只归 accent，状态走 success/warning/error）
   · emphasis  强度：tint（默认，淡底+描边）/ solid（实心，仅"必须被看见"的告警）/ dashed（无值占位）
   · size      sm = 20（表内/卡内）· bar = 24（§三·1 品牌区非按钮项恒 24）

   🔴 圆角恒 `radius.sm`（2026-10-06 D6 用户裁决）：§二 rounded 的 `full` 档只给头像与状态圆点，
      徽章属交互元素语系 ⇒ 方角。三份旧实现都是胶囊，已一并改掉。
   🔴 字号走 caption-uppercase 档（12/700 + 0.04em 字距 + 大写化）—— §三 badge 行的字面要求；
      中文不受 uppercase 影响，拉丁标签（如 "LIVE"）本就大写，故零视觉代价。
   ═══════════════════════════════════════════════════════════════════════════ */

export type BadgeTone = "neutral" | "success" | "warning" | "error";
export type BadgeEmphasis = "tint" | "solid" | "dashed";
export type BadgeSize = "sm" | "bar";

const HEIGHT: Record<BadgeSize, string> = { sm: "20px", bar: "24px" };

/** 各 tone 的主色（solid 与 tint 共用同一个来源，避免两套色值各改各的）。 */
const MAIN: Record<BadgeTone, string> = {
  neutral: "var(--aegis-ink)",
  success: "var(--aegis-success)",
  warning: "var(--aegis-warning)",
  error: "var(--aegis-error)",
};

const Frame = styled.span<{
  $tone: BadgeTone;
  $emphasis: BadgeEmphasis;
  $size: BadgeSize;
}>`
  display: inline-flex;
  align-items: center;
  height: ${(p) => HEIGHT[p.$size]};
  /* §三 badge：pad 4×10 —— 纵向 4 + 12/1 行高 = 20（sm 档），横向 10 直接落值 */
  padding: 0 10px;
  border-radius: var(--aegis-radius-sm);
  font-size: var(--aegis-type-caption-uppercase-size);
  font-weight: var(--aegis-type-caption-uppercase-weight);
  line-height: 1;
  letter-spacing: var(--aegis-type-caption-uppercase-tracking);
  text-transform: var(--aegis-type-caption-uppercase-transform);
  white-space: nowrap;
  flex: none;

  ${(p) => {
    const main = MAIN[p.$tone];
    if (p.$emphasis === "solid") {
      /* 实心只留给"必须被看见"的告警（概览温度 critical 是既有用例）：
         文字走 on-dark 白 —— error 的白字对比由 error-fill 岗位保证（§五 Dark 表）。 */
      return `
        background: ${p.$tone === "error" ? "var(--aegis-error-fill)" : main};
        color: var(--aegis-on-dark);
        border: 1px solid transparent;
      `;
    }
    if (p.$emphasis === "dashed") {
      /* 无值占位（"未检测"）：虚线 + 透明底，与"有读数的灰底"在视觉上分开，
         否则会被读成"温度正常"（temp-badge 原注释的口径，原样保留）。 */
      return `
        background: transparent;
        color: var(--aegis-mute);
        border: 1px dashed var(--aegis-hairline);
      `;
    }
    if (p.$tone === "neutral") {
      return `
        background: var(--aegis-surface-inset);
        color: var(--aegis-ink);
        border: 1px solid transparent;
      `;
    }
    return `
      background: color-mix(in srgb, ${main} 14%, transparent);
      border: 1px solid color-mix(in srgb, ${main} 42%, transparent);
      color: ${main};
    `;
  }}
`;

export function Badge({
  tone = "neutral",
  emphasis = "tint",
  size = "sm",
  title,
  children,
}: {
  tone?: BadgeTone;
  emphasis?: BadgeEmphasis;
  size?: BadgeSize;
  /** 悬停说明（走 i18n，禁裸中文） */
  title?: string;
  children: React.ReactNode;
}) {
  return (
    <Frame $tone={tone} $emphasis={emphasis} $size={size} title={title}>
      {children}
    </Frame>
  );
}
