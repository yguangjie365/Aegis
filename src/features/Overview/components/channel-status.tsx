import { useTranslation } from "react-i18next";
import styled from "@emotion/styled";
import type { MonitorStatus } from "@/types/overview";

// 传感器通道状态行（F-12 降级留痕的用户可见面 / N-06「每次降级都要说清为什么」）。
//
// 就绪时整行不渲染（常态不该被一行系统噪音占位）；其余四态必须给出**具体故障形态**，
// 因为这一刻界面里"为什么没有温度"只有这一处能回答。
// 内核驱动引导按 2026-10-03 拍板走**纯文字 + 可复制 URL**：不放按钮、不开浏览器、零外呼（N-08）。

const Row = styled.div`
  display: flex;
  flex-wrap: wrap;
  align-items: baseline;
  gap: var(--aegis-space-xs) var(--aegis-space-sm);
  padding: var(--aegis-space-sm) var(--aegis-space-md);
  border: 1px solid var(--aegis-hairline);
  border-radius: var(--aegis-radius-sm);
  background: var(--aegis-surface-soft);
  font-size: 12px;
  line-height: 1.5;
  color: var(--aegis-mute-on-soft);
`;

const StateName = styled.strong`
  color: var(--aegis-warning);
  font-weight: 700;
  white-space: nowrap;
`;

// URL 可选中复制但**不可点击**：`user-select` 保文本，`a` 元素会引入外呼入口
const Url = styled.code`
  font-family: var(--font-mono);
  user-select: all;
  color: var(--aegis-ink);
`;

const DRIVER_REPO_URL = "https://github.com/namazso/PawnIO";

export function ChannelStatus({ monitor }: { monitor: MonitorStatus }) {
  const { t } = useTranslation("overview");
  if (monitor.state === "ready") return null;
  return (
    <Row role="status">
      <StateName>{t(`monitor.state.${monitor.state}`)}</StateName>
      <span>{monitor.reason ?? t("monitor.reason.unknown")}</span>
      {!monitor.driverInstalled ? (
        <span>
          {t("monitor.driver.guide")} <Url>{DRIVER_REPO_URL}</Url>
        </span>
      ) : null}
    </Row>
  );
}
