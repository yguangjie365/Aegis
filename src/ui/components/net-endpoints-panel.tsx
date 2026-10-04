import { useTranslation } from "react-i18next";
import styled from "@emotion/styled";
import {
  Panel,
  PanelTitle,
  PanelNote,
  PanelReason,
  DataTable,
  Thead,
  Th,
  Tbody,
  Td,
} from "./panel";
import type { NetEndpoints } from "@/types/overview";

/**
 * 🔴 2026-10-04 迁入 `panel.tsx` 共享语汇（原先自有一套 surface-soft +
 * elevation-sticky 阴影 + 600 字重，与 StatCard 互斥；详见 panel.tsx 头注）。
 * 本文件原先还内联写死了 reason 的样式（`style={{color:…, fontSize:…}}`），
 * 与另两个面板的 `Reason` 组件重复，现统一为 `PanelReason`。
 */

const Summary = styled.p`
  font-size: var(--aegis-type-body-size);
  line-height: 1.5;
  color: var(--aegis-mute);
  margin: 0 0 var(--aegis-space-md) 0;
`;

interface NetEndpointsPanelProps {
  data: NetEndpoints;
}

export function NetEndpointsPanel({ data }: NetEndpointsPanelProps) {
  const { t } = useTranslation("overview");

  return (
    <Panel>
      <PanelTitle>{t("endpoints.title")}</PanelTitle>
      <Summary>
        {t("endpoints.totalEndpoints")}: <strong>{data.total}</strong>
      </Summary>

      {data.rows.length === 0 ? (
        <PanelNote>{t("endpoints.noData")}</PanelNote>
      ) : (
        <DataTable aria-label={t("endpoints.title")}>
          <Thead>
            <tr>
              <Th $width="40px" scope="col">
                #
              </Th>
              <Th scope="col">{t("endpoints.processName")}</Th>
              <Th $numeric scope="col">
                {t("endpoints.endpointCount")}
              </Th>
              <Th scope="col">{t("endpoints.listenPorts")}</Th>
            </tr>
          </Thead>
          <Tbody>
            {data.rows.slice(0, 5).map((row) => (
              <tr key={`${row.processName}-${row.rank}`}>
                <Td>{row.rank}</Td>
                <Td title={row.processName}>{row.processName}</Td>
                <Td $numeric>{row.endpointCount}</Td>
                <Td title={row.listenPorts || undefined}>{row.listenPorts || "—"}</Td>
              </tr>
            ))}
          </Tbody>
        </DataTable>
      )}

      {data.reason ? <PanelReason>{data.reason}</PanelReason> : null}
    </Panel>
  );
}
