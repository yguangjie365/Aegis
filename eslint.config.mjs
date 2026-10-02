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
    files: ["vite.config.ts", "eslint.config.mjs"],
    extends: [js.configs.recommended, ...tseslint.configs.recommended],
    languageOptions: {
      globals: globals.node,
      parserOptions: { project: false, projectService: false },
    },
  },
);
