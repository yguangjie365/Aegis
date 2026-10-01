import { useTranslation } from "react-i18next";
import styled from "@emotion/styled";
import { isTauriRuntime } from "@/lib/tauri/invoke";

// 状态条（§三 双色模的黑带件）：26px 黑底 micro 字号。
// 左槽 = 运行态文本；偏离③已批准（2026-10-02，规范 §三 status-bar 行）：降级计数/日志入口槽位只预留，随 W1-08 接入；
// 右槽 = 版本 + 零遥测声明（常驻红线呈现）。

const Bar = styled.footer`
  grid-column: 1 / 3;
  height: 26px;
  background: var(--aegis-chrome-dark);
  color: var(--aegis-on-dark-mute);
  font-size: 11px;
  line-height: 1.4;
  display: flex;
  align-items: center;
  gap: 16px;
  padding: 0 12px;
  flex: none;
`;

const Right = styled.span`
  margin-left: auto;
  font-family: var(--font-mono);
`;

const Dot = styled.span`
  width: 7px;
  height: 7px;
  border-radius: var(--aegis-rounded-full);
  background: var(--aegis-accent);
  flex: none;
`;

const Left = styled.span`
  display: inline-flex;
  align-items: center;
  gap: 7px;
`;

export function StatusBar() {
  const { t } = useTranslation("shell");
  return (
    <Bar>
      <Left>
        <Dot aria-hidden="true" />
        <span>{isTauriRuntime() ? t("status.ready") : t("status.browserPreview")}</span>
      </Left>
      <Right>
        {t("status.version")} · {t("status.noTelemetry")}
      </Right>
    </Bar>
  );
}
