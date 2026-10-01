import { useTranslation } from "react-i18next";
import { useNavigate } from "react-router-dom";
import { PageHeader } from "@/ui/components/page-header";
import { EmptyStateBlock } from "@/ui/components/state-block";

export function NotFoundPage() {
  const { t } = useTranslation(["common", "shell"]);
  const navigate = useNavigate();
  return (
    <>
      <PageHeader title="404" />
      <EmptyStateBlock
        titleKey="state.notFound.title"
        bodyKey="state.notFound.body"
        ns="common"
        actionLabel={t("nav.item.overview", { ns: "shell" })}
        onAction={() => navigate("/overview")}
      />
    </>
  );
}
