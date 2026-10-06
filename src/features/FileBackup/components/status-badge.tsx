// 状态徽章：快照终态与校验三态的**唯一**渲染实现。
//
// 🔴 为什么单独成文件（而不是在表格单元格里各写一遍）：
//   ① R-11「状态诚实」要求三态各有**独立措辞 + 独立色彩**，
//      两处各写一遍必然漂移（表格里写「未做完整校验」、弹窗里写「未校验」）。
//   ② 无障碍：状态**不能只靠颜色**（§八.2）—— 每处都同时给文字，
//      颜色只是辅助。色觉障碍用户读到的信息量与常人相同。
//   ③ 色彩一律 `var(--aegis-*)` 派生，换主题不会漂。

import { useTranslation } from "react-i18next";
import { Badge } from "@/ui/components/badge";
import { integrityKey, integrityTone, statusKey, statusTone } from "../lib/format";
import type { IntegrityStatus, SnapshotStatus } from "@/types/backup";

/* 🔴 外形已上收到 `ui/components/badge`（2026-10-06 UI 审查 M11）：
   本文件只保留**语义映射**（ok/warn/bad → success/warning/error）与 R-11 的三态措辞纪律。
   原先这里自己写了一份 Pill（含 14%/42% 色配方 + 胶囊圆角）—— 配方与 temp-badge、
   file-transfer 各写一遍，正是 §三·1 点名的漂移温床。圆角按 D6 裁决改方角。 */
function Pill({ $tone, children }: { $tone: "ok" | "warn" | "bad"; children: React.ReactNode }) {
  return (
    <Badge tone={$tone === "ok" ? "success" : $tone === "warn" ? "warning" : "error"}>
      {children}
    </Badge>
  );
}

/**
 * 校验徽章（三态）。
 *
 * 🔴 **不额外提供「简化版」**：调用方若想省掉 `notFullyVerified`，
 *   正确做法是加一个 prop 显式声明，而不是在这层悄悄折叠。
 */
export function IntegrityBadge({ integrity }: { integrity: IntegrityStatus }) {
  const { t } = useTranslation("backup");
  return <Pill $tone={integrityTone(integrity)}>{t(integrityKey(integrity))}</Pill>;
}

/** 快照终态徽章。 */
export function StatusBadge({ status }: { status: SnapshotStatus }) {
  const { t } = useTranslation("backup");
  return <Pill $tone={statusTone(status)}>{t(statusKey(status))}</Pill>;
}

/**
 * 启用/停用徽章。
 *
 * 🔴 停用态**不隐藏规则**（布局稿 §五.1）：隐藏会让用户以为规则丢了。
 *   降不透明度 + 文字徽章，规则本体仍完整可点。
 */
export function EnabledBadge({ enabled }: { enabled: boolean }) {
  const { t } = useTranslation("backup");
  return (
    <Pill $tone={enabled ? "ok" : "warn"}>{t(enabled ? "rule.enabled" : "rule.disabled")}</Pill>
  );
}

/** 草稿徽章（F-FB-04：来源全不存在）。 */
export function DraftBadge() {
  const { t } = useTranslation("backup");
  return <Pill $tone="warn">{t("rule.draft")}</Pill>;
}
