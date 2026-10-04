// 设计令牌唯一事实源 —— 键集与《02-专题设计/03-UI设计规范.md》§二逐一对应
// 覆盖基数：colors 23 + typography 12 + spacing 7 + rounded 3 + elevation 3 + motion 10 = 58 key
// 增删任何 key：先改规范 §二 与计数，再改本文件（规范 §五 红线）。
//
// 🔴 **motion 为第六维**（用户裁决 2026-10-04）：此前动效参数（时长/缓动）是裸值，
// 既不在令牌契约内、也不在任何守卫扫描面内 → 同一产品的交互快慢由 7 处分散常量各自决定，
// 改动无法被审计。现已入契约，W1-04 裸值棘轮扫描面同步加入时长与缓动两项。
//
// 对比度基线（白底 #ffffff / surface-soft #f7f7f7，WCAG 2.1 AA：正文 4.5:1、大字 3:1、
// 非文本 1.4.11 3:1）。带 ⚠️ 者为压线或不过，注释里的「用途/禁用」即据此定档。

export const COLOR_TOKEN_KEYS = [
  "accent",
  "accent-pressed",
  "accent-strong",
  "accent-on",
  "ink",
  "canvas",
  "surface-soft",
  "surface-inset",
  "hairline",
  "border-interactive",
  "mute",
  "mute-on-soft",
  "stone",
  "disabled",
  "chrome-dark",
  "on-dark",
  "on-dark-mute",
  "chrome-elevated",
  "success",
  "warning",
  "warning-bright",
  "error",
  "link",
] as const;
export type ColorToken = (typeof COLOR_TOKEN_KEYS)[number];

export const TYPOGRAPHY_TOKEN_KEYS = [
  "display",
  "h1",
  "h2",
  "h3",
  "body-lg",
  "body-lg-strong",
  "body",
  "body-strong",
  "caption",
  "caption-uppercase",
  "micro",
  "mono",
] as const;
export type TypographyToken = (typeof TYPOGRAPHY_TOKEN_KEYS)[number];

export const SPACING_TOKEN_KEYS = ["xs", "sm", "md", "lg", "xl", "xxl", "section"] as const;
export type SpacingToken = (typeof SPACING_TOKEN_KEYS)[number];

export const ROUNDED_TOKEN_KEYS = ["none", "sm", "full"] as const;
export type RoundedToken = (typeof ROUNDED_TOKEN_KEYS)[number];

export const ELEVATION_TOKEN_KEYS = ["flat", "hairline", "sticky"] as const;
export type ElevationToken = (typeof ELEVATION_TOKEN_KEYS)[number];

/* ══ 第六维：motion ══════════════════════════════════════════════════════════
   分两组：duration（时长）与 easing（缓动）。

   🔴 **键名按「角色」而非按数值命名**（如 `fast` 而不是 `120`）—— 换主题包调快整体节奏时
   改的是值而不是键，组件代码不动。数值相同的档不重复设键（避免「两套名字同一个值」漂移）。

   ⚠️ 收录原则：**只收跨组件复用的档**。一次性的、组件私有的数值（如导航指示器由
   `HANDOFF_MS × rate` 运行期算出的拉伸/落位时长）**不入契约** —— 它是组件的手感旋钮，
   不是一个设计系统级的时间刻度。 */
export const MOTION_DURATION_TOKEN_KEYS = [
  "instant", // 0ms—— 状态立即切换（激活态前景：底色已就位，再渐变就是自造不可读窗口）
  "fast", // 120ms —— 微交互（hover / 焦点环 / 前景色）
  "normal", // 200ms —— 常规过渡（面板展开、图标切换）
  "slow", // 400ms —— 进度类（分段条推进，须与 2s 数据节拍区分：能看出在动又不永远追不上）
  "deliberate", // 320ms —— 有回弹的落位（overshoot 曲线的配套时长）
] as const;
export type MotionDurationToken = (typeof MOTION_DURATION_TOKEN_KEYS)[number];

export const MOTION_EASING_TOKEN_KEYS = [
  "linear", // 匀速 —— 循环类装饰（流光环、骨架扫光）
  "standard", // 常规 ease —— 微交互默认
  "decelerate", // ease-out —— 进入/落位，「先快后慢」
  "draw", // 无过冲 ease-out —— 🔴 被「拉开/拉伸」的阶段专用：冲过终点再回来会读成抖动
  "overshoot", // 带过冲 ease —— 落位阶段专用（冲过目标再回弹）
] as const;
export type MotionEasingToken = (typeof MOTION_EASING_TOKEN_KEYS)[number];

export interface MotionValue {
  duration: Record<MotionDurationToken, string>;
  easing: Record<MotionEasingToken, string>;
}

export const TOKEN_KEY_COUNT =
  COLOR_TOKEN_KEYS.length +
  TYPOGRAPHY_TOKEN_KEYS.length +
  SPACING_TOKEN_KEYS.length +
  ROUNDED_TOKEN_KEYS.length +
  ELEVATION_TOKEN_KEYS.length +
  MOTION_DURATION_TOKEN_KEYS.length +
  MOTION_EASING_TOKEN_KEYS.length;

export interface TypographyValue {
  fontSize: string;
  fontWeight: 400 | 700;
  lineHeight: number;
  letterSpacing?: string;
  textTransform?: "uppercase";
  fontFamily?: string;
}

export interface ThemePack {
  name: "Nvidia.Light" | "Nvidia.Dark";
  colors: Record<ColorToken, string>;
  typography: Record<TypographyToken, TypographyValue>;
  spacing: Record<SpacingToken, string>;
  rounded: Record<RoundedToken, string>;
  elevation: Record<ElevationToken, string>;
  /** 第六维（用户裁决 2026-10-04）：动效时长与缓动 */
  motion: MotionValue;
}

// 等宽字体栈单一事实源：theme.fonts.mono（Chakra 组件）+ :root --font-mono（纯 CSS 层）
// + typography.mono 共三处消费，历史上各写一遍，故在此收口（P2-10）。
export const MONO_FONT_STACK = '"JetBrains Mono","Consolas",monospace';

// 首期仅交付浅色一包（TASKS Q-06 已拍板 2026-10-02）；Dark 包后续里程碑加入时，
// 必须逐键覆盖全部 key 并过 §五/§六 对比度门禁——不得只反相。
export const nvidiaLight: ThemePack = {
  name: "Nvidia.Light",
  colors: {
    accent: "#76b900", // 2.41:1 ⚠️ 仅作填充/描边/图形；**禁用**作文字色（须用 accent-on）
    "accent-pressed": "#5a8d00", // accent 按下态；与原 strong 同值，允许独立演化
    // 🔴 2026-10-04 压暗为 #4d7900（原 #5a8d00）。**原注释写「文字用它的 5.2:1 满足 AA」是错的**
    // ——独立复算（WCAG 2.1 相对亮度）#5a8d00 在白底仅 **4.00:1**、surface-soft 3.74、
    // surface-inset 3.48，三档**全部不达 4.5:1 正文门槛**。而 `app-button.tsx` 的 secondary
    // 按钮字与 `overview-page.tsx` 的分区重试按钮字都真的拿它当**文字色**用（必现）。
    // 现值实测：白底 5.18 / soft 4.84 / inset 4.51，三档均过 AA；焦点环非文本 3:1 场景
    // （原按 4.00 判定）自然更稳。色相不变（仍为 NVIDIA Green 的深档）。
    // 连带同步：UI 规范 §二 令牌表 + §六 #8 数值、§三 button-secondary 的偏离依据。
    "accent-strong": "#4d7900",
    "accent-on": "#000000", // accent 底上的前景（黑字压绿）
    ink: "#1a1a1a", // 17.4:1 正文主色
    canvas: "#ffffff", // 页面底
    "surface-soft": "#f7f7f7", // 卡片/抬升面；对比度一律按下表的 surface-soft 列判
    "surface-inset": "#efefef", // 内嵌槽（输入框底、代码块）
    hairline: "#cccccc", // 1px 分隔线（非文本，无对比度门槛）
    "border-interactive": "#8f8f8f", // 3.23:1 恰好过 1.4.11 非文本 3:1；与 stone 同值，
    //   但语义不同（边框 vs 文本），**刻意未合并**，允许未来分叉
    // 4.61:1 ⚠️ 原值压线、surface-soft 上 4.30 跌破 AA → 2026-10-04 压暗为 #6e6e6e
    // → 5.10:1 / surface-soft 4.76:1，两处过 AA
    mute: "#6e6e6e",
    // 5.20:1（独立复算；🔴 原注释写 5.57 是**错的**，与 UI 规范 §六 #2 的 5.20 也不一致 —— 以复算值为准）
    "mute-on-soft": "#686868", // mute 的 surface-soft 安全替代
    stone: "#8f8f8f", // 3.23:1 ❌ **禁用于任何文字**（正文/小字均不过 4.5:1）；仅图形/占位
    disabled: "#a7a7a7", // 禁用态（1.4.3 豁免）
    "chrome-dark": "#000000", // 深色 chrome 底
    "on-dark": "#ffffff", // 深底前景
    "on-dark-mute": "#bfbfbf", // 深底次级前景
    "chrome-elevated": "#1a1a1a", // 深色 chrome 抬升层
    success: "#3f8500", // 4.60:1 ⚠️ 与 error/mute 同压线，surface-soft 上 4.29 亦跌破
    warning: "#9a4d00", // 6.11:1 深橙（文字安全档）
    "warning-bright": "#ef9100", // 2.41:1 ❌ **禁用于浅底文字**（仅限图形/深底上）
    error: "#d41b1b", // 5.29:1 / surface-soft 4.94:1（原 #e52020 为 4.61 / 4.30，压线）
    link: "#0046a4", // 8.71:1 链接
  },
  typography: {
    display: { fontSize: "32px", fontWeight: 700, lineHeight: 1.25 }, // 页面大标题
    h1: { fontSize: "24px", fontWeight: 700, lineHeight: 1.25 }, // 区块标题
    h2: { fontSize: "20px", fontWeight: 700, lineHeight: 1.25 }, // 小节标题
    h3: { fontSize: "17px", fontWeight: 700, lineHeight: 1.4 },
    "body-lg": { fontSize: "16px", fontWeight: 400, lineHeight: 1.5 },
    "body-lg-strong": { fontSize: "16px", fontWeight: 700, lineHeight: 1.5 },
    body: { fontSize: "14px", fontWeight: 400, lineHeight: 1.5 }, // 全局基准（html 默认字号）
    "body-strong": { fontSize: "14px", fontWeight: 700, lineHeight: 1.5 },
    caption: { fontSize: "12px", fontWeight: 400, lineHeight: 1.4 }, // 辅助说明
    "caption-uppercase": {
      // 标签/表头
      fontSize: "12px",
      fontWeight: 700,
      lineHeight: 1.4,
      letterSpacing: "0.04em",
      textTransform: "uppercase",
    },
    micro: { fontSize: "11px", fontWeight: 400, lineHeight: 1.4 }, // 元信息（禁用正文）
    mono: {
      // fontFamily 由 :root --font-mono 统一提供（见 MONO_FONT_STACK）
      fontSize: "13px",
      fontWeight: 400,
      lineHeight: 1.6,
    },
  },
  spacing: {
    xs: "4px",
    sm: "8px",
    md: "12px",
    lg: "16px",
    xl: "24px",
    xxl: "32px",
    section: "48px",
  },
  rounded: { none: "0", sm: "2px", full: "9999px" },
  elevation: {
    flat: "none",
    // hairline 语义 = 1px 描边（非阴影）：引用 color 令牌的 CSS 变量，换 Dark 包自动跟随
    hairline: "0 0 0 1px var(--aegis-hairline)",
    sticky: "0 2px 5px rgba(0,0,0,0.30)",
  },
  // 第六维 motion（2026-10-04）。数值来自本轮动效审查对全仓裸值的清点：
  // 120ms 微交互 ×2 处、400ms 进度 ×2、200ms 面板、320ms 回弹落位。
  // ⚠️ 循环类装饰的**周期**（1.4s 骨架 / 2s 直播点 / 3.2s 流光环）**刻意不入契约** ——
  // 它们是「循环速率」而非「过渡时长」，语义不同，混进 duration 刻度会让人误以为
  // 改 slow 档会连带改流光环速度。循环周期仍留在组件内就近声明。
  motion: {
    duration: {
      instant: "0ms",
      fast: "120ms",
      normal: "200ms",
      slow: "400ms",
      deliberate: "320ms",
    },
    easing: {
      linear: "linear",
      standard: "ease",
      // 🔴 参照工程给出的关键细节：**两个阶段必须用不同曲线**。被「拉开」的阶段用无过冲
      // ease-out（冲过终点再回来会读成抖动），只有落位阶段才用带过冲的曲线。
      // 同用 overshoot 实测多出 ~11px 抖动（h 384→282）。口诀：被拉开的不要弹，落到位的才弹。
      decelerate: "cubic-bezier(0, 0, 0.2, 1)",
      draw: "cubic-bezier(0.32, 0.72, 0.24, 1)",
      overshoot: "cubic-bezier(0.28, 1.38, 0.36, 1)",
    },
  },
};

export const themePacks: readonly ThemePack[] = [nvidiaLight];
