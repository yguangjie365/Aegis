import { useTranslation } from "react-i18next";
import styled from "@emotion/styled";
import type { TempBadge } from "@/types/overview";

// temp-badge：F-01 卡右上角温度徽章（R-09 分级 + R-08 诚实降级）。
// 🔴 三态分流在 Rust 侧已判完（badges.rs::build_badge），此处只渲染：
//   measured → 温度数字；undetected → 「未检测」中性样式（有温度语义但本次无证据）；
//   notApplicable → 🔴 整个徽章不渲染（该硬件本次无温度来源），不得用占位文字顶位。

const Chip = styled.span<{ tone: "normal" | "warning" | "critical" | "mute" }>`
  display: inline-flex;
  align-items: center;
  gap: 3px;
  height: 20px;
  padding: 0 7px;
  border-radius: var(--aegis-radius-full);
  font-size: 12px;
  font-weight: 700;
  line-height: 1;
  white-space: nowrap;
  background: ${(p) =>
    p.tone === "critical"
      ? "var(--aegis-error)"
      : p.tone === "warning"
        ? "rgba(239, 145, 0, 0.16)"
        : p.tone === "mute"
          ? "transparent"
          : "var(--aegis-surface-inset)"};
  color: ${(p) =>
    p.tone === "critical"
      ? "var(--aegis-on-dark)"
      : p.tone === "warning"
        ? "var(--aegis-warning)"
        : p.tone === "mute"
          ? "var(--aegis-mute)"
          : "var(--aegis-ink)"};
  /* 未检测走虚线：与"有读数的灰底"在视觉上分开，避免被读成"温度正常" */
  border: 1px ${(p) => (p.tone === "mute" ? "dashed" : "solid")}
    ${(p) => (p.tone === "mute" ? "var(--aegis-hairline)" : "transparent")};
`;

export function TempBadgeChip({ badge }: { badge: TempBadge }) {
  const { t } = useTranslation("overview");
  if (badge.state === "notApplicable") return null;
  if (badge.state === "undetected" || badge.temperatureC === null) {
    return <Chip tone="mute">{t("card.badge.undetected")}</Chip>;
  }
  const tone = badge.level ?? "normal";
  return (
    <Chip tone={tone} title={t(`badge.level.${tone}`)}>
      {Math.round(badge.temperatureC)}℃
    </Chip>
  );
}
