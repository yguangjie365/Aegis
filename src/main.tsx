import React from "react";
import ReactDOM from "react-dom/client";
import { ChakraProvider, createLocalStorageManager } from "@chakra-ui/react";
import { RouterProvider } from "react-router-dom";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";

// 字体全部本地打包（规范 §七 禁 CDN）：fontsource 子集 woff2 + unicode-range
import "@fontsource/inter/latin-400.css";
import "@fontsource/inter/latin-700.css";
import "@fontsource/noto-sans-sc/chinese-simplified-400.css";
import "@fontsource/noto-sans-sc/chinese-simplified-700.css";
import "@fontsource/noto-sans-sc/latin-400.css";
import "@fontsource/noto-sans-sc/latin-700.css";
import "@fontsource/jetbrains-mono/latin-400.css";

import { theme } from "@/ui/theme";
import { router } from "@/app/router";
import "@/i18n";

// ── 阶段一 100% 复刻（2026-10-03）：音乐域 Provider 链 ──
// NexBox `src/main.tsx:123-138` 的嵌套为：
//   AppStartup > Update > Background > ThemeColor > Font > LiquidGlassSvgFilter > ThemeMode > App
// 其中 Update（更新检查）与 ThemeMode（主题模式）属 NexBox 壳层、Aegis 无对应能力，本轮不引入；
// 音乐页面实际消费的是以下 4 个 Provider + SVG filter，**嵌套顺序照抄 NexBox**。
import { AppStartupProvider } from "@/features/MusicManager/contexts/app-startup-context";
import { BackgroundProvider } from "@/features/MusicManager/contexts/background-context";
import { ThemeColorProvider } from "@/features/MusicManager/contexts/theme-color-context";
import { LiquidGlassSvgFilter } from "@/features/MusicManager/components/liquid-glass-svg-filter";

// W0-15 完整装配（AbortSignal 透传 invoke）在途；此处先立 QueryClient 挂载点
const queryClient = new QueryClient();

// 🔴 2026-10-05 单主题硬裁决（纯 Nvidia 深色包，用户 2026-10-04 两轮定稿）：
// ChakraProvider 默认 colorModeManager 会读 localStorage["chakra-ui-color-mode"]，
// 遗留 "light"（历史版本或其他应用的同源污染）会覆盖 initialColorMode="dark"，
// 使所有 useColorModeValue / colorScheme 组件走**浅色分支** —— CDP 假后端实测：
// 商店页面按钮 / 用户名 / ⋯ 图标渲染成 rgb(26,32,44) 深字（深底上近乎隐身）、
// 「未运行」徽章渲染成 gray.100 亮白底 pill（深色界面上全屏最亮的元素）。
// 单主题下 colorMode 恒为 dark：基于官方 manager 覆盖 get/set —— get 忽略
// localStorage 恒返回 "dark"，set 为 no-op（无可切换模式，不落存储；
// CSS 变量 --aegis-* 本就不经 colorMode 分支，不受影响）。
const darkOnlyColorModeManager = {
  ...createLocalStorageManager("chakra-ui-color-mode"),
  get: () => "dark" as const,
  set: () => {},
};

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    <ChakraProvider theme={theme} colorModeManager={darkOnlyColorModeManager}>
      <QueryClientProvider client={queryClient}>
        {/* 音乐域 Provider 链（顺序对齐 NexBox main.tsx:123-138） */}
        <AppStartupProvider>
          <BackgroundProvider>
            <ThemeColorProvider>
              {/* 🔴 FontProvider 已随桌面歌词能力一并移除（2026-10-03 用户裁决：
                  Aegis 未实现多窗口，桌面歌词浮窗无法打开 → 该能力整体移除，
                  其依赖的 font-context / fonts / hotkey-recorder / desktop-lyrics-* 一并删除）*/}
              <LiquidGlassSvgFilter />
              <RouterProvider router={router} />
            </ThemeColorProvider>
          </BackgroundProvider>
        </AppStartupProvider>
      </QueryClientProvider>
    </ChakraProvider>
  </React.StrictMode>,
);
