// 刷新节拍策略（M-01 F-08 / R-19，2026-10-03）。
//
// 🔴 规格口径：节拍的启停**完全**由「是否停留本页 × 窗口是否在前台」决定，
// 不存在全局常驻定时器（R-19）。因此本文件只提供"当前该不该跳表"的判定，
// 真正的定时器交给 TanStack Query 的 `refetchInterval`——它在 `false` 时彻底清表，
// 比再造一个 setInterval 少一份状态（且切页卸载即自然停）。

import { useEffect, useState } from "react";
import { onWindowFocusChanged } from "@/lib/tauri/window";

/** N-01：实时数据节拍固定 2 秒 */
export const REFRESH_INTERVAL_MS = 2000;

/**
 * 本页是否处于"可跳表"状态：**窗口聚焦** 且 **文档可见**。
 *
 * 两个信号缺一不可（缺一个就会出现"看不见的常驻轮询"）：
 * - 切页停表靠的是**组件卸载**（TanStack Query 在 `refetchInterval: false`
 *   时彻底清表，本页组件随路由卸载即停），**不需要**在这里判路由 ——
 *   反过来做会引入 `useLocation` 依赖与无意义的样板代码。本仓无 keep-alive，
 *   故卸载即等价于"不在本页"。
 * - 聚焦：仅 `visibilitychange` 拦不住"窗口在前景但被别的程序盖住"（WebView 仍算可见）。
 * - 可见：仅聚焦事件拦不住最小化到任务栏后部分平台不发 blur 的情形。
 *
 * 🔴 三个状态初值都为"活跃"：取 false 会让首拍延后一个事件周期，
 * 与 F-01「进入页面时先显示采集值」相悖。
 */
export function usePageLiveness(): boolean {
  const [focused, setFocused] = useState(true);
  const [visible, setVisible] = useState(() => document.visibilityState !== "hidden");

  useEffect(() => onWindowFocusChanged(setFocused), []);

  useEffect(() => {
    const onChange = (): void => setVisible(document.visibilityState !== "hidden");
    document.addEventListener("visibilitychange", onChange);
    return () => document.removeEventListener("visibilitychange", onChange);
  }, []);

  return focused && visible;
}
