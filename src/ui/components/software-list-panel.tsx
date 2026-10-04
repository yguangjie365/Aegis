import { useMemo, useState } from "react";
import { useTranslation } from "react-i18next";
import styled from "@emotion/styled";
import { Panel, PanelTitle, PanelNote, DataTable, Thead, Th, Tbody, Td } from "./panel";
import type { SoftwareList } from "@/types/overview";

/**
 * 🔴 2026-10-04 迁入 `panel.tsx` 共享语汇（原先自有一套 surface-soft +
 * elevation-sticky 阴影 + 600 字重，与 StatCard 互斥；详见 panel.tsx 头注）。
 * 并修三处：
 * ① 搜索框 placeholder 原来**硬编码中文**，绕过了 i18n —— 换语言时它不跟着变。
 * ② `&:focus { outline: none }` 违反规范 §八「🔴 禁 outline:none 且无替代焦点指示」：
 *    原实现只改了 `border-color`（accent 绿 2.41:1），在 hairline 1.61:1 的底上
 *    几乎看不出变化 ⇒ 等于既没outline 也没替代。现改为保留 `outline`（全局
 *    `*:focus-visible` 已是 2px accent-strong 焦点环），并把 border 一并提亮作为叠加信号。
 * ③ 表头固定宽度改由 `DataTable` 的 `table-layout: fixed` 保证（原先用 auto 布局，
 *    声明的 300/150/240/120/100px 会被内容撑开，「版本/发布商」两列因内容长度差异而错位）。
 */

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
  border: 1px solid var(--aegis-border-interactive);
  border-radius: var(--aegis-radius-sm);
  background: var(--aegis-surface-inset);
  color: var(--aegis-ink);
  font-size: var(--aegis-type-body-size);
  font-family: inherit;

  /* 🔴 聚焦：**不**写 outline: none —— 全局 :focus-visible 已提供 2px accent-strong
     焦点环（浅底合规值）。此处只把边框提到 interactive 档作为叠加的定位信号，
     不替换焦点指示。 */
  &:focus-visible {
    border-color: var(--aegis-accent-strong);
  }

  &::placeholder {
    color: var(--aegis-stone);
  }
`;

const Count = styled.span`
  font-size: var(--aegis-type-body-size);
  color: var(--aegis-mute);
  font-variant-numeric: tabular-nums;
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
    <Panel>
      <PanelTitle>{t("software.title")}</PanelTitle>

      <Toolbar>
        <SearchInput
          type="text"
          placeholder={t("software.searchPlaceholder")}
          aria-label={t("software.searchPlaceholder")}
          value={searchTerm}
          onChange={(e) => setSearchTerm(e.target.value)}
        />
        <Count>{t("software.totalApps", { count: filteredApps.length })}</Count>
      </Toolbar>

      {filteredApps.length === 0 ? (
        searchTerm ? (
          <PanelNote>{t("software.noData")}</PanelNote>
        ) : (
          <PanelNote>{data.reason ?? t("software.emptyState")}</PanelNote>
        )
      ) : (
        <DataTable aria-label={t("software.title")}>
          <Thead>
            <tr>
              <Th $width="300px" scope="col">
                {t("software.name")}
              </Th>
              <Th $width="150px" scope="col">
                {t("software.version")}
              </Th>
              <Th $width="240px" scope="col">
                {t("software.publisher")}
              </Th>
              <Th $width="120px" scope="col">
                {t("software.installDate")}
              </Th>
              <Th $width="100px" $numeric scope="col">
                {t("software.size")}
              </Th>
            </tr>
          </Thead>
          <Tbody>
            {filteredApps.map((app, idx) => (
              <tr key={`${app.name}-${idx}`}>
                <Td title={app.name}>{app.name}</Td>
                <Td title={app.version ?? undefined}>{app.version ?? "—"}</Td>
                <Td title={app.publisher ?? undefined}>{app.publisher ?? "—"}</Td>
                <Td>{app.installDate ?? "—"}</Td>
                <Td $numeric>{formatSize(app.sizeBytes)}</Td>
              </tr>
            ))}
          </Tbody>
        </DataTable>
      )}
    </Panel>
  );
}
