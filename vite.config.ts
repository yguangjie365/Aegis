import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";
import path from "path";

// 端口与 NexBox 惯例一致；src-tauri 落地后由 Tauri 侧配置引用本 devUrl
export default defineConfig({
  // 🔴 构建时刻标记（2026-10-04）：连续三次出现「界面不是我改的那版」后加入。
  //    经 `import.meta.env.VITE_BUILD_STAMP` 暴露，页头会以等宽小字显示 `构建 HH:MM:SS`。
  //    作用是让用户**一眼自证**当前窗口跑的是哪次构建，而不必靠猜 exe 时间戳。
  //    ⚠️ 这是开发期诊断标记，正式发布前应在页头隐藏（见 OverviewPage 的 BUILD_STAMP 注释）。
  define: {
    "import.meta.env.VITE_BUILD_STAMP": JSON.stringify(new Date().toISOString().slice(11, 19)),
  },
  plugins: [react()],
  resolve: {
    alias: {
      "@": path.resolve(__dirname, "./src"),
    },
  },
  clearScreen: false,
  server: {
    port: 3010,
    host: "127.0.0.1",
    strictPort: true,
    // 🔴 与 NexBox 一致：Rust 侧改动由 tauri CLI 自身的 watcher 触发 cargo 重编，
    // vite 若监听 src-tauri/target 会在 cargo 持锁时崩于 EBUSY
    watch: {
      ignored: ["**/src-tauri/**"],
    },
  },
});
