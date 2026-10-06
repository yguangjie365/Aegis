import { extendTheme } from "@chakra-ui/react";
import {
  themePacks,
  DECORATIVE_RADII,
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
// 🔴 当前生效的主题包 = themePacks[0] = **Nvidia.Dark**（2026-10-04 用户裁决：
// NVIDIA 品牌风格更适合深色，本期交付**纯深色单主题**，浅色包另行设计）。
// 多主题包机制**不退役** —— nvidiaLight 仍注册在 themePacks 里，键集守卫逐维校验两包。
//
// 🔴 本文件**不得出现 nvidiaLight / nvidiaDark 的字面取值**，一切取值经 activePack 消费
// （原注释里的「`??` 兜底值除外」已随兜底删除而失效 —— 见下方 activePack 处的显式 throw）。

// 取 activePack：themePacks 的唯一消费点。
// 🔴 2026-10-04 复审：原写法 `themePacks[0] ?? nvidiaDark` 的兜底**永远不会生效**（数组是
// 字面量且非空），却会在真为空时把错误伪装成"正常降级"。改为显式失败。
const [activePack] = themePacks;
if (!activePack) {
  throw new Error(
    "[theme] themePacks 为空，无可激活的主题包。这是模块初始化错误（打包配置 / 循环依赖），" +
      "不应用 ?? 兜底掩盖。",
  );
}

// ── 键集守卫（P0-3）────────────────────────────────────────────────────────────
// 原实现只比对「五维 key 数之和 == 48」，改错一个键名（如 "mute-on-soft"→"muteOnSoft"）
// 同时漏一个键时数量守恒，守卫静默通过。改为逐维比对键集，missing/extra 一并报出。
// 2026-10-04：motion 升为第六维后，motion 内部再分duration / easing 两组分别校验 ——
// 时长与缓动是两类不同的值，混在一维里比对会漏掉「加了时长没加缓动」这类半吊子改动。
const assertKeys = (
  actual: readonly string[],
  declared: readonly string[],
  dim: string,
  packName: string,
) => {
  const miss = declared.filter((k) => !actual.includes(k));
  const extra = actual.filter((k) => !declared.includes(k));
  if (miss.length || extra.length) {
    throw new Error(
      `[theme] 主题包 ${packName} 维度 "${dim}" 键集漂移：missing=[${miss.join(", ")}]` +
        ` extra=[${extra.join(", ")}]\n` +
        `契约见 tokens.ts 的 *_TOKEN_KEYS（覆盖基数 ${TOKEN_KEY_COUNT}）`,
    );
  }
};

/* 🔴 2026-10-04 复审：原实现只对 activePack 校验，而上方注释写着「键集守卫逐维校验两包」
 * —— 注释比代码更激进。后果是**非激活的那一包**（当前是 nvidiaLight）缺键时无人报错：
 * TS 的 Record<ColorToken, string> 只在「字面量直接赋值」时做 excess-property 检查，
 * 一旦将来某个包经 spread / 工厂 / 动态导入构造，类型层就失效，只剩运行时守卫兜底。
 * ⇒ 改为遍历 themePacks 全量校验，并让报错信息带上**包名**（原来永远指 activePack）。 */
const validatePack = (pack: typeof activePack): void => {
  assertKeys(Object.keys(pack.colors), COLOR_TOKEN_KEYS, "colors", pack.name);
  assertKeys(Object.keys(pack.typography), TYPOGRAPHY_TOKEN_KEYS, "typography", pack.name);
  assertKeys(Object.keys(pack.spacing), SPACING_TOKEN_KEYS, "spacing", pack.name);
  assertKeys(Object.keys(pack.rounded), ROUNDED_TOKEN_KEYS, "rounded", pack.name);
  assertKeys(Object.keys(pack.elevation), ELEVATION_TOKEN_KEYS, "elevation", pack.name);
  assertKeys(
    Object.keys(pack.motion.duration),
    MOTION_DURATION_TOKEN_KEYS,
    "motion.duration",
    pack.name,
  );
  assertKeys(Object.keys(pack.motion.easing), MOTION_EASING_TOKEN_KEYS, "motion.easing", pack.name);
};
themePacks.forEach(validatePack);

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
  /* 🔴 2026-10-04 复审修复：装饰圆角档此前**只有 Chakra 出口、没有 CSS 变量出口**。
   * 上面那行只循环 `activePack.rounded`（3 键）⇒ 生成三个变量；而 `theme.radii` 里
   * 另有 lg / xl / 2xl / 3xl 四档，纯 CSS 层（伪元素 / 全局 css）根本拿不到。
   * 这里补上循环，保证「两个出口同源」（与 tokens.ts 顶部纪律一致）。 */
  ...Object.fromEntries(
    Object.entries(DECORATIVE_RADII).map(([k, v]) => [`--aegis-radius-${k}`, v]),
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
  // 壳层（2026-10-04 改版）：标题条不再是文档流的一行，而是 fixed 透明覆盖层
  // （拖动区 + 窗口控制），因此**不占布局**，主内容区拿到完整视口高度。
  // 该变量只描述覆盖层自身高度，供其内部按钮对齐使用。
  "--aegis-dragstrip-h": "32px",
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

/* 🔴🔴 N1（2026-10-06 UI 审查，渲染取证撞出）：变量表**不能走 Chakra `styles.global` 的对象通道**。
 *
 * 现象：含**顶层逗号**的令牌值被静默切碎 ——
 *   `--font-mono`   声明 "JetBrains Mono","Consolas",monospace ⇒ 浏览器里只剩 "Consolas"
 *                   ⇒ 全站 14 处 `font-family: var(--font-mono)` 用不到 §七 指定的等宽面
 *   `--aegis-elevation-sticky` 两层阴影 ⇒ 只剩那道 1px 亮边，黑阴影层丢失
 *   对照：`cubic-bezier(0, 0, 0.2, 1)`（逗号在括号内）完好 ⇒ 只有**顶层**逗号会被切
 * 定位：把同样的内容用**裸 CSS 文本**写进 <style> 则完好无损 ⇒ 截断发生在 JS→CSS 生成期
 *   （Chakra global 对象 → styled-system/emotion 的值处理），不是浏览器解析。
 * 危害等级：`tsc` / `vitest` / `vite build` 全绿，且 Consolas 恰好也是等宽 ⇒ 肉眼不可见。
 *   这是本仓「已知陷阱」第 ① 类（静默失效）的放大版：整段令牌值被改写而无人知晓。
 *
 * 处置：本模块在 **import 期**把变量表序列化成裸 CSS 文本挂进 <head>，完全不经 CSS-in-JS。
 *   单一事实源不变（仍由 tokens.ts 的包 → cssVars 派生），只换出口通道。
 *   完整性断言见 theme-pack.test.ts 的「N1 回归」用例（断言的是**生成出来的文本**，
 *   所以这条守卫在 node 环境也成立，不需要 jsdom）。 */
export const cssVarsText = `:root{${Object.entries(cssVars)
  .map(([k, v]) => `${k}:${v}`)
  .join(";")}}`;

if (typeof document !== "undefined") {
  const TAG = "data-aegis-tokens";
  const prev = document.head.querySelector(`[${TAG}]`);
  if (prev) {
    // HMR：theme.ts 被重新执行时替换内容，不累积 <style> 节点
    prev.textContent = cssVarsText;
  } else {
    const el = document.createElement("style");
    el.setAttribute(TAG, "");
    el.textContent = cssVarsText;
    document.head.appendChild(el);
  }
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
// 🔴 2026-10-04 复审：原为 `toScale<string>((v) => String(v.fontWeight))`，把字重转成了字符串。
// Chakra 默认 fontWeights 是 `{ normal: 400, bold: 700 }`（**数字**），转字符串后
// ① 与 Chakra 自身刻度类型不一致 ② 任何参与算术/比较的用法会失效（字符串 "400" * 2 = NaN）。
// ⇒ 改回 number。fontSizes / letterSpacings 保持字符串是对的（Chakra 默认即字符串）。
const fontWeights = toScale<number>((v) => v.fontWeight);
const lineHeights = toScale<number>((v) => v.lineHeight);
const letterSpacings = toScale<string>((v) => v.letterSpacing);
const textStyles = Object.fromEntries(
  Object.entries(activePack.typography).map(([k, v]) => [
    k,
    {
      fontSize: v.fontSize,
      fontWeight: v.fontWeight,
      lineHeight: v.lineHeight,
      // 🔴 2026-10-04 复审修复：原生成器漏了 fontFamily，导致 `typography.mono` 的等宽
      // 字体栈在 textStyle 出口丢失 —— `<Text textStyle="mono">` 静默回退正文字体。
      // 现在由 tokens 侧声明（`mono.fontFamily = var(--font-mono)`），此处只负责透传。
      ...(v.fontFamily ? { fontFamily: v.fontFamily } : {}),
      ...(v.letterSpacing ? { letterSpacing: v.letterSpacing } : {}),
      ...(v.textTransform ? { textTransform: v.textTransform } : {}),
    },
  ]),
);

// ── 控件尺寸档（控件规范 v1 §2.1）───────────────────────────────────────────
// 🔴 显式标注 string：activePack.typography.body 若被改成 undefined / number，
//    期望在此处的类型层就暴露，而不是等到 sizes 被 Chakra 消费时才出问题。
const BODY_SIZE: string = activePack.typography.body.fontSize; // 14px，按钮/输入框统一字号

/* 🔴 2026-10-04 复审修复（真实缺陷）：`::after` 是 absolute，必须有**定位宿主**。
 * Chakra v2 的 Button / Input / Select / IconButton baseStyle **均无** `position: relative`
 *（已查 node_modules/@chakra-ui/theme/dist/esm/components/button.mjs），
 * 因此命中区会相对**最近的已定位祖先**定位 —— 往上找穿 Stack / Modal 直到 #root，
 * 结果是 28×28 的按钮被一块覆盖整页的透明 ::after 包住，**拦截旁边所有元素的点击**。
 * ⇒ 宿主必须由本 sizes 显式提供。relative 不脱离文档流，不影响任何布局。 */
const RELATIVE = { position: "relative" } as const;

/* 🔴 sm 档的命中区补偿：视觉 28 但可点区补到 32（Q4 裁决 A —— 桌面非纯触屏不必一律 44，
 * 但命中区不得 <32）。用 ::after 而非 padding —— absolute 不参与布局，间距不变。
 * 🔴 此常量**必须声明在 controlSizes 之前**：对象字面量在模块求值时就会读取它，
 * 写反了是 TDZ ReferenceError（`const` 无提升）。 */
const HIT_AREA_SM = {
  content: '""',
  position: "absolute",
  top: "-2px",
  bottom: "-2px",
  left: "0",
  right: "0",
} as const;

/** sm 28 / md 36 / lg 44；xs 是 sm 的别名（见 components 处的说明）。 */
const controlSizes = {
  xs: { ...RELATIVE, h: "28px", px: 3, fontSize: BODY_SIZE, _after: HIT_AREA_SM },
  sm: { ...RELATIVE, h: "28px", px: 3, fontSize: BODY_SIZE, _after: HIT_AREA_SM },
  md: { ...RELATIVE, h: "36px", px: 4, fontSize: BODY_SIZE },
  lg: { ...RELATIVE, h: "44px", px: 5, fontSize: BODY_SIZE },
};
/** IconButton 无 px，改用 minW 固定正方形；命中区同理。 */
const iconButtonSizes = {
  xs: { ...RELATIVE, h: "28px", minW: "28px", fontSize: BODY_SIZE, _after: HIT_AREA_SM },
  sm: { ...RELATIVE, h: "28px", minW: "28px", fontSize: BODY_SIZE, _after: HIT_AREA_SM },
  md: { ...RELATIVE, h: "36px", minW: "36px", fontSize: BODY_SIZE },
  lg: { ...RELATIVE, h: "44px", minW: "44px", fontSize: BODY_SIZE },
};

export const theme = extendTheme({
  // 🔴 initialColorMode = dark（2026-10-04 用户裁决：本期纯深色单主题）。
  // useSystemColorMode 必须保持 false —— 单主题下若跟随系统，用户在浅色系统上会拿到未交付的
  // 浅色包，而本期没有浅色包（nvidiaLight 虽在 themePacks 里但**不在交付范围**）。
  config: { initialColorMode: "dark", useSystemColorMode: false },
  colors: { aegis: activePack.colors },
  fonts: {
    // 中文主面 = 思源黑体（Noto Sans SC，已定档 2026-10-02，规范 §二/§七）；雅黑仅兜底
    heading: `"Inter","Noto Sans SC","PingFang SC","Microsoft YaHei",sans-serif`,
    body: `"Inter","Noto Sans SC","PingFang SC","Microsoft YaHei",sans-serif`,
    mono: MONO_FONT_STACK,
  },
  /* 🔴 间距刻度必须**同时**接进 Chakra 的 `space` 键（2026-10-06 UI 审查 B1）。
     §二 spacing 维此前只以 CSS 变量出口（下方 `--aegis-space-*`），Chakra 侧从未接线；
     而 styled-system 解析 `spacing` / `gap` / `margin` 这类属性时查的是 **`theme.space`**
     —— 与下方 `radii` 那次完全同一种错法（当年写 `borderRadius:` 是死代码，实证后改 `radii`）。
     查不到键 ⇒ **原样透传** ⇒ 产出 `gap: lg` 这类非法声明 ⇒ 浏览器静默丢弃 ⇒ 计算值 `normal`(=0)。
     实测后果：Settings 页 28 处 `spacing="xs|sm|md|lg"` 全部归零、三个配置区贴死在一起，
     而 tsc / eslint / vite build 一声不响（本仓「已知陷阱」第 ① 类的形态）。
     值是带单位的字符串（"16px"），与 Chakra 的数字档并存：`spacing={3}` 走 n×4px 换算、
     不查刻度 ⇒ 复刻域与既有数字写法零影响，本行属**纯增量、零回归面**。
     守卫：theme-pack.test.ts 断言 `theme.space` 键集 === SPACING_TOKEN_KEYS（只查 CSS 变量口
     拦不住本类问题，这正是它当初没被拦住的原因）。 */
  space: { ...activePack.spacing },
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
    // 装饰档（Chakra 兼容）：与 cssVars 同源常量，见 tokens.ts 的说明
    ...DECORATIVE_RADII,
    full: activePack.rounded.full, // 9999px（头像/状态圆点，规范档）
  },
  fontSizes,
  fontWeights,
  lineHeights,
  letterSpacings,
  textStyles,
  // 🟡 组件级兜底 = Badge + Card，二者**默认值与规范明确相反**，必须逐组件纠偏：
  //   · Badge：Chakra styleConfig 默认 `borderRadius: "full"`（胶囊），规范要求恒 2px
  //   · Card：默认 `size="md"` 取 `radii.md`，本主题已压到 2px ✓ ——
  //     此处仍显式兜底，是为了防止将来有人改 `radii.md` 而让 Card 静默漂走
  // （其余组件走 `radii.md`=2px，无需再列。）
  components: {
    Badge: {
      baseStyle: { borderRadius: activePack.rounded.sm },
    },
    Card: {
      baseStyle: { borderRadius: activePack.rounded.sm },
    },
    /* ══ 控件高度三档（控件规范 v1 §2.1 定稿，2026-10-04）══════════════════════
     * sm 28 / md 36 / lg 44。**必须在主题层覆盖** —— Chakra 自带 xs24/sm32/md40/lg48 四档，
     * 不覆盖则源码可以随便写 size，规范约束不住。
     *
     * 🔴 `xs` 保留但**映射到 sm**（Q2 裁决 A）：现状 `Button size="xs"` 有 20 处，
     * 若此处不设 xs，它们会回退 Chakra 默认 24 ⇒ 凭空多出第五种高度。
     * 下方注释已注明「xs 是 sm 的别名」，防后人误以为它是独立小号。
     *
     * px 用 Chakra spacing token（3/4/5 = 12/16/20px）而非字面量，以便跟随 spacing 刻度。
     * fontSize 取 typography.body（14px）—— 本项目 typography 12 键里没有 sm 档，
     * 写 fontSize:"sm" 会取到 undefined 而静默回退 Chakra 默认，不如写死来源。
     */
    /* 🟠 曾在此加 baseStyle fontWeight 700 想一处修掉 Chakra 默认的 600（T5），**已回退**：
       主题层的 Button 覆盖对**复刻域约 116 处** Chakra 按钮同样生效
       （MusicPage 83 / AudioEqPage 17 / MusicPlayer 7 / MusicLoginSection 6 / MiniMusicPlayer 2 / game-card 1），
       而 2026-10-07 用户刚裁定「复刻域视觉语系整体让位 NexBox」（规范 §一 #1 例外条款）
       ⇒ 全局改字重等于替复刻域改视觉语系，越界。
       现由调用点局部声明（见 Settings/steam-api-key-section.tsx），
       彻底修法挂 §八 C4「AppButton 与 Chakra Button 收敛为单一实现」。 */
    Button: { sizes: controlSizes },
    Input: { sizes: controlSizes },
    Select: { sizes: controlSizes },
    IconButton: { sizes: iconButtonSizes },
    // Textarea 高度**不设档**（控件规范 §2.1）：只给最小高度，按 3 / 4 / 5 行递增。
    Textarea: {
      sizes: {
        sm: { minH: "66px", fontSize: BODY_SIZE }, // 3 行
        md: { minH: "88px", fontSize: BODY_SIZE }, // 4 行
        lg: { minH: "110px", fontSize: BODY_SIZE }, // 5 行
      },
    },
  },
  /* 🔴 2026-10-04 复审修复：**焦点环双绘**。
   *
   * 事实链（均已查源码，非推测）。
   * 注：复审原建议「把 shadows.outline 改成同色」被**否决**——那样会叠成
   * 「内层 shadow 环 + 外层 outline 环」两圈分离的环，比一环更糟；
   * 正确做法是禁用组件层，让全局 outline 成为唯一来源。
   *   ① Chakra 组件的 `_focusVisible` 是 `{ boxShadow: "outline" }`
   *      （node_modules/@chakra-ui/theme/dist/esm/components/button.mjs:11-13）；
   *   ② `shadows.outline` 默认值 = `0 0 0 3px rgba(66, 153, 225, 0.6)` —— **蓝色**
   *      （同包 foundations/shadows.mjs:9）；
   *   ③ 全局 `*:focus-visible` 另有一条 `outline: 2px solid focus-ring`（绿色）。
   *   `outline` 与 `box-shadow` 是**两个独立属性**，互不覆盖 ⇒ 组件聚焦时
   *   **蓝色 3px 光晕 + 绿色 2px 描边同时出现**（原 P2-12「组件层控制、全局兜底」的
   *   推理不成立 —— 那条 `:focus:not(:focus-visible)` 只处理「非 focus-visible 状态」，
   *   对「两个属性同时画在 focus-visible 上」毫无作用）。
   *
   * 处置选择（**刻意不采用**「把 shadows.outline 改成同色」）：
   *   同色方案下 shadow 在内、outline 在外（offset 2px 把 outline 推到 shadow 之外），
   *   视觉是**两圈分离的环**，比一环更糟，且参数（spread vs offset）语义不同、难以对齐。
   *   ⇒ 改为**禁用组件层**（shadows.outline = none），焦点环**单一来源** = 全局 outline。
   *   好处：裸元素（<a> / <details> / 原生 checkbox）与 Chakra 组件表现完全一致，
   *   且严格符合规范 §八「焦点环 2px、偏移 2px」的字面描述。
   */
  shadows: {
    outline: "none",
    /* 🔴 §二 elevation 的 sticky 档必须**登记在 shadows 刻度里**，不能在组件 sx 里写
       boxShadow —— Chakra 的 `shadow` prop 解析成 `var(--chakra-shadows-<值>)`，
       且 ModalContent 自带一层 shadow，sx.boxShadow 会被它压掉
       （2026-10-06 UI 审查渲染取证实测：量到的是 Chakra 默认
       `0 0 0 1px rgba(255,255,255,.05)` 而不是我们的 sticky）。
       与当年 `radii` 那次同一课：**Chakra 的样式属性查的是 theme 刻度，不是 CSS 变量。** */
    sticky: "var(--aegis-elevation-sticky)",
  },
  styles: {
    global: {
      /* 🔴 `:root` 变量表**不在此处**（2026-10-06 UI 审查 N1）—— 见本文件上方的 cssVarsText
         与裸 <style> 注入段。走 Chakra 的对象通道会被切碎，理由与对照实验都写在那里。 */
      html: {
        background: activePack.colors.canvas,
        // 🔴 深色下必须声明 color-scheme，否则**原生控件与滚动条**（表单控件的原生内阴影、
        // 选中文本高亮、滚动条轨道）仍按浅色渲染，在近黑底上出现刺眼白条。
        // 🔴 已改为从包派生（不再硬编码 "dark"）：切回 Light 包时原生控件自动跟随，
        //    不会再出现「换了包但原生滚动条/表单控件仍按深色渲染」的静默错位。
        colorScheme: activePack.mode,
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
      // 规范 §八：焦点环 2px，偏移 2px，禁 outline:none
      // 🔴 2026-10-04：色值改读 **focus-ring 语义键**，不再写死 accent-strong。
      // 原因：focus-ring 在浅色包 = accent-strong（白底 5.18:1）、深色包 = accent（深底 8.06:1）；
      // 写死 strong 在深底只有 3.75:1，虽越过非文本 3:1 但余量极薄，且与 03 规范 §五
      // 「Dark 下焦点环可直接用 accent」的明文不一致。
      "*:focus-visible": {
        outline: `2px solid ${activePack.colors["focus-ring"]}`,
        outlineOffset: "2px",
      },
      // 🔴 2026-10-04 复审重写：原注释「焦点环统一由组件层 _focusVisible 控制，全局规则仅作
      // 兜底」的前提是**组件层能画对颜色** —— 实际它画的是 Chakra 默认蓝色
      // （shadows.outline，见上方 shadows 段），与全局的绿色 focus-ring 叠成双环。
      // 现已把 shadows.outline 置 none ⇒ 组件层不再绘制，**焦点环唯一来源是这条全局规则**。
      // 本行只负责「非 focus-visible（例如用鼠标点过）时不画环」，与双环问题无关，保留。
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
