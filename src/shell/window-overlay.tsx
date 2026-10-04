import { useCallback, useEffect, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import styled from "@emotion/styled";
import { Minus, X } from "lucide-react";
import { isTauriRuntime } from "@/lib/tauri/invoke";

// ── 窗口拖动条（2026-10-05 由 fixed 覆盖层改为进文档流）──────────────────────
// 参照 NexBox `src/components/ui/title-bar.tsx`：透明、只承载拖动区 + 窗口控制。
//
// 🔴 2026-10-05 变更（vertical-geometry-spec.md §七 迁移表第 1 条）：
//   旧：`position: fixed` 覆盖层，**不占布局** ⇒ Frame 的 SAFE_TOP 必须写
//       `calc(dragstrip-h + 10px)` 来给整条内容让出空间。
//   新：**进文档流**，自己占 --aegis-dragstrip-h(32) ⇒ Frame 顶部内缩归零，
//       否则内容会多出一段 32 的空隙（双重计算）。
//   代价：拖动条与内容不再重叠，所以「拖动区压住首行内容」的问题自然消失，
//         指针事件也不需要像旧版那样逐处豁免。
//
// 与更早的 Grid 版 TitleBar 的差异：
//   更早：Grid 文档流 32px 黑条（占高度、有底色、显示 AEGIS 大字）
//   现在：文档流 32px **透明**条（占高度、无底色、左侧仅一个状态点）
//
// 拖动用 JS onMouseDown + getCurrentWindow().startDragging()（NexBox 同款），
// 而非 data-tauri-drag-region：后者无法排除交互元素，点在按钮上会误启动拖动。

const Overlay = styled.header`
  /* 🔴 进文档流：flex:none 保证不被 Frame 的 flex 布局压缩，且高度就是 32。 */
  flex: none;
  height: var(--aegis-dragstrip-h);
  display: flex;
  align-items: center;
  gap: 10px;
  /* 透明：视觉上「没有标题栏」，但整条都是拖动区 */
  background: transparent;
  color: var(--aegis-on-dark-mute);
  font-size: 11px;
  user-select: none;
  -webkit-user-select: none;
  /* 🔴 左右内缩与 Frame 的 SAFE_LEFT/SAFE_RIGHT 同为 20 —— 窗口键的右缘
     必须与内容区右缘（1280−20）对齐，否则「距边 20」在壳层出现两个口径。
     同一组数值在 shell-layout 的 SAFE_LEFT / SAFE_RIGHT，改一处必须同改另一处。 */
  padding: 0 20px;
`;

const Caps = styled.div`
  margin-left: auto;
  display: flex;
  pointer-events: auto;
`;

const CapBtn = styled.button<{ $danger?: boolean }>`
  width: 40px;
  height: var(--aegis-dragstrip-h);
  display: flex;
  align-items: center;
  justify-content: center;
  border: 0;
  background: transparent;
  color: var(--aegis-on-dark-mute);
  cursor: pointer;
  transition:
    background-color var(--aegis-motion-duration-fast) var(--aegis-motion-easing-standard),
    color var(--aegis-motion-duration-fast) var(--aegis-motion-easing-standard);
  &:focus-visible {
    outline: 2px solid var(--aegis-accent);
    outline-offset: -2px;
  }
  &:hover:not(:disabled) {
    background: ${(p) => (p.$danger ? "var(--aegis-error)" : "var(--aegis-chrome-elevated)")};
    color: var(--aegis-on-dark);
  }
  &:disabled {
    cursor: default;
    color: var(--aegis-disabled);
  }
`;

export function WindowOverlay() {
  const { t } = useTranslation("shell");
  const live = isTauriRuntime();
  const stripRef = useRef<HTMLElement | null>(null);
  const [dragging, setDragging] = useState(false);

  /* 拖动 + 双击最大化：一次按下，两个动作，靠**手动判定**而非 dblclick 事件。

     为什么不用 onDoubleClick（上一版就是这么写的，用户反馈"依然没生效"）：
       startDragging() 进入系统拖动循环后，鼠标事件被系统捕获，浏览器**不再派发
       dblclick**；而双击的第二次 mousedown 恰好落在拖动循环内 → 事件永远到不了页面。
     所以改为：mousedown 时自己比对「距上次按下的时间 + 位移」，命中即当作双击，
     绝不启动拖动；否则**零延迟**立即启动拖动（不牺牲手感）。

     判定阈值：400ms（Windows DoubleClickTime 默认上限）/ 6px 位移容差，
     与系统双击语义一致。排除清单沿用 NexBox（button / input / a）。 */
  const lastDown = useRef({ t: 0, x: 0, y: 0 });
  const DOUBLE_MS = 400;
  const DOUBLE_SLOP = 6;

  const onMouseDown = useCallback(
    (e: React.MouseEvent) => {
      if (!live) return;
      if (e.button !== 0) return;
      const target = e.target as HTMLElement;
      if (target.closest("button") || target.closest("input") || target.closest("a")) return;
      const now = Date.now();
      const p = lastDown.current;
      const isDouble =
        now - p.t < DOUBLE_MS &&
        Math.abs(e.clientX - p.x) < DOUBLE_SLOP &&
        Math.abs(e.clientY - p.y) < DOUBLE_SLOP;
      p.t = now;
      p.x = e.clientX;
      p.y = e.clientY;
      if (isDouble) {
        e.preventDefault();
        void (async () => {
          try {
            const { getCurrentWindow } = await import("@tauri-apps/api/window");
            await getCurrentWindow().toggleMaximize();
          } catch (err) {
            /* 失败的两个已查实原因（都不是代码逻辑问题）：
               ① capabilities/default.json 缺 core:window:allow-toggle-maximize —— Tauri 直接拒绝；
               ② tauri.conf.json 的 maximizable: false。
               两者都需改配置后**重启进程**才生效（配置不热重载）。 */
            console.error(
              "[window-overlay] toggleMaximize 被拒：检查 capabilities 的 allow-toggle-maximize 与 maximizable 配置，并重启进程",
              err,
            );
          }
        })();
        return;
      }
      void (async () => {
        try {
          setDragging(true);
          const { getCurrentWindow } = await import("@tauri-apps/api/window");
          await getCurrentWindow().startDragging();
        } catch (err) {
          console.error("[window-overlay] startDragging 失败", err);
        } finally {
          setDragging(false);
        }
      })();
    },
    [live],
  );

  /* 覆盖层本身不吃指针（除按钮区），所以只有指针进入这一条时才可能拖动。
     这里额外兜一层：dblclick 不做 maximize（§四 禁最大化）。 */
  useEffect(() => {
    const el = stripRef.current;
    if (!el) return;
    const prevent = (e: Event) => {
      if (dragging) e.preventDefault();
    };
    el.addEventListener("dragstart", prevent);
    return () => el.removeEventListener("dragstart", prevent);
  }, [dragging]);

  return (
    <Overlay ref={stripRef} onMouseDown={onMouseDown} aria-label={t("window.overlayAria")}>
      <Caps>
        <CapBtn
          type="button"
          aria-label={t("window.minimize")}
          title={live ? t("window.minimize") : t("status.browserPreview")}
          disabled={!live}
          onClick={() => {
            void import("@tauri-apps/api/window").then((m) => m.getCurrentWindow().minimize());
          }}
        >
          <Minus size={14} strokeWidth={1.5} aria-hidden="true" />
        </CapBtn>
        {/* 最大化 / 还原：与双击同一动作（2026-10-04 用户要求「双击最大化等原有交互」），
            旧「恒灰不可点」实现已下线；系统侧开关见 tauri.conf.json 的 maximizable */}
        <CapBtn
          type="button"
          aria-label={t("window.maximize")}
          title={live ? t("window.maximize") : t("status.browserPreview")}
          disabled={!live}
          onClick={() => {
            void import("@tauri-apps/api/window")
              .then((m) => m.getCurrentWindow().toggleMaximize())
              .catch((err) => {
                console.error("[window-overlay] 最大化按钮 toggleMaximize 被拒", err);
              });
          }}
        >
          <svg
            width="11"
            height="11"
            viewBox="0 0 11 11"
            aria-hidden="true"
            style={{ display: "block" }}
          >
            <rect
              x="1.5"
              y="1.5"
              width="8"
              height="8"
              fill="none"
              stroke="currentColor"
              strokeWidth="1.2"
            />
          </svg>
        </CapBtn>
        <CapBtn
          type="button"
          $danger
          aria-label={t("window.close")}
          title={live ? t("window.close") : t("status.browserPreview")}
          disabled={!live}
          onClick={() => {
            void import("@tauri-apps/api/window").then((m) => m.getCurrentWindow().close());
          }}
        >
          <X size={14} strokeWidth={1.5} aria-hidden="true" />
        </CapBtn>
      </Caps>
    </Overlay>
  );
}
