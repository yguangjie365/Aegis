import { createBrowserRouter, Navigate } from "react-router-dom";
import type { ReactElement } from "react";
import { ShellLayout } from "@/shell/shell-layout";
import { SHELL_MODULES } from "@/shell/modules";
import { ModulePlaceholder } from "@/shell/module-placeholder";
import { NotFoundPage } from "@/shell/not-found-page";
import { MusicManagerPage } from "@/features/MusicManager/pages/music-page";

// 路由由模块注册表派生（W1-06）：未注册 = 404，与导航缺失同源。
// 各 M-xx 开工时逐模块在 PAGE_OVERRIDES 登记真实页，未登记者继续落 ModulePlaceholder。
const PAGE_OVERRIDES: Record<string, ReactElement> = {
  "music-manager": <MusicManagerPage />,
  // MU-4.13：在线音乐页面已集成到 MusicManagerPage 内部（通过标签页切换）
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
