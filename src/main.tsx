import React from "react";
import ReactDOM from "react-dom/client";
import { ChakraProvider } from "@chakra-ui/react";
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

// W0-15 完整装配（AbortSignal 透传 invoke）在途；此处先立 QueryClient 挂载点
const queryClient = new QueryClient();

// K-S03 探针（spike）：仅当构建期注入 VITE_SPIKE=K-S03 时挂载；常规构建该分支为
// 静态 false，死代码被摇树剔除，探针代码不进产物。
const SpikePanel =
  (import.meta.env.VITE_SPIKE as string | undefined) === "K-S03"
    ? React.lazy(() =>
        import("@/features/MusicManager/spike/k-s03-selftest").then((m) => ({
          default: m.KS03SelfTest,
        })),
      )
    : null;

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    <ChakraProvider theme={theme}>
      <QueryClientProvider client={queryClient}>
        <RouterProvider router={router} />
        {SpikePanel && (
          <React.Suspense fallback={null}>
            <SpikePanel />
          </React.Suspense>
        )}
      </QueryClientProvider>
    </ChakraProvider>
  </React.StrictMode>,
);
