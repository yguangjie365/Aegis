import { useRef } from "react";
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
  Tr,
} from "./panel";
import type { TopProcesses, ProcessRow } from "@/types/overview";

/**
 * 🔴 2026-10-04 迁入 `panel.tsx` 共享语汇（原先本文件自有一套 surface-soft +
 * elevation-sticky 阴影 + 600 字重，与 StatCard 互斥；详见 panel.tsx 头注）。
 */

/**
 * 行值变化检测：只给**真的变了**的行加一次性底色回落。
 *
 * 🔴 为什么需要它：进程榜每 2 秒一刷，CPU 百分比是本页面变化最快的数字。
 * 没有变化提示时，用户只能逐行扫读去找「刚才哪个进程跳了」；而表格本身
 * 在 2 秒内会整体重排（名次变了行也跟着动），**运动本身不能区分"重排"与"变值"**。
 *
 * 🔴 为什么用 ref 比较而不是 `useEffect` + state：那会在每拍多一次渲染
 * （2 秒一次 × 5 行，可接受但没必要），且首拍必然全亮一次。ref 比较在
 * 渲染期完成、零额外渲染，且首拍 prev 为空 ⇒ 不脉冲（正确：那不是"变化"）。
 */
function useRowPulse(value: number | null): boolean {
  const prev = useRef<number | null | undefined>(undefined);
  const changed = prev.current !== undefined && prev.current !== value;
  prev.current = value;
  return changed;
}

/** 单行：把值变化转成一次性底色回落（动画定义见 panel.tsx 的 `Tr`）。 */
function ProcessRowLine({
  row,
  valueKey,
}: {
  row: ProcessRow;
  valueKey: "cpuPercent" | "memoryMb";
}) {
  const raw = row[valueKey];
  const displayValue =
    raw == null ? "—" : valueKey === "cpuPercent" ? `${raw.toFixed(1)}%` : `${raw.toFixed(0)} MB`;
  const pulsed = useRowPulse(raw);
  return (
    <Tr $pulse={pulsed}>
      <Td>{row.rank}</Td>
      <Td title={row.name}>{row.name}</Td>
      <Td>{row.pid}</Td>
      <Td $numeric>{displayValue}</Td>
    </Tr>
  );
}

/**
 * 🔴 CPU / 内存双榜**并排两列**（母本「实时占用进程」区 `UniformGrid Columns="2"`，
 * 见 `OverviewView.xaml:360`；用户 2026-10-04 裁决「CPU 占用和内存占用也分两列显示」）。
 * 原实现是上下堆叠（Fragment 里两个 `ProcessTable` 竖排）。
 *
 * 🔴 固定两列而非 `auto-fit`：母本是硬 `Columns=2`；`auto-fit` 会按可用宽度排出更多列
 * （实测 1280 窗口下 `minmax(320px,1fr)` 排成三列），与母本不符。
 */
const DualGrid = styled.div`
  display: grid;
  grid-template-columns: repeat(2, minmax(0, 1fr));
  gap: var(--aegis-space-md);
  align-items: start;
`;

interface ProcessTableProps {
  title: string;
  rows: ProcessRow[];
  valueKey: "cpuPercent" | "memoryMb";
  valueLabel: string;
  hasBaseline: boolean;
  /** 🟢 无障碍：表头关联需`scope` + 表需 `aria-label`，否则读屏只会念出一串裸数字 */
  caption: string;
}

export function ProcessTable({
  title,
  rows,
  valueKey,
  valueLabel,
  hasBaseline,
  caption,
}: ProcessTableProps) {
  const { t } = useTranslation("overview");

  // 首拍无基线时显示占位文案
  if (!hasBaseline && rows.length === 0) {
    return (
      <Panel>
        <PanelTitle>{title}</PanelTitle>
        <PanelNote>{t("processes.firstSampling")}</PanelNote>
      </Panel>
    );
  }

  // 有数据或无基线但有空榜单
  const displayRows = rows.slice(0, 5); // 固定最多 5 行

  return (
    <Panel>
      <PanelTitle>{title}</PanelTitle>
      {displayRows.length === 0 ? (
        <PanelNote>{t("processes.noData")}</PanelNote>
      ) : (
        <DataTable aria-label={caption}>
          <Thead>
            <tr>
              <Th $width="40px" scope="col">
                #
              </Th>
              <Th scope="col">{t("processes.name")}</Th>
              <Th $width="80px" scope="col">
                PID
              </Th>
              <Th $numeric scope="col">
                {valueLabel}
              </Th>
            </tr>
          </Thead>
          <Tbody>
            {displayRows.map((row) => (
              <ProcessRowLine key={`${row.pid}-${row.rank}`} row={row} valueKey={valueKey} />
            ))}
          </Tbody>
        </DataTable>
      )}
    </Panel>
  );
}

interface DualProcessTablesProps {
  data: TopProcesses;
}

export function DualProcessTables({ data }: DualProcessTablesProps) {
  const { t } = useTranslation("overview");

  return (
    <>
      <DualGrid>
        <ProcessTable
          title={t("processes.cpuTitle")}
          rows={data.cpu}
          valueKey="cpuPercent"
          valueLabel={t("processes.cpuPercent")}
          hasBaseline={data.hasBaseline}
          caption={t("processes.cpuTitle")}
        />
        <ProcessTable
          title={t("processes.memoryTitle")}
          rows={data.memory}
          valueKey="memoryMb"
          valueLabel={t("processes.memoryMb")}
          hasBaseline={data.hasBaseline}
          caption={t("processes.memoryTitle")}
        />
      </DualGrid>
      {data.reason ? <PanelReason>{data.reason}</PanelReason> : null}
    </>
  );
}
