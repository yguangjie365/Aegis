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
