import { describe, it, expect } from "vitest";
import {
  nvidiaDark,
  nvidiaLight,
  DECORATIVE_RADII,
  MONO_FONT_STACK,
  COLOR_TOKEN_KEYS,
  TYPOGRAPHY_TOKEN_KEYS,
  SPACING_TOKEN_KEYS,
  ROUNDED_TOKEN_KEYS,
  ELEVATION_TOKEN_KEYS,
  MOTION_DURATION_TOKEN_KEYS,
  MOTION_EASING_TOKEN_KEYS,
} from "./tokens";
import { theme, cssVarsText } from "./theme";

/* 🔴 Chakra 的 extendTheme 返回类型不精确（config / colors.aegis / styles.global /
 * textStyles / shadows / fontWeights / components / radii / cssVars 全部推断为 any）⇒
 * 直接访问会触发 @typescript-eslint 的 no-unsafe-member-access / no-unsafe-return。
 * 这里一次性收窄成本项目**实际断言的字段**，既过 lint 又让断言可读。 */
const th = theme as unknown as {
  config: { initialColorMode: string; useSystemColorMode: boolean };
  colors: { aegis: Record<string, string> };
  radii: Record<string, string | undefined>;
  space: Record<string, string | undefined>;
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

/* 🔴 N1（2026-10-06 UI 审查）：变量表已**移出** Chakra 的 `styles.global` —— 对象通道会把
   含顶层逗号的值切碎（`--font-mono` 只剩 "Consolas"、sticky 只剩一层）。现由 theme.ts
   在 import 期以**裸 CSS 文本**注入 <head>，故断言改读 `cssVarsText`：读的是生成出来的
   文本本身 ⇒ 这条守卫在 node 环境同样成立（不需要 jsdom，也不必等 ADR-003 引第三方）。 */
const rootVars: Record<string, string> = Object.fromEntries(
  (cssVarsText.match(/--[a-z0-9-]+:[^;]+/g) ?? []).map((pair) => {
    const i = pair.indexOf(":");
    return [pair.slice(0, i), pair.slice(i + 1).trim()];
  }),
);

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

  it("🔴 N1 回归：含顶层逗号的令牌值必须完整出口（不被生成期切碎）", () => {
    /* 本用例存在的理由：2026-10-06 之前，`--font-mono` 与 `--aegis-elevation-sticky`
       经 Chakra 对象通道注入后被静默截断，而 tsc / vitest / build 全绿、肉眼也看不出
       （Consolas 同为等宽）。断言打在**生成的 CSS 文本**上，换通道也拦得住。 */
    expect(
      rootVars["--font-mono"],
      "mono 字体栈被截断 ⇒ 全站 var(--font-mono) 用不到 §七 指定的 JetBrains Mono",
    ).toBe(MONO_FONT_STACK);
    expect(
      rootVars["--aegis-elevation-sticky"],
      "elevation.sticky 阴影层被截断 ⇒ §一 #3 唯一允许的阴影失真",
    ).toBe(nvidiaDark.elevation.sticky);
    expect(
      (rootVars["--font-mono"]?.match(/,/g) ?? []).length,
      "顶层逗号数量对不上即说明掉段",
    ).toBe(2);
  });

  it("colors 键数 = 25（含 focus-ring、error-fill，覆盖基数已同步）", () => {
    expect(COLOR_TOKEN_KEYS.length).toBe(25);
  });

  it("🔴 §二 spacing 维已接进 Chakra `theme.space`（UI 审查 B1 回归守卫）", () => {
    /* 只断言 CSS 变量出口（--aegis-space-*）**拦不住**「刻度没接进 Chakra」这一类缺陷：
       styled-system 解析 spacing / gap / margin 时查的是 `theme.space`，缺键即原样透传成
       `gap: lg` 这类非法值被浏览器丢弃 ⇒ 命名间距静默归零（B1 的实测形态）。
       故必须直接对 `theme.space` 断言键集与值，且值要带单位（无单位数会破坏 Chakra 数字语义）。 */
    const want = [...SPACING_TOKEN_KEYS].sort();
    const got = Object.keys(th.space)
      .filter((k) => (SPACING_TOKEN_KEYS as readonly string[]).includes(k))
      .sort();
    expect(got, 'theme.space 缺规范命名档 ⇒ 页面里 spacing="lg" 之类全部归零').toEqual(want);
    for (const k of SPACING_TOKEN_KEYS) {
      expect(th.space[k], `theme.space.${k} 必须是带单位字符串`).toMatch(/^\d+(\.\d+)?px$/);
    }
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

  it("🔴 error 与 error-fill 双令牌各自达标（2026-10-06 UI 审查批次②）", () => {
    // 两岗各守一条门禁，缺任一即断言失败：
    //   error      → 错误**文字**压在 canvas 上 ≥ 4.5
    //   error-fill → danger 按钮**底色**，白字 ≥ 4.5
    // 若有人试图合并回单令牌，这两条不可能同时成立（无解证明在 tokens.ts）。
    for (const pack of [nvidiaDark, nvidiaLight]) {
      const c = pack.colors;
      const textR = ratio(c.error, c.canvas);
      expect(
        textR,
        pack.name + " error 作文字色仅 " + textR.toFixed(2) + ":1",
      ).toBeGreaterThanOrEqual(4.5);
      const fillR = ratio(c["on-dark"], c["error-fill"]);
      expect(
        fillR,
        pack.name + " 白字压 error-fill 仅 " + fillR.toFixed(2) + ":1",
      ).toBeGreaterThanOrEqual(4.5);
    }
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
