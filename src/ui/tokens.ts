// 五维设计令牌唯一事实源 —— 键集与《02-专题设计/03-UI设计规范.md》§二逐一对应
// 覆盖基数：colors 23 + typography 12 + spacing 7 + rounded 3 + elevation 3 = 48 key
// 增删任何 key：先改规范 §二 与计数，再改本文件（规范 §五 红线）。

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
export type EationToken = (typeof ELEVATION_TOKEN_KEYS)[number];

export const TOKEN_KEY_COUNT =
  COLOR_TOKEN_KEYS.length +
  TYPOGRAPHY_TOKEN_KEYS.length +
  SPACING_TOKEN_KEYS.length +
  ROUNDED_TOKEN_KEYS.length +
  ELEVATION_TOKEN_KEYS.length;

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
  elevation: Record<EationToken, string>;
}

// 首期仅交付浅色一包（TASKS Q-06 已拍板 2026-10-02）；Dark 包后续里程碑加入时，
// 必须逐键覆盖全部 key 并过 §五/§六 对比度门禁——不得只反相。
export const nvidiaLight: ThemePack = {
  name: "Nvidia.Light",
  colors: {
    accent: "#76b900",
    "accent-pressed": "#5a8d00",
    "accent-strong": "#5a8d00",
    "accent-on": "#000000",
    ink: "#1a1a1a",
    canvas: "#ffffff",
    "surface-soft": "#f7f7f7",
    "surface-inset": "#efefef",
    hairline: "#cccccc",
    "border-interactive": "#8f8f8f",
    mute: "#757575",
    "mute-on-soft": "#686868",
    stone: "#8f8f8f",
    disabled: "#a7a7a7",
    "chrome-dark": "#000000",
    "on-dark": "#ffffff",
    "on-dark-mute": "#bfbfbf",
    "chrome-elevated": "#1a1a1a",
    success: "#3f8500",
    warning: "#9a4d00",
    "warning-bright": "#ef9100",
    error: "#e52020",
    link: "#0046a4",
  },
  typography: {
    display: { fontSize: "32px", fontWeight: 700, lineHeight: 1.25 },
    h1: { fontSize: "24px", fontWeight: 700, lineHeight: 1.25 },
    h2: { fontSize: "20px", fontWeight: 700, lineHeight: 1.25 },
    h3: { fontSize: "17px", fontWeight: 700, lineHeight: 1.4 },
    "body-lg": { fontSize: "16px", fontWeight: 400, lineHeight: 1.5 },
    "body-lg-strong": { fontSize: "16px", fontWeight: 700, lineHeight: 1.5 },
    body: { fontSize: "14px", fontWeight: 400, lineHeight: 1.5 },
    "body-strong": { fontSize: "14px", fontWeight: 700, lineHeight: 1.5 },
    caption: { fontSize: "12px", fontWeight: 400, lineHeight: 1.4 },
    "caption-uppercase": {
      fontSize: "12px",
      fontWeight: 700,
      lineHeight: 1.4,
      letterSpacing: "0.04em",
      textTransform: "uppercase",
    },
    micro: { fontSize: "11px", fontWeight: 400, lineHeight: 1.4 },
    mono: {
      fontSize: "13px",
      fontWeight: 400,
      lineHeight: 1.6,
      fontFamily: "var(--font-mono)",
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
    hairline: "none", // 1px solid var(--aegis-hairline) —— 由 border 令牌消费，不入 box-shadow
    sticky: "0 2px 5px rgba(0,0,0,0.30)",
  },
};

export const themePacks: readonly ThemePack[] = [nvidiaLight];
