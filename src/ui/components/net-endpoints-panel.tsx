import { useTranslation } from "react-i18next";
import styled from "@emotion/styled";
import type { NetEndpoints } from "@/types/overview";

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

const Summary = styled.p`
  font-size: var(--aegis-font-sm);
  color: var(--aegis-text-secondary);
  margin: 0 0 var(--aegis-space-md) 0;
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

interface NetEndpointsPanelProps {
  data: NetEndpoints;
}

export function NetEndpointsPanel({ data }: NetEndpointsPanelProps) {
  const { t } = useTranslation("overview");

  return (
    <Container>
      <Title>{t("endpoints.title")}</Title>
      <Summary>
        {t("endpoints.totalEndpoints")}:{" "}
        <strong>{data.total}</strong>
      </Summary>

      {data.rows.length === 0 ? (
        <Placeholder>{t("endpoints.noData")}</Placeholder>
      ) : (
        <Table>
          <Thead>
            <tr>
              <Th style={{ width: "40px" }}>#</Th>
              <Th>{t("endpoints.processName")}</Th>
              <Th className="numeric">{t("endpoints.endpointCount")}</Th>
              <Th>{t("endpoints.listenPorts")}</Th>
            </tr>
          </Thead>
          <Tbody>
            {data.rows.slice(0, 5).map((row) => (
              <tr key={`${row.processName}-${row.rank}`}>
                <Td>{row.rank}</Td>
                <Td>{row.processName}</Td>
                <Td className="numeric">{row.endpointCount}</Td>
                <Td>{row.listenPorts || "—"}</Td>
              </tr>
            ))}
          </Tbody>
        </Table>
      )}

      {data.reason ? (
        <p style={{ color: "var(--aegis-text-warning)", fontSize: "var(--aegis-font-xs)", marginTop: "var(--aegis-space-sm)" }}>
          {data.reason}
        </p>
      ) : null}
    </Container>
  );
}
