import { useState } from "react";
import { useQueryClient } from "@tanstack/react-query";
import { useTranslation } from "react-i18next";
import { save, message } from "@tauri-apps/plugin-dialog";
import type { LiveSample } from "@/types/overview";
import { fetchSensorTable, fetchStaticInfo, writeReportFile } from "@/lib/tauri/overview";
import { log, newCorrelationId } from "@/lib/logging";
import { OVERVIEW_STATIC_KEY } from "./use-overview-live";
import { buildReport, defaultFileName, splitStaticCards } from "../lib/export-report";

// F-11 导出硬件信息报告的取数与落盘编排（规格 §3.4 流程图逐步对应）。
//
// 🔴 三条前置提示而非静默返回（R-13「点了没反应」是母本踩过的坑）：
// 采集中 → 提示稍候；无数据 → 提示等待首次采集；用户取消另存为 → 零副作用（Cancelled 只记日志，不打扰用户）。
//
// 报告文本由 `lib/export-report.ts` 纯函数组装（可单测），本 hook 只负责取数、弹窗、落盘。

const FILE_FILTER = [{ name: "Markdown", extensions: ["md"] }];

export function useExportReport(data: LiveSample | undefined, busy: boolean) {
  const { t } = useTranslation("overview");
  const queryClient = useQueryClient();
  const [exporting, setExporting] = useState(false);

  async function runExport(): Promise<void> {
    const correlationId = newCorrelationId();
    setExporting(true);
    try {
      // 静态上行优先取缓存（慢变量，与 F-02/F-03 详情区同源同一份），无缓存才现采
      const staticInfo = await queryClient.fetchQuery({
        queryKey: OVERVIEW_STATIC_KEY,
        queryFn: fetchStaticInfo,
        staleTime: Infinity,
      });
      // 传感器逐条读数每次导出现取：报告要的是"导出这一刻"的快照
      const sensors = await fetchSensorTable();
      const { detailCards, systemCards } = splitStaticCards(staticInfo.cards);
      const markdown = buildReport(
        {
          computerName: data?.header.computerName ?? "",
          generatedAt: new Date(),
          statCards: data?.cards ?? [],
          detailCards,
          systemCards,
          sensors: sensors.rows,
        },
        (key, options) => String(t(key, options ?? {})),
      );

      const target = await save({
        defaultPath: defaultFileName(t("report.fileName"), new Date()),
        filters: FILE_FILTER,
        title: t("report.button"),
      });
      if (target === null || target === "") {
        log({
          level: "info",
          module: "Overview",
          action: "exportReport",
          result: "Cancelled",
          correlationId,
        });
        return;
      }

      await writeReportFile(target, markdown);
      log({
        level: "info",
        module: "Overview",
        action: "exportReport",
        result: "Success",
        correlationId,
      });
      void message(t("report.saved", { path: target }), {
        kind: "info",
        title: t("report.button"),
      });
    } catch (error) {
      const reason = error instanceof Error ? error.message : String(error);
      log({
        level: "error",
        module: "Overview",
        action: "exportReport",
        result: "Failed",
        exception: reason,
        correlationId,
      });
      void message(t("report.failed", { reason }), { kind: "error", title: t("report.button") });
    } finally {
      setExporting(false);
    }
  }

  function onClick(): void {
    const correlationId = newCorrelationId();
    // R-13：采集在途时导出的会是一半新一半旧的数据，直接挡在门外
    if (busy || exporting) {
      log({
        level: "warn",
        module: "Overview",
        action: "exportReport",
        result: "Rejected",
        exception: "采集中，拒绝导出",
        correlationId,
      });
      void message(t("report.busy"), { kind: "warning", title: t("report.button") });
      return;
    }
    if (data === undefined || data.cards.length === 0) {
      log({
        level: "warn",
        module: "Overview",
        action: "exportReport",
        result: "Rejected",
        exception: "尚无数据",
        correlationId,
      });
      void message(t("report.noData"), { kind: "warning", title: t("report.button") });
      return;
    }
    void runExport();
  }

  return { exporting, onClick };
}
