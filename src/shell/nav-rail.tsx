import { NavLink, useLocation } from "react-router-dom";
import { useCallback, useEffect, useLayoutEffect, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import styled from "@emotion/styled";
import { SHELL_MODULES, matchModule, type ShellModule } from "./modules";

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
 *  400px 路程会冲过 40px —— 那不是弹性，是甩尾
 *  ⚠️ 单位 = **槽高 44**（非槽距 pitch 50）：3 × 44 = 132px ≈ 2.6 个槽距。
 *    2026-10-06 审查 #10 曾质疑「132 ≠ 3 格」—— 单位本就是槽高，本注释即准绳。 */
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
  /* 用户裁决 2026-10-04：仅保留折叠态，宽度固定 64px（10 + 44 + 10 = 64，圆形正好居中） */
  width: 64px;
  --nav-item-mx: 10px;
  --slot-w: 44px;
  --slot-x: 10px;
  /* 26 → 30：胶囊更「鼓」（64px 宽时 30 已接近半圆，视觉更饱满） */
  border-radius: 30px;
  /* 绿色流光：静止的**盖底层**。职责有两条，缺一不可：
     ① 遮住三条同心带（RingBand）伸进胶囊**内侧**的那一半 —— 每条带都是**填满**的
        圆角矩形（不是描边，见 RingBand 注释：border-width 会走布局），
        本层用不透明底色把内侧切掉，相减之后露出来的才正好是 2/4/6px 的环。
        ⚠️ 这也是四条带**必须**画成填满矩形的原因：填充 + 盖底 = 描边，且只动 opacity。
        ② 遮住 RingAura 的 8px 光晕**向内**溢出的那一半 —— box-shadow 的 blur 是双向的
        （0 0 8px 从盒边向内外各洇 8px），不盖住的话胶囊内缘会染上一圈绿，
        读起来像「胶囊内壁发绿」而不是「胶囊悬浮在面板之上」。向外那半正是要留给用户看的。
     🔴 2026-10-05 动效改版前的理由是「盖掉旋转载具伸进内部的部分」，那个载具已随改版删除。
     ⚠️ 因此本层不可删、不可调透明度：删了会看到内壁绿染 + 四条带糊成一坨实心绿，
        调透明会让它失去盖底作用。
     本注释在模板字符串内部，**禁止出现反引号** —— 会提前终止 template literal 报 TS1005
     （本轮已因此踩坑两次；写降级/性能类注释时尤其容易带进来）。 */
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
 * 流光环：**宽度呼吸**（2026-10-05 用户指令「由内向外扩散加粗」第二轮改版；
 * 2026-10-06 第四轮改为「恒亮亮核 + 中/外级联」的涟漪节奏 —— 见下【级联呼吸】）。
 *
 * 【与上一版的区别】上一版是「一道亮弧绕胶囊转」（2026-10-04 P1 把 conic 渐变静态化 +
 * 载具 rotate）；本版是**环本身由细变粗再回缩** —— 这是两种不同的动效，不是同一动的调参。
 * 载具（正方形 + conic-gradient）与上一版 Ring 的 overflow:hidden 裁切**一并删除**。
 *
 * 🔴 为什么不能直接「动宽度」：能改变可见宽度的属性都不是合成器属性 ——
 *   border-width / inset / width 走**布局**（每帧重排）；box-shadow 的 spread 与
 *   渐变尺寸走**重绘**；而 @property 注册的自定义属性被 var() 消费时，代入发生在
 *   **样式计算**阶段，合成器仍然拿不到 ⇒ 退化成每帧「样式重算 → 重绘整块渐变」，
 *   这正是 2026-10-04 P1 之前的病根（实测 RecalcStyleCount 300、单核 1.65% 常驻）。
 *   ⇒ 只剩一条合规的路：**只动 opacity**。
 *
 * 【本版做法】**三条同心带的 opacity 交叉淡入淡出**：
 *   每层是一块**填满**的圆角矩形（不是描边），同心、依次内缩 2px；
 *   内侧由 Rail::after（inset:0 的不透明盖底）挡住 ⇒ 每层露出来的正是一条 2px 宽的带。
 *   于是「第 k 条带可见」= 把它 opacity 从 0 抬到 1，「三条全可见」= 6px 环。
 *
 *   露宽 = inset 4 / 2 / 0 ⇒ 2 / 4 / 6px，圆角同步 32 / 34 / 36（始终同心）。
 *   🔴 三档**全是偶数 CSS px**（2026-10-06 用户裁决）：150% 缩放下 1 CSS px = 1.5 设备 px，
 *      奇数档的带边界落在半个设备像素上，渲染器同向取整 ⇒ 该条带平移 0.5 设备 px，
 *      环读成一边粗一边细（1px 档实测左 1 列 / 右 2 列）。2 / 4 / 6 档每边都是整数设备像素。
 *      完整推导与逐列取色实测见 RingBand 顶部注释。
 *   绘制顺序 = DOM 顺序，**由外向内**（band6 先画、band2 最后画）：
 *   于是外侧那条带只由最外层决定、内侧依次被更内层盖住。
 *
 * 【级联呼吸】（🔴 2026-10-06 第四轮，用户裁决「内侧 2px 恒亮，接着中间 2px 亮起，
 *   最后外侧 2px 亮起；关闭时先关外侧、接着关中间；三档亮度要连贯，不能有明显色阶」）：
 *   ⓐ band2 = **恒亮亮核**：撤掉动画、opacity 恒 1。原「峰值把最内层压暗到 0.62」的
 *      光通量平衡做法整体作废 —— 亮核不参与呼吸，环的粗细变化全由中/外两档承担。
 *   ⓑ 中/外两档各持**自己的关键帧**（不是同一条错相位 —— 两档的占空比本就不同），
 *      周期 5.2s（用户选定；含 10% 静息段），时间轴：
 *        0–10%   静息：只剩 2px 亮核（0.52s，让「恒亮核」每轮都被读到）
 *        10–34%  中档亮起（0→1）
 *        40–58%  外档**最后**亮起（0→1；比中档满座晚 6% ≈ 0.3s，形成「接着」的先后）
 *        58–68%  6px 停（0.52s，中/外全亮）
 *        68–88%  外档**最先**关闭（1→0）
 *        72–100% 中档随后关闭（1→0）—— 首尾同值 ⇒ 循环接缝在数学上不存在
 *      缓动写在**每一帧**上（决定本帧→下一段；写在元素上会被整条动画统一覆盖）。
 *   ⓒ 🔴 为什么不用 animation-delay / Motion 的 stagger 做错开：delay 会让首帧不是静息态
 *      （要等 offset 走完），且停帧后 offset 残留在 animation-delay 上不归零。
 *      错开**写进关键帧位置**，首帧恒为静息态、停帧/兜底语义不受影响（沿用 2026-10-05 结论）。
 *   ⓓ 🔴 为什么中/外两带要**羽化**（band4 blur(1px)、band6 blur(2px)，静态滤镜）：
 *      纯色带阶梯式叠出的三档亮度，在 4px / 6px 两条分界上是**硬边**（实测复合绿度
 *      232 / 120 / 50，两次跳变都肉眼可见）。羽化把这两条硬边化成渐变，亮度由内向外
 *      连续衰减 —— 用户裁决「不能有明显的色阶」即指此。亮核 **不羽化**：它与中档之间
 *      那条锐利亮线正是「恒亮核」的视觉主体。滤镜是**静态**的（只栅格化一次、随层缓存），
 *      动画仍只动 opacity ⇒ 仍在合成器路径（逐帧样式重算实测见
 *      .worknotes/changes/2026-10-06-qoder-nav-ring-ripple-choreography.md）。
 *   ⓔ 亮核恒亮后，最宽态复合亮度由内向外 = 85% 叠(34%+20%) / 34%+20% / 20%：
 *      内圈不再压暗 —— 用户「恒亮」的裁决**优先于**旧版「亮线化成暗绿柔边」的平衡。
 *
 * 【颜色】一律 color-mix 配 var(--aegis-accent) 派生（2026-10-04 审查一般 8 的纪律）：
 *   绿值不写死 rgba(118, 185, 0, …)，否则换主题时环与按钮的绿会漂成两种。
 *
 * 【性能】3 条 opacity 动画（RingAura + 中/外两条带；亮核已无动画），全部走合成层；
 *   无 conic 渐变重绘、无布局属性动画；外两带的 blur 是**静态滤镜**（栅格化一次后随层缓存，
 *   不随 opacity 逐帧重绘）。Rail 是 flex:none、不在任何滚动容器内 ⇒ 常驻合成层不绑滚动线程。
 *
 * 🟢 原「失焦/不可见时暂停动画」规则仍**不恢复**（用户 2026-10-04 裁决）。存档理由：
 *   该规则的必要性来自 P1 之前每帧重算样式 + 重绘整块 conic 渐变与 8px 光晕；
 *   现在只剩 opacity，暂停省下的基本归零，而代价是「应用可见但未聚焦（放副屏、
 *   用户在别的窗口打字）时环停住，看起来像应用卡死」。即必要性被后续优化架空了。
 *   🔴 下方 prefers-reduced-motion 那条**保留不动** —— 那是规范 §八 的无障碍要求，
 *   与性能无关，且属用户系统偏好，不得以「省 CPU」为由削弱。
 *
 * ⚠️ @keyframes 必须定义在**使用它的元素**上（即本层），随 emotion 注入的是本层的样式块。
 */
const Ring = styled.span`
  position: absolute;
  /* 🔴 外扩量 = **6px**（2026-10-06 用户裁决「2/4/6 三档」连带确定）：环要能长出 6px 的带，
     外缘就必须离胶囊 6px。原值 4px（2026-10-04 裁决「环外缘距窗口边应为 16」）随之失效 ——
     SAFE_LEFT 20 量的是**胶囊本体**，外扩 6 ⇒ 环外缘距窗口边 20 − 6 = **14px**，
     光晕（box-shadow 8px）外缘距边 20 − 6 − 8 = **6px**，仍在框内、不被 Frame 裁切。
     ⚠️ 改这个值须同改 shell-layout.tsx 里 SAFE_LEFT / SAFE_RIGHT 的联动注释（硬下限
        也从「4 + 8 = 12」变成「6 + 8 = 14」，当前 20 仍满足）。 */
  inset: -6px;
  /* 本层自身**不画任何颜色**（上一版这里承载 conic 渐变，已删）；它现在的职责只有两个：
     ① 给子层提供「胶囊外扩 6px」的定位参照；② 用 z-index:0 把整组光环压到最底。
     圆角随之定为 36（= 胶囊 30 + 外扩 6），子层按 inset 逐层递减保持同心。 */
  border-radius: 36px;
  pointer-events: none;
  z-index: 0; /* 光环组：最底。盖底层 1 < 指示器 2 < 图标内容 3 */

  /* ⚠️ 上一版的 overflow: hidden 已删：它当初只为裁切那个旋转的正方形载具而存在，
     载具随改版删除；而外光晕（RingAura 的 box-shadow）必须能向外洇到 6px 之外，
     加裁切反而把柔和轮廓削成一圈硬边。子元素都落在 inset 0…4 之间，本身不会溢出。 */
`;

/**
 * 外光晕：一圈 8px 柔和辉光，**只动 opacity**（blur / spread / 颜色全部写死）。
 * 它负责「悬浮于面板之上」的**静态**那部分 —— 面板与胶囊之间永远有一层极淡的绿，
 * 最细态也不会读成「胶囊贴在面板上」。宽度呼吸由三条带负责，光晕只做强度起伏。
 */
const RingAura = styled.span`
  position: absolute;
  inset: 0; /* = 胶囊外扩 6px，与最外圈带同轮廓 */
  border-radius: 36px;
  /* 🔴 绿值一律 color-mix 配 var(--aegis-accent) 派生，不写死 rgba。 */
  box-shadow: 0 0 8px color-mix(in srgb, var(--aegis-accent) 45%, transparent);
  /* 谷值 0.34 而非 0：全灭时光晕会硬生生消失，胶囊读成「一条贴边细线」；
     留三成余量，最细态也保有一圈极淡的分离感（这正是要的分层）。
     本行的静态值同时是**动画失效时的兜底**（见 RingBand 顶部同款说明）。 */
  opacity: 0.34;
  /* 3.6s → 5.2s（2026-10-06 第四轮）：旧节奏只有「一段推进 + 一段收回」，3.6s 够用；
     新节奏拆成「静息 → 中档 → 外档 → 6px 停 → 外收 → 中收」六段，时长必须容纳
     0.52s 静息段与 0.52s 全亮段，3.6s 会把四段过渡挤成闪现。
     周期属「循环速率」，按 motion 契约刻意不入 duration 刻度（tokens.ts §六 同款理由）。 */
  animation: aegis-nav-aura 5.2s infinite;
  will-change: opacity;

  /* 🔴 峰值与 6px 停**对齐**（58–68%）：第四轮改版后层次已由中/外两档的级联自己承担，
     光晕不必再靠错相位制造「余晖」（旧版「峰值挪到 62% 与环错开」的理由随同相五帧一并作废）。
     新口径：光晕随中/外推进一起上升、随收回一起落回，最亮的一段正好罩住 6px 停。
     ⚠️ 为什么不用 animation-delay 做相位错开（Motion 的 stagger 同款做法）：
        delay 会让整条动画**整体后移**，首次进入页面时首帧不是静息态（要等offset 走完），
        且负delay 在prefers-reduced-motion 停帧后残留在 animation-delay 上不归零。
        相位差写进关键帧位置则**首帧恒为静息态**，停帧/兜底语义不受影响。
     ⚠️ 首尾仍同值（0.34 / 0.34）⇒ 循环接缝依旧不存在。 */

  /* ⬇ 元素级 @keyframes —— 与属性声明同层，**不在上方任何 & 选择器内**
     （分隔属可读性加固：本块历史上从未嵌套，只是与 &[...] 块挤在一起易被误读。2026-10-06） */
  @keyframes aegis-nav-aura {
    0% {
      opacity: 0.34;
      animation-timing-function: linear; /* 0→10% 静息段：与中/外同宽，不参与缓动 */
    }
    10% {
      opacity: 0.34;
      animation-timing-function: cubic-bezier(0.45, 0, 0.55, 1); /* 随中/外推进上升 10→58% */
    }
    58% {
      opacity: 1;
      animation-timing-function: linear; /* 6px 停 58→68%，与环同步 */
    }
    68% {
      opacity: 1;
      animation-timing-function: cubic-bezier(0.45, 0, 0.55, 1); /* 随收回落回 68→100% */
    }
    100% {
      opacity: 0.34;
    }
  }

  /* 规范 §八：prefers-reduced-motion 下**停在这一帧**，而不是把时长压到 0.01ms ——
     无限动画 + 0.01ms 等于每帧重放上千次（ADR-006 §三 记录的教训）。
     🔴 停帧必须**显式复位**被动画的属性：这里动画动的是 opacity，若只写 animation: none
     而不复位，浏览器回落到本层静态 opacity（本次恰好也是 0.34，属「巧合对」而非
     「写明对」）—— 静态值一改就会静默停在错误亮度。故显式写出。
     ⚠️ 主题层（theme.ts 的 reduced-motion 全局兜底）只压 duration/iteration-count、
        不声明 animation-name，故这里的 animation: none 仍然拿得到主动权。 */
  @media (prefers-reduced-motion: reduce) {
    animation: none;
    opacity: 0.34;
  }
`;

/**
 * 三条同心带：从内到外 **2 / 4 / 6px**，用各自独立的 opacity 淡入窗口做出「亮核恒亮、
 * 中/外依次推出、外先收回」的涟漪（级联时间轴见 Ring 顶部【级联呼吸】）。
 *
 * 🔴 **三档必须全是偶数 CSS px**（2026-10-06 用户裁决：先「最细档抬到 2px」，再定「2/4/6」）：
 *   150% 显示缩放下 1 CSS px = **1.5 设备 px**，奇数档的带边界会落在半个设备像素上，
 *   渲染器把矩形两端同向取整 ⇒ 该条带整体平移 0.5 设备 px，环读成「一边粗一边细」
 *   （实测：1px 档时左 1 设备列 / 右 2 设备列，亮度差一倍；3px 档时 4 列 / 5 列）。
 *   奇数档的差值是**奇数个设备像素**，任何 ±0.33px 的位移补偿都只是把偏差换边，改不掉 ——
 *   只有偶数档（2 CSS px = 3 设备 px）能让左右都落在整数设备像素上。
 *   实测见 .worknotes/changes/2026-10-06-qoder-nav-ring-asymmetry-diagnosis.md。
 *   ⚠️ 别"顺手"加回 1px / 3px 档 —— 那不是设计退步，是 150% 下的渲染硬约束。
 *   dpr 1.0 / 2.0 时 1px 是整数设备像素，本问题自动不存在；本工程用户机 = 150%。
 *
 * 🔴 为什么是**填满的矩形**而不是 border 描边：border-width 走布局，每帧重排；
 * 填满 + 外侧盖底（Rail::after）相减得到的可见宽度与描边逐像素等价，却是纯合成层操作。
 *
 * 🔴 绘制顺序即 DOM 顺序，**由外向内**：band6（最宽）先画、band2（最窄）最后画。
 * 反过来（由内向外排）会让最宽的 band6 盖住内两层，三条永远只剩一条 6px 带，
 * 「2px ↔ 6px」两个极值就都读不出来了 —— 这是本实现最容易写反的一处。
 *
 * ⚠️ 静态 opacity = **各自的谷值**，不只是给 reduced-motion 用：它是动画未加载 /
 *   被浏览器忽略时的兜底，语义是「停在最细的那一帧」（2px 亮带），而不是
 *   「停在最粗的那一帧」（6px 暗绿）—— 后者在降级场景下过于抢眼。
 */
const RingBand = styled.span`
  position: absolute;
  pointer-events: none;
  will-change: opacity;

  /* band2 = 最内 2px = **恒亮亮核**（2026-10-06 第四轮）：撤掉动画、opacity 恒 1，
     不再参与呼吸。85% 亮线色从原 band1 挪到这里（第二轮），观感是「一条亮线浮在暗柔边里」；
     **不羽化** —— 它与中档之间那条锐利亮线正是「恒亮核」的视觉主体（外两档已羽化）。
     ⚠️ inset 4 = 环盒外扩 6 − 可见宽 2（见 Ring 顶部『外扩 6px』）；半径 32 = 36 − 4。 */
  &[data-band="2"] {
    inset: 4px;
    border-radius: 32px;
    background: color-mix(in srgb, var(--aegis-accent) 85%, transparent);
    opacity: 1; /* 恒亮：无动画，本值即终值 */
  }

  /* band4 = 4px。停帧 / 兜底时留 0.03 而非 0：全 0 时那条亮带是「浮」在面板上的
     孤立色块，外缘没有任何过渡会显出锯齿；3% 的暗绿相当于给它做一次抗锯齿外扩。
     🔴 注意：动画**运行期间**谷值由 keyframes 决定（= 0），此处的 0.03 只在
        动画未生效时（reduced-motion 停帧 / 动画名失效）起作用。
     34% / 20% 两个色值从 2026-10-06 第二轮沿用至今未动 —— 第四轮只加了**羽化**
     与新的淡入窗口，不再有「三条同相」的约束。 */
  &[data-band="4"] {
    inset: 2px;
    border-radius: 34px;
    background: color-mix(in srgb, var(--aegis-accent) 34%, transparent);
    opacity: 0.03;
    filter: blur(1px); /* 静态羽化：把 4px 分界的硬边化成渐变（见 Ring 顶部 ⓓ） */
    animation: aegis-nav-wave-mid 5.2s infinite;
  }

  &[data-band="6"] {
    inset: 0;
    border-radius: 36px;
    background: color-mix(in srgb, var(--aegis-accent) 20%, transparent);
    opacity: 0;
    filter: blur(2px); /* 同上，外档用更大的羽化半径 */
    animation: aegis-nav-wave-outer 5.2s infinite;
  }

  /* 中/外两档各一条关键帧；错开**写死在关键帧位置**里（理由见 Ring 顶部 ⓒ）。
     时间轴（5.2s，与 RingAura 同周期）：
       mid  ：静息 0–10% → 亮起 10–34% → 恒亮 34–72% → 最后关闭 72–100%
       outer：静息 0–40% → 最后亮起 40–58% → 6px 停 58–68% → 最先关闭 68–88% → 静息 88–100%
     两条都首尾同值（0 / 0）⇒ 循环接缝在数学上不存在。
     平台 / 静息段用 linear 而非继续 ease-in-out：此刻 opacity 恒为 1 / 恒为 0，
     缓动曲线不产生任何视觉变化；写成 linear 是**明示「这段不参与缓动」**，避免后人以为漏配。

     ⚠️ 禁止把两档改回「共用同一条 keyframes + 各自 animation-delay」：两档占空比本就不同，
        delay 表达不了；且 delay 会毁掉首帧静息态（见 Ring 顶部 ⓒ）。 */

  /* ⬇ 元素级 @keyframes —— 与上方 &[...] 属性选择器同层，**不在它们内部**。 */
  @keyframes aegis-nav-wave-mid {
    0% {
      opacity: 0;
      animation-timing-function: linear;
    }
    10% {
      opacity: 0;
      animation-timing-function: cubic-bezier(0.45, 0, 0.55, 1);
    }
    34% {
      opacity: 1;
      animation-timing-function: linear;
    }
    72% {
      opacity: 1;
      animation-timing-function: cubic-bezier(0.45, 0, 0.55, 1);
    }
    100% {
      opacity: 0;
    }
  }

  @keyframes aegis-nav-wave-outer {
    0% {
      opacity: 0;
      animation-timing-function: linear;
    }
    40% {
      opacity: 0;
      animation-timing-function: cubic-bezier(0.45, 0, 0.55, 1);
    }
    58% {
      opacity: 1;
      animation-timing-function: linear;
    }
    68% {
      opacity: 1;
      animation-timing-function: cubic-bezier(0.45, 0, 0.55, 1);
    }
    88% {
      opacity: 0;
      animation-timing-function: linear;
    }
    100% {
      opacity: 0;
    }
  }

  /* 规范 §八：停帧而非压时长（同 RingAura 的理由，ADR-006 §三）。
     各条带的 opacity 全部**显式复位**到各自的静态值 —— 这里若只写 animation: none，
     会回落到各条静态 opacity（本次恰好一致），但那是巧合：静态值与 keyframes 是两处
     真相，改一处另一处不动 ⇒ 静默停在错误亮度。

     🔴🔴 2026-10-05 打磨时用无头Edge + CDP 抓到的**真缺陷**：原来这里只有外层那一行
     animation: none，它在真机上**完全没生效** —— 各条带的 animation 声明写在
     & 属性选择器里（specificity 0,2,0），外层基类是 0,1,0 ⇒ 属性选择器赢。
     连上面那句「显式复位 opacity」也被压过：CSS 动画值的优先级高于普通作者声明，
     动画在跑时opacity 复位不参与计算。⇒ 规范 §八 的无障碍要求实际是空的。
     探针复刻真实结构的实测（Emulation.setEmulatedMedia把 reduced-motion 设为
     reduce，再读 getComputedStyle().animationName；下面两条是**当时四条带**的编号）：
       [data-band=1] animationName = bloom-core   ⇒ 仍在无限动画（应为 none）
       [data-band=2] animationName = bloom        ⇒ 仍在无限动画（应为 none）
       对照组（animation:none 也写进 & 属性选择器）⇒ none  ✅
     修法：把 animation: none 补进 & 属性选择器（与动画声明同特异性、后写者赢）。
     外层那一行**保留**，将来若新增不带 data-band 的动画层时仍有一层网。 */
  @media (prefers-reduced-motion: reduce) {
    animation: none;

    &[data-band] {
      animation: none;
    }

    &[data-band="2"] {
      /* band2 自 2026-10-06 第四轮起无动画，本行只是与静态值保持一致的显式复位
         （留着它，将来若亮核重新参与动画，停帧语义已就位）。 */
      opacity: 1;
    }
    &[data-band="4"] {
      opacity: 0.03;
    }
    &[data-band="6"] {
      opacity: 0;
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
  /* 🔴 悬停底色必须是圆（2026-10-05 用户指出「悬停时图标被方形遮罩盖住」）：
     原写 --aegis-radius-sm(2px)，而 Item 在 64 宽胶囊内是 44×44 的**正方盒**
     （64 − 2×--nav-item-mx(10)），2px 圆角读起来就是方块；激活态绿底用的是
     IndicatorFill 的 --aegis-radius-full ⇒ 形状不一致。两者取同一令牌后
     悬停圆与绿底同心同径（都由 44×44 + full 得出）。 */
  border-radius: var(--aegis-radius-full);
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
        {/* 22px（2026-10-06 用户裁决，原 20）—— 44px 圆形底衬内仍留 11px 边距 */}
        <Icon size={22} strokeWidth={1.5} aria-hidden="true" />
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
  /* 谓词与 BrandBar 标题派生共用 matchModule（modules.ts）——
     「子路由归属哪个模块」只有一处定义（2026-10-06 代码审查 #4，方向=前缀继承）。 */
  const activeId = matchModule(pathname)?.id;
  // （原此处有 `usePageLiveness()` + Rail 的 `data-live`，用于失焦时暂停流光环。
  //   2026-10-04 移除，2026-10-05 宽度呼吸改版后**仍未恢复**（新实现只动 opacity，
  //   暂停省下的基本归零，代价却是「可见未聚焦时环停住像卡死」）。
  //   理由见 Ring 顶部「失焦/不可见时暂停动画」存档段。
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
     故用 ref 读最新值，依赖里不出现 activeId。
     🔴 2026-10-06（代码审查 #2）：ref 同步由「渲染体内直写」改为 useLayoutEffect ——
     渲染期写 ref 在并发渲染下会落进**未提交**的渲染（被丢弃 ⇒ ref 留幽灵值），
     且违反「render 期间 refs 只读」的纪律。改后时序不变：本 effect 声明在几何/动画
     两个 effect 之上 ⇒ 同一次 commit 里先同步 ref；RO 回调是异步的，只会读到
     最新已提交值。依赖 [activeId] 只驱动本 effect，不会波及几何 effect（关键，
     见上段）。 */
  const activeIdRef = useRef(activeId);
  useLayoutEffect(() => {
    activeIdRef.current = activeId;
  }, [activeId]);

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
      {/* 流光环组：外光晕 + 三条同心带（见 Ring 注释）。必须在指示器之前，
          以便靠 z-index 链（光环组 0 < 盖底 1 < 指示器 2 < 图标 3）分层 ——
          盖底层会把每条带伸进胶囊内侧的那半盖掉，露出来的正是 2/4/6px 的环。
          🔴 三条带的 DOM 顺序 = 绘制顺序，**由外向内**（band6 先、band2 后），
             顺序写反会让最宽的带盖住内两层、两个极值都读不出来。 */}
      <Ring aria-hidden>
        <RingAura />
        <RingBand data-band="6" />
        <RingBand data-band="4" />
        <RingBand data-band="2" />
      </Ring>
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
