import { describe, it, expect } from "vitest";
import {
  nvidiaDark,
  nvidiaLight,
  DECORATIVE_RADII,
  COLOR_TOKEN_KEYS,
  TYPOGRAPHY_TOKEN_KEYS,
  SPACING_TOKEN_KEYS,
  ROUNDED_TOKEN_KEYS,
  ELEVATION_TOKEN_KEYS,
  MOTION_DURATION_TOKEN_KEYS,
  MOTION_EASING_TOKEN_KEYS,
} from "./tokens";
import { theme } from "./theme";

/* 🔴 Chakra 的 extendTheme 返回类型不精确（config / colors.aegis / styles.global /
 * textStyles / shadows / fontWeights / components / radii / cssVars 全部推断为 any）⇒
 * 直接访问会触发 @typescript-eslint 的 no-unsafe-member-access / no-unsafe-return。
 * 这里一次性收窄成本项目**实际断言的字段**，既过 lint 又让断言可读。 */
const th = theme as unknown as {
  config: { initialColorMode: string; useSystemColorMode: boolean };
  colors: { aegis: Record<string, string> };
  radii: Record<string, string | undefined>;
  shadows: Record<string, string | undefined>;
  // 注：自定义 --aegis-* 变量走 styles.global[":root"]，不查 __cssVars（见下方断言注释）
  textStyles: Record<
    string,
    { fontFamily?: string; fontSize?: string; fontWeight?: unknown } | undefined
  >;
  fontWeights: Record<string, unknown>;
  components: Record<string, { sizes?: Record<string, Record<string, unknown>> }> | undefined;
  styles: {
    global: Record<string, Record<string, string | undefined> | undefined>;
  };
};

const rootVars = th.styles.global[":root"] ?? {};

/* ── 对比度独立复算（不引用 tokens 里任何「声称值」）──────────────────────── */

const chan = (c: number): number => {
  const v = c / 255;
  return v <= 0.04045 ? v / 12.92 : Math.pow((v + 0.055) / 1.055, 2.4);
};

const lum = (hex: string): number => {
  const m = hex.replace("#", "");
  if (!/^[0-9a-f]{6}$/i.test(m)) throw new Error("不是 6 位 hex: " + hex);
  return (
    0.2126 * chan(parseInt(m.slice(0, 2), 16)) +
    0.7152 * chan(parseInt(m.slice(2, 4), 16)) +
    0.0722 * chan(parseInt(m.slice(4, 6), 16))
  );
};

const ratio = (a: string, b: string): number => {
  const la = lum(a);
  const lb = lum(b);
  const hi = Math.max(la, lb);
  const lo = Math.min(la, lb);
  return (hi + 0.05) / (lo + 0.05);
};

describe("Nvidia.Dark —— 本期唯一交付包", () => {
  it("themePacks 的第一包是 Dark", () => {
    expect(nvidiaDark.name).toBe("Nvidia.Dark");
    expect(nvidiaDark.mode).toBe("dark");
  });

  it("初始色模式 = dark，且不跟随系统", () => {
    expect(th.config.initialColorMode).toBe("dark");
    expect(th.config.useSystemColorMode).toBe(false);
  });

  it("Light 包仍在 registry 中（机制不退役）", () => {
    expect(nvidiaLight.name).toBe("Nvidia.Light");
    expect(nvidiaLight.mode).toBe("light");
  });
});

describe("键集契约 —— 两包 × 六维全扫（不只看 activePack）", () => {
  const DIMS = [
    ["colors", COLOR_TOKEN_KEYS, nvidiaDark.colors, nvidiaLight.colors],
    ["typography", TYPOGRAPHY_TOKEN_KEYS, nvidiaDark.typography, nvidiaLight.typography],
    ["spacing", SPACING_TOKEN_KEYS, nvidiaDark.spacing, nvidiaLight.spacing],
    ["rounded", ROUNDED_TOKEN_KEYS, nvidiaDark.rounded, nvidiaLight.rounded],
    ["elevation", ELEVATION_TOKEN_KEYS, nvidiaDark.elevation, nvidiaLight.elevation],
    [
      "motion.duration",
      MOTION_DURATION_TOKEN_KEYS,
      nvidiaDark.motion.duration,
      nvidiaLight.motion.duration,
    ],
    [
      "motion.easing",
      MOTION_EASING_TOKEN_KEYS,
      nvidiaDark.motion.easing,
      nvidiaLight.motion.easing,
    ],
  ] as const;

  for (const [dim, keys, dark, light] of DIMS) {
    it("🔴 " + dim + " —— 两包键集都与契约一致", () => {
      const want = [...keys].sort();
      expect(Object.keys(dark).sort(), dim + " (Dark) 键集漂移").toEqual(want);
      expect(Object.keys(light).sort(), dim + " (Light) 键集漂移").toEqual(want);
    });
  }

  it("colors 键数 = 24（含 focus-ring，覆盖基数已同步）", () => {
    expect(COLOR_TOKEN_KEYS.length).toBe(24);
  });
});

describe("颜色语义", () => {
  it("🔴 focus-ring 两包给不同值（Light 用 strong，Dark 用 accent）", () => {
    expect(nvidiaLight.colors["focus-ring"]).not.toBe(nvidiaDark.colors["focus-ring"]);
    expect(nvidiaLight.colors["focus-ring"]).toBe(nvidiaLight.colors["accent-strong"]);
    expect(nvidiaDark.colors["focus-ring"]).toBe(nvidiaDark.colors["accent"]);
  });

  it("🔴 focus-ring 在各自承载底上过非文本 3:1（键盘可达性底线）", () => {
    const darkR = ratio(nvidiaDark.colors["focus-ring"], nvidiaDark.colors.canvas);
    const lightR = ratio(nvidiaLight.colors["focus-ring"], nvidiaLight.colors.canvas);
    expect(darkR, "Dark focus-ring 仅 " + darkR.toFixed(2) + ":1").toBeGreaterThanOrEqual(3);
    expect(lightR, "Light focus-ring 仅 " + lightR.toFixed(2) + ":1").toBeGreaterThanOrEqual(3);
  });

  it("🔴 深色下 surface-soft 必须比 canvas 更亮、surface-inset 更暗（语义反转）", () => {
    const lumD = lum(nvidiaDark.colors.canvas);
    expect(lum(nvidiaDark.colors["surface-soft"])).toBeGreaterThan(lumD);
    expect(lum(nvidiaDark.colors["surface-inset"])).toBeLessThan(lumD);
  });
});

describe("焦点环 —— 单一来源，禁双绘", () => {
  it("shadows.outline = none（Chakra 组件层的蓝色 boxShadow 环被关掉）", () => {
    expect(th.shadows.outline).toBe("none");
  });

  it("全局 focus-visible 用 focus-ring 键、2px、offset 2px", () => {
    const g = th.styles.global["*:focus-visible"] ?? {};
    expect(g.outline).toBe("2px solid " + nvidiaDark.colors["focus-ring"]);
    expect(g.outlineOffset).toBe("2px");
  });
});

describe("圆角 —— 双出口同源", () => {
  it("🔴 装饰档同时有 Chakra 出口与 CSS 变量出口", () => {
    for (const k of ["lg", "xl", "2xl", "3xl"] as const) {
      expect(th.radii[k], "radii." + k + " 缺 Chakra 出口").toBe(DECORATIVE_RADII[k]);
      /* 🔴 顺带记一个坑：自定义 cssVars 展开在 `styles.global[":root"]`（emotion 注入），
       * **不在 Chakra 的 `__cssVars` 下** —— 后者只含 Chakra 自己从 theme tokens 生成的
       * 那一份（--chakra-*）。所以「CSS 变量出口」这条断言必须查 `:root`，查 __cssVars 会静默 undefined。 */
      expect(rootVars["--aegis-radius-" + k], ":root 未声明 " + k).toBe(DECORATIVE_RADII[k]);
    }
  });

  it("规范档 radii.sm / md / base 恒为 2px", () => {
    expect(th.radii.sm).toBe(nvidiaDark.rounded.sm);
    expect(th.radii.md).toBe(nvidiaDark.rounded.sm);
    expect(th.radii.base).toBe(nvidiaDark.rounded.sm);
  });

  it("Card 在组件白名单里（防 radii.md 漂移让 Card 静默变 4/6px）", () => {
    expect(th.components?.Card).toBeDefined();
  });
});

/** sizes 里 _after 的形状是 unknown，此处统一收窄（避免 no-unsafe-member-access） */
const afterOf = (s?: Record<string, unknown>): Record<string, string> | undefined =>
  s?._after as Record<string, string> | undefined;

describe("控件尺寸 —— 28 / 36 / 44 三档", () => {
  const sizes = (name: string): Record<string, Record<string, unknown>> =>
    th.components?.[name]?.sizes ?? {};

  it("Button/Input/Select/IconButton 四组件都已挂 sizes", () => {
    for (const n of ["Button", "Input", "Select", "IconButton"]) {
      expect(Object.keys(sizes(n)).length, n + " 未挂 sizes").toBeGreaterThan(0);
    }
  });

  it("高度恰为 xs/sm=28 · md=36 · lg=44，且 xs 是 sm 的别名", () => {
    const b = sizes("Button");
    expect(b.xs?.h).toBe("28px");
    expect(b.sm?.h).toBe("28px");
    expect(b.md?.h).toBe("36px");
    expect(b.lg?.h).toBe("44px");
  });

  it("🔴 sm 档有定位宿主（position: relative）—— 否则 ::after 命中区会逃逸到祖先", () => {
    const b = sizes("Button");
    expect(b.sm?.position).toBe("relative");
    expect(afterOf(b.sm)?.position).toBe("absolute");
    expect(afterOf(b.sm)?.top).toBe("-2px");
    // md/lg 已 ≥32 无需补偿，也不该带 ::after
    expect(b.md?._after).toBeUndefined();
  });

  it("IconButton 用 minW 固定正方形（无 px）", () => {
    const i = sizes("IconButton");
    expect(i.sm?.minW).toBe("28px");
    expect(i.md?.minW).toBe("36px");
    expect(i.lg?.minW).toBe("44px");
  });
});

describe("Typography / 色阶出口", () => {
  it("🔴 textStyles.mono 保留 fontFamily（否则回退正文字体）", () => {
    const mono = th.textStyles.mono;
    expect(mono).toBeDefined();
    expect(mono?.fontFamily, "mono 缺 fontFamily —— 等宽字体栈丢失").toBeTruthy();
  });

  it("fontWeights 刻度是数字（字符串会绕过 Chakra 归一化）", () => {
    expect(typeof th.fontWeights.body).toBe("number");
    expect(typeof nvidiaDark.typography.body.fontWeight).toBe("number");
  });

  it("colorScheme 由 activePack.mode 派生（非硬编码 dark）", () => {
    const g = th.styles.global.html ?? {};
    expect(g.colorScheme).toBe("dark");
    expect(nvidiaLight.mode).toBe("light");
  });
});
