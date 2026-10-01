// 外壳占位页共用件：页头 + 空态三态件（空必须写明原因，R-08 纪律的壳层落实）。
import { useTranslation } from "react-i18next";
import { PageHeader } from "@/ui/components/page-header";
import { EmptyStateBlock } from "@/ui/components/state-block";
import type { ShellModule } from "./modules";

export function ModulePlaceholder({ module: m }: { module: ShellModule }) {
  const { t } = useTranslation(["shell", "common"]);
  const outOfScope = m.featureDir === null;
  const Icon = m.icon;
  return (
    <>
      <PageHeader
        title={t(m.nameKey)}
        sub={
          outOfScope
            ? t("state.notStarted.subOutOfScope", { ns: "common" })
            : t("state.notStarted.subFeature", { ns: "common", milestone: m.milestone ?? "-" })
        }
      />
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
    </>
  );
}
