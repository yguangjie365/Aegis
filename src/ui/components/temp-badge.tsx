import { useTranslation } from "react-i18next";
import { Badge } from "./badge";
import type { TempBadge } from "@/types/overview";

// temp-badge：F-01 卡右上角温度徽章（R-09 分级 + R-08 诚实降级）。
// 🔴 三态分流在 Rust 侧已判完（badges.rs::build_badge），此处只渲染：
//   measured → 温度数字；undetected → 「未检测」中性样式（有温度语义但本次无证据）；
//   notApplicable → 🔴 整个徽章不渲染（该硬件本次无温度来源），不得用占位文字顶位。

/* 🔴 外形已上收到 ./badge（2026-10-06 UI 审查 M11）：本文件只留**分级 → tone/emphasis** 的映射。
   critical 走实心（温度越界是必须被看见的告警，是既有用例、不是新发明）；
   undetected 走 dashed —— 与"有读数的灰底"在视觉上分开，否则会被读成"温度正常"。
   原裸色 rgba(239,145,0,.16) 已随 M11 由 warning 令牌派生取代。 */
type ChipTone = "normal" | "warning" | "critical" | "mute";

const CHIP: Record<
  ChipTone,
  { tone: "neutral" | "warning" | "error"; emphasis: "tint" | "solid" | "dashed" }
> = {
  normal: { tone: "neutral", emphasis: "tint" },
  warning: { tone: "warning", emphasis: "tint" },
  critical: { tone: "error", emphasis: "solid" },
  mute: { tone: "neutral", emphasis: "dashed" },
};

function Chip({
  tone,
  title,
  children,
}: {
  tone: ChipTone;
  title?: string;
  children: React.ReactNode;
}) {
  const c = CHIP[tone];
  return (
    <Badge tone={c.tone} emphasis={c.emphasis} title={title}>
      {children}
    </Badge>
  );
}

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
