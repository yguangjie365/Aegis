import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";
import path from "path";

// 端口与 NexBox 惯例一致；src-tauri 落地后由 Tauri 侧配置引用本 devUrl
export default defineConfig({
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
  },
});
