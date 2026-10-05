import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";
import path from "path";

// 端口与 NexBox 惯例一致；src-tauri 落地后由 Tauri 侧配置引用本 devUrl
export default defineConfig({
  // 🔴 2026-10-05 用户裁决删除「构建时刻标记」（原 define VITE_BUILD_STAMP）。
  //    它经 `import.meta.env.VITE_BUILD_STAMP` 暴露，页头以等宽小字显示 `HH:MM:SS`，
  //    作用是让用户一眼自证当前窗口跑的是哪次构建。
  //    与 OverviewPage 的 `BUILD_STAMP` + `BuildStamp` **同批删除** ——
  //    只删 define 不删消费侧，`as string` 断言会把 undefined 渲染成字面量。
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
