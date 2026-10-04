import { useTranslation } from "react-i18next";
import styled from "@emotion/styled";
import type { DiskHealthRowDto } from "@/types/overview";

// disk-health-panel：F-06 磁盘健康（母本 SystemToolkit「磁盘健康」分区）。
//
// 🔴 2026-10-04 重写：原实现整份用 Tailwind 类名（`text-muted-foreground` / `bg-background` /
// `rounded-lg`…），而本项目是**Chakra + Emotion 设计令牌制**、无 Tailwind 管线，
// 故那些类**一个都不生效** —— 面板实际渲染为无样式的裸 div（换皮遗留缺陷）。
// 全部改写为 `styled` + `var(--aegis-*)` 令牌，与 `KeyValueCard` / `NetEndpointsPanel` 同构。
//
// 布局：用户 2026-10-04 裁决「磁盘健康分两列显示」→ 行以 `auto-fit/ minmax` 双列落位，
// 单列时（窄窗或仅一块盘）自然退化为单列，不留空槽。

const Card = styled.section`
  background: var(--aegis-surface-soft);
  border-radius: var(--aegis-radius-sm);
  padding: var(--aegis-space-lg);
  box-shadow: var(--aegis-elevation-sticky);
`;

const Title = styled.h3`
  font-size: var(--aegis-type-h3-size);
  font-weight: 600;
  margin: 0 0 var(--aegis-space-md) 0;
  color: var(--aegis-ink);
`;

const Empty = styled.p`
  font-size: var(--aegis-type-body-size);
  color: var(--aegis-mute);
  margin: 0;
`;

/**
 * 行容器：单列。
 * 🔴 两列由**页面级** `HealthGrid` 承担（母本「网络与存储健康」区 `UniformGrid Columns="2"`
 * 装两个面板），面板内部再分两列会变成四列 —— 与母本不符。
 */
const RowList = styled.div`
  display: flex;
  flex-direction: column;
  gap: var(--aegis-space-sm);
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
  min-height: 34px;
`;

const Device = styled.span`
  flex: 1;
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  font-size: var(--aegis-type-body-size);
  font-weight: 500;
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

/** 健康状态色：与 SegBar / TempBadge 同源令牌，避免"红绿"在两套体系里不一致 */
const HealthText = styled.span<{ $level: "healthy" | "warning" | "unhealthy" | "unknown" }>`
  color: ${(p) =>
    p.$level === "healthy"
      ? "var(--aegis-success)"
      : p.$level === "warning"
        ? "var(--aegis-warning)"
        : p.$level === "unhealthy"
          ? "var(--aegis-error)"
          : "var(--aegis-stone)"};
  font-weight: 600;
`;

/** 采集失败原因整行占位（与 KeyValueCard 的 Reason 同口径：空态必写为什么是空的，R-08） */
const Reason = styled.p`
  font-size: var(--aegis-type-caption-size);
  color: var(--aegis-warning);
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
      <Card>
        <Title>{t("diskHealth.title")}</Title>
        <Empty>{t("diskHealth.noData")}</Empty>
      </Card>
    );
  }

  return (
    <Card>
      <Title>{t("diskHealth.title")}</Title>
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
                <Reason>{row.reason}</Reason>
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
    </Card>
  );
}
