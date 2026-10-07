/**
 * 系统优化入口页 —— 逐字复刻 NexBox `src/pages/OptimizePage.tsx`
 * （GPL-3.0 同源衍生，Aegis = NexBox 换皮工程）。
 *
 * 🔴 复刻基线：Source/NexBox/src/pages/OptimizePage.tsx（216 行）
 *
 * 最小改动（逐项登记）：
 *   1. `defaultTools` → `../lib/tools` 的 `optimizeTools`（同 builtin-tools-page 的理由）。
 *   2. `@/lib/store` → 模块私有 `../lib/store`；键名加 `aegis_sysopt_` 前缀。
 *   3. `useAdaptiveTextColor` / `LiquidGlassCard` 改走 `@/shared` 门面。
 *   4. 卡片 path 由登记表派生到 `/system-optimization/tools/<id>`。
 *   其余（网格/列表切换、拖拽排序、卡片渲染）逻辑与样式**逐字未改**。
 */
import { useState, useCallback } from "react";
import { Box, Heading, VStack, Flex } from "@chakra-ui/react";
import { useTranslation } from "react-i18next";
import { ViewGrid } from "../components/view-grid";
import { ViewList } from "../components/view-list";
import { LayoutToggle, type LayoutMode } from "../components/layout-toggle";
import { LiquidGlassCard, useAdaptiveTextColor } from "@/shared";
import { store } from "../lib/store";
import { optimizeTools } from "../lib/tools";
import type { ToolEntry, ViewItem } from "../lib/tools";

const STORE_KEY = "aegis_sysopt_optimize_tools_order";
const LS_KEY = "aegis_sysopt_optimize_tools_order";

function loadOrder(): string[] | null {
  try {
    const raw = localStorage.getItem(LS_KEY);
    if (raw) {
      const ids: string[] = JSON.parse(raw);
      if (Array.isArray(ids) && ids.length > 0) return ids;
    }
  } catch {
    /* ignore */
  }
  return null;
}

function saveOrder(ids: string[]) {
  try {
    localStorage.setItem(LS_KEY, JSON.stringify(ids));
    store.set(STORE_KEY, ids).then(() => store.save());
  } catch {
    /* ignore */
  }
}

function applyOrder(allTools: ToolEntry[], orderIds: string[]): ToolEntry[] {
  const map = new Map(allTools.map((t) => [t.id, t]));
  const ordered: ToolEntry[] = [];
  for (const id of orderIds) {
    const tool = map.get(id);
    if (tool) {
      ordered.push(tool);
      map.delete(id);
    }
  }
  for (const tool of map.values()) {
    ordered.push(tool);
  }
  return ordered;
}

export default function OptimizePage() {
  const { t } = useTranslation();
  const adaptiveTitle = useAdaptiveTextColor();
  const [layoutMode, setLayoutMode] = useState<LayoutMode>("grid");

  const [tools, setTools] = useState<ToolEntry[]>(() => {
    const saved = loadOrder();
    if (saved) return applyOrder(optimizeTools, saved);
    return optimizeTools;
  });

  const handleReorder = useCallback((newTools: ViewItem[]) => {
    setTools(newTools as ToolEntry[]);
    saveOrder(newTools.map((t) => t.id));
  }, []);

  const content = (
    <VStack align="start" spacing={6}>
      <Flex w="full" justify="space-between" align="center">
        <Heading size="lg" color={adaptiveTitle.text} textShadow={adaptiveTitle.shadow}>
          {t("optimization.pageTitle")}
        </Heading>
        <LiquidGlassCard display="inline-flex" p={1} boxShadow="sm">
          <LayoutToggle mode={layoutMode} onChange={setLayoutMode} />
        </LiquidGlassCard>
      </Flex>
      {layoutMode === "grid" ? (
        <ViewGrid tools={tools} onReorder={handleReorder} />
      ) : (
        <ViewList tools={tools} onReorder={handleReorder} />
      )}
    </VStack>
  );

  return <Box pt={8}>{content}</Box>;
}
