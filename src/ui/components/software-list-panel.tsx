import { useMemo, useState } from "react";
import { useTranslation } from "react-i18next";
import styled from "@emotion/styled";
import type { SoftwareList } from "@/types/overview";

const Container = styled.div`
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

const Toolbar = styled.div`
  display: flex;
  align-items: center;
  justify-content: space-between;
  margin-bottom: var(--aegis-space-md);
  gap: var(--aegis-space-md);
`;

const SearchInput = styled.input`
  flex: 1;
  max-width: 400px;
  padding: var(--aegis-space-xs) var(--aegis-space-sm);
  border: 1px solid var(--aegis-hairline);
  border-radius: var(--aegis-radius-sm);
  background: var(--aegis-surface-inset);
  color: var(--aegis-ink);
  font-size: var(--aegis-type-body-size);

  &:focus {
    outline: none;
    border-color: var(--aegis-accent);
  }

  &::placeholder {
    color: var(--aegis-stone);
  }
`;

const Count = styled.span`
  font-size: var(--aegis-type-body-size);
  color: var(--aegis-mute);
`;

const Table = styled.table`
  width: 100%;
  border-collapse: collapse;
  font-size: var(--aegis-type-body-size);
`;

const Thead = styled.thead`
  border-bottom: 2px solid var(--aegis-hairline);
`;

const Th = styled.th<{ width?: string }>`
  text-align: left;
  padding: var(--aegis-space-xs) var(--aegis-space-sm);
  color: var(--aegis-mute);
  font-weight: 600;
  white-space: nowrap;
  ${(props) => props.width && `width: ${props.width};`}
`;

const Tbody = styled.tbody`
  tr {
    border-bottom: 1px solid var(--aegis-hairline);

    &:last-child {
      border-bottom: none;
    }
  }
`;

const Td = styled.td`
  padding: var(--aegis-space-xs) var(--aegis-space-sm);
  color: var(--aegis-ink);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
`;

const Placeholder = styled.p`
  color: var(--aegis-warning);
  font-size: var(--aegis-type-body-size);
  margin: var(--aegis-space-md) 0 0 0;
`;

const EmptyState = styled.p`
  color: var(--aegis-stone);
  font-size: var(--aegis-type-body-size);
  margin: var(--aegis-space-md) 0 0 0;
`;

interface SoftwareListPanelProps {
  data: SoftwareList;
}

export function SoftwareListPanel({ data }: SoftwareListPanelProps) {
  const { t } = useTranslation("overview");
  const [searchTerm, setSearchTerm] = useState("");

  // F-07：按名称或发布商过滤（不区分大小写）
  const filteredApps = useMemo(() => {
    if (!searchTerm.trim()) {
      return data.apps;
    }
    const term = searchTerm.toLowerCase();
    return data.apps.filter(
      (app) =>
        app.name.toLowerCase().includes(term) ||
        (app.publisher?.toLowerCase().includes(term) ?? false),
    );
  }, [data.apps, searchTerm]);

  // 格式化字节数为可读单位
  const formatSize = (bytes: number | null): string => {
    if (bytes == null || bytes === 0) return "—";
    const kb = bytes / 1024;
    if (kb < 1024) return `${kb.toFixed(0)} KB`;
    const mb = kb / 1024;
    if (mb < 1024) return `${mb.toFixed(1)} MB`;
    const gb = mb / 1024;
    return `${gb.toFixed(2)} GB`;
  };

  return (
    <Container>
      <Title>{t("software.title")}</Title>

      <Toolbar>
        <SearchInput
          type="text"
          placeholder="按名称或发布商搜索…"
          value={searchTerm}
          onChange={(e) => setSearchTerm(e.target.value)}
        />
        <Count>{t("software.totalApps", { count: filteredApps.length })}</Count>
      </Toolbar>

      {filteredApps.length === 0 ? (
        searchTerm ? (
          <Placeholder>{t("software.noData")}</Placeholder>
        ) : (
          <EmptyState>{data.reason ?? t("software.emptyState")}</EmptyState>
        )
      ) : (
        <Table>
          <Thead>
            <tr>
              <Th width="300px">{t("software.name")}</Th>
              <Th width="150px">{t("software.version")}</Th>
              <Th width="240px">{t("software.publisher")}</Th>
              <Th width="120px">{t("software.installDate")}</Th>
              <Th width="100px">{t("software.size")}</Th>
            </tr>
          </Thead>
          <Tbody>
            {filteredApps.map((app, idx) => (
              <tr key={`${app.name}-${idx}`}>
                <Td title={app.name}>{app.name}</Td>
                <Td title={app.version ?? undefined}>{app.version ?? "—"}</Td>
                <Td title={app.publisher ?? undefined}>{app.publisher ?? "—"}</Td>
                <Td>{app.installDate ?? "—"}</Td>
                <Td>{formatSize(app.sizeBytes)}</Td>
              </tr>
            ))}
          </Tbody>
        </Table>
      )}
    </Container>
  );
}
