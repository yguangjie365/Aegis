import { extendTheme } from "@chakra-ui/react";
import {
  nvidiaLight,
  themePacks,
  MONO_FONT_STACK,
  COLOR_TOKEN_KEYS,
  TYPOGRAPHY_TOKEN_KEYS,
  SPACING_TOKEN_KEYS,
  ROUNDED_TOKEN_KEYS,
  ELEVATION_TOKEN_KEYS,
  MOTION_DURATION_TOKEN_KEYS,
  MOTION_EASING_TOKEN_KEYS,
  TOKEN_KEY_COUNT,
  type TypographyValue,
} from "./tokens";

// 令牌 → Chakra theme + CSS 变量双出口：
// - Chakra 组件经 theme.aegis.* / theme.radii.* 取键（W1-04 裸值棘轮的对照面）
// - 非组件层（全局 css、伪元素）经 --aegis-<key> CSS 变量，两处同源
//
// 🔴 当前生效的主题包 = themePacks[0]（首期仅浅色一包，TASKS Q-06）。
// 换 Dark 包时只改 activePack 的来源，theme 层不得再出现 nvidiaLight 字面引用。

// 取 activePack：themePacks 的唯一消费点。 Dark 包加入时改为按 activePackId 取出即可。
const activePack = themePacks[0] ?? nvidiaLight;

// ── 键集守卫（P0-3）────────────────────────────────────────────────────────────
// 原实现只比对「五维 key 数之和 == 48」，改错一个键名（如 "mute-on-soft"→"muteOnSoft"）
// 同时漏一个键时数量守恒，守卫静默通过。改为逐维比对键集，missing/extra 一并报出。
// 2026-10-04：motion 升为第六维后，motion 内部再分duration / easing 两组分别校验 ——
// 时长与缓动是两类不同的值，混在一维里比对会漏掉「加了时长没加缓动」这类半吊子改动。
const assertKeys = (actual: readonly string[], declared: readonly string[], dim: string) => {
  const miss = declared.filter((k) => !actual.includes(k));
  const extra = actual.filter((k) => !declared.includes(k));
  if (miss.length || extra.length) {
    throw new Error(
      `[theme] 主题包 ${activePack.name} 维度 "${dim}" 键集漂移：missing=[${miss.join(", ")}]` +
        ` extra=[${extra.join(", ")}]\n` +
        `契约见 tokens.ts 的 *_TOKEN_KEYS（覆盖基数 ${TOKEN_KEY_COUNT}）`,
    );
  }
};
assertKeys(Object.keys(activePack.colors), COLOR_TOKEN_KEYS, "colors");
assertKeys(Object.keys(activePack.typography), TYPOGRAPHY_TOKEN_KEYS, "typography");
assertKeys(Object.keys(activePack.spacing), SPACING_TOKEN_KEYS, "spacing");
assertKeys(Object.keys(activePack.rounded), ROUNDED_TOKEN_KEYS, "rounded");
assertKeys(Object.keys(activePack.elevation), ELEVATION_TOKEN_KEYS, "elevation");
assertKeys(Object.keys(activePack.motion.duration), MOTION_DURATION_TOKEN_KEYS, "motion.duration");
assertKeys(Object.keys(activePack.motion.easing), MOTION_EASING_TOKEN_KEYS, "motion.easing");

// ── 派生：CSS 变量表 ──────────────────────────────────────────────────────────
const cssVars: Record<string, string> = {
  // 颜色 / 间距：全量循环，键集即契约
  ...Object.fromEntries(Object.entries(activePack.colors).map(([k, v]) => [`--aegis-${k}`, v])),
  ...Object.fromEntries(
    Object.entries(activePack.spacing).map(([k, v]) => [`--aegis-space-${k}`, v]),
  ),
  // 圆角 / 高度：同样循环生成，避免逐条手写漏键（原实现只写了 sm/full 两条，flat/hairline/none 缺失）
  ...Object.fromEntries(
    Object.entries(activePack.rounded).map(([k, v]) => [`--aegis-radius-${k}`, v]),
  ),
  ...Object.fromEntries(
    Object.entries(activePack.elevation).map(([k, v]) => [`--aegis-elevation-${k}`, v]),
  ),
  // 第六维 motion（2026-10-04）：时长与缓动各出一组 --aegis-motion-* 变量。
  // 🔴 变量名带 motion 前缀是刻意的 —— 组件里写 transition: var(--aegis-motion-duration-fast)
  // 时，一眼能看出它该来令牌层拿而不是随手写个 120ms。
  ...Object.fromEntries(
    Object.entries(activePack.motion.duration).map(([k, v]) => [`--aegis-motion-duration-${k}`, v]),
  ),
  ...Object.fromEntries(
    Object.entries(activePack.motion.easing).map(([k, v]) => [`--aegis-motion-easing-${k}`, v]),
  ),
  // 字体栈（P2-10 三源收口：theme.fonts.mono / --font-mono / typography.mono）
  "--font-mono": MONO_FONT_STACK,
};
// typography 12 键 × size/weight/leading（+ tracking/transform）纯 CSS 出口，
// 供伪元素与业务组件（原本只能硬编码）使用
for (const [k, v] of Object.entries(activePack.typography)) {
  cssVars[`--aegis-type-${k}-size`] = v.fontSize;
  cssVars[`--aegis-type-${k}-weight`] = String(v.fontWeight);
  cssVars[`--aegis-type-${k}-leading`] = String(v.lineHeight);
  if (v.letterSpacing) cssVars[`--aegis-type-${k}-tracking`] = v.letterSpacing;
  if (v.textTransform) cssVars[`--aegis-type-${k}-transform`] = v.textTransform;
}

// ── 派生：typography 刻度 + textStyles（P0-2）─────────────────────────────────
// 原实现只声明了 token 却无出口：没有 fontSizes/lineHeights 刻度（Chakra 组件仍在用
// 默认值）、没有 textStyles（<Text textStyle="h2"> 用不了）、body.fontSize 是手写字面量
// "14px"（与 typography.body.fontSize 两份事实源）。此处统一由 typography 派生。
const toScale = <V extends string | number>(
  pick: (v: TypographyValue) => V | undefined,
): Record<string, V> => {
  const out: Record<string, V> = {};
  for (const [k, v] of Object.entries(activePack.typography)) {
    const val = pick(v);
    if (val !== undefined) out[k] = val;
  }
  return out;
};
const fontSizes = toScale<string>((v) => v.fontSize);
const fontWeights = toScale<string>((v) => String(v.fontWeight));
const lineHeights = toScale<number>((v) => v.lineHeight);
const letterSpacings = toScale<string>((v) => v.letterSpacing);
const textStyles = Object.fromEntries(
  Object.entries(activePack.typography).map(([k, v]) => [
    k,
    {
      fontSize: v.fontSize,
      fontWeight: v.fontWeight,
      lineHeight: v.lineHeight,
      ...(v.letterSpacing ? { letterSpacing: v.letterSpacing } : {}),
      ...(v.textTransform ? { textTransform: v.textTransform } : {}),
    },
  ]),
);

export const theme = extendTheme({
  config: { initialColorMode: "light", useSystemColorMode: false },
  colors: { aegis: activePack.colors },
  fonts: {
    // 中文主面 = 思源黑体（Noto Sans SC，已定档 2026-10-02，规范 §二/§七）；雅黑仅兜底
    heading: `"Inter","Noto Sans SC","PingFang SC","Microsoft YaHei",sans-serif`,
    body: `"Inter","Noto Sans SC","PingFang SC","Microsoft YaHei",sans-serif`,
    mono: MONO_FONT_STACK,
  },
  // 🔴 圆角刻度（规范 §二 rounded + §三「交互元素恒 2px」）
  //
  // 阶段二换皮 D-04 的根因修复：Chakra 各组件默认 `borderRadius: md`(6px) / `lg`(8px)，
  // 此前只逐个 props 改 `borderRadius` 属性，**未显式写圆角的组件仍落回默认值**。
  //
  // ⚠️⚠️ 键名必须是 **radii** —— 2026-10-04 实证：早期版写的是 `borderRadius:`，
  // 该键虽被 extendTheme 原样挂到 theme 上，但 styled-system 解析 `border-radius` 样式
  // 属性时查的是 `theme.radii`（@chakra-ui/styled-system border.d.ts:
  // `borderRadius?: Token<..., "radii">`），故整段是**死代码**，D-04 从未通过主题层生效。
  // 实测（SSR 抽 emotion CSS）：Card/Tag/Input/MenuList/Popover/Kbd/Editable 全部
  // `border-radius: var(--chakra-radii-md)` = 6px，仅被 baseStyle 白名单救回的
  // Button/Badge 是 2px。
  //
  // 处置：覆写 `radii` 刻度，把 **Chakra 内部作为默认值使用的档位**（base 4px / md 6px）
  // 压到 sm(2px)，使「不写 borderRadius」默认即合规，无需再逐组件打补丁 —— Button/Input/
  // Card/Tag/MenuList/ModalContent/Popover/Editable/Kbd 全部走 md，实测已由 6px 变 2px。
  //
  // 🟡 保留档（刻意不压）：`lg` 8px / `xl` 12px / `2xl` 16px / `3xl` 24px 仍为 Chakra 原值。
  //    这四档是复刻域与换皮页的**显式装饰圆角**（MusicPage 封面 `rounded="2xl"`、封面占位
  //    `rounded="2xl"`、SteamPage 卡片 `rounded="xl"`/菜单 `rounded="lg"` 共 11 处），压平会
  //    破坏「音乐/游戏模块完全复用 NexBox、仅适配视觉层」的视觉基线。属规范 §二「rounded
  //    仅三档」与复刻件既有视觉的冲突，按既有口径**优先保持复刻**，登记于差异清单台账。
  radii: {
    none: activePack.rounded.none, // 0（规范档）
    sm: activePack.rounded.sm, // 2px（规范档）
    base: activePack.rounded.sm, // 2px（Chakra 默认 4px，**默认落点**）
    md: activePack.rounded.sm, // 2px（Chakra 默认 6px，**默认落点**）
    lg: "8px", // Chakra 默认，保留（换皮页显式使用）
    xl: "12px", // Chakra 默认，保留（复刻/换皮显式使用）
    "2xl": "16px", // Chakra 默认，保留（NexBox 复刻封面）
    "3xl": "24px", // Chakra 默认，保留以免组件取到 undefined
    full: activePack.rounded.full, // 9999px（头像/状态圆点，规范档）
  },
  fontSizes,
  fontWeights,
  lineHeights,
  letterSpacings,
  textStyles,
  // ⚠️ 组件级兜底只剩 Badge：其 styleConfig 默认 `borderRadius: "full"`（胶囊），
  // 而规范要求 badge 恒 2px —— 这不是「默认落点」问题，只能逐组件纠偏。
  // Button/Input/Textarea/Select/IconButton 已走上面的 `radii.md`=2px，无需再列。
  components: {
    Badge: {
      baseStyle: { borderRadius: activePack.rounded.sm },
    },
  },
  styles: {
    global: {
      ":root": {
        ...cssVars,
        // 壳层（2026-10-04 改版）：标题条不再是文档流的一行，而是 fixed 透明覆盖层
        // （拖动区 + 窗口控制），因此**不占布局**，主内容区拿到完整视口高度。
        // 该变量只描述覆盖层自身高度，供其内部按钮对齐使用。
        "--aegis-dragstrip-h": "32px",
      },
      html: {
        background: activePack.colors.canvas,
        // P1-6：`overflow-x: hidden` 在 html 上会隐式把 overflow-y 变 auto → 创建滚动容器，
        // 破坏后代 position:sticky / 100vh 语义与滚动锚定。`clip` 不创建滚动容器。
        overflowX: "clip",
      },
      body: {
        color: activePack.colors.ink,
        // P0-2：原为手写字面量 "14px"，现取 typography.body 单一事实源
        fontSize: activePack.typography.body.fontSize,
        lineHeight: activePack.typography.body.lineHeight,
        // 规范 §二：禁系统合成伪粗（回退雅黑时尤须）
        fontSynthesis: "none",
        WebkitFontSmoothing: "antialiased",
        overflowX: "clip",
      },
      // 规范 §八：焦点环 2px accent-strong（浅底）偏移 2px，禁 outline:none
      "*:focus-visible": {
        outline: `2px solid ${activePack.colors["accent-strong"]}`,
        outlineOffset: "2px",
      },
      // P2-12：Chakra 组件自身的 _focusVisible 特异性更高、会与之叠加成双环，
      // 故显式收口 —— 焦点环统一由组件层 _focusVisible 控制，全局规则仅作兜底
      ":focus:not(:focus-visible)": { outline: "none", outlineOffset: "0" },
      // 🔴 阶段一 100% 复刻补入（2026-10-03）：以下全局规则来自参照工程
      // NexBox `src/index.css`（64-92 / 255-292 行）。Aegis 无 index.css，
      // 搬 MusicPage 原件时这些规则一并缺失，导致两类可见缺陷：
      //   ① `.real-liquid-glass-svg-defs`（液态玻璃 SVG 滤镜容器）无 `width/height:0`
      //      → 该 <svg> 只有 viewBox 无显式尺寸，浏览器按默认 **150px** 渲染，
      //        把整个页面撑高 150px —— 即用户报告的「页面被拉长」；
      //   ② 滚动条未被隐藏，LiquidGlassCard 内列表与参照工程观感不一致。
      // 均按参照工程原值照搬，未改数值。
      // ⚠️ P2-13：`.real-liquid-glass*` 属业务组件（LiquidGlassCard）的实现细节而非设计
      //   令牌，置于全局主题会让「换皮」与「某个页面的某个组件」耦合。**终局**应迁入
      //   LiquidGlassCard 自身的 sx 或 CSS Module；此处仅为「无 index.css」期的临时搬迁。
      // 隐藏 SVG 滤镜容器：占位 0×0，不参与布局
      ".real-liquid-glass-svg-defs": {
        position: "absolute",
        width: "0",
        height: "0",
        overflow: "hidden",
        pointerEvents: "none",
      },
      // 液态玻璃基础类：isolation 建立层叠上下文 + 多层 inset 阴影模拟玻璃边缘
      ".real-liquid-glass": {
        isolation: "isolate",
        boxShadow:
          "inset 0 0 2px 1px rgba(255,255,255,0.18), inset 0 0 10px 4px rgba(255,255,255,0.06), inset 0 4px 16px rgba(0,0,0,0.04), inset 0 8px 24px rgba(0,0,0,0.03), 0 4px 16px rgba(0,0,0,0.08), 0 8px 24px rgba(0,0,0,0.04)",
      },
      // 🔴 2026-10-04 移除全局隐藏滚动条（原 `*: { scrollbarWidth: none }` +
      // `*::-webkit-scrollbar: { display: none }`，搬自 NexBox index.css:66-72）。
      //
      // 移除理由（**不是**「复刻域需要它」而是「它对复刻域本就是死代码」）：
      // 复刻域 MusicPage 自己就用 `scrollbarSx` 逐容器画了 4px 细滚动条
      // （`&::-webkit-scrollbar: {width:4px}` + thumb 上色 + `scrollbarGutter: stable`，
      // 见 MusicPage.tsx:190-195）。该规则编译后是 `.css-xxx::-webkit-scrollbar`，
      // 特异性 (0,1,1) **高于**全局的 `*::-webkit-scrollbar` (0,0,1) —— 复刻域一直走的是
      // 自己那套，全局这条从未生效过。
      //
      // 而对**非复刻域**（本机概览这类高密度表格/清单页）它是唯一起作用的规则，
      // 后果是全站失去「可滚动」与「当前位置」的可见提示 —— 与规范 §一.5
      // 「信息密度优先、表格/清单为主角」的定位直接冲突。
      // （2026-10-04 UI/动效审查「提示 1」；该报告建议「收窄到复刻域容器」，
      //   实测复刻域不需要，故按实测取更彻底的删除。）
      // ⚠️ 若日后复刻域某处需隐藏滚动条，应在**该容器**上就地处理，不得再放全局。
      // 视口级禁横向滚动（原 NexBox index.css:80-83 写 `html, body { overflow-x: hidden }`，
      // 现按 P1-6 改为 clip，语义不变且不再制造滚动容器）
      "#root": { minHeight: "100vh" },
      /* ══ 全局 reduced-motion 兜底（2026-10-04 用户裁决恢复）══════════════════
         此前按用户裁决 2026-10-03 整段移除，理由是「只压时长不限次数」会让 infinite 动画
         在 60fps 下每帧重放约 1670次（0.01ms 跑完一圈又无限循环）→ 疯狂旋转 + 剧烈闪烁；
         当时改由各组件局部实现。但**局部实现必然漏**（本轮审查就抓到骨架屏是本域唯一
         漏做的一处），且整段删除等于放弃 WCAG 2.3.3 / 2.2.2。

         🔴 现按标准写法恢复（Bootstrap / GOV.UK 同款）：**同时**限时长**与次数**。
         两者缺一不可：
           · 只压时长 → infinite 动画变成「0.01ms 一圈 × 无限次」= 每帧重放上千次（原文的坑）
           · 只限次数 → 动画仍以原时长播放 2.4s，只是不再循环（观感上仍在动，不够）
         两者同限 → 唱片停在第一帧、骨架停在最终态、过渡瞬间完成，皆为静态且信息不丢。

         ⚠️ 与组件局部实现的关系（已按 CSS 级联实测确认，非推测）：
         局部规则写在组件类上（`.css-xxx::after { animation: none }`），全局写在 `*` 上，
         **单看 specificity 局部确实更强**。但本条全局带 `!important`，按 CSS 规范
         importance 先于 specificity —— 全局的 `animation-duration: 0.01ms !important`
         确实会压掉局部的 `animation-duration: 0s`。
         ✅ **局部设计仍能保住**，原因是全局**只声明 duration / iteration-count，
         从不声明 `animation-name`**：`animation-name` 仍是局部的 `none`，
         没有动画名就没有时间轴，被压掉的 duration 无从生效。
         故：全局兜底只兜住「没写局部规则」的组件，不夺走组件的停帧判断
         （如流光环刻意停在最亮处 120deg 而非 0deg）。*/
      "@media (prefers-reduced-motion: reduce)": {
        "*, *::before, *::after": {
          animationDuration: "0.01ms !important",
          // 🔴 关键：必须同时限次数。只压时长会让 infinite 动画每帧重放上千次。
          animationIterationCount: "1 !important",
          transitionDuration: "0.01ms !important",
          // 编程式平滑滚动同样属动效（scroll-behavior: smooth 会让锚点跳转长距离滑动）
          scrollBehavior: "auto !important",
        },
      },
    },
  },
});
