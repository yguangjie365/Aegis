import { useTranslation } from "react-i18next";
import { useNavigate } from "react-router-dom";
import { PageHeader } from "@/ui/components/page-header";
import { EmptyStateBlock } from "@/ui/components/state-block";

export function NotFoundPage() {
  const { t } = useTranslation(["common", "shell"]);
  const navigate = useNavigate();
  // router v7 的 navigate 返回可空 Promise（惰性路由解码路径）；显式 void 丢弃，
  // 不把 Promise 泄进 void 型回调，也不留飞行 Promise
  const goOverview = () => {
    void navigate("/overview");
  };
  return (
    <>
      <PageHeader title="404" />
      <EmptyStateBlock
        titleKey="state.notFound.title"
        bodyKey="state.notFound.body"
        ns="common"
        actionLabel={t("nav.item.overview", { ns: "shell" })}
        onAction={goOverview}
      />
    </>
  );
}
