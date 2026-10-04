import { useTranslation } from "react-i18next";
import styled from "@emotion/styled";
import { Panel, PanelTitle, PanelNote, PanelReason } from "./panel";
import type { DiskHealthRowDto } from "@/types/overview";

// disk-health-panel：F-06 磁盘健康（母本 SystemToolkit「磁盘健康」分区）。
//
// 🔴 2026-10-04 重写：原实现整份用 Tailwind 类名（`text-muted-foreground` / `bg-background` /
// `rounded-lg`…），而本项目是**Chakra + Emotion 设计令牌制**、无 Tailwind 管线，
// 故那些类**一个都不生效** —— 面板实际渲染为无样式的裸 div（换皮遗留缺陷）。
// 全部改写为 `styled` + `var(--aegis-*)` 令牌，与 `KeyValueCard` / `NetEndpointsPanel` 同构。
//
// 🔴 2026-10-04 再改：迁入 `panel.tsx` 共享语汇（原先自有一套 surface-soft +
// elevation-sticky 阴影 + 600 字重，与 StatCard 互斥；详见 panel.tsx 头注）。
//
// 布局：行以单列纵向排列 —— **两列由页面级 `HealthGrid` 承担**（母本「网络与存储健康」区
// `UniformGrid Columns="2"` 装两个面板），面板内部再分两列会变成四列，与母本不符。

const RowList = styled.div`
  display: flex;
  flex-direction: column;
  gap: var(--aegis-space-xs);
`;

const Row = styled.div`
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: var(--aegis-space-sm);
  padding: var(--aegis-space-xs) var(--aegis-space-sm);
  border: 1px solid var(--aegis-hairline);
  border-radius: var(--aegis-radius-sm);
  background: var(--aegis-canvas);
  min-height: 32px;
`;

const Device = styled.span`
  flex: 1;
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  font-size: var(--aegis-type-body-size);
  /* 🔴 700 而非 500/600：规范 §一.4 只允许 400/700，且 500/600 在 CJK 回退字体上
     不存在（会被合成或就近取档，导致同页字重实际不一致）。设备名是本行的主标识，
     用 700 承担识别，与右侧元信息拉开层级。 */
  font-weight: 700;
  color: var(--aegis-ink);
`;

const Meta = styled.span`
  display: inline-flex;
  align-items: center;
  gap: var(--aegis-space-md);
  flex: none;
  font-size: var(--aegis-type-caption-size);
  color: var(--aegis-mute);
  white-space: nowrap;
`;

/**
 * 健康状态**文字**：颜色 + 文案双通道。
 *
 * 🔴 规范 §八「状态呈现不得只依赖颜色」—— 颜色只是强化，文字本身已经说明状态
 * （健康/ 警告 / 故障 / 未知），故色盲用户与灰度打印同样可读。
 * 色值与 `SegBar` / `TempBadge` 同源令牌，避免"红绿"在两套体系里不一致。
 */
const HealthText = styled.span<{ $level: "healthy" | "warning" | "unhealthy" | "unknown" }>`
  color: ${(p) =>
    p.$level === "healthy"
      ? "var(--aegis-success)"
      : p.$level === "warning"
        ? "var(--aegis-warning)"
        : p.$level === "unhealthy"
          ? "var(--aegis-error)"
          : "var(--aegis-stone)"};
  font-weight: 700;
`;

/** 采集失败原因整行占位（空态必写为什么是空的，R-08） */
const RowReason = styled(PanelReason)`
  margin: 0;
  padding: var(--aegis-space-xs) 0;
`;

function healthLevel(status: string): "healthy" | "warning" | "unhealthy" | "unknown" {
  switch (status) {
    case "healthy":
      return "healthy";
    case "warning":
      return "warning";
    case "unhealthy":
      return "unhealthy";
    default:
      return "unknown";
  }
}

interface DiskHealthPanelProps {
  rows: DiskHealthRowDto[];
}

/** F-06 磁盘健康面板：逐行列出每块存储设备 —— 设备名、最高温度、SMART 属性数、健康度。 */
export function DiskHealthPanel({ rows }: DiskHealthPanelProps) {
  const { t } = useTranslation("overview");

  const hasData = rows.some((row) => row.deviceName || row.reason);
  if (!hasData) {
    return (
      <Panel>
        <PanelTitle>{t("diskHealth.title")}</PanelTitle>
        <PanelNote>{t("diskHealth.noData")}</PanelNote>
      </Panel>
    );
  }

  return (
    <Panel>
      <PanelTitle>{t("diskHealth.title")}</PanelTitle>
      <RowList>
        {rows.map((row, index) => {
          // 空行占位：保持行数不变，卡片高度不随采集结果跳动（L-08）
          if (!row.deviceName && !row.reason) {
            return <Row key={index} aria-hidden />;
          }

          // 采集失败：整行给原因，不显示半截数据
          if (row.reason && !row.deviceName) {
            return (
              <Row key={index}>
                <RowReason>{row.reason}</RowReason>
              </Row>
            );
          }

          return (
            <Row key={index}>
              <Device title={row.deviceName}>{row.deviceName}</Device>
              <Meta>
                <span>
                  {t("diskHealth.maxTemperature")}:{" "}
                  {row.temperatureC != null ? `${row.temperatureC}℃` : "—"}
                </span>
                <HealthText $level={healthLevel(row.healthStatus)}>
                  {t("diskHealth.health")}:{" "}
                  {row.healthPercent != null ? `${row.healthPercent}%` : "—"}
                </HealthText>
                {row.smartAttributeCount > 0 ? <span>SMART {row.smartAttributeCount}</span> : null}
              </Meta>
            </Row>
          );
        })}
      </RowList>
    </Panel>
  );
}
