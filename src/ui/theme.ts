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
  fonts: {
    // 中文主面 = 思源黑体（Noto Sans SC，已定档 2026-10-02，规范 §二/§七）；雅黑仅兜底
    heading: `"Inter","Noto Sans SC","PingFang SC","Microsoft YaHei",sans-serif`,
    body: `"Inter","Noto Sans SC","PingFang SC","Microsoft YaHei",sans-serif`,
    mono: `"JetBrains Mono","Consolas",monospace`,
  },
  styles: {
    global: {
      ":root": {
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
      "#root": { minHeight: "100vh" },
      "@media (prefers-reduced-motion: reduce)": {
        "*, *::before, *::after": { animationDuration: "0.01ms !important", transitionDuration: "0.01ms !important" },
      },
    },
  },
});
