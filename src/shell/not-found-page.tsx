import { useMemo } from "react";
import { useTranslation } from "react-i18next";
import { useLocation, useNavigate } from "react-router-dom";
import { EmptyStateBlock } from "@/ui/components/state-block";
import { useRegisterBrand } from "./brand-bar";

export function NotFoundPage() {
  const { t } = useTranslation(["common", "shell"]);
  const navigate = useNavigate();
  const { pathname } = useLocation();
  // router v7 的 navigate 返回可空 Promise（惰性路由解码路径）；显式 void 丢弃，
  // 不把 Promise 泄进 void 型回调，也不留飞行 Promise
  const goOverview = () => {
    void navigate("/overview");
  };
  /* 404 不在 modules.ts 里，壳层无法派生主标题 ⇒ 页面自己注册 "404"
     （品牌区的 title 覆盖能力就是为这类路由准备的，规范 §三）。
     🔴 2026-10-06（代码审查 #13）：owner 参数由硬编码 "/404" 改为真实 pathname ——
     本页挂在 path="*" 上，真实地址是任意坏路径，原值恒 ≠ pathname ⇒ owns 恒 false。
     今天只注册 title（title 不受 owns 门禁）故无可见缺陷，但一旦本页要挂
     sub/actions 就会**静默不渲染**（违反「禁静默失败」）。owner = 本页真正拥有的
     路由 = 当前地址。 */
  const content = useMemo(() => ({ title: "404" }), []);
  useRegisterBrand(pathname, content);
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
