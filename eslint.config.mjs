// ESLint 9 flat config（01-编码与命名规范 §四：ESLint 9 flat config；版本口径见 DEPENDENCIES.md §二）
// 本批只装"提交前检查点"要求的两条腿之一：可判定红绿的类型/React/异味规则集。
// 架构守卫规则族（boundaries 跨模块互引、ipc/only-ipc-layer、jsx-a11y、裸值棘轮）属 W0-15 空跑批，此处不预埋。
// 未装 eslint-plugin-react-refresh：其 only-export-components 是 HMR 体验建议（非正确性），
// 与本项目"自建 ui/ 组件层、组件文件同时导出常量"的既有写法冲突，且不在规范检查点要求内。
import js from "@eslint/js";
import tseslint from "typescript-eslint";
import reactHooks from "eslint-plugin-react-hooks";
import globals from "globals";

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
    ],
  },
  {
    files: ["src/**/*.{ts,tsx}"],
    extends: [js.configs.recommended, ...tseslint.configs.recommendedTypeChecked],
    plugins: {
      "react-hooks": reactHooks,
    },
    languageOptions: {
      ecmaVersion: 2022,
      globals: globals.browser,
      parserOptions: {
        projectService: true,
        tsconfigRootDir: import.meta.dirname,
      },
    },
    rules: {
      // 编码规范 §一.1：禁 any 逃逸，未知形状一律 unknown + 收窄
      "@typescript-eslint/no-explicit-any": "error",
      // 编码规范 §三：禁静默失败。allowEmptyCatch:false 故意把空 catch 也打红——
      // 本工程不接受"刻意的 no-op catch"，吞异常前必须落日志（当前存量 0 处，实测无噪声）。
      "no-empty": ["error", { allowEmptyCatch: false }],
      // 只重新包装不改做的 try/catch（同规则族，堵"看起来处理了其实没处理"）
      "no-useless-catch": "error",
      "@typescript-eslint/no-unused-vars": [
        "error",
        { argsIgnorePattern: "^_", varsIgnorePattern: "^_" },
      ],
      // 编码规范 §三：exhaustive-deps 必须为 error，不得整文件禁用
      "react-hooks/rules-of-hooks": "error",
      "react-hooks/exhaustive-deps": "error",
    },
  },
  {
    // 配置文件跑在 Node 下、不在 tsconfig include 内，故只套非类型感知规则
    files: ["vite.config.ts", "vitest.config.ts", "eslint.config.mjs"],
    extends: [js.configs.recommended, ...tseslint.configs.recommended],
    languageOptions: {
      globals: globals.node,
      parserOptions: { project: false, projectService: false },
    },
  },

  // ── 阶段一 100% 复刻：音乐域 lint 豁免（2026-10-03 用户裁决）──
  // 依据 Docs/功能规格/07-音乐管理.md §〇·0.4：音乐模块是 NexBox 换皮版，代码原样搬入。
  // 下列规则在 Aegis 属「strict 类型门禁」族，而 NexBox 基线为 strict:false —— 照搬代码必然触发。
  // 处置：只关闭**依赖 strict 类型信息**或**与 strict 收窄耦合**的规则；
  //      结构性规则（rules-of-hooks / exhaustive-deps / no-empty / no-useless-catch 等）**全部保留**，
  //      即「不豁免可判定红绿的正确性规则，只豁免因编译目标不同产生的噪声」。
  // 对照：tsconfig.music-replica.json 的豁免项，二者口径一致。
  {
    files: ["src/features/MusicManager/**/*.{ts,tsx}", "src/types/music.ts"],
    rules: {
      // 浮空 Promise：NexBox 大量 `invoke(...)` 不 await（fire-and-forget 语义），
      // Aegis 规则要求必须 await / then / void 显式标注。属写法偏好差异，非缺陷。
      "@typescript-eslint/no-floating-promises": "off",
      // JSX 事件处理器传 async 函数：NexBox 大量 `onClick={async () => ...}`，
      // 规则要求 void 包裹。此为 NexBox 的既定写法（返回值被忽略无害）。
      "@typescript-eslint/no-misused-promises": "off",
      // require-await：NexBox 部分 async 函数无 await（直接 return Promise）。
      "@typescript-eslint/require-await": "off",
      // no-base-to-string：strict 下 any/unknown 收窄产物，非真实类型缺陷。
      "@typescript-eslint/no-base-to-string": "off",
      // unsafe-* 系列：strict:false 下 NexBox 允许隐式 any 流转。
      "@typescript-eslint/no-unsafe-member-access": "off",
      "@typescript-eslint/no-unsafe-assignment": "off",
      "@typescript-eslint/no-unsafe-argument": "off",
      "@typescript-eslint/no-unsafe-return": "off",
      // no-explicit-any：NexBox 原文含 any（Aegis 编码规范 §一.1 禁 any 逃逸）。
      // 登记为 §〇·0.4 豁免项，阶段二换皮时随改写一并消除。
      "@typescript-eslint/no-explicit-any": "off",
      // no-unnecessary-type-assertion / prefer-promise-reject-errors：strict 收窄耦合。
      "@typescript-eslint/no-unnecessary-type-assertion": "off",
      "@typescript-eslint/prefer-promise-reject-errors": "off",
      // ── 以下两条依据 NexBox 自身的 lint 口径 ──
      // NexBox `eslint.config.mjs:22` 用 `next/core-web-vitals`（Next.js 官方规则集），
      // 其中 react-hooks/rules-of-hooks 与 exhaustive-deps 为 **warn 级**，
      // 且其 `package.json` 的 lint 脚本是 `eslint .`（**不带 --max-warnings 0**）。
      // Aegis 侧把这两条提升为 error 并零容忍，属本项目自加的更严标准；
      // 阶段一照搬 NexBox 原文时必然触发（NexBox 源码存在 early-return 后调用 Hook 等写法）。
      // 处置：音乐域降为 warn，与参照工程口径一致；非音乐域仍保持 error。
      "react-hooks/rules-of-hooks": "warn",
      "react-hooks/exhaustive-deps": "warn",
      // prefer-const：NexBox 原文含 let 声明后未再赋值（其 ESLint 未启用该规则）。
      "prefer-const": "warn",
      // no-empty（含 allowEmptyCatch:false）：Aegis 编码规范 §三「禁静默失败」把它设为 error，
      // 且用 allowEmptyCatch:false 连空 catch 也打红。
      // 但实测 NexBox 原文**同样有 20 处 `catch {}`**（如 MusicPage.tsx:3896 localStorage.setItem
      // 失败即忽略、3976 音量恢复失败即忽略），而 NexBox 的 eslint 配置（eslint.config.mjs:22
      // 的 next/core-web-vitals）**不含 no-empty 规则** —— 即参照工程允许空 catch。
      // 阶段一 100% 复刻要求照搬这些空 catch，故音乐域降为 warn；
      // 非音乐域仍保持 error + allowEmptyCatch:false（Aegis 自加的更严标准不放弃）。
      // 登记为 §〇·0.4 豁免项，阶段二换皮时应逐处补日志而非保留空 catch。
      "no-empty": ["warn", { allowEmptyCatch: false }],
      "no-useless-catch": "warn",
      // no-unused-vars：NexBox 原件含未使用的形参与解构位（保持原文，不删不改）。
      "@typescript-eslint/no-unused-vars": [
        "warn",
        { argsIgnorePattern: "^_", varsIgnorePattern: "^_", ignoreRestSiblings: true },
      ],
    },
  },
  // ── 阶段一 100% 复刻：游戏（Steam）域 lint 豁免（2026-10-03 用户裁决）──
  // 依据 Docs/开发文档/02-专题设计/09-游戏管理模块移植调研.md §〇：游戏模块**基线是 NexBox**
  // （仅卡片视觉参考 SystemToolkit），代码原样搬入，故与音乐域同口径处理。
  // 处置原则与上方音乐域块完全一致：**只关闭依赖 strict 类型信息或与 strict 收窄耦合的规则**，
  // 结构性规则（rules-of-hooks / no-empty / no-useless-catch 等）保留。
  // 实测当前触发项（22 处）全部来自 NexBox 原件既有的 async 事件处理器与 fire-and-forget invoke：
  //   - no-misused-promises：`onClick={async () => { await invoke(...) }}` 共 15 处
  //   - no-floating-promises：不 await 的 `invoke(...)`（含 `.then()` 无 rejection handler）共 7 处
  // 两者在 NexBox 的 `next/core-web-vitals` 口径下均不报错，属参照工程既定写法，非缺陷。
  {
    files: ["src/features/GameManager/**/*.{ts,tsx}"],
    rules: {
      "@typescript-eslint/no-floating-promises": "off",
      "@typescript-eslint/no-misused-promises": "off",
      "@typescript-eslint/require-await": "off",
      "@typescript-eslint/no-base-to-string": "off",
      "@typescript-eslint/no-unsafe-member-access": "off",
      "@typescript-eslint/no-unsafe-assignment": "off",
      "@typescript-eslint/no-unsafe-argument": "off",
      "@typescript-eslint/no-unsafe-return": "off",
      "@typescript-eslint/no-explicit-any": "off",
      "@typescript-eslint/no-unnecessary-type-assertion": "off",
      "@typescript-eslint/prefer-promise-reject-errors": "off",
      "react-hooks/rules-of-hooks": "warn",
      "react-hooks/exhaustive-deps": "warn",
      "prefer-const": "warn",
      "no-empty": ["warn", { allowEmptyCatch: false }],
      "no-useless-catch": "warn",
      "@typescript-eslint/no-unused-vars": [
        "warn",
        { argsIgnorePattern: "^_", varsIgnorePattern: "^_", ignoreRestSiblings: true },
      ],
    },
  },
  // ── M-09 系统优化域 lint 豁免（2026-10-07 用户裁决新增模块）──
  // 依据：NexBox「内置工具 + 优化」两模块全量搬迁（GPL-3.0 同源衍生），
  // 口径与上面两个复刻域**完全一致**：只关闭依赖 strict 类型信息 / 与 strict 收窄耦合的规则，
  // 结构性规则（rules-of-hooks / exhaustive-deps / no-empty / no-useless-catch）保留但降 warn。
  // 与音乐/游戏域的差别：复刻代码**不含** `src/types/music.ts` 那类共享类型文件，
  // 故豁免范围只有 feature 目录本身。
  {
    files: ["src/features/SystemOptimization/**/*.{ts,tsx}"],
    rules: {
      "@typescript-eslint/no-floating-promises": "off",
      "@typescript-eslint/no-misused-promises": "off",
      "@typescript-eslint/require-await": "off",
      "@typescript-eslint/no-base-to-string": "off",
      "@typescript-eslint/no-unsafe-member-access": "off",
      "@typescript-eslint/no-unsafe-assignment": "off",
      "@typescript-eslint/no-unsafe-argument": "off",
      "@typescript-eslint/no-unsafe-return": "off",
      "@typescript-eslint/no-explicit-any": "off",
      "@typescript-eslint/no-unnecessary-type-assertion": "off",
      "@typescript-eslint/prefer-promise-reject-errors": "off",
      "react-hooks/rules-of-hooks": "warn",
      "react-hooks/exhaustive-deps": "warn",
      "prefer-const": "warn",
      "no-empty": ["warn", { allowEmptyCatch: false }],
      "no-useless-catch": "warn",
      "@typescript-eslint/no-unused-vars": [
        "warn",
        { argsIgnorePattern: "^_", varsIgnorePattern: "^_", ignoreRestSiblings: true },
      ],
    },
  },
);
