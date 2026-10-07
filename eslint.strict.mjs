// lint:strict 专用配置（审查 R-1，2026-10-07 用户裁决方案 C：双门禁）。
//
// 🔴 为什么要有这个文件：`eslint.config.mjs` 按用户裁决（2026-10-03）把复刻域
//   （MusicManager / GameManager，NexBox 100% 原文照搬）的规则降为 **warn**，
//   而 `npm run lint:strict` 全局 `--max-warnings 0` ⇒ 70 条复刻域 warning 让该
//   门禁**永远不可能绿**——与「backtick 假绿」同构的反向死法：恒红的门禁等于
//   没有门禁，非豁免域的新违规混在噪声里无法分辨。
//
// 口径：**可见性与拦截分离**。
// - `npm run lint`        → 主 config，全量展示复刻域 warning（阶段二换皮的整改清单）；
// - `npm run lint:strict` → 本 config，复刻域整目录排除，只对非豁免域零容忍。
//
// 🔴 ignores 清单必须与 `eslint.config.mjs` 的两个复刻域豁免块（音乐域 + 游戏域，
//   含 `src/types/music.ts`）**保持同源**：一侧增删，另一侧同步。登记出处：
//   `Docs/功能规格/07-音乐管理.md §〇·0.4` 与 `09-游戏管理模块移植调研.md §〇`。
import tseslint from "typescript-eslint";
import base from "./eslint.config.mjs";

export default tseslint.config(
  {
    ignores: [
      "node_modules/**",
      "dist/**",
      "coverage/**",
      ".worknotes/**",
      "src-tauri/**",
      "public/**",
      "spikes/**",
      // ── 复刻域（与主 config 豁免块同源，见文件头注释）──
      "src/features/MusicManager/**/*.{ts,tsx}",
      "src/features/GameManager/**/*.{ts,tsx}",
      // M-09 系统优化（2026-10-07 新增复刻域，与主 config 同源登记）
      "src/features/SystemOptimization/**/*.{ts,tsx}",
      "src/types/music.ts",
    ],
  },
  ...base,
);
