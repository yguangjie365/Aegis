import { useEffect } from "react";
import { Outlet } from "react-router-dom";
import styled from "@emotion/styled";
import { revealMainWindow } from "@/lib/tauri/window";
import { WindowOverlay } from "./window-overlay";
import { NavRail } from "./nav-rail";

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

/* 🔴 安全区 / 隐形框架（2026-10-04 用户裁决）
   全屏化之后内容会顶到视口四边，与「窗口覆盖层」（左上状态点、右上窗口按钮）以及
   页面自身的头像/徽标等元素抢同一块地方。处置：外框统一内缩，四边再叠加
   env(safe-area-inset-*) 兼容刘海屏与底部指示条；顶部内缩量显式 ≥ 覆盖层高度
   （--aegis-dragstrip-h = 32），保证任何页面的首行内容都不会被覆盖层压住。
   注意：覆盖层本身是 fixed 定位，**不受本 padding 影响**，所以两者的高度关系必须
   在这里用同一个变量对齐。 */
const SAFE_TOP = "calc(var(--aegis-dragstrip-h) + 10px + env(safe-area-inset-top, 0px))";
const SAFE_RIGHT = "calc(16px + env(safe-area-inset-right, 0px))";
const SAFE_BOTTOM = "calc(24px + env(safe-area-inset-bottom, 0px))";
const SAFE_LEFT = "calc(16px + env(safe-area-inset-left, 0px))";

const Frame = styled.div`
  height: 100vh;
  overflow: hidden;
  display: flex;
  flex-direction: column;
  padding: ${SAFE_TOP} ${SAFE_RIGHT} ${SAFE_BOTTOM} ${SAFE_LEFT};
  background: var(--aegis-canvas);
  color: var(--aegis-ink);
`;

// flex-basis:0 + min-height:0 → Middle 严格等于视口高度，不再被内容高度撑开
// （这是 flex 布局中避免内容撑破视口的标准写法）。
const Middle = styled.div`
  flex: 1 1 0;
  display: flex;
  min-height: 0;
  /* 🔴 2026-10-04 不能用 overflow: hidden。
     NavRail 的流光环是 Rail::before，向四周外扩 3.5px；而本容器的裁剪边界恰好
     与 Rail 左边缘重合（两者 x 都是 Frame 的 SAFE_LEFT=16）—— 左边那 3.5px 落在
     裁剪区之外被整条切掉，右边因容器有余量所以完好。实测（左缘绿度 0 / 右缘 40）
     表现为「只有右半个环」。去掉裁剪即对称；横向溢出改由 Main 的
     overflow: auto + min-width: 0 承担（Rail 是 flex:none，撑不开容器）。 */
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
  /* 左侧 padding 与 Frame 的 SAFE_LEFT(16px) 取齐 → 「窗口左边框 → 胶囊」与
     「胶囊 → 内容」两侧留白严格相等，视觉不偏重（2026-10-04 用户裁决：导航栏要真正居中）。
     其余方向只留极小呼吸间距，外边距统一由外框提供。 */
  padding: 2px 2px 0 16px;

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
      <Middle>
        <NavRail />
        <Main>
          <Outlet />
        </Main>
      </Middle>
      {/* 覆盖层：拖动区 + 最小化/关闭。不占布局（fixed），故放在 Middle 之后不影响 flex 计算 */}
      <WindowOverlay />
    </Frame>
  );
}
