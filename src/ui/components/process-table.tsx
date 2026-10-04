import { useTranslation } from "react-i18next";
import styled from "@emotion/styled";
import type { TopProcesses, ProcessRow } from "@/types/overview";

const Container = styled.div`
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

const Table = styled.table`
  width: 100%;
  border-collapse: collapse;
  font-size: var(--aegis-font-sm);
`;

const Thead = styled.thead`
  border-bottom: 2px solid var(--aegis-border-color);
`;

const Th = styled.th`
  text-align: left;
  padding: var(--aegis-space-xs) var(--aegis-space-sm);
  color: var(--aegis-text-secondary);
  font-weight: 600;
  white-space: nowrap;
`;

const Tbody = styled.tbody`
  tr {
    border-bottom: 1px solid var(--aegis-border-color);

    &:last-child {
      border-bottom: none;
    }
  }
`;

const Td = styled.td`
  padding: var(--aegis-space-xs) var(--aegis-space-sm);
  color: var(--aegis-text-primary);
  white-space: nowrap;

  &.numeric {
    text-align: right;
    font-variant-numeric: tabular-nums;
  }
`;

const Placeholder = styled.p`
  color: var(--aegis-text-muted);
  font-size: var(--aegis-font-sm);
  margin: var(--aegis-space-md) 0 0 0;
`;

const Reason = styled.p`
  color: var(--aegis-text-warning);
  font-size: var(--aegis-font-xs);
  margin: var(--aegis-space-sm) 0 0 0;
`;

interface ProcessTableProps {
  title: string;
  rows: ProcessRow[];
  valueKey: "cpuPercent" | "memoryMb";
  valueLabel: string;
  hasBaseline: boolean;
}

export function ProcessTable({
  title,
  rows,
  valueKey,
  valueLabel,
  hasBaseline,
}: ProcessTableProps) {
  const { t } = useTranslation("overview");

  // 首拍无基线时显示占位文案
  if (!hasBaseline && rows.length === 0) {
    return (
      <Container>
        <Title>{title}</Title>
        <Placeholder>{t("processes.firstSampling")}</Placeholder>
      </Container>
    );
  }

  // 有数据或无基线但有空榜单
  const displayRows = rows.slice(0, 5); // 固定最多 5 行

  return (
    <Container>
      <Title>{title}</Title>
      {displayRows.length === 0 ? (
        <Placeholder>{t("processes.noData")}</Placeholder>
      ) : (
        <Table>
          <Thead>
            <tr>
              <Th style={{ width: "40px" }}>#</Th>
              <Th>{t("processes.name")}</Th>
              <Th style={{ width: "80px" }}>PID</Th>
              <Th className="numeric">{valueLabel}</Th>
            </tr>
          </Thead>
          <Tbody>
            {displayRows.map((row) => {
              const value = row[valueKey];
              const displayValue =
                value == null
                  ? "—"
                  : valueKey === "cpuPercent"
                    ? `${value.toFixed(1)}%`
                    : `${value.toFixed(0)} MB`;

              return (
                <tr key={`${row.pid}-${row.rank}`}>
                  <Td>{row.rank}</Td>
                  <Td>{row.name}</Td>
                  <Td>{row.pid}</Td>
                  <Td className="numeric">{displayValue}</Td>
                </tr>
              );
            })}
          </Tbody>
        </Table>
      )}
    </Container>
  );
}

interface DualProcessTablesProps {
  data: TopProcesses;
}

export function DualProcessTables({ data }: DualProcessTablesProps) {
  const { t } = useTranslation("overview");

  return (
    <>
      <ProcessTable
        title={t("processes.cpuTitle")}
        rows={data.cpu}
        valueKey="cpuPercent"
        valueLabel={t("processes.cpuPercent")}
        hasBaseline={data.hasBaseline}
      />
      <ProcessTable
        title={t("processes.memoryTitle")}
        rows={data.memory}
        valueKey="memoryMb"
        valueLabel={t("processes.memoryMb")}
        hasBaseline={data.hasBaseline}
      />
      {data.reason ? <Reason>{data.reason}</Reason> : null}
    </>
  );
}
