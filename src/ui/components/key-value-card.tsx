import { useTranslation } from "react-i18next";
import styled from "@emotion/styled";
import type { DetailCardDto } from "@/types/overview";

const Card = styled.div`
  background: var(--aegis-bg-surface);
  border-radius: var(--aegis-radius-md);
  padding: var(--aegis-space-lg);
  box-shadow: var(--aegis-shadow-sm);
`;

const Title = styled.h3`
  font-size: var(--aegis-font-lg);
  font-weight: 600;
  margin: 0 0 var(--aegis-space-md) 0;
  color: var(--aegis-text-primary);
`;

const Row = styled.div`
  display: flex;
  justify-content: space-between;
  align-items: baseline;
  padding: var(--aegis-space-xs) 0;
  border-bottom: 1px solid var(--aegis-border-color);

  &:last-child {
    border-bottom: none;
  }
`;

const Key = styled.span`
  color: var(--aegis-text-secondary);
  font-size: var(--aegis-font-sm);
`;

const Value = styled.span<{ $missing?: boolean }>`
  color: ${(props) => (props.$missing ? "var(--aegis-text-muted)" : "var(--aegis-text-primary)")};
  font-size: var(--aegis-font-sm);
  font-weight: 500;
`;

const Reason = styled.p`
  color: var(--aegis-text-warning);
  font-size: var(--aegis-font-xs);
  margin: var(--aegis-space-sm) 0 0 0;
`;

// 🔴 详情卡 kind → i18n 文案前缀的**唯一**映射表：界面（本组件）与 F-11 导出报告
// （features/Overview/lib/export-report.ts）共用同一份，避免两处各写一份中文键而漂移。
export const DETAIL_KIND_I18N_PREFIX: Record<string, string> = {
  cpu: "detail.cpu",
  gpu: "detail.gpu",
  storage: "detail.storage",
  memory: "detail.memory",
  board: "detail.board",
  bios: "detail.bios",
  network: "detail.network",
  audio: "detail.audio",
  display: "detail.display",
  os: "detail.os",
  userRegion: "detail.userRegion",
};

/** F-02 硬件详情 vs F-03 系统信息 的分流依据（后端同装一个 DTO，见 commands.rs 头注）。 */
export const SYSTEM_INFO_KINDS: ReadonlySet<string> = new Set(["os", "userRegion"]);

interface KeyValueCardProps {
  card: DetailCardDto;
}

export function KeyValueCard({ card }: KeyValueCardProps) {
  const { t } = useTranslation("overview");

  const prefix = DETAIL_KIND_I18N_PREFIX[card.kind] ?? `detail.${card.kind}`;

  return (
    <Card>
      <Title>{t(`${prefix}.title`)}</Title>
      {card.rows.map((row) => {
        const label = t(`${prefix}.${row.key}`);
        const isMissing = row.value == null || row.value === "";
        const displayValue = isMissing ? t("badge.undetected") : row.value;

        return (
          <Row key={row.key}>
            <Key>{label}</Key>
            <Value $missing={isMissing}>{displayValue}</Value>
          </Row>
        );
      })}
      {card.reason ? <Reason>{card.reason}</Reason> : null}
    </Card>
  );
}
