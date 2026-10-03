import { createBrowserRouter, Navigate } from "react-router-dom";
import type { ReactElement } from "react";
import { ShellLayout } from "@/shell/shell-layout";
import { SHELL_MODULES } from "@/shell/modules";
import { ModulePlaceholder } from "@/shell/module-placeholder";
import { NotFoundPage } from "@/shell/not-found-page";
// 阶段一 100% 复刻（2026-10-03）：直接挂载 NexBox 原件 MusicPage（default 导出）
import MusicPage from "@/features/MusicManager/pages/MusicPage";
import { OverviewPage } from "@/features/Overview/pages/overview-page";
// B-G1 Steam 主线（2026-10-03）：NexBox SteamPage.tsx 原件（卡片视觉按 SystemToolkit 重做）
import SteamPage from "@/features/GameManager/pages/SteamPage";
// 设置页（2026-10-03 用户裁决）：Steam 在线库存的 API Key 录入放这里，**不放游戏管理页**
import SettingsPage from "@/features/Settings/pages/settings-page";

// 路由由模块注册表派生（W1-06）：未注册 = 404，与导航缺失同源。
// 各 M-xx 开工时逐模块在 PAGE_OVERRIDES 登记真实页，未登记者继续落 ModulePlaceholder。
const PAGE_OVERRIDES: Record<string, ReactElement> = {
  // 阶段一 100% 复刻：NexBox MusicPage 原件即为单页多视图（左栏视图状态机 + 右栏发现区）
  "music-manager": <MusicPage />,
  overview: <OverviewPage />,
  // B-G1 Steam 主线：NexBox SteamPage 原件（数据流/命令复用 NexBox，卡片视觉 = SystemToolkit）
  "game-manager": <SteamPage />,
  settings: <SettingsPage />,
};

export const router = createBrowserRouter([
  {
    path: "/",
    element: <ShellLayout />,
    children: [
      { index: true, element: <Navigate to="/overview" replace /> },
      ...SHELL_MODULES.map((m) => ({
        path: m.route.slice(1),
        element: PAGE_OVERRIDES[m.id] ?? <ModulePlaceholder module={m} />,
      })),
      { path: "*", element: <NotFoundPage /> },
    ],
  },
]);
