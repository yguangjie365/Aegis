import { useEffect } from "react";
import { Outlet } from "react-router-dom";
import { useTranslation } from "react-i18next";
import styled from "@emotion/styled";
import { revealMainWindow } from "@/lib/tauri/window";
import { MiniMusicPlayer } from "@/shared";
import { WindowOverlay } from "./window-overlay";
import { NavRail } from "./nav-rail";
import { BrandBar } from "./brand-bar";

// 🔴 2026-10-04 壳层改版（用户裁决：主界面参照 NexBox 形态）
// 变更要点（属规范 §三 / §四 变更，登记见 TASKS Q-27）：
//   ① TitleBar 不再是 Grid 文档流行 → WindowOverlay 以 `position: fixed` 覆盖层承载
//      「拖动区 + 窗口控制」，**不占布局**，主内容区拿到完整视口高度（NexBox 同款做法）。
//   ② StatusBar 整体移除 → 版本 + 零遥测声明迁入「设置 > 关于」（合规红线不丢、常驻改常在），
//      运行态（就绪 / 浏览器预览）并入覆盖层左侧一个状态点。
//   ③ NavRail 由「贴左边缘满高黑列」改为「悬浮胶囊」：左右上下留边距、两端全圆角。
// 旧实现留档：原为「Frame(height:100vh) > TitleBar(32) + Middle(flex:1) + StatusBar(26)」
// 与 `min-height:100vh` 的坑（见 git 历史）—— 现在 Frame 直接就是 Middle 的父级，
// 高度由 100vh 锁死，浮动层/覆盖层都不参与流式布局。

/* 🔴 垂直几何（2026-10-05 落地 vertical-geometry-spec.md §七 迁移八条 之 ②⑧）
   四段垂直契约：拖动条 32 + 品牌区 72 + 隐形框架 664 + 底部留白 32 = 800。

   本文件只管 Frame 的**安全区**与**内容列结构**；拖动条本体在 window-overlay.tsx
   （已由 fixed 覆盖层改为进文档流），品牌区本体在 brand-bar.tsx。

   🔴 SAFE_TOP 的**双重计算陷阱**（此前踩过）：拖动条改为文档流之后，它已经自己占 32，
   Frame 顶部内缩必须**归零**，否则内容会多出一段 32 的空隙。
   🔴 底部同理：底部留白 32 由 Frame 的 padding-bottom 提供（不再有独立的 safe-bottom）。
   🔴 硬下限：SAFE_LEFT / SAFE_RIGHT 必须 ≥ 流光环外扩 4 + box-shadow 光晕 8 = **12px**，
   否则 Ring 的光晕被 Frame 的 overflow:hidden 削成断口。当前 20 ⇒ 环外缘距窗口边 16px、
   光晕外缘距边 8px，余量 8px。改这个值时必须同改 nav-rail.tsx 的 Ring inset。 */
const SAFE_RIGHT = "calc(20px + env(safe-area-inset-right, 0px))";
/** 底部纯留白（规范 §六：不是状态栏条，无底色无内容无交互）*/
const FRAME_PAD_BOTTOM = "calc(32px + env(safe-area-inset-bottom, 0px))";
const SAFE_LEFT = "calc(20px + env(safe-area-inset-left, 0px))";

const Frame = styled.div`
  height: 100vh;
  overflow: hidden;
  display: flex;
  flex-direction: column;
  /* 🔴 顶部内缩 0：拖动条（window-overlay.tsx）已进文档流并自己占 --aegis-dragstrip-h，
     顶部再留一段就是双重计算。左右 20 是「距边 20」（量胶囊本体，环外缘 16 跑在安全区内）。 */
  padding: 0 ${SAFE_RIGHT} ${FRAME_PAD_BOTTOM} ${SAFE_LEFT};
  background: var(--aegis-canvas);
  color: var(--aegis-ink);
`;

// flex-basis:0 + min-height:0 → Middle 严格等于视口高度，不再被内容高度撑开
// （这是 flex 布局中避免内容撑破视口的标准写法）。
const Middle = styled.div`
  flex: 1 1 0;
  display: flex;
  min-height: 0;
  /* 🔴 2026-10-04 曾想用 overflow: hidden，但不能：
     NavRail 的流光环（Ring）是 inset:-4px，向四周外扩 4px；裁剪边界若与 Rail 左边缘
     重合，左边那 4px 会被整条切掉（实测表现为「只有右半个环」）。去掉裁剪即对称；
     横向溢出改由 Main 的 overflow: auto + min-width: 0 承担（Rail 是 flex:none）。
     纵向不裁剪同样安全：底部留白 32 与品牌区都是文档流内的高度，不依赖溢出可见。 */
`;

/** 内容列（迁移表第 4 条）：品牌区 + Main 竖排，横向占满 Middle 余量 */
const Column = styled.div`
  flex: 1 1 0;
  min-width: 0;
  min-height: 0;
  display: flex;
  flex-direction: column;
`;

// 🔴 Main 改为 flex 容器：音乐页等子页用 h="100%" 跟随父容器高度，父容器必须有确定高度。
// flex:1 已给出确定高度，子页的 h="100%" 才能解析成实际像素。
const Main = styled.main`
  flex: 1 1 0;
  min-height: 0;
  min-width: 0;
  display: flex;
  flex-direction: column;
  overflow: auto;
  /* ── 水平几何（2026-10-04 裁决「左右对称 20 / 20」，Q5 结案；10-05 复核**维持原判**）──
     启动窗口 1280 逐段拆开：
       窗口 [0,1280] →SAFE_LEFT 20→ 胶囊 [20,84] →20→ 内容 [104,1260] →SAFE_RIGHT 20→ 窗口右缘
     内容宽 = 1280 − 20 − 64 − 20 − 20 = **1156**（用户给的算式，不可改）。
     · padding-left 20：Main 盒左缘 = 胶囊右缘 84，故「胶囊 → 内容」留白 = 20，与
       「窗口左边框 → 胶囊」的 SAFE_LEFT 20 严格相等 ⇒ 胶囊在视觉上真正居中。
     🔴 **2026-10-05 血的教训（我改错又改回）**：
       曾把本行 padding-left 改 0（想让标题与内容贴边框），**结果同时破坏两条**：
       ① 取消「胶囊 → 内容」的 20 呼吸 ⇒ 卡片贴住胶囊（用户截图指出）；
       ② 内容宽 1156 → 1176，违反上面那条算式。
       而「标题与内容左缘对齐」**本来就是对的** —— BrandBar 有 padding-left 20（标题在 104）、
       Main 也有 padding-left 20（内容在 104），两者天然同在 104。
       ⇒ **「对齐」不等于「都改成 0」**。别把已对齐的东西拆开。
     · 胶囊外缘的流光环外扩 4px ⇒ 环外缘 [16,88]，box-shadow 光晕再 8px ⇒ [8,96]，
       两侧都落在窗口内（Frame 无裁切），环与光晕完整可见。
     · padding-right **0**：内容右缘 = Main 盒右缘 = Frame 内容右缘 = 1280−20。
       右边距**只有 SAFE_RIGHT 一个来源**，不再有多层叠加（此前是 2px，与左边 16px
       不对称，正是 Q5 的来源）。
     🔴 scrollbar-gutter: stable 与本行 padding 的关系（2026-10-05 实测澄清，与旧注释相反）：
       原以为「启用 gutter 会把内容宽从 1156 压到 1141」，故建议补 15px 右 padding。
       **实测结论相反**：Windows 经典滚动条**现在就占 15px**（注入探针实测：
         可滚态 clientW=**1161**、不可滚态 **1176**，差 15px）⇒
         · 可滚页面的**现状**内容宽已经是 1161，不是 1156；
         · 加 scrollbar-gutter: stable 后恒定 1161 = **与现状可滚态一致** ⇒ **无需补 padding**；
         · 反之若隐藏滚动条但**不加** gutter，内容会**变宽 15px 到 1176** ⇒ 卡片网格多排一列。
       ⇒ 本行 padding 保持 0（右边距仍只有 SAFE_RIGHT 一个来源，见上）。 */
  padding: 12px 0 0 20px; /* 🔴 padding-top 12（迁移表第 5 条，Q1 裁决）：品牌区底 → 内容顶的呼吸。
     原值 2 等于没有 —— 头重脚轻，且与页面 gap（--aegis-space-lg = 16）不同值。 */

  /* == 隐藏纵向滚动条（2026-10-05 用户裁决，方案见 .worknotes/scrollbar-plan.md）==
     四行属性写全，不做 UA 判断（纯 CSS 渐进增强）：
       scrollbar-width: none          —— Firefox / 标准
       -ms-overflow-style: none       —— 旧 Edge（WebView2 已 Chromium，可省但无害）
       overscroll-behavior-y: contain —— 只抑边界回弹，**不改滚轮语义**
       + ::-webkit-scrollbar 宽高归零  —— Chromium / WebKit（本项目主路径）
     ⚠️ **刻意不用 display: none**：Chromium 下 ::-webkit-scrollbar{display:none}
        会让该元素**变成不可滚动容器**（滚轮失效）—— 那是 NexBox 原 index.css:66-72
        的写法，本项目移除全局规则时**未验证过重新加回来的后果**。width:0 才是对的。
     🔴 scrollbar-gutter: stable 是**必须**的，不是可选项：见上方「与本行 padding 的关系」
        实测 —— 不加则内容宽在可滚/不可滚之间跳 15px，卡片网格会变列、居中布局会抖。
     ⚠️ 本注释位于模板字符串内部，**禁止出现反引号** —— 会提前终止 template literal
        报 TS1005（本项目已踩 6 次，同族见长期笔记）。 */
  scrollbar-width: none;
  -ms-overflow-style: none;
  overscroll-behavior-y: contain;
  scrollbar-gutter: stable;

  &::-webkit-scrollbar {
    width: 0;
    height: 0; /* 横向也一并（本项目 overflow-x 已 clip，无横向滚动） */
  }

  > * {
    flex: 1 1 0;
    min-height: 0;
    min-width: 0;
  }
`;

export function ShellLayout() {
  const { t } = useTranslation("shell");
  useEffect(() => {
    revealMainWindow();
  }, []);
  return (
    <Frame>
      {/* ① 拖动条（迁移表第 1 条）：已由 fixed 覆盖层改为**进文档流**，自己占 32。
          顺序很重要 —— 它必须是最先那个子元素，否则 Middle 会把它算进自己的高度。 */}
      <WindowOverlay />
      <Middle>
        <NavRail />
        {/* 内容列 = 品牌区 + Main（迁移表第 4 条） */}
        <Column>
          <BrandBar />
          {/* 🔴 隐藏滚动条的无障碍补偿（方案 §3.6）：滚动条是视觉提示，隐藏后必须
             保留「可滚动」的可发现性 ⇒ 给它 region 语义 + 可编程聚焦。
             `tabIndex={-1}` 使其**不进入 Tab 序列**（不污染焦点顺序），
             但屏幕阅读器与「跳到主内容」类操作仍能定位到这里，PageDown/方向键可滚动。 */}
          <Main tabIndex={-1} role="region" aria-label={t("mainRegionLabel", { ns: "shell" })}>
            <Outlet />
          </Main>
        </Column>
      </Middle>
      {/* 🔴 迷你播放器（左下角悬浮胶囊）—— 2026-10-05 挂载。
          *此前它**从未被挂载**（组件零引用 + MusicProvider 未挂载）⇒ 永不显示。
          挂载点选Frame 内的**最后一位**、Middle 之外：
            它是 `position: fixed` 的全局浮层（播放中在任意页面都应可见），
            放在 Middle 内会继承内容列的定位上下文，且语义上不属于「内容列」。
          ⚠️ 依赖 `@/shared` 门面而非直接 import 复刻域（跨 feature 共享纪律，见门面注释）。
          ⚠️ 组件自身 `if (!isPlaying || !currentSong) return null` ⇒ 空闲时不占布局。 */}
      <MiniMusicPlayer />
    </Frame>
  );
}
