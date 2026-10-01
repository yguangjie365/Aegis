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

const Frame = styled.div`
  min-height: 100vh;
  display: flex;
  flex-direction: column;
  background: var(--aegis-canvas);
  color: var(--aegis-ink);
`;

const Middle = styled.div`
  flex: 1;
  display: flex;
  min-height: 0;
`;

const Main = styled.main`
  flex: 1;
  overflow: auto;
  padding: 20px 24px 28px;
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
