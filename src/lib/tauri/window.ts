// 窗口壳接线：tauri.conf.json 主窗 visible:false —— 首帧就绪后由 Shell 显形，避免 WebView 白闪。
// 本文件属于 lib/tauri 包装层（04 分册 §一「前端不触引擎」的白名单侧）；业务代码不得绕开此处裸调窗口 API。

import { getCurrentWindow } from "@tauri-apps/api/window";
import { isTauriRuntime } from "./invoke";

/** 浏览器预览模式下为无害空操作（与 invoke 包装器的显式抛错策略不同：显形是纯宿主动作，无后端语义）。 */
export function revealMainWindow(): void {
  if (!isTauriRuntime()) return;
  const win = getCurrentWindow();
  void win.show().then(() => win.setFocus());
}

/**
 * 订阅窗口聚焦变化（M-01 R-19：实时节拍的启停信号）。
 *
 * 返回退订函数。🔔 只报"焦点"，不含"是否最小化"——宿主二者都走这条事件，
 * 但文档可见性仍需 `visibilitychange` 兜底，兜底侧在 `lib/timing/use-tick.ts`。
 * 浏览器预览模式退回 DOM 的 focus/blur（无原生窗口）。
 */
export function onWindowFocusChanged(cb: (focused: boolean) => void): () => void {
  if (!isTauriRuntime()) {
    const onFocus = (): void => cb(true);
    const onBlur = (): void => cb(false);
    window.addEventListener("focus", onFocus);
    window.addEventListener("blur", onBlur);
    return () => {
      window.removeEventListener("focus", onFocus);
      window.removeEventListener("blur", onBlur);
    };
  }
  const win = getCurrentWindow();
  let unlisten: (() => void) | undefined;
  let disposed = false;
  void win
    .onFocusChanged((event) => cb(event.payload))
    .then((dispose) => {
      if (disposed) dispose();
      else unlisten = dispose;
    });
  return () => {
    disposed = true;
    unlisten?.();
  };
}
