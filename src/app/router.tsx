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
// M-05 文件互传片1（2026-10-04）：电脑↔电脑通道页面骨架
import { FileTransferPage } from "@/features/FileTransfer/pages/file-transfer-page";
// M-06 文件备份（2026-10-05）：左栏规则 + 右栏快照 + 恢复向导
import { FileBackupPage } from "@/features/FileBackup/pages/file-backup-page";
// M-09 系统优化（2026-10-07 用户裁决新增）：NexBox「内置工具 + 优化」全量搬迁
// ——批次 0 交付两个入口页 + 诚实占位；37 个工具子页按批次逐个迁入（见模块 README）。
import SystemOptimizationBuiltinToolsPage from "@/features/SystemOptimization/pages/builtin-tools-page";
import SystemOptimizationOptimizePage from "@/features/SystemOptimization/pages/optimize-page";
import { ToolPlaceholderPage } from "@/features/SystemOptimization/pages/tool-placeholder-page";

// 路由由模块注册表派生（W1-06）：未注册 = 404，与导航缺失同源。
// 各 M-xx 开工时逐模块在 PAGE_OVERRIDES 登记真实页，未登记者继续落 ModulePlaceholder。
const PAGE_OVERRIDES: Record<string, ReactElement> = {
  // 阶段一 100% 复刻：NexBox MusicPage 原件即为单页多视图（左栏视图状态机 + 右栏发现区）
  "music-manager": <MusicPage />,
  overview: <OverviewPage />,
  // B-G1 Steam 主线：NexBox SteamPage 原件（数据流/命令复用 NexBox，卡片视觉 = SystemToolkit）
  "game-manager": <SteamPage />,
  settings: <SettingsPage />,
  // M-05 文件互传片1（2026-10-04）：页面骨架 + 空态；命令面随后端协议批落地
  "file-transfer": <FileTransferPage />,
  // M-06 文件备份（2026-10-05）：规则 CRUD / 立即备份 / 快照校验 / 恢复向导
  "file-backup": <FileBackupPage />,
  // M-09 系统优化（2026-10-07）：入口 = NexBox「内置工具」页（逐字复刻）
  "system-optimization": <SystemOptimizationBuiltinToolsPage />,
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
      // 🔴 M-09 系统优化的模块内子路由（2026-10-07）。
      //   入口页在 PAGE_OVERRIDES（与其余模块同源派生）；子路由因是**模块私有命名空间**
      //   （`/system-optimization/**`），不进 SHELL_MODULES 派生表。
      //   · optimize            → NexBox `OptimizePage`（逐字复刻）
      //   · tools/:toolId       → 37 个工具子页；批次 0 统一落诚实占位（不假装可用），
      //                          B1~B5 逐个替换为真实页（保持同一路由 ⇒ 卡片零改动）
      {
        path: "system-optimization/optimize",
        element: <SystemOptimizationOptimizePage />,
      },
      { path: "system-optimization/tools/:toolId", element: <ToolPlaceholderPage /> },
      { path: "*", element: <NotFoundPage /> },
    ],
  },
]);
