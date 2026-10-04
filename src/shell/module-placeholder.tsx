// 外壳占位页共用件：空态三态件（空必须写明原因，R-08 纪律的壳层落实）。
//
// 🔴 2026-10-05 垂直迁移：原先这里的 PageHeader 已删除 —— 主标题由壳层品牌区
// 从 modules.ts 的 nameKey 自动派生（十模块零重复声明），页面只上报副标题。
import { useMemo } from "react";
import { useTranslation } from "react-i18next";
import { EmptyStateBlock } from "@/ui/components/state-block";
import { useRegisterBrand } from "./brand-bar";
import type { ShellModule } from "./modules";

export function ModulePlaceholder({ module: m }: { module: ShellModule }) {
  const { t } = useTranslation(["shell", "common"]);
  const outOfScope = m.featureDir === null;
  const Icon = m.icon;

  const sub = useMemo(
    () =>
      outOfScope
        ? t("state.notStarted.subOutOfScope", { ns: "common" })
        : t("state.notStarted.subFeature", { ns: "common", milestone: m.milestone ?? "-" }),
    [outOfScope, t, m.milestone],
  );
  useRegisterBrand(m.route, { sub });

  return (
    <EmptyStateBlock
      icon={
        <span style={{ color: "var(--aegis-mute)" }}>
          <Icon size={28} strokeWidth={1.5} aria-hidden="true" />
        </span>
      }
      titleKey="state.notStarted.title"
      bodyKey={outOfScope ? "state.notStarted.bodyOutOfScope" : "state.notStarted.bodyFeature"}
      ns="common"
    />
  );
}
