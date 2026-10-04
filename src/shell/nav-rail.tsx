import { NavLink, useLocation } from "react-router-dom";
import { useCallback, useEffect, useLayoutEffect, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import styled from "@emotion/styled";
import { SHELL_MODULES, type ShellModule } from "./modules";
import { usePageLiveness } from "@/lib/timing/use-tick";

// nav-rail（§三）：192px 展开 ↔ 64px 折叠（2026-10-02 用户要求自 232 收窄）；黑底 chrome；激活项 = chrome-elevated 底 +
// 2px accent 左竖条（黑底上 accent 8.71:1 合规）+ 白字 700。
// 折叠态交互契约（方案 A-2 定稿）：tooltip 与 aria-label 同源 i18n key；折叠按钮进 Tab 序；
// 不做响应式自动切换——两态由用户显式控制（§四 无断点纪律）。
// 用户裁决 2026-10-04：导航栏只保留折叠态（64px），折叠/展开双态与其按钮已下线。
//
// 🔴 2026-10-04：激活态改为**显式派生**（`useLocation` + `data-active`），不再依赖
// `.active` class —— react-router v6+ 的 `NavLink` 默认不再输出该 class，原规则已是死代码，
// 激活样式实际靠不住。指示器与文字色现在同源于 `data-active`，只有一处真相。

/* ══ 弹性指示器（移植自参照工程的 IconNav：两阶段 stretch → settle）═════════════
   一条指示器在槽位之间移动：先按「两槽并集 × dilate」横向撑开（stretch），落位阶段用
   带 overshoot 的 cubic-bezier 冲过目标再回弹（settle）。

   三个旋钮都以 50 为中点 = 组件原本调好的手感（rate 50 → 1.0×、overshoot 50 → 原 tuned 值），
   两端才是可交付的极端：rate 0 = 慢 1.6×，rate 100 = 快 2.5×；
   overshoot 100 时 y = 2·tuned−1，冲过头再回。                                        */

const rate = (speed: number) => 1.6 - (speed / 100) * 1.2;

const overshoot = (bounce: number, tuned: number) =>
  Number((1 + (bounce / 100) * (tuned - 1) * 2).toFixed(3));

/* transform 与尺寸用不同 overshoot：盒子先到位、边缘还在收，读起来是「让步」而不是「矩形在弹」 */
const SETTLE_MOVE_TUNED = 1.38; // 1.28 → 1.38：位移过冲更明显
const SETTLE_SIZE_TUNED = 1.5; // 1.34 → 1.50：尺寸过冲更明显（更"挤"一下再回弹）

const DILATE = 100; // 拉伸强度（0 = 纯平移，100 = 走满封顶值）

/** 预冲位移上限，单位=槽高倍数。1.35 ≈ 59px：「领先一步」更明显（用户要求更 Q 弹） */
const LEAD_MAX = 1.35;
/** 超过这个距离（槽高倍数）改用无过冲曲线：长距离回弹的过冲量 = 路程 × 约 10%，
 *  400px 路程会冲过 40px —— 那不是弹性，是甩尾 */
const FAR_SLOTS = 3;
const BOUNCE = 60; // 50 = 原设计手感；60 → 过冲约 1.46/1.60（用户要求更 Q 弹）
const SPEED = 58; // 50 = 1.0×；58 → 时长 ×0.904（略快，收得更利落）
/* 🔴 stretch 阶段时长基准（乘 rate）—— 2026-10-04 从 150改为 190，与 CSS 侧对齐。
   原来 CSS 写 `190 * rate`(=172ms) 而 JS 定时器用 `150 * rate`(=136ms)，
   两者差 36ms（21%）：JS 在 CSS 过渡的**79% 处**就交棒给落位阶段，
   stretch 被提前打断 —— 这正是「不够 Q 弹 / 拉伸没走完就回弹」的来源。
   注释里写着「交接与阶段同钟」但基准并不相同，属注释与实现不一致。
   现在两处都引用本常量，物理上不可能再漂。⚠️ 改这里会同时改 CSS 与 JS，改一处即可。 */
const STRETCH_BASE_MS = 190;
/** settle 阶段时长基准（乘 rate）。与 stretch 同样只由 CSS 消费，故无「双钟」风险。 */
const SETTLE_BASE_MS = 420;
/** 图标槽边长 = 圆形激活底衬的直径。40 → 44（用户要求"适当大点"）：
 *  胶囊 64 宽，44 + 左右各 10 = 64，正好内嵌。 */
const SLOT = 44;

const Rail = styled.nav`
  /* 🔴 2026-10-04 悬浮胶囊（用户裁决：参照 NexBox / 原版 demo 形态）
     旧：贴左边缘、上下满高的黑列（grid-row: 2）。
     新：两端全圆角 26px 的悬浮胶囊（26 = 胶囊宽 52 的一半，与原版 .gnav 的 --gnav-r 同值），
         垂直居中；**左右外边距由 shell-layout 的 Frame 安全区统一提供**（不各自为政），
         因此这里不再自带 margin-left。 */
  align-self: center;
  margin: 0;
  max-height: 100%;
  background: var(--aegis-chrome-dark);
  display: flex;
  flex-direction: column;
  padding: 8px 0;
  /* 用户裁决 2026-10-04：仅保留折叠态，宽度固定 64px（12 + 40 + 12 = 64，圆形正好居中） */
  width: 64px;
  --nav-item-mx: 10px;
  --slot-w: 44px;
  --slot-x: 10px;
  /* 26 → 30：胶囊更「鼓」（64px 宽时 30 已接近半圆，视觉更饱满） */
  border-radius: 30px;
  /* 绿色流光：绕胶囊外缘旋转的亮环（纯几何两层，不用 mask）。
     **本层只剩静止的盖底层** —— 发光的那一半（裁切层 + 旋转载具）已抽到下方
     Ring 元素，改造理由与三层分工见其注释。
     🔴 本注释位于模板字符串内部，**禁止出现反引号** —— 会提前终止 template literal 报 TS1005。 */
  &::after {
    content: "";
    position: absolute;
    inset: 0;
    border-radius: 30px;
    background: var(--aegis-chrome-dark); /* 盖掉环伸进内部的部分 */
    pointer-events: none;
    z-index: 1;
  }
  flex: none;
  /* 指示器以 Rail 为定位参照（槽位 offsetTop 相对 offsetParent） */
  position: relative;
  /* 动效参数下传给指示器：CSS 负责形状，JS 只给数字。
     🔴 时长基准与 JS 侧共用 STRETCH_BASE_MS（见其注释：此前 190/150 不一致导致 21% 截断）。
     缓动取第六维 motion 令牌（draw = 无过冲、overshoot = 带过冲；两者不可混用）。 */
  --ind-stretch: ${Math.round(STRETCH_BASE_MS * rate(SPEED))}ms;
  --ind-settle: ${Math.round(SETTLE_BASE_MS * rate(SPEED))}ms;
  /* 两个阶段**必须用不同曲线**（参照工程给出的关键细节）：
     拉伸阶段用无 overshoot 的 ease-out —— 指示器是被「拉开」的，冲过终点再回来会读成抖动；
     只有落位阶段才用带 overshoot 的曲线 —— 那是「落到槽里、稍微压一下再回弹」。
     口诀：被拉开的不要弹，落到位的才弹。 */
  --ind-draw: var(--aegis-motion-easing-draw);
  --ind-move: cubic-bezier(0.28, ${overshoot(BOUNCE, SETTLE_MOVE_TUNED)}, 0.36, 1);
  --ind-size: cubic-bezier(0.24, ${overshoot(BOUNCE, SETTLE_SIZE_TUNED)}, 0.38, 1);
`;

/**
 * 流光环的**裁切层 + 旋转载具**（2026-10-04 P1：把环搬进合成层）。
 *
 * 🔴 改的是什么：原先环画在 `Rail::before` 上，动画动的是 `@property --flow-angle`，
 * 而它被 `conic-gradient(from var(--flow-angle), …)` 消费。自定义属性必须经 `var()` 代入，
 * 代入发生在**样式计算**阶段 —— 合成器拿不到，于是只要窗口可见，就是 60fps 常驻的
 * 「样式重算 → 重绘整块 conic-gradient → 8px 光晕模糊」。失焦暂停只是兜底，前台仍全额付费。
 *
 * 🔴 改法：**渐变静态化，让一个载具旋转**。conic-gradient 的取值只取决于相对中心的角度，
 * 且新旧载具与胶囊**同心** ⇒ 同一角度上颜色完全相同；可见区又被裁成同一个轮廓，
 * 因此这是**逐像素等价**的改造，不是重新设计。
 *
 * 三层分工：
 *   本层         —— 静止。`inset:-3.5px` + `overflow:hidden` 把载具裁成原 `::before` 的轮廓
 *                  （胶囊外扩 3.5px、圆角 30+3.5）。光晕也留在这一层：载具旋转后它的投影
 *                  会变成「一个大方块的投影」，不能跟着走。
 *   ::before     —— 载具。与胶囊同心的正方形，边长 ≥ 对角线故任意旋转角都覆盖整条胶囊；
 *                   只做 `rotate`（合成属性）。
 *   `Rail::after` —— 静止的盖底层，盖回胶囊内部；一减一留就是那 3.5px 的边缘环。
 *
 * ⚠️ 为什么不直接给 `Rail` 加 `clip-path`（那样不必加元素）：`clip-path` 会把外扩约 8px 的
 * 光晕一并裁掉，环外的柔和轮廓消失 —— 那是可见回归。独立裁切层能同时保住环宽与光晕。
 *
 * ⚠️ `@keyframes` 必须定义在**使用它的元素**上（即本层），随 emotion 注入的是本层的样式块。
 */
const Ring = styled.span`
  position: absolute;
  inset: -3.5px;
  border-radius: 33.5px; /* = 胶囊圆角 30 + 外扩 3.5，与胶囊同心 */
  overflow: hidden;
  pointer-events: none;
  /* 整圈 60%绿作底 + 实色 accent 亮段在其上跑动。
     35% 在黑底上太淡（用户反馈"颜色太淡、左侧不显示、看着像半环"）→ 提到 60%，
     再叠同色光晕，让环在暗背景上有明确轮廓而不只是一条细亮线。
     🔴 绿值一律由 color-mix 配 var(--aegis-accent) 派生（2026-10-04 审查一般 8）：
     此前四处写死 rgba(118, 185, 0, …)，换主题时环与按钮的绿会漂成两种。 */
  box-shadow: 0 0 8px color-mix(in srgb, var(--aegis-accent) 45%, transparent);
  z-index: 0; /* 流光环：最底 */

  &::before {
    content: "";
    position: absolute;
    left: 50%;
    top: 50%;
    /* 🔴 正方形载具：边长须 ≥ 本层对角线，才能保证**任意旋转角**都覆盖整条胶囊。
       用 aspect-ratio 让宽度跟随高度 —— 百分比 width 会按胶囊宽(64px)解析，远不够高。
       150% 的下限：本 rail 高 ≥ 64px 时恒够（10 项实测约 516px）。
       ⚠️ 若将来 rail 可能矮于 64px，这里要改成 vmax 兜底。 */
    height: 150%;
    aspect-ratio: 1;
    translate: -50% -50%;
    /* 与改前逐值相同的 conic，**只去掉 from var(--flow-angle)**：亮弧固定朝 120deg，
       由载具旋转把它带走。颜色定义一个字没改。 */
    background: conic-gradient(
      color-mix(in srgb, var(--aegis-accent) 60%, transparent) 0deg,
      color-mix(in srgb, var(--aegis-accent) 60%, transparent) 80deg,
      var(--aegis-accent) 120deg,
      color-mix(in srgb, var(--aegis-accent) 60%, transparent) 160deg,
      color-mix(in srgb, var(--aegis-accent) 60%, transparent) 360deg
    );
    /* 周期 3.2s 属「循环速率」，按 motion 契约不入 duration 刻度；缓动取令牌 */
    animation: aegis-nav-flow 3.2s infinite var(--aegis-motion-easing-linear);
    will-change: rotate;
  }

  @keyframes aegis-nav-flow {
    to {
      rotate: 360deg;
    }
  }

  /* 🔴 失焦/不可见时暂停（2026-10-04）。3.2s infinite 是纯装饰、零信息量，
     窗口不可见时不该继续逐帧干活。用 animation-play-state 而非 display:none ——
     后者会让环消失（用户看不见≠看不见），且切换时会有一次重排。
     信号源用现成的 usePageLiveness（已含聚焦 + 可见双条件），挂在 Rail 的 data-live 上；
     此处用祖先属性选择器而非引用 Rail 标识符，免得两者声明顺序变成隐式依赖。 */
  [data-live="false"] &::before {
    animation-play-state: paused;
  }

  /* 规范 §八：prefers-reduced-motion 下**停在这一帧**而不是把时长压到 0.01ms ——
     无限动画 + 0.01ms 等于每帧重放上千次（ADR-006 §三 记录的教训）。 */
  @media (prefers-reduced-motion: reduce) {
    &::before {
      animation: none;
      rotate: 120deg; /* 停在最亮处，仍然看得见这是一道流光环 */
    }
  }
`;

/**
 * 指示器 = **两层嵌套**，而不是一个元素同时动`transform` 与 `height`。
 *
 * 🔴 为什么分两层（2026-10-04）：
 *   ① **都要进合成层**。原实现位置走 `transform`（合成）、尺寸走 `height`（布局，每帧重排），
 *      两者不同步，布局抖动还会拖累同层文本。改为两层后，位移与缩放都是 transform。
 *   ② **两条曲线必须能独立**。参照工程刻意让「盒子先到位、边缘还在收」—— 即位移用
 *      SETTLE_MOVE_TUNED(1.38)、尺寸用 SETTLE_SIZE_TUNED(1.5)两个不同的过冲量。
 *      若把两者并进同一条 `transform` 过渡（单元素方案），就只能用一条曲线，
 *      这个刻意的「让步感」会丢失。外层管位移、内层管缩放，两条 transition 各自独立。
 *
 * 几何等价性（已复算）：w44×h13 r50% → rx22/ry6.5；w44×h44 r50% + scaleY(13/44)
 * → 同样 rx22/ry6.5。内层无内容且 aria-hidden，非均匀缩放不扭曲任何东西。
 */
const IndicatorBox = styled.span`
  position: absolute;
  /* 圆形绿底：包裹激活图标（用户要求「图标被圆形绿色包裹」）。
     left/width 走 --slot-x / --slot-w：折叠态圆心与胶囊中轴重合，展开态贴左包住图标。 */
  left: var(--slot-x);
  top: 0;
  width: var(--slot-w);
  /* 高度固定 44px = --slot-w，实际高度由内层 scaleY 给出（本层不参与尺寸动画） */
  height: var(--slot-w);
  box-sizing: border-box;
  pointer-events: none;
  will-change: transform;
  z-index: 2; /* 绿底压在图标之下、盖底之上 */
  &[data-phase="idle"] {
    transition: none;
  }
  &[data-phase="stretch"] {
    transition: transform var(--ind-stretch) var(--ind-draw);
  }
  &[data-phase="settle"] {
    transition: transform var(--ind-settle) var(--ind-move);
  }
  /* 远距离落位：去掉 overshoot（见 FAR_SLOTS）。用属性选择器而非 JS 换 class，
     这样"远"只影响曲线，不影响 phase 状态机 */
  &[data-phase="settle"][data-far="true"] {
    transition: transform var(--ind-settle) var(--ind-draw);
  }
  /* 规范 §八：prefers-reduced-motion 关闭装饰性过渡但**保留状态变化**（指示器仍要落到目标槽）。
     指示器无 infinite 动画，故 0.01ms + 单次触发不存在「每帧重放」问题。 */
  @media (prefers-reduced-motion: reduce) {
    transition-duration: 0.01ms !important;
  }
`;

const IndicatorFill = styled.span`
  display: block;
  width: 100%;
  height: 100%;
  transform-origin: 50% 0; /* 从槽顶向下展开，与原 height 增长同向 */
  /* 绿色实色（黑底上唯一能读出「它在动」的信号）。前景随之为**深色**：
     #76b900 亮度 0.386，对白仅 2.41:1（不足 AA），
     对 --aegis-ink(#1a1a1a) 实测 **7.22:1** ✅（2026-10-04 复算；
     🔴 原注释写 10.2:1 是未实测值，已改）。 */
  background: var(--aegis-accent);
  /* 正圆；scaleY 压缩后自然成为椭圆（border-radius 50% 随尺寸自适应） */
  border-radius: var(--aegis-radius-full);
  will-change: transform;
  &[data-phase="idle"] {
    transition: none;
  }
  &[data-phase="stretch"] {
    transition: transform var(--ind-stretch) var(--ind-draw);
  }
  /* 🔴 尺寸用**独立的**过冲曲线（overshoot 1.6 > 位移的 1.46）—— 这正是「盒子先到位、
     边缘还在收」的来源，两层分工才能各自用不同曲线（单元素方案做不到）。 */
  &[data-phase="settle"] {
    transition: transform var(--ind-settle) var(--ind-size);
  }
  &[data-phase="settle"][data-far="true"] {
    transition: transform var(--ind-settle) var(--ind-draw);
  }
  @media (prefers-reduced-motion: reduce) {
    transition-duration: 0.01ms !important;
  }
`;

// 两态差异经 Rail 上的 CSS 变量下传：styled(NavLink) 是自定义组件，emotion 不会自动过滤
// 自定义 prop（$ 前缀只对原生元素生效），透传会触发 React 非法属性告警。
const Item = styled(NavLink)`
  display: flex;
  align-items: center;
  /* 展开态贴左：Glyph 40 与激活圆的 left(--slot-x)/width(--slot-w) 精确重合。
     折叠态 Item 宽 = 胶囊宽 - 2×边距 = 40 = Glyph 宽，天然居中。 */
  justify-content: flex-start;
  gap: 10px;
  height: 44px;
  /* 全部导航项等距（用户裁决 2026-10-04：分组间距一并去掉，宁可完全均匀） */
  margin: 3px var(--nav-item-mx);
  padding: 0;
  color: var(--aegis-on-dark-mute);
  font-size: 14px;
  text-decoration: none;
  position: relative;
  /* 层级链：流光环 0 < 盖底 ::after 1 < 圆形绿底 2 < 图标内容 3。
     之前 Indicator(2) 压过 Item(1)，激活绿圆把图标整个盖住了（用户截图）。 */
  z-index: 3;
  flex: none;
  border-radius: var(--aegis-radius-sm);
  transition:
    background-color var(--aegis-motion-duration-fast) var(--aegis-motion-easing-standard),
    color var(--aegis-motion-duration-fast) var(--aegis-motion-easing-standard);
  /* 规范 §八：黑底焦点环用 accent（浅底才是 accent-strong） */
  &:focus-visible {
    outline-color: var(--aegis-accent);
  }
  & span {
    overflow: hidden;
    white-space: nowrap;
  }
  /* 激活态底色与「可读性」由 Indicator 承担（圆形绿底会随动效移动），此处只切前景色与字重：
     绿底上必须用深色前景（**实测 7.22:1**，ink #1a1a1a on accent #76b900），
     沿用白字只有 2.41:1 达不到 AA。
     🔴 2026-10-04：激活瞬间走 **0ms**，取消项仍走渐变。
     理由：新的 stretch 公式保证绿底从第 0 帧就盖住目标槽，底色已就位 → 没有不可读窗口；
     若此处还渐变 120ms，中间色在绿底上实测只有 1.31~2.86:1（如 #5a5a5a = 2.86），
     等于自己造一个短暂不可读窗口。取消方向绿底正在离开，渐变是需要的。 */
  &[data-active="true"] {
    color: var(--aegis-ink);
    font-weight: 700;
    transition:
      background-color var(--aegis-motion-duration-fast) var(--aegis-motion-easing-standard),
      color var(--aegis-motion-duration-instant) var(--aegis-motion-easing-standard);
  }
  &:hover:not([data-active="true"]) {
    color: var(--aegis-on-dark);
    /* 🔴 由on-dark 令牌派生 6% 白（2026-10-04 审查一般 8）：此前写死 rgba(255,255,255,0.06) */
    background: color-mix(in srgb, var(--aegis-on-dark) 6%, transparent);
  }
`;

/** 图标槽：40×40 的正方形，激活时正好被圆形绿底包住（--slot-w）。
 *  不用 JS 测它 —— 指示器量的是 Item 的 offsetTop 再按 (44-40)/2 居中，结果等价。 */
const Glyph = styled.span`
  width: var(--slot-w);
  height: var(--slot-w);
  flex: none;
  display: flex;
  align-items: center;
  justify-content: center;
  color: inherit;
`;

const Spacer = styled.div`
  flex: 1;
`;

type Ind = { p: number; s: number };
type Phase = "idle" | "stretch" | "settle";

function RailItem({
  m,
  active,
  innerRef,
}: {
  m: ShellModule;
  active: boolean;
  innerRef: (el: HTMLAnchorElement | null) => void;
}) {
  const { t } = useTranslation("shell");
  const label = t(m.nameKey);
  const Icon = m.icon;
  return (
    <Item
      to={m.route}
      ref={innerRef}
      data-active={active}
      title={label}
      aria-label={label}
      aria-current={active ? "page" : undefined}
    >
      <Glyph>
        <Icon size={20} strokeWidth={1.5} aria-hidden="true" />
      </Glyph>
    </Item>
  );
}

export function NavRail() {
  const { t } = useTranslation("shell");
  const modules = SHELL_MODULES.filter((m) => m.group === "modules");
  const orchestration = SHELL_MODULES.filter((m) => m.group === "orchestration");
  const utility = SHELL_MODULES.filter((m) => m.group === "utility");

  const { pathname } = useLocation();
  const isActive = useCallback(
    (m: ShellModule) => pathname === m.route || pathname.startsWith(`${m.route}/`),
    [pathname],
  );
  const activeId = SHELL_MODULES.find(isActive)?.id;
  // R-19 现成信号源：聚焦 + 可见双条件，用于失焦时暂停流光环（见 Rail 的 data-live 规则）
  const live = usePageLiveness();

  const railRef = useRef<HTMLElement | null>(null);
  const slots = useRef<Record<string, HTMLAnchorElement | null>>({});
  const indRef = useRef<Ind | null>(null);
  const timer = useRef<number | undefined>(undefined);
  const [ind, setInd] = useState<Ind | null>(null);
  const [phase, setPhase] = useState<Phase>("idle");
  /** 远距离切换：落位曲线换成无过冲版（见 FAR_SLOTS 注释） */
  const [far, setFar] = useState(false);

  /* 几何 effect 要读「当前激活项」却不能把它列进依赖 —— 否则激活项一变，它会抢在
     动画 effect 之前把指示器直接落位（两阶段被抹平，实测 y 一步到位、phase 恒 idle）。
     故用 ref 读最新值，依赖里不出现 activeId。 */
  const activeIdRef = useRef(activeId);
  activeIdRef.current = activeId;

  /* 量 Item（高 44）再把 40 的圆心对齐其中心：p = top + (44 - SLOT) / 2。
     不去量 Glyph 本身 —— 两者等价，但这样两态切换时只需 SLOT 一个常量。 */
  const measure = useCallback((id: string | undefined): Ind | null => {
    const el = id ? slots.current[id] : null;
    return el ? { p: el.offsetTop + (el.offsetHeight - SLOT) / 2, s: SLOT } : null;
  }, []);

  // 槽位几何随「窗口缩放/字体缩放」变化：直接落位（idle，无过渡），不做两阶段动画
  useLayoutEffect(() => {
    const settleNow = () => {
      const next = measure(activeIdRef.current);
      if (!next) return;
      indRef.current = next;
      setFar(false);
      setPhase("idle");
      setInd(next);
    };
    settleNow();
    const rail = railRef.current;
    if (!rail) return;
    const ro = new ResizeObserver(settleNow);
    ro.observe(rail);
    return () => ro.disconnect();
  }, [measure]);

  // 激活项切换：两阶段指示器
  useLayoutEffect(() => {
    const to = measure(activeId);
    if (!to) return;
    const from = indRef.current;
    window.clearTimeout(timer.current);
    if (!from) {
      indRef.current = to;
      setPhase("idle");
      setInd(to);
      return;
    }
    if (from.p === to.p && from.s === to.s) return; // 尺寸未变（重复激活）→ 不播
    /* ── stretch：朝来向「预冲 + 拉伸」，两项都按槽高封顶 ──────────────────
       原参照工程的公式是「位置回退到起点 + 长度撑满两槽并集」，那在它自己的场景里成立
       （5 个槽、最远跨 4 格 ≈ 184px）。Aegis 的 rail 有 10 个槽、展开态 713px 高：
       跨 4 格实测绿块高度 44 → 332（7.5 倍），跨 8 格会到 450px（占屏一半），
       而位置还要先退回起点再滑回来 = 两次大幅位移 —— 这正是"跨图标切换不流畅"的来源。
       故保留手感但封顶：短距离（≤ 封顶值）行为与原版完全一致，长距离收敛为
       「固定幅度的预冲 + 固定长度的拉伸」，动作紧凑可预期。 */
    const grow = DILATE / 100;
    const travel = Math.abs(to.p - from.p);
    const dir = Math.sign(from.p - to.p) || 1; // 1 = 目标在下方，向上预冲
    const leadBy = Math.min(travel, to.s * LEAD_MAX) * grow;
    setFar(travel > to.s * FAR_SLOTS);
    setPhase("stretch");
    /* 🔴 2026-10-04 修正不可读窗口（此前是 p = to.p + dir * leadBy）。

       症状：向上切（dir=+1）时绿块被推到 `to.p + leadBy`，**目标槽完全落在绿块之外**
       （实测 leadBy=59.4 时绿块 [259.4, 373.8]、目标槽 [200, 244]，间隙 15.4px）。
       而图标的前景色是**立即**切成深墨色的 —— 于是「黑底(#000) + 墨字(#1a1a1a)」
       实测 **1.21:1**，远低于 3:1。这不是渐变中间态，是「颜色已切完、底色还没到」的
       稳定错配，持续整个 stretch（172ms）。
       （向下切 dir=-1 恰好相反：绿块 [to.p-leadBy, to.p-leadBy+span] 包含目标槽，正常。）

       为什么不能靠换一个前景色解决：**数学上无解**。要一个前景色同时对黑(L=0)与
       绿(L=0.3855)都 ≥3:1，需Lf ≥ 0.1000 且 Lf ≤ 0.0952 —— 空集。只能靠时序解决。

       改法：blob **从目标槽出发、朝来向延伸**，p 绝不越过目标：
         stretchP    = to.p + min(0, dir * leadBy)   → 向下切向上伸、向上切不伸
         stretchSpan = to.s + leadBy                → 把伸出的部分算进长度
       ⇒ 目标槽从第 0 帧起就被覆盖（两个方向都验证过），黑底期消失。
       手感仍是橡皮筋（朝来向拉开再收），语义从「甩过来」变成「收进新位置」。

       ⚠️ span 上限 = 44 + 59.4 = 103.4，未超过原封顶 114.4（2.6 槽高倍数）。
       故原 SPAN_MAX 常量已随之移除—— 新的 span 由 SLOT + leadBy 决定，
       leadBy 本身仍受 LEAD_MAX 封顶，长距离不会无限拉长。 */
    const stretchP = to.p + Math.min(0, dir * leadBy);
    const stretchSpan = to.s + leadBy;
    setInd({ p: stretchP, s: stretchSpan });
    /* 交接与阶段**同钟**：定时器基准必须等于 CSS 的 --ind-stretch，否则会在拉伸途中
       提前打断（本轮修正：原 CSS 用 190 基准而这里用 HANDOFF_MS=150，差 21%）。
       引用同一个 STRETCH_BASE_MS，物理上不可能再漂。 */
    timer.current = window.setTimeout(
      () => {
        indRef.current = to;
        setPhase("settle");
        setInd(to);
      },
      STRETCH_BASE_MS * rate(SPEED),
    );
  }, [measure, activeId]);

  useEffect(() => () => window.clearTimeout(timer.current), []);

  /* 宽度用 calc 而非测量值：Item 的左右 margin 来自 --nav-item-mx（随折叠态变化），
     calc 让指示器宽度自动等于槽位宽；写成 100% 会比激活项宽出一圈。
     高度仍用测量值 —— 它才是两阶段动效真正要拉伸的量。 */
  /* 宽度由 CSS 的 --slot-w 决定（两态可不同档），这里只给位置（外层）与缩放（内层）。
     🔴 2026-10-04：尺寸从 height 改为内层 scaleY —— height 是布局属性
     （每帧触发布局、与位置动画不同步），scaleY 走合成层。
     除以 SLOT 是因为内层 height:100% 对应外层固定的 --slot-w。 */
  const boxStyle = ind ? { transform: "translate3d(0, " + ind.p + "px, 0)" } : { opacity: 0 };
  const fillStyle = ind ? { transform: "scaleY(" + ind.s / SLOT + ")" } : { opacity: 0 };

  return (
    <Rail ref={railRef} aria-label={t("nav.toggleAria")} data-live={live ? "true" : "false"}>
      {/* 流光环：静止裁切层 + 内部旋转载具（见 Ring 注释）。必须在指示器之前，
          以便靠 z-index 链（环 0 < 盖底 1 < 指示器 2 < 图标 3）分层。 */}
      <Ring aria-hidden />
      <IndicatorBox
        data-phase={phase}
        data-far={far ? "true" : "false"}
        style={boxStyle}
        aria-hidden
      >
        <IndicatorFill data-phase={phase} data-far={far ? "true" : "false"} style={fillStyle} />
      </IndicatorBox>
      {modules.map((m) => (
        <RailItem
          key={m.id}
          m={m}
          active={m.id === activeId}
          innerRef={(el) => {
            slots.current[m.id] = el;
          }}
        />
      ))}
      {orchestration.map((m) => (
        <RailItem
          key={m.id}
          m={m}
          active={m.id === activeId}
          innerRef={(el) => {
            slots.current[m.id] = el;
          }}
        />
      ))}
      <Spacer />
      {utility.map((m) => (
        <RailItem
          key={m.id}
          m={m}
          active={m.id === activeId}
          innerRef={(el) => {
            slots.current[m.id] = el;
          }}
        />
      ))}
    </Rail>
  );
}
