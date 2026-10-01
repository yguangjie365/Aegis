import { useTranslation } from "react-i18next";
import styled from "@emotion/styled";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { Minus, Square, X } from "lucide-react";
import { isTauriRuntime } from "@/lib/tauri/invoke";

// 32px 自绘标题条（Tauri decorations=false 方案）。🔴 最大化按钮恒灰——§四"禁最大化/全屏"的界面化表达。

const Bar = styled.header`
  grid-column: 1 / 3;
  height: 32px;
  background: var(--aegis-chrome-dark);
  color: var(--aegis-on-dark);
  display: flex;
  align-items: center;
  padding: 0 8px 0 12px;
  font-size: 12px;
  user-select: none;
  -webkit-user-select: none;
`;

const Title = styled.span`
  font-weight: 700;
  letter-spacing: 0.04em;
`;

const Codename = styled.span`
  color: var(--aegis-on-dark-mute);
  margin-left: 10px;
`;

const Caps = styled.div`
  margin-left: auto;
  display: flex;
`;

const CapBtn = styled.button<{ disabled?: boolean; $danger?: boolean }>`
  width: 44px;
  height: 32px;
  display: flex;
  align-items: center;
  justify-content: center;
  border: 0;
  background: transparent;
  color: ${(p) => (p.disabled ? "var(--aegis-disabled)" : "var(--aegis-on-dark-mute)")};
  cursor: ${(p) => (p.disabled ? "default" : "pointer")};
  transition: background-color 120ms ease, color 120ms ease;
  &:focus-visible {
    outline-color: var(--aegis-accent);
  }
  &:hover:not(:disabled) {
    background: ${(p) => (p.$danger ? "var(--aegis-error)" : "var(--aegis-chrome-elevated)")};
    color: var(--aegis-on-dark);
  }
`;

export function TitleBar() {
  const { t } = useTranslation("shell");
  const live = isTauriRuntime();
  return (
    <Bar data-tauri-drag-region>
      <Title>{t("app.title")}</Title>
      <Codename>{t("app.codename")}</Codename>
      <Caps>
        <CapBtn
          type="button"
          aria-label={t("window.minimize")}
          title={live ? t("window.minimize") : t("status.browserPreview")}
          disabled={!live}
          onClick={() => void getCurrentWindow().minimize()}
        >
          <Minus size={14} strokeWidth={1.5} aria-hidden="true" />
        </CapBtn>
        <CapBtn type="button" aria-label={t("window.maximizeDisabled")} title={t("window.maximizeDisabled")} disabled>
          <Square size={12} strokeWidth={1.5} aria-hidden="true" />
        </CapBtn>
        <CapBtn
          type="button"
          $danger
          aria-label={t("window.close")}
          title={live ? t("window.close") : t("status.browserPreview")}
          disabled={!live}
          onClick={() => void getCurrentWindow().close()}
        >
          <X size={14} strokeWidth={1.5} aria-hidden="true" />
        </CapBtn>
      </Caps>
    </Bar>
  );
}
