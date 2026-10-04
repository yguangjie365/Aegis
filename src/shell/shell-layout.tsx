import { useEffect } from "react";
import { Outlet } from "react-router-dom";
import styled from "@emotion/styled";
import { revealMainWindow } from "@/lib/tauri/window";
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
  /* ── 水平几何（2026-10-04 用户裁决：左右对称 20 / 20，Q5 结案）─────────────
     以启动窗口 1280 宽为例，逐段拆开：
       窗口 [0,1280] →SAFE_LEFT 20→ 胶囊 [20,84] →20→ 内容 [104,1260] →SAFE_RIGHT 20→ 窗口右缘
     内容宽 = 1280 − 20 − 64 − 20 − 20 = **1156**（即用户给的算式）。
     · padding-left 20：Main 盒左缘 = 胶囊右缘 84，故「胶囊 → 内容」留白 = 20，与
       「窗口左边框 → 胶囊」的 SAFE_LEFT 20 严格相等 ⇒ 胶囊在视觉上真正居中。
     · 胶囊外缘的流光环外扩 4px ⇒ 环外缘 [16,88]，box-shadow 光晕再 8px ⇒ [8,96]，
       两侧都落在窗口内（Frame 无裁切），环与光晕完整可见。
     · padding-right **0**：内容右缘 = Main 盒右缘 = Frame 内容右缘 = 1280−20。
       右边距**只有 SAFE_RIGHT 一个来源**，不再有多层叠加（此前是 2px，与左边 16px
       不对称，正是 Q5 的来源）。
     🔴 若日后启用「隐藏滚动条 + scrollbar-gutter: stable」（方案 D 的 Q2），
       本行需要补 ≥15px 右 padding 给 gutter 让位，否则 gutter 会把内容宽从
       1156 压到 1141 —— 列数仍够（概览 4 列 / 游戏 3 列），但 1156 这个数要重算。 */
  /* 🔴 padding-top 12（迁移表第 5 条，Q1 裁决）：品牌区底 → 内容顶的呼吸。
     原值 2 等于没有 —— 头重脚轻，且与页面 gap（--aegis-space-lg = 16）不同值。 */
  padding: 12px 0 0 20px;

  > * {
    flex: 1 1 0;
    min-height: 0;
    min-width: 0;
  }
`;

export function ShellLayout() {
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
          <Main>
            <Outlet />
          </Main>
        </Column>
      </Middle>
    </Frame>
  );
}
