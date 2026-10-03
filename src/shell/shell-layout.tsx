import { useEffect } from "react";
import { Outlet } from "react-router-dom";
import styled from "@emotion/styled";
import { revealMainWindow } from "@/lib/tauri/window";
import { TitleBar } from "./title-bar";
import { NavRail } from "./nav-rail";
import { StatusBar } from "./status-bar";

// 方案 A 外壳：双色模竖向双栏。纵向 32 + 内容 + 26，横向 [192|64] + 内容。
// 窗口缩放机制（§四 Scale=clamp(min(W/1440,H/900),.75,2)，无断点无重排）依赖 Tauri 窗口事件，
// 列入 W1 接线；浏览器预览模式下为 100% 原始尺寸。

// 🔴 阶段一 100% 复刻配套（2026-10-03）
// 原为 min-height:100vh：min-height 不给确定高度，flex 子项（MiddLe/ Main）的高度
// 会被内容撑开 —— 音乐页内含 100+ 首曲目的 VirtualList 时，整页被拉到远超视口，
// 表现为「页面被拉长 + 底部播放条被推到视口外」。
// 改为 height:100vh + overflow:hidden：外壳锁死为视口高度，溢出交给内部各自滚动
// （Main 已 overflow:hidden、音乐页根容器已 overflow:hidden、VirtualList 内部滚动）。
const Frame = styled.div`
  height: 100vh;
  overflow: hidden;
  display: flex;
  flex-direction: column;
  background: var(--aegis-canvas);
  color: var(--aegis-ink);
`;

// flex-basis:0 + min-height:0 → Middle 严格等于「视口 - TitleBar - StatusBar」，
// 不再被内容高度撑开（这是 flex 布局中避免内容撑破视口的标准写法）。
const Middle = styled.div`
  flex: 1 1 0;
  display: flex;
  min-height: 0;
  overflow: hidden;
`;

// 🔴 阶段一 100% 复刻配套（2026-10-03）：音乐页（NexBox MusicPage 原件）用 h="100%"
// 跟随父容器高度（原件是 h="calc(100vh - 120px)"，按参照工程「无外壳全屏页」写的）。
// 百分比高度要求父容器有确定高度，故 Main 改为 flex 容器：flex:1 已给出确定高度，
// 子页的 h="100%" 才能解析成实际像素；同时保留 overflow:auto 与原 padding 不变。
// 该改动只影响**子页高度解析方式**，不影响外壳本身的视觉与交互。
const Main = styled.main`
  flex: 1 1 0;
  min-height: 0;
  min-width: 0;
  display: flex;
  flex-direction: column;
  overflow: auto;
  padding: 20px 24px 28px;

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
      <TitleBar />
      <Middle>
        <NavRail />
        <Main>
          <Outlet />
        </Main>
      </Middle>
      <StatusBar />
    </Frame>
  );
}
