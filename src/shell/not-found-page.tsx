import { useMemo } from "react";
import { useTranslation } from "react-i18next";
import { useNavigate } from "react-router-dom";
import { EmptyStateBlock } from "@/ui/components/state-block";
import { useRegisterBrand } from "./brand-bar";

export function NotFoundPage() {
  const { t } = useTranslation(["common", "shell"]);
  const navigate = useNavigate();
  // router v7 的 navigate 返回可空 Promise（惰性路由解码路径）；显式 void 丢弃，
  // 不把 Promise 泄进 void 型回调，也不留飞行 Promise
  const goOverview = () => {
    void navigate("/overview");
  };
  /* 404 不在 modules.ts 里，壳层无法派生主标题 ⇒ 页面自己注册 "404"
     （品牌区的 title 覆盖能力就是为这类路由准备的，规范 §三）。 */
  const content = useMemo(() => ({ title: "404" }), []);
  useRegisterBrand("/404", content);
  return (
    <EmptyStateBlock
      titleKey="state.notFound.title"
      bodyKey="state.notFound.body"
      ns="common"
      actionLabel={t("nav.item.overview", { ns: "shell" })}
      onAction={goOverview}
    />
  );
}
