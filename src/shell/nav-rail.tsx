import { NavLink, useLocation } from "react-router-dom";
import { useCallback, useEffect, useLayoutEffect, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import styled from "@emotion/styled";
import { SHELL_MODULES, type ShellModule } from "./modules";

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
  /* 🔴 2026-10-05 垂直迁移第 6 条 · **用户二次改判（澄清原裁决）**
   *
   * 原裁决「导航胶囊改为贯通除拖动条和底部空白外的区域」表达的是**上下界约束**，
   * 不是「必须填满」：胶囊**不得越过**拖动条下沿与底部留白上沿，而**内容不足时按实际
   * 尺寸在该区间内上下居中**。先前实现把它读成「align-self: stretch 拉满 736」，
   * 结果 10 个槽位（516px）被硬撑到 736，胶囊内出现大片空白 —— 属实现过度。
   *
   * 正确实现两行即可（**不写死任何高度**）：
   *   align-self: center  → 高度由内容决定（槽位数 × 间距 + padding），在 736 区间居中
   *   max-height: 100%   → 槽位多到装不下时的**上界**（内容溢出时贴住上沿，不会越界）
   *
   * 当前 10 槽：8(padding) + 10×44 + 9×6(margin 3×2) + 8 = 516 ⇒ 736 内上下各留 110。
   * 将来加到 13 槽（636）仍居中；14 槽（686）贴上下沿前仍有 50 余量。
   * ⚠️ 若槽位多到超过 736，max-height 会**收住上界**（不越界）但内容会被 Frame 的
   *   overflow:hidden 裁掉 —— 那时需另加滚动。**刻意不在此刻加 overflow-y**：
   *   它会创建滚动容器，与「隐藏滚动条」方案（scrollbar-plan.md 的 Q1 待裁决）交互，
   *   属于需要单独立项的决策，不该顺手塞进本次改判。
   */
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
 *   本层         —— 静止。`inset:-4px` + `overflow:hidden` 把载具裁成原 `::before` 的轮廓
 *                  （胶囊外扩 4px、圆角 30+4=34）。光晕也留在这一层：载具旋转后它的投影
 *                  会变成「一个大方块的投影」，不能跟着走。
 *   ::before     —— 载具。与胶囊同心的正方形，边长 ≥ 对角线故任意旋转角都覆盖整条胶囊；
 *                   只做 `rotate`（合成属性）。
 *   `Rail::after` —— 静止的盖底层，盖回胶囊内部；一减一留就是那 4px 的边缘环。
 *
 * ⚠️ 为什么不直接给 `Rail` 加 `clip-path`（那样不必加元素）：`clip-path` 会把外扩约 8px 的
 * 光晕一并裁掉，环外的柔和轮廓消失 —— 那是可见回归。独立裁切层能同时保住环宽与光晕。
 *
 * ⚠️ `@keyframes` 必须定义在**使用它的元素**上（即本层），随 emotion 注入的是本层的样式块。
 */
const Ring = styled.span`
  position: absolute;
  /* 🔴 2026-10-04 用户裁决：safe-x 20 量的是**胶囊本体**，环外缘距窗口边应为 16
     ⇒ 外扩量 = 20 − 16 = 4px（原 3.5px 是按「环外缘 16.5 对 20」的老口径估的，
     那时本该是 4 才对；16.5 这个数是从边框位置反推的近似，不是设计量）。
     环外缘 → 光晕 8px → 距窗口边 8px ⇒ 光晕完整在框内，余量 8px。 */
  inset: -4px;
  border-radius: 34px; /* = 胶囊圆角 30 + 外扩 4，与胶囊同心 */
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

  /* 🟢 原「失焦/不可见时暂停动画」规则已于 2026-10-04 移除（用户裁决）。
     —— 移除理由（保留存档，避免日后被"优化回来"）：
     该规则是 **P1 之前**加的。当时环的动画动的是 @property 变量 --flow-angle，每帧在主线程
     重算样式并重绘整块 conic-gradient（实测 1.65% 单核、永不停歇），失焦暂停能省下一大块。
     P1 把环改成「静态渐变 + 旋转载具」进合成层之后（实测 RecalcStyleCount 由 300 降为 0），
     失焦暂停省下的只剩一点 GPU，**收益基本归零**；而代价仍在：
     Aegis **可见但未聚焦**时（放副屏、用户在别的窗口打字），环会停住，看起来像应用卡死。
     即：该优化的必要性被后续那次优化架空了，故撤销。
     🔴 下方 prefers-reduced-motion 那条**保留不动** —— 那是规范 §八的无障碍要求，
     与性能无关，且属用户系统偏好，不得以"省 CPU"为由削弱。
     🔴 本注释在模板字符串内部，**禁止出现反引号** —— 会提前终止 template literal 报 TS1005
     （本轮已因此踩坑一次；写降级/性能类注释时尤其容易带进来）。 */

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
 * 指示器 = **两层嵌套**：外层管位移（transform，合成），内层管尺寸（height，布局）。
 *
 * 🔴 为什么分两层：**两条曲线必须能独立**。参照工程刻意让「盒子先到位、边缘还在收」——
 *   位移用 SETTLE_MOVE_TUNED(1.38)、尺寸用 SETTLE_SIZE_TUNED(1.5) 两个不同的过冲量。
 *   两层各持一条 transition，互不干扰；单元素方案只能把两者并进一条曲线。
 *
 * 🔴 尺寸**为什么必须用 height，不能用 scaleY**（2026-10-04 实测纠正）：
 *   `border-radius` 是在**变换之前**按元素自身的盒子算出来的。内层盒是 44×44，
 *   `--aegis-radius-full`(9999px) 在它上面得到一个**正圆**；`scaleY` 再把这个圆整体
 *   纵向压/拉 ⇒ 渲染结果是**椭圆**。而参照件动的是 `height`，半径每帧按**新盒子**重算，
 *   9999px 在 44×114 上退化为 6.5px 级圆角 ⇒ 得到「两端半圆、两侧平直」的**胶囊形**。
 *
 *   逐行宽度剖面实测（无头 Edge + 逐像素扫描；三者包围盒同为 44×114，可直接对比）：
 *     参照件 height + 9999px ： 6 36 44 44 44 44 44 44 44 44 43 36 5  ← 宽度恒定
 *     曾用   scaleY + 9999px ： 4 25 33 38 42 43 44 43 42 38 33 25 4  ← 连续收窄
 *     对照   height + 50%   ： 4 25 33 38 42 43 44 43 42 38 33 25 4  ← 与上一行**逐样本相同**
 *   即 scaleY 渲染出来的东西与「手写 border-radius:50%」没有任何区别，是椭圆。
 *   用户看到的「圆形 → 椭圆 → 圆形」就是这一条。
 *
 *   ⚠️ 代价：height 是布局属性，该元素每帧参与布局。可接受的理由 —— 它是绝对定位盒子
 *   内部的单个 44px 宽元素，不牵动任何兄弟或祖先的高度（外层盒高恒为 --slot-w）。
 *   ⚠️ 曾经的错误论证：「w44×h44 r50% + scaleY(13/44) ≡ w44×h13 r50%」在**半径为 50%**
 *   时数学上成立，但本实现用的是 9999px，前提不成立 ⇒ 该结论对本实现无效。
 */
const IndicatorBox = styled.span`
  position: absolute;
  /* 圆形绿底：包裹激活图标（用户要求「图标被圆形绿色包裹」）。
     left/width 走 --slot-x / --slot-w：折叠态圆心与胶囊中轴重合，展开态贴左包住图标。 */
  left: var(--slot-x);
  top: 0;
  width: var(--slot-w);
  /* 高度固定 44px = --slot-w：本层**只管位移**，尺寸动画由内层用 height 给出（见上 JSDoc）。
     内层高度超出本盒时向下溢出（本层无 overflow 裁剪），与「从槽顶向下展开」同向。 */
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
  /* 🔴 高度由内联 style 给出（ind.s px）；这里的值只是无 JS 时的兜底。
     用 height 而不是 scaleY 的理由见上方 IndicatorBox 的 JSDoc —— 一句话：
     border-radius 按**变换前**的盒子算，44×44 得到正圆，scaleY 会把它压成椭圆；
     height 让半径每帧按新盒子重算，才是参照件那种「两端半圆、两侧平直」的胶囊形。 */
  height: var(--slot-w);
  /* 绿色实色（黑底上唯一能读出「它在动」的信号）。前景随之为**深色**：
     #76b900 亮度 0.386，对白仅 2.41:1（不足 AA），
     对 --aegis-accent-on(#000000) 实测 **8.71:1** ✅（tokens.ts 已录该值）。 */
  background: var(--aegis-accent);
  /* 胶囊半径：在 44×44 上正好是正圆，在 44×114 上退化为「两端半圆 + 两侧平直」 */
  border-radius: var(--aegis-radius-full);
  &[data-phase="idle"] {
    transition: none;
  }
  &[data-phase="stretch"] {
    transition: height var(--ind-stretch) var(--ind-draw);
  }
  /* 🔴 尺寸用**独立的**过冲曲线（overshoot 1.6 > 位移的 1.46）—— 这正是「盒子先到位、
     边缘还在收」的来源。外层持 --ind-move、内层持 --ind-size，两条 transition 各自独立。 */
  &[data-phase="settle"] {
    transition: height var(--ind-settle) var(--ind-size);
  }
  &[data-phase="settle"][data-far="true"] {
    transition: height var(--ind-settle) var(--ind-draw);
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
     绿底上必须用深色前景。
     🔴🔴 2026-10-05 修正一个**被深色改造暴露的语义键误用**：此处原写 var(--aegis-ink)，
        注释的依据是「ink #1a1a1a on accent 实测 7.22:1」—— 但 **#1a1a1a 是浅色包的 ink**。
        换成深色包后 ink 变成 **#e8e8e8（浅色）**，绿底上只剩 **1.91:1** ⇒
        激活项的图标「浅色字压在绿底上」几乎看不见（像素实测圆心为纯 #76B900，无图标色）。
        正确键是 **accent-on**（语义就是「accent 底上的前景」）：#000000，**8.71:1**。
        ⚠️ 这是「注释里的对比度数字必须随主题包重算」的又一例：注释声称 ≠ 代码事实。 */
  &[data-active="true"] {
    color: var(--aegis-accent-on);
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
  /* 🔴 2026-10-05 垂直迁移的实现修正（规范 §七 第 6 条原文写的是「槽位
     justify-content: center」，落地时改为沿用本 Spacer）：
     胶囊内槽位如何排布有两个选择 ——
       A) 全部槽位居中：modules / orchestration / utility 会一起跑到中间，
          **设置等 utility 项不再贴底**，破坏标准侧栏惯例（用户此前按 NexBox 校准过）。
       B) 保持 Spacer(flex:1) 撑开：上组贴顶、utility 贴底，其余留白集中在中部。
     取 B —— 胶囊高度由内容决定（当前 516）后，Spacer 只在**胶囊内部**把 utility 推到
     胶囊底端，胶囊本身仍由 align-self: center 在 736 区间居中。
     规范已回改留痕（规范 §七 第 6 条「贯通」= 上下界约束，非填满）。 */
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
  // （原此处有 `usePageLiveness()` + Rail 的 `data-live`，用于失焦时暂停流光环。
  //   2026-10-04 与 Ring 的暂停规则一并移除，理由见 Ring 顶部「原失焦暂停规则已移除」注释。
  //   注意：`usePageLiveness` 在概览取数（features/Overview）处仍是必需，故 use-tick 不动。）

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

  /* 宽度由 CSS 的 --slot-w 决定；这里外层给位置（transform，合成），内层给高度（height）。
     🔴 2026-10-04 纠正：内层曾用 `transform: scaleY(ind.s / SLOT)` 以求「进合成层」，
     但那会把 44×44 的正圆压成**椭圆**（border-radius 按变换前的盒子算），
     与参照件的「胶囊形」不一致 —— 用户实测反馈「圆形变椭圆再变回」即此。
     现已回到 height，逐行宽度剖面与参照件一致。详见 IndicatorBox 的 JSDoc。 */
  const boxStyle = ind ? { transform: "translate3d(0, " + ind.p + "px, 0)" } : { opacity: 0 };
  const fillStyle = ind ? { height: ind.s + "px" } : { opacity: 0 };

  return (
    <Rail ref={railRef} aria-label={t("nav.toggleAria")}>
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
