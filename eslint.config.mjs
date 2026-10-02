// ESLint 9 flat config（01-编码与命名规范 §四：ESLint 9 flat config；版本口径见 DEPENDENCIES.md §二）
// 本批只装"提交前检查点"要求的两条腿之一：可判定红绿的类型/React/异味规则集。
// 架构守卫规则族（boundaries 跨模块互引、ipc/only-ipc-layer、jsx-a11y、裸值棘轮）属 W0-15 空跑批，此处不预埋。
import js from "@eslint/js";
import tseslint from "typescript-eslint";
import reactHooks from "eslint-plugin-react-hooks";
import reactRefresh from "eslint-plugin-react-refresh";
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
    files: ["**/*.{ts,tsx}"],
    extends: [js.configs.recommended, ...tseslint.configs.recommendedTypeChecked],
    plugins: {
      "react-hooks": reactHooks,
      "react-refresh": reactRefresh,
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
      // 编码规范 §三：禁静默失败——空 catch 与只重新包装的 throw 在此层拦截
      "no-empty": ["error", { allowEmptyCatch: false }],
      "@typescript-eslint/no-unused-vars": [
        "error",
        { argsIgnorePattern: "^_", varsIgnorePattern: "^_" },
      ],
      // 编码规范 §三：exhaustive-deps 必须为 error，不得整文件禁用
      "react-hooks/exhaustive-deps": "error",
      "react-hooks/rules-of-hooks": "error",
      "react-refresh/only-export-components": ["warn", { allowConstantExport: true }],
    },
  },
  {
    // 配置文件跑在 Node 下，不参与类型检查（不在 tsconfig include 内）
    files: ["vite.config.ts", "eslint.config.mjs"],
    extends: [tseslint.configs.recommended],
    languageOptions: { globals: globals.node },
  },
);
