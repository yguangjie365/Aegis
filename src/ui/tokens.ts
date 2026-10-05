// 设计令牌唯一事实源 —— 键集与《02-专题设计/03-UI设计规范.md》§二逐一对应
// 覆盖基数：colors 24 + typography 12 + spacing 7 + rounded 3 + elevation 3 + motion 10 = 59 key
// （2026-10-04 新增 focus-ring ⇒ colors 由 23 变 24；总数由 58 变 59）
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
  // 🆕 focus-ring（2026-10-04）：焦点环语义键。**不写死 accent-strong** ——
  // 浅底上 strong 是合规硬性要求（白底 5.18:1），深底上它只有 3.75:1 而 accent 有 8.06:1。
  // 一个键写死会在换包时静默失去对比度，故独立成键由主题包各给一值。
  "focus-ring",
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
  /** 🔴 2026-10-04 复审新增：包的明暗模态。`html { color-scheme }` 由此派生，
   * 不再硬编码 "dark" —— 否则切回 Light 包时原生控件/滚动条仍按深色渲染（静默难查）。 */
  mode: "light" | "dark";
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

/* ══ 装饰圆角档（Chakra 兼容用，非规范档）═════════════════════════════════════
 * 背景：项目圆角契约只有 **3 键**（`none` / `sm` / `full`），但 Chakra 内置组件
 * 会按 `radii.base/md/lg/xl/2xl/3xl` 取默认值（如 `Card` 取 `lg`）。
 * 若 `radii` 里没有这些档位 ⇒ 组件拿不到值 ⇒ 圆角回退/异常。
 *
 * 🔴 因此把「Chakra 需要的额外档位」作为**独立常量**导出，两个出口共用：
 *   - Chakra 出口：`theme.radii` 展开它（让组件取到值）
 *   - CSS 变量出口：`cssVars` 循环生成 `--aegis-radius-lg/xl/2xl/3xl`（让非组件层取到值）
 *
 * ⚠️ 它**不进 `ROUNDED_TOKEN_KEYS` 契约**：规范档是 3 键，这 4 档只是兼容 Chakra 内部默认值，
 * 属于"实现细节"而非"设计契约"。混淆二者会让守卫把兼容档当成契约缺失而报错。
 */
export const DECORATIVE_RADII = {
  lg: "8px",
  xl: "12px",
  "2xl": "16px",
  "3xl": "24px",
} as const;

// 首期交付包已按用户 2026-10-04 裁决改为**深色单主题**（Nvidia 品牌风格更适合深色，
// 浅色包另行设计、不在本期范围）。多主题包**机制不退役**：nvidiaLight 原样保留，
// 未来加回浅色只需把它放回 themePacks —— 键集守卫会逐维校验，两包都不得缺键。
export const nvidiaLight: ThemePack = {
  name: "Nvidia.Light",
  mode: "light",
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
    // 🆕 focus-ring：浅色包 = accent-strong（白底 5.18:1，浅底合规硬性要求）。
    // 键集守卫要求**两包都逐键覆盖**，故此处也必须显式给值，不能靠守卫放行。
    "focus-ring": "#4d7900",
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
      // 🔴 2026-10-04 复审修复：fontFamily 必须**在本键上声明**，不能只靠 `:root --font-mono`。
      // 原因：theme.ts 的 textStyles 生成器把 typography 逐键转成 Chakra textStyle，
      // 若此处无 fontFamily，`textStyles.mono` 就只剩字号/字重/行高 ⇒
      // `<Text textStyle="mono">` 静默回退正文字体（Inter / Noto Sans SC）——
      // 数值全对、tsc 全绿、**唯独字体是错的**，属于最难发现的一类回归。
      // 用 var() 而非字面量，保持「字体栈单一事实源 = MONO_FONT_STACK」不变。
      fontFamily: "var(--font-mono)",
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
  // 🔴 **消费路径只有 CSS 变量一条**（--aegis-motion-duration-* / --aegis-motion-easing-*）。
  // theme 层**刻意不造** `theme.motion` 命名空间：Chakra 无 transition scale，硬造一个的
  // 收益与直接用 CSS 变量完全等价，却多出一处可漂移的副本。
  // ⇒ 组件里写 `transition: all var(--aegis-motion-duration-fast)`；
  //   **不要**去 theme.motion 找（那里没有），更不要写 `transition="all 0.2s"` 裸值。
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
      /* ⚠️ 本值 = 导航指示器在 **BOUNCE 50**（原调校手感）下的**位移**过冲。
         🔴 但导航当前跑在 `BOUNCE = 60`，实际用的是 `cubic-bezier(0.28, 1.456, 0.36, 1)`
         —— 运行期由 `overshoot(60, 1.38)` 算出，落在 `shell/nav-rail.tsx` 的 `--ind-move`。
         它**刻意不消费本令牌**：同一处还要给尺寸用第二个过冲量（`overshoot(60, 1.5)` = 1.6），
         一个键装不下两个值，而"位移与尺寸过冲量不同"正是那个刻意的「让步感」。
         故：**不要把 `--ind-move` 改成 `var(--aegis-motion-easing-overshoot)`** ——
         那会把回弹从 60 静默降回 50（视觉上指示器"没那么 Q"，但没有任何报错）。
         本键留给"只需要一个过冲量"的通用落位场景。 */
      overshoot: "cubic-bezier(0.28, 1.38, 0.36, 1)",
    },
  },
};

/* ══ Nvidia.Dark（本期唯一交付包，2026-10-04 用户裁决）═════════════════════
 * 逐键实测记录见 Docs/开发文档/02-专题设计/03-UI设计规范.md §五，
 * 复算脚本 .worknotes/dark-pack-proposal.txt（WCAG 2.1 独立复算，24/24 达标）。
 *
 * 🔴 **三处与浅色语义相反，改值前必读**：
 *
 * 1. canvas = #131313（2026-10-05 抬底，原 #0d0d0d，10-04 裁决值）。
 *    抬底理由：实测 NVIDIA App 底色 #191919（L 0.0097）**比我们原来的 #0d0d0d（L 0.0040）更亮**，
 *    且它的导航与内容底**同色**、靠 1px 描边分界 ⇒ 「深黑」不是主因，**灰阶断层**才是。
 *    抬底后全表 24 键重算（`.worknotes/verify-batch2.txt`）：文字类与非文本门禁**零失败**。
 * 2. **surface-soft（抬升）= 更亮 #1f1f1f，surface-inset（凹）= 更暗 #0e0e0e** ——
 *    与浅色（soft 更深 / inset 更浅）**方向相反**。沿用浅色值会把卡片变成亮块。
 * 3. error 从 #e52020 提亮为 #ff5c5c：原值在 canvas 上仅 4.16:1，做错误提示文字不可读。
 *
 * 🔴 **chrome-dark 的关键不是"够不够黑"，而是必须比 canvas 亮**（2026-10-05 用户裁决）
 * 原值 #000000 比 canvas 更暗 6 级 ⇒ 导航胶囊在视觉上是个「洞」，L 0.386 的流光环浮在
 * L 0.0000 的虚空里没有依附面 ⇒ 用户判为「突兀、缺设计感」。
 * 两步演进：#000000 →（一批）#1c1c1c →（二批，随 canvas 抬底）#212121。
 * 每次抬 canvas 时**必须同抬 chrome**，否则胶囊边界被削弱（实测：canvas #131313 + chrome #1c1c1c
 * 只有 1.090，弱于一批的 1.140）。
 *
 * ⚠️ 换 canvas 必须整表重算：本表所有对比度均以 canvas = #131313 为基准。
 */
export const nvidiaDark: ThemePack = {
  name: "Nvidia.Dark",
  mode: "dark",
  colors: {
    accent: "#76b900", // 7.71 (canvas) — 填充/图形；**禁作文字色**（见 03 规范 §二）
    "accent-pressed": "#5f9a00", // 5.40 — 按下态，比 accent 略深
    // 🔴 Dark 下与 accent 同值（03 规范 §五明文）。**不得合并成一个键**：Light 下 strong
    // 是浅底合规的焦点环/文字色，角色不同。
    "accent-strong": "#76b900",
    "accent-on": "#000000", // 8.71（绿底黑字）
    "focus-ring": "#76b900", // 7.71 — 浅底包用 accent-strong（5.18 白底），深底包用 accent
    ink: "#e8e8e8", // 15.16 (canvas) / 13.45 (surface-soft)
    canvas: "#131313", // 基准（页面底）— 2026-10-05 抬底，原 #0d0d0d
    "surface-soft": "#1f1f1f", // 1.13 — 深色「抬升」= 更亮
    "surface-inset": "#0e0e0e", // 1.04 — 深色「凹」= 更暗；靠 border-interactive 保证可见
    hairline: "#3f3f3f", // 1.76 豁免（装饰线）
    "border-interactive": "#6e6e6e", // 3.64 (canvas) ✅ 非文本 3:1
    mute: "#9a9a9a", // 6.60 (canvas) ✅
    "mute-on-soft": "#a8a8a8", // 6.93 (surface-soft) ✅
    stone: "#6e6e6e", // 3.64 — **门禁 3:1，仅图形/占位，禁文字**（与 border 同值但语义不同，刻意不合并）
    disabled: "#555555", // 2.49 — 1.4.3 豁免；须比 mute 暗才不会成为视觉噪点
    // 🔴 chrome-dark 两步演进：#000000 →(2026-10-05 一批) #1c1c1c →(二批) #212121。
    // 关键不是"够不够黑"，而是**必须比 canvas 亮** —— 纯黑时它比底暗 6 级、胶囊成了"洞"，
    // 流光环浮在虚空里无依附面（用户判为"突兀"）。二批 canvas 抬到 #131313 后同步提亮，
    // 保住 1.15:1 的层差（NVIDIA App 实测导航与内容底**同色**，1.000，靠 1px 描边分界）。
    "chrome-dark": "#212121", // 1.15 (canvas) — 比底亮一级（浮起）
    "on-dark": "#ffffff", // 15.98 (chrome-dark)
    "on-dark-mute": "#a8a8a8", // 6.77 (chrome-dark)
    "chrome-elevated": "#2e2e2e", // 1.21 — 胶囊 hover 底
    success: "#4fb500", // 7.35 (canvas) / 6.45 (surface-soft)
    warning: "#ef9100", // 8.07
    "warning-bright": "#ef9100", // 8.07 — 与 warning 同值（单主题下不再有「浅色面」场景）
    error: "#ff5c5c", // 6.42 (canvas) / 5.63 (surface-soft) — 🔴 由 #e52020 提亮
    link: "#6cb0ff", // 8.22 (canvas) / 7.29 (surface-soft)
  },
  // typography / spacing / rounded / motion 是**结构维度**，与主题模态无关（字号、间距、
  // 圆角、动效时长不因深浅而改）⇒ 共用同一份对象。键集守卫仍会逐维校验，不因此放宽。
  typography: nvidiaLight.typography,
  spacing: nvidiaLight.spacing,
  rounded: nvidiaLight.rounded,
  elevation: {
    flat: "none",
    // 描边语义不变，只是描边色跟随 hairline 键（深色下是 #3f3f3f 而非浅色 #cccccc）
    hairline: "0 0 0 1px var(--aegis-hairline)",
    // 🔴 深色下纯黑阴影落在近黑底上 = 不可见（这是 elevation 维度**唯一**需要按模态重定的地方）。
    // 处置：阴影加深 + 叠一道极弱亮边做边界，双保险。
    sticky: "0 2px 8px rgba(0,0,0,0.55), 0 0 0 1px rgba(255,255,255,0.05)",
  },
  motion: nvidiaLight.motion,
};

// Dark 在前：theme.ts 取 themePacks[0] 为 activePack。Light 保留在列表中，机制不退役。
export const themePacks: readonly ThemePack[] = [nvidiaDark, nvidiaLight];
