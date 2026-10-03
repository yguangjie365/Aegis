import { extendTheme } from "@chakra-ui/react";
import { nvidiaLight, TOKEN_KEY_COUNT } from "./tokens";

// 令牌 → Chakra theme + CSS 变量双出口：
// - Chakra 组件经 theme.aegis.* 取键（W1-04 裸值棘轮的对照面）
// - 非组件层（全局 css、伪元素）经 --aegis-<key> CSS 变量，两处同源

// 覆盖基数守卫：48 key 缺任一键，构建期即失败（规范 §二 覆盖基数句）
const declared =
  Object.keys(nvidiaLight.colors).length +
  Object.keys(nvidiaLight.typography).length +
  Object.keys(nvidiaLight.spacing).length +
  Object.keys(nvidiaLight.rounded).length +
  Object.keys(nvidiaLight.elevation).length;
if (declared !== TOKEN_KEY_COUNT) {
  throw new Error(
    `[theme] 主题包 ${nvidiaLight.name} 令牌覆盖 ${declared} ≠ 契约 ${TOKEN_KEY_COUNT}，禁止构建`,
  );
}

export const theme = extendTheme({
  config: { initialColorMode: "light", useSystemColorMode: false },
  colors: { aegis: nvidiaLight.colors },
  // 🔴 组件级圆角根配置（规范 §二 rounded + §三「交互元素恒 2px」）
  //
  // 阶段二换皮 D-04 的根因修复：Chakra 各组件默认带 `borderRadius: md`(6px) / `lg`(8px)，
  // 此前只逐个 props 改 `borderRadius` 属性，**未显式写圆角的组件仍落回默认值** ——
  // 实测（CDP 量 computed style）音乐页残留 6px×3 / 12px×6。
  //
  // 处置：覆盖 Chakra 的 `borderRadius` 刻度为规范刻度（none/sm/full 三档），
  // 使「不写 borderRadius」默认即合规。⚠️ 刻度被压缩后，原来写 `lg`/`md`/`xl` 的地方
  // 会全部落到 sm(2px) —— 与 D-04 的 props 替换结果一致，故两处口径自洽。
  // `full` 保留 9999px：规范 §二「full 仅头像/状态圆点」，头像组件依赖它。
  borderRadius: {
    none: nvidiaLight.rounded["none"],
    sm: nvidiaLight.rounded["sm"],
    base: nvidiaLight.rounded["sm"],
    md: nvidiaLight.rounded["sm"],
    lg: nvidiaLight.rounded["sm"],
    xl: nvidiaLight.rounded["sm"],
    "2xl": nvidiaLight.rounded["sm"],
    "3xl": nvidiaLight.rounded["sm"],
    full: nvidiaLight.rounded["full"],
  },
  fonts: {
    // 中文主面 = 思源黑体（Noto Sans SC，已定档 2026-10-02，规范 §二/§七）；雅黑仅兜底
    heading: `"Inter","Noto Sans SC","PingFang SC","Microsoft YaHei",sans-serif`,
    body: `"Inter","Noto Sans SC","PingFang SC","Microsoft YaHei",sans-serif`,
    mono: `"JetBrains Mono","Consolas",monospace`,
  },
  // 🔴 组件级圆角基线（规范 §三「button/input/card/badge 恒 radius sm」）
  //
  // 仅覆写 `borderRadius` 刻度**不足以**覆盖 Chakra 部分组件 —— Button / IconButton /
  // Input / Badge 等在自身 styleConfig 里按 size 档写死了 `borderRadius: md/lg`
  // （实测 Button 仍为 6px）。故补一层 base 样式作为兜底：
  // 组件自身未显式指定圆角时，一律落 2px。
  //
  // ⚠️ 顺序说明：base 在组件默认样式**之前**生效，故显式写了 `borderRadius` 的地方
  //（如 `borderRadius="full"` 的头像圆点）仍按自身声明，不受影响。
  components: {
    Button: {
      baseStyle: { borderRadius: nvidiaLight.rounded["sm"] },
    },
    IconButton: {
      baseStyle: { borderRadius: nvidiaLight.rounded["sm"] },
    },
    Input: {
      baseStyle: { borderRadius: nvidiaLight.rounded["sm"] },
    },
    Textarea: {
      baseStyle: { borderRadius: nvidiaLight.rounded["sm"] },
    },
    Select: {
      baseStyle: { borderRadius: nvidiaLight.rounded["sm"] },
    },
    Badge: {
      baseStyle: { borderRadius: nvidiaLight.rounded["sm"] },
    },
  },
  styles: {
    global: {      ":root": {
        ...Object.fromEntries(
          Object.entries(nvidiaLight.colors).map(([k, v]) => [`--aegis-${k}`, v]),
        ),
        ...Object.fromEntries(
          Object.entries(nvidiaLight.spacing).map(([k, v]) => [`--aegis-space-${k}`, v]),
        ),
        "--aegis-rounded-sm": nvidiaLight.rounded["sm"],
        "--aegis-rounded-full": nvidiaLight.rounded["full"],
        "--aegis-elevation-sticky": nvidiaLight.elevation["sticky"],
        "--font-mono": '"JetBrains Mono","Consolas",monospace',
      },
      html: { background: nvidiaLight.colors.canvas },
      body: {
        color: nvidiaLight.colors.ink,
        fontSize: "14px",
        lineHeight: 1.5,
        // 规范 §二：禁系统合成伪粗（回退雅黑时尤须）
        fontSynthesis: "none",
        WebkitFontSmoothing: "antialiased",
      },
      // 规范 §八：焦点环 2px accent-strong（浅底）偏移 2px，禁 outline:none
      "*:focus-visible": {
        outline: `2px solid ${nvidiaLight.colors["accent-strong"]}`,
        outlineOffset: "2px",
      },
      // 🔴 阶段一 100% 复刻补入（2026-10-03）：以下 4 条全局规则来自参照工程
      // NexBox `src/index.css`（64-92 / 255-292 行）。Aegis 无 index.css，
      // 搬 MusicPage 原件时这些规则一并缺失，导致两类可见缺陷：
      //   ① `.real-liquid-glass-svg-defs`（液态玻璃 SVG 滤镜容器）无 `width/height:0`
      //      → 该 <svg> 只有 viewBox 无显式尺寸，浏览器按默认 **150px** 渲染，
      //        把整个页面撑高 150px —— 即用户报告的「页面被拉长」；
      //   ② 滚动条未被隐藏，LiquidGlassCard 内列表与参照工程观感不一致。
      // 均按参照工程原值照搬，未改数值。
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
      // 隐藏所有滚动条（NexBox index.css:66-72：滚动照常，仅不渲染条）
      "*": { scrollbarWidth: "none", msOverflowStyle: "none" },
      "*::-webkit-scrollbar": { display: "none" },
      // 视口级禁横向滚动（NexBox index.css:80-83，果冻动画的 transform 溢出兜底）
      "html, body": { overflowX: "hidden" },
      "#root": { minHeight: "100vh" },
      // 🔴 原先此处有一段全局 `@media (prefers-reduced-motion: reduce)` 规则，把
      // animation/transition 时长压到 0.01ms（未限 iteration-count），对 `infinite` 动画
      // （彩胶唱片 24s linear infinite、加载骨架 1.4s infinite、直播点 2s infinite）会造成
      // 「0.01ms 跑完一圈且无限循环」→ 60fps 下每帧重复约 1670 次 → 疯狂旋转 + 剧烈闪烁。
      // 现按**用户裁决（2026-10-03）整段移除**：
      //   ① 回归参照工程 NexBox 的行为（其无此全局规则，盘面在任何系统上均照常旋转），
      //      符合「音乐模块完全复用 NexBox、仅适配视觉层」铁律；
      //   ② 规范 §8.3 所要求的 reduced-motion 支持暂由各组件**局部**按需实现
      //      （参照工程 MRBackground / use-splash-webgl 即为局部处理），不在全局层面强制。
      // 如日后要恢复全局支持，必须同时限制 iteration-count（Bootstrap/GOV.UK 标准写法）：
      //   animationDuration: "0.01ms !important", animationIterationCount: "1 !important"
      // —— 切勿只压时长，否则 infinite 动画会退化为上述反模式。
    },
  },
});
