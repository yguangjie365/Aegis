import { defineConfig } from "vitest/config";
import path from "path";

// vitest 配置（M-01 片1-e 引入，2026-10-03）：门禁第 7 腿 `vitest run` 的运行环境。
//
// 环境取 `node` 而非 jsdom：本批用例全是纯函数（展示层口径），不需要 DOM。
// 🔴 将来给 hooks/组件写用例时要加 jsdom + testing-library，那属于新的第三方依赖，
// 必须按 ADR-003 五步逐个登记后才准进 package.json——此处不预埋未使用的重依赖。

export default defineConfig({
  resolve: {
    alias: {
      "@": path.resolve(__dirname, "./src"),
    },
  },
  test: {
    environment: "node",
    include: ["src/**/*.test.{ts,tsx}"],
  },
});
