import { NavLink } from "react-router-dom";
import { useTranslation } from "react-i18next";
import styled from "@emotion/styled";
import { PanelLeftClose } from "lucide-react";
import { SHELL_MODULES, type ShellModule } from "./modules";
import { useShellStore } from "./store";

// nav-rail（§三）：192px 展开 ↔ 64px 折叠（2026-10-02 用户要求自 232 收窄）；黑底 chrome；激活项 = chrome-elevated 底 +
// 2px accent 左竖条（黑底上 accent 8.71:1 合规）+ 白字 700。
// 折叠态交互契约（方案 A-2 定稿）：tooltip 与 aria-label 同源 i18n key；折叠按钮进 Tab 序；
// 不做响应式自动切换——两态由用户显式控制（§四 无断点纪律）。
// 两态开关用 transient prop（$folded）：布尔 prop 直传原生元素会被 React 写进 DOM 属性并告警。

const Rail = styled.nav<{ $folded: boolean }>`
  grid-row: 2;
  background: var(--aegis-chrome-dark);
  display: flex;
  flex-direction: column;
  padding: 8px 0;
  width: ${(p) => (p.$folded ? "64px" : "192px")};
  --nav-item-mx: ${(p) => (p.$folded ? "14px" : "8px")};
  overflow-x: hidden;
  flex: none;
`;

const FoldBtn = styled.button<{ $folded: boolean }>`
  display: flex;
  align-items: center;
  gap: 10px;
  height: 32px;
  margin: 8px 8px 4px;
  padding: ${(p) => (p.$folded ? "0" : "0 12px")};
  justify-content: center;
  color: var(--aegis-on-dark-mute);
  font-size: 12px;
  font-family: inherit;
  background: transparent;
  /* 黑底 chrome 专用发丝线，取值同规范 §五 Dark 包 hairline（Light 的 #cccccc 在黑上过亮） */
  border: 1px solid #3a3a3a;
  border-radius: var(--aegis-rounded-sm);
  cursor: pointer;
  transition:
    background-color 120ms ease,
    border-color 120ms ease,
    color 120ms ease;
  &:focus-visible {
    outline-color: var(--aegis-accent);
  }
  &:hover {
    color: var(--aegis-on-dark);
    border-color: var(--aegis-stone);
  }
  & svg {
    transform: ${(p) => (p.$folded ? "rotate(180deg)" : "none")};
    flex: none;
  }
`;

const GroupLabel = styled.div<{ $folded: boolean }>`
  font-size: ${(p) => (p.$folded ? "10px" : "11px")};
  font-weight: 700;
  letter-spacing: 0.04em;
  /* 黑底分组眉题：低于 on-dark-mute 的三级灰，仅承载分组语义，不作正文 */
  color: #6b6b6b;
  padding: ${(p) => (p.$folded ? "10px 0 4px" : "12px 14px 4px")};
  text-align: center;
  text-transform: uppercase;
`;

// 两态差异经 Rail 上的 CSS 变量下传：styled(NavLink) 是自定义组件，emotion 不会自动过滤
// 自定义 prop（$ 前缀只对原生元素生效），透传会触发 React 非法属性告警。
const Item = styled(NavLink)`
  display: flex;
  align-items: center;
  justify-content: center;
  gap: 10px;
  height: 44px;
  margin: 3px var(--nav-item-mx);
  padding: 0;
  color: var(--aegis-on-dark-mute);
  font-size: 14px;
  text-decoration: none;
  position: relative;
  flex: none;
  border-radius: var(--aegis-rounded-sm);
  transition:
    background-color 120ms ease,
    color 120ms ease;
  /* 规范 §八：黑底焦点环用 accent（浅底才是 accent-strong） */
  &:focus-visible {
    outline-color: var(--aegis-accent);
  }
  & span {
    overflow: hidden;
    white-space: nowrap;
  }
  &.active {
    background: var(--aegis-chrome-elevated);
    color: var(--aegis-on-dark);
    font-weight: 700;
    &::before {
      content: "";
      position: absolute;
      left: 0;
      top: 0;
      bottom: 0;
      width: 2px;
      background: var(--aegis-accent);
    }
  }
  &:hover:not(.active) {
    color: var(--aegis-on-dark);
    background: rgba(255, 255, 255, 0.06);
  }
`;

const Spacer = styled.div`
  flex: 1;
`;

function RailItem({ m, folded }: { m: ShellModule; folded: boolean }) {
  const { t } = useTranslation("shell");
  const label = t(m.nameKey);
  const Icon = m.icon;
  return (
    <Item to={m.route} title={folded ? label : undefined} aria-label={label}>
      <Icon size={20} strokeWidth={1.5} aria-hidden="true" />
      {!folded && <span>{label}</span>}
    </Item>
  );
}

export function NavRail() {
  const { t } = useTranslation("shell");
  const folded = useShellStore((s) => s.navFolded);
  const toggle = useShellStore((s) => s.toggleNavFolded);
  const modules = SHELL_MODULES.filter((m) => m.group === "modules");
  const orchestration = SHELL_MODULES.filter((m) => m.group === "orchestration");
  const utility = SHELL_MODULES.filter((m) => m.group === "utility");

  return (
    <Rail $folded={folded} aria-label={t("nav.toggleAria")}>
      {modules.map((m) => (
        <RailItem key={m.id} m={m} folded={folded} />
      ))}
      <GroupLabel $folded={folded}>{t("nav.group.orchestration")}</GroupLabel>
      {orchestration.map((m) => (
        <RailItem key={m.id} m={m} folded={folded} />
      ))}
      <Spacer />
      <FoldBtn
        type="button"
        $folded={folded}
        onClick={toggle}
        aria-expanded={!folded}
        aria-label={t("nav.toggleAria")}
      >
        <PanelLeftClose size={16} strokeWidth={1.5} aria-hidden="true" />
        {!folded && <span>{t("nav.fold")}</span>}
      </FoldBtn>
      {utility.map((m) => (
        <RailItem key={m.id} m={m} folded={folded} />
      ))}
    </Rail>
  );
}
