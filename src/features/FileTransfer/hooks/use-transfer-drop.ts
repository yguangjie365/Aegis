// 接收系统拖放（M-05 片1 的"拖到设备行发送"）。
//
// 🔴 本文件的每一条纪律都来自 K-S01 的**实测**（`spikes/K-S01-drag-drop.md` §四）：
//
// 1. **DOM 收不到拖放**（`dragDropEnabled: true` 时 wry 在 OS 层拦截，DOM 零事件）
//    ⇒ 只能走 Tauri 的 `onDragDropEvent`，不能用 HTML5 的 `onDrop`。
// 2. **一次拖放投递两条完全相同的 drop 事件**（两个 emit 目标都匹配 `listen()` 的默认目标，
//    官方 `onDragDropEvent` **也不规避**）⇒ 必须自己去重，否则**一次拖放入队两个任务**。
// 3. **`drag-over` 约 238 次/秒**（≈ 每帧 4 次，每次跨 IPC）⇒ 绝不能逐次 `setState`。
// 4. 位置是**物理像素**，DOM 是 CSS 像素 ⇒ 命中测试要按 `devicePixelRatio` 换算。

import { useEffect, useRef, useState } from "react";
import { getCurrentWebview } from "@tauri-apps/api/webview";
import { dropKey, hitTestDropRow, isDuplicateDrop, type DropRowRect } from "../lib/drop-target";
// 🔴 2026-10-07：降级路径原本只写 console ⇒ 不进八字段 JSONL，
//   用户报障「拖放发送不可用」时日志里查不到痕迹。改走 `log()`。
import { log } from "@/lib/logging";

/** 拖放回调（由页面提供）。 */
export interface TransferDropHandlers {
  /**
   * 现取当前可投递的设备行（**每次事件现取**）。
   * 不缓存是刻意的：列表会随发现/离线变化，缓存的矩形会把文件发到已经消失的行上。
   */
  rows: () => DropRowRect[];
  /** 命中某设备行（`paths` 是原生**绝对路径**）。 */
  onDropOnRow: (rowId: string, paths: string[]) => void;
  /** 未命中任何行（R-7：必须提示用户"请拖到目标设备行上"，**不许**回退到选中行）。 */
  onDropOutside: (paths: string[]) => void;
}

/**
 * 订阅系统拖放；返回当前高亮行（`null` = 无）。
 *
 * 🔴 高亮只在**命中的行发生变化**时才 `setState`：`over` 每秒来 238 次，
 * 逐次渲染会让整页在拖拽期间以 4 倍帧率重渲染。
 */
export function useTransferDrop(handlers: TransferDropHandlers): { hoveredRowId: string | null } {
  const [hoveredRowId, setHoveredRowId] = useState<string | null>(null);
  const hoveredRef = useRef<string | null>(null);
  const lastKeyRef = useRef<string | null>(null);
  const lastHandledAtRef = useRef(0);

  // 回调放进 ref：监听器只注册一次，但每次事件都调用**最新的**回调
  // （否则 `rows()` 会闭包住首次渲染时的 ref 集合，永远看不到后来出现的设备行）
  const handlersRef = useRef(handlers);
  handlersRef.current = handlers;

  useEffect(() => {
    let unlisten: (() => void) | undefined;
    let disposed = false;

    const setHovered = (next: string | null): void => {
      if (hoveredRef.current === next) return; // 🔴 去抖的关键一行
      hoveredRef.current = next;
      setHoveredRowId(next);
    };

    void (async () => {
      try {
        const stop = await getCurrentWebview().onDragDropEvent((event) => {
          const payload = event.payload;
          const dpr = window.devicePixelRatio;

          if (payload.type === "enter" || payload.type === "over") {
            // 🔴 只比位置、只在高亮行变化时渲染（见函数头第 3 条）
            setHovered(hitTestDropRow(handlersRef.current.rows(), payload.position, dpr));
            return;
          }
          if (payload.type === "leave") {
            setHovered(null);
            return;
          }

          // ── drop ──
          // 🔴 去重：一次拖放会来两条相同的 drop（见函数头第 2 条）
          const key = dropKey(payload.paths, payload.position);
          const now = performance.now(); // 单调时钟：不受系统时间被改的影响
          if (isDuplicateDrop(lastKeyRef.current, lastHandledAtRef.current, key, now)) {
            return;
          }
          lastKeyRef.current = key;
          lastHandledAtRef.current = now;

          setHovered(null);
          // 用**落点**判目标，而不是用"当前高亮"：高亮可能因 `leave` 丢过一次，
          // 而落点由这条 drop 事件自带，是权威来源（R-7：目标来自"拖到哪"，不是"选中了哪"）
          const hit = hitTestDropRow(handlersRef.current.rows(), payload.position, dpr);
          if (hit === null) {
            handlersRef.current.onDropOutside(payload.paths);
          } else {
            handlersRef.current.onDropOnRow(hit, payload.paths);
          }
        });
        if (disposed) {
          stop(); // 竞态：effect 已清理时才拿到 unlisten
        } else {
          unlisten = stop;
        }
      } catch (error) {
        // 拿不到监听器（不在 Tauri 运行时内、或权限未开）不是致命错误：
        // 拖放只是**快捷方式**，"发送文件"按钮始终可用。
        // 但必须留痕——静默 catch 会让"拖放没反应"完全无从排查。
        log({
          level: "warn",
          module: "Transfer",
          action: "subscribe-system-drop",
          result: "Failed",
          exception: error instanceof Error ? error.message : String(error),
        });
      }
    })();

    return () => {
      disposed = true;
      unlisten?.();
    };
  }, []);

  return { hoveredRowId };
}
