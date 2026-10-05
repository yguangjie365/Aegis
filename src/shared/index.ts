/**
 * Aegis 跨 feature 共享基建门面（Shared Infrastructure Facade）
 * ============================================================================
 * 背景（2026-10-03 · B-G1 游戏模块开工）
 * ---------------------------------------------------------------------------
 * NexBox 把 `LiquidGlassCard` / `useDynamicIsland` / `useBackground` /
 * `useThemeColor` / `CustomSelect` 放在**全局** `src/components` `src/contexts`
 * 下，故其 `SteamPage` 与 `MusicPage` 天然共享。
 *
 * Aegis 音乐模块（M-07）阶段一复刻时按 feature 分层落到了
 * `features/MusicManager/{components,contexts,hooks}`。游戏模块（B-G1）依赖同样的
 * 5 项基建，若 GameManager 直接 `import` 音乐域路径，会形成
 * **feature → feature 的反向耦合**，违反 `02-架构与依赖规范` 的分层约束。
 *
 * 处置口径（本文件即该裁决的落地）
 * ---------------------------------------------------------------------------
 * 1. **本门面只做 re-export，不含任何实现** —— 音乐域那 5 个文件的源码逐字未动
 *    （音乐模块受「完全复用 NexBox 铁律」保护：❌ 禁止重写、❌ 禁止搬动其目录）。
 * 2. GameManager 只认 `@/shared/*` 这条路径，**不感知**实现落在哪个 feature。
 * 3. 后续若要彻底物理上移（把实现搬进 `src/components` 等公共层并改音乐域 import），
 *    只需改本门面一处，GameManager 侧零改动。
 * 4. 这是一个**显式的技术债隔离层**，不是终局形态。终局口径与音乐域 import 改写
 *    需用户单独裁决（涉及动音乐域复刻基线）。
 *
 * 已上移项（5 项，对应调研报告 §3.4 的依赖面）：
 *   - components/liquid-glass-card.tsx
 *   - components/dynamic-island.tsx
 *   - contexts/background-context.tsx
 *   - contexts/theme-color-context.tsx
 *   - hooks/use-adaptive-text-color.ts
 */

// ── 组件 ──
export { LiquidGlassCard } from "@/features/MusicManager/components/liquid-glass-card";
export { useDynamicIsland } from "@/features/MusicManager/components/dynamic-island";
export type {
  DynamicIslandToast,
  IslandOptions,
  IslandStatus,
  IconKey,
} from "@/features/MusicManager/components/dynamic-island";
export { CustomSelect } from "@/features/MusicManager/components/custom-select";

/* 🔴 2026-10-05 新增：迷你播放器经本门面供**壳层**挂载。
 *
 * 为什么不直接 import：壳层（`shell-layout.tsx`）若直接 import 复刻域组件，
 * 虽然主 tsconfig（include: src）允许，但会形成「壳层 → 复刻域」的源码级耦合，
 * 与本项目「跨 feature 共享一律走门面」的纪律不符（见文件头说明）。
 * ⚠️ 方向说明：门面做 re-export，不含实现（与上面几条同口径）。 */
export { MiniMusicPlayer } from "@/features/MusicManager/components/MiniMusicPlayer";

// ── Contexts ──
// 注：Provider 在 `src/main.tsx` 顶层挂载（BackgroundProvider > ThemeColorProvider），
//     全应用单例，任何 feature 的页面均可消费。
export {
  BackgroundProvider,
  useBackground,
} from "@/features/MusicManager/contexts/background-context";
export {
  ThemeColorProvider,
  useThemeColor,
} from "@/features/MusicManager/contexts/theme-color-context";

// ── Hooks ──
export { useAdaptiveTextColor } from "@/features/MusicManager/hooks/use-adaptive-text-color";

/* 🔴 2026-10-05 新增：品牌区注册 hook 经本门面导出（供 **GameManager** 使用）。
 *
 * 🔴 **音乐域不可用此路径** —— `tsconfig.music-replica.json` 的 include 只有
 *   「音乐域 + types/music.ts」，连本门面都不在清单内 ⇒ 音乐域 import 本门面也撞 TS6307。
 *   实测两条路都失败（`@/shell/brand-bar` 与 `@/shared`），故音乐页**放弃注册品牌区**，
 *   由 `brand-bar.tsx` 的归属校验保证它「不显示别人的副标题」（安全失败）。
 *   GameManager **无独立 tsconfig**，故可正常走门面。 */
export { useRegisterBrand } from "@/shell/brand-bar";
export type { BrandContent } from "@/shell/brand-bar";
