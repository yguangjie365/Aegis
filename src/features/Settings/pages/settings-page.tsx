/**
 * 设置页（`/settings`）
 * ============================================================================
 * 现状：导航里已注册该模块（`shell/modules.ts` 的 `id: "settings"`），但一直落
 * `ModulePlaceholder`。本轮按用户裁决（2026-10-03）接入**第一个真实配置区**：
 * Steam 在线库存的 API Key 录入。
 *
 * 🔴 配置区归属纪律（与游戏管理页的分工）：
 * - **设置页**：只放「凭据 / 全局开关」类配置（本页的 API Key）。
 * - **功能页**：只放「该功能自己的操作」（启动游戏、切账户、刷新）。
 * 同一件事不两处都做 —— SystemToolkit 把 Key 弹窗塞在游戏管理页内，是 Aegis
 * 明确不采的做法（用户 2026-10-03 裁决）。
 *
 * 后续配置区往这里加（不预告，仅记录本页的定位）：主题色、背景、通知、语言等。
 */
import { useMemo } from "react";
import { Box, VStack } from "@chakra-ui/react";

import { useRegisterBrand } from "@/shell/brand-bar";
import { SteamApiKeySection } from "@/features/Settings/components/steam-api-key-section";
import { AboutSection } from "@/features/Settings/components/about-section";

export function SettingsPage() {
  /* 品牌区注册（2026-10-05 垂直迁移第 7 条）：主标题「设置」已由壳层从
     modules.ts 的 nameKey 自动派生，此处只上报副标题。 */
  const sub = useMemo(
    () => "全局配置与凭据。各功能页只放该功能自己的操作，凭据类一律在此维护。",
    [],
  );
  useRegisterBrand("/settings", { sub });

  return (
    <VStack spacing="lg" align="stretch" maxW="760px">
      <SteamApiKeySection />
      {/* 版本 + 零遥测声明：2026-10-04 状态栏移除后由这里承载（合规红线不丢） */}
      <AboutSection />
      <Box />
    </VStack>
  );
}

export default SettingsPage;
