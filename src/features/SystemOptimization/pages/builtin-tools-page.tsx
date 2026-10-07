/**
 * 内置工具入口页 —— 逐字复刻 NexBox `src/pages/BuiltinToolsPage.tsx`
 * （GPL-3.0 同源衍生，Aegis = NexBox 换皮工程）。
 *
 * 🔴 复刻基线：Source/NexBox/src/pages/BuiltinToolsPage.tsx（313 行）
 *
 * 最小改动（逐项登记）：
 *   1. `defaultTools` 数组 → 移入 `../lib/tools`（登记表，含路由派生与状态字段）。
 *      NexBox 把数组内联在页面内；Aegis 抽出以便占位页按 id 反查。取值与顺序未改。
 *   2. `IS_STORE_BUILD` 分支 → 移除（Aegis 无商店版，恒 false；见 lib/build-flags.ts）。
 *   3. `@/lib/store` → 模块私有 `../lib/store`（独立存储文件，避免与音乐域耦合）。
 *   4. localStorage / store 键名加 `aegis_sysopt_` 前缀（防与 NexBox 同源键互串）。
 *   5. `useAdaptiveTextColor` 改从 `@/shared` 门面取（跨 feature 基建走门面）。
 *   6. 路由前缀：卡片 path 由登记表派生到 `/system-optimization/tools/<id>`。
 *   其余（网格/列表切换、拖拽排序、卡片渲染、玻璃卡分支）逻辑与样式**逐字未改**。
 */
import { useState, useCallback } from "react";
import { Box, Heading, VStack, Flex } from "@chakra-ui/react";
import { useTranslation } from "react-i18next";
import { ViewGrid } from "../components/view-grid";
import { ViewList } from "../components/view-list";
import { LayoutToggle, type LayoutMode } from "../components/layout-toggle";
import { LiquidGlassCard } from "@/shared";
import { useAdaptiveTextColor } from "@/shared";
import { store } from "../lib/store";
import { builtinTools } from "../lib/tools";
import type { ToolEntry, ViewItem } from "../lib/tools";

const STORE_KEY = "aegis_sysopt_builtin_tools_order";
const LS_KEY = "aegis_sysopt_builtin_tools_order";

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
  // Append any new tools not in the saved order
  for (const tool of map.values()) {
    ordered.push(tool);
  }
  return ordered;
}

export default function BuiltinToolsPage() {
  const { t } = useTranslation();
  const [layoutMode, setLayoutMode] = useState<LayoutMode>("grid");

  const [tools, setTools] = useState<ToolEntry[]>(() => {
    const saved = loadOrder();
    if (saved) return applyOrder(builtinTools, saved);
    return builtinTools;
  });

  const handleReorder = useCallback((newTools: ViewItem[]) => {
    setTools(newTools as ToolEntry[]);
    saveOrder(newTools.map((t) => t.id));
  }, []);

  const adaptiveTitle = useAdaptiveTextColor();

  const content = (
    <VStack align="start" spacing={6}>
      <Flex w="full" justify="space-between" align="center">
        <Heading size="lg" color={adaptiveTitle.text} textShadow={adaptiveTitle.shadow}>
          {t("builtinTools.title")}
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
