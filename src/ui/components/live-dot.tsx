import { useId, useState } from "react";
import { useTranslation } from "react-i18next";
import styled from "@emotion/styled";
import type { MonitorStatus } from "@/types/overview";

// live-dot：F-08 刷新生命周期徽章（LIVE · 每 2 秒 / 已暂停）。
// 用户靠直接看这颗点验证 R-19「切页/失焦立即停表」，所以状态必须一眼可辨：
// 活跃 = 实心绿 + 脉动光环；暂停 = 静默灰点 + 文案改「已暂停」。
//
// 🔴 2026-10-07 用户裁决「降级横幅不要出现在概览页面」（影响布局）：
//   原`ChannelStatus` 是一块**独占整行**的横幅（状态名 + 原因 + 驱动引导 + URL），
//   常驻把「详细信息」整段往下顶。本组件改为**承载**该信息：
//   · 常态（ready）**完全不变**，仍是「绿点 · LIVE · 每 2 秒」，零新增元素；
//   · 降级时点变**警示色** + 文案旁出现可展开的 ▸，**默认收起** ⇒ 不占行高；
//   · 展开后在其下方流内展开详情（`position: static`，🔴 不是浮层 ——
//     浮层会遮挡内容且在截图/导出中不可见）。
//
// 🔴 为什么信息不丢：N-06 要求「每次静默降级都要可追溯」。所以**只是移位**，
//   不删除；且 `title` 属性让悬停即可看到状态名与原因。

const Wrap = styled.span`
  display: inline-flex;
  align-items: center;
  gap: var(--aegis-space-xs);
  /* 🔴 24 = 品牌区**非按钮项**档高（规范 §三 brand-bar 行：按钮 h=28 / 徽章与状态点 h=24）。
     原值 18 是 12px 文字自然行高，与同行的 24 徽章、28 按钮三种高度并存 ⇒ 同一排三种基线。
     只撑行盒高，点（8px）与字号都不动。 */
  height: 24px;
  font-size: 12px;
  color: var(--aegis-mute);
  white-space: nowrap;
`;

const Dot = styled.span<{ live: boolean; degraded: boolean }>`
  position: relative;
  width: 8px;
  height: 8px;
  flex: none;
  border-radius: var(--aegis-radius-full);
  /* 🔴 降级时点转警示色：绿点会被读成「一切正常」，而此刻温度等增强项
     恰恰不可用（页面其余处只写「未检测」，不解释为何）。 */
  background: ${(p) =>
    p.degraded
      ? "var(--aegis-warning)"
      : p.live
        ? "var(--aegis-success)"
        : "var(--aegis-disabled)"};
  &::after {
    content: "";
    position: absolute;
    inset: 0;
    border-radius: inherit;
    background: ${(p) => (p.live && !p.degraded ? "var(--aegis-success)" : "transparent")};
    /* 周期 2s 属「循环速率」，按 motion 契约不入 duration 刻度；缓动取令牌 */
    animation: ${(p) =>
      p.live && !p.degraded
        ? "aegis-live-pulse 2s var(--aegis-motion-easing-decelerate) infinite"
        : "none"};
  }
  @keyframes aegis-live-pulse {
    from {
      transform: scale(1);
      opacity: 0.55;
    }
    to {
      transform: scale(2.6);
      opacity: 0;
    }
  }
  @media (prefers-reduced-motion: reduce) {
    &::after {
      animation: none;
    }
  }
`;

// 🔴 展开器：与文案同行，**默认收起**（这正是「不占布局」的关键）。
const Toggle = styled.button`
  display: inline-flex;
  align-items: center;
  gap: 2px;
  height: 20px;
  padding: 0 2px;
  border: 0;
  border-radius: var(--aegis-radius-sm);
  background: none;
  color: var(--aegis-warning);
  font: inherit;
  font-weight: 700;
  cursor: pointer;
  &:hover {
    background: var(--aegis-surface-soft);
  }
  &:focus-visible {
    outline: 2px solid var(--aegis-focus-ring, currentColor);
    outline-offset: 1px;
  }
  /* 🔴 三角朝向跟随展开态（省一个图标资源与一次布局抖动）。 */
  &::before {
    content: "▸";
    display: inline-block;
    transition: transform var(--aegis-motion-duration-fast, 120ms)
      var(--aegis-motion-easing-standard, ease);
    transform: rotate(0deg);
  }
  &[aria-expanded="true"]::before {
    transform: rotate(90deg);
  }
  @media (prefers-reduced-motion: reduce) {
    &::before {
      transition: none;
    }
  }
`;

// 🔴 详情块：在 Wrap 下方**流内**展开，随内容撑高；默认 `display: none`。
const Detail = styled.span`
  display: flex;
  flex-wrap: wrap;
  gap: var(--aegis-space-xs) var(--aegis-space-sm);
  padding: var(--aegis-space-xs) var(--aegis-space-sm);
  border: 1px solid var(--aegis-hairline);
  border-radius: var(--aegis-radius-sm);
  background: var(--aegis-surface-soft);
  line-height: 1.5;
  color: var(--aegis-mute-on-soft);
  /* 跨出 Wrap 的 flex 行：让它换行到下一行而不被压成单行 */
  flex-basis: 100%;
`;

// 品牌区是 flex 容器，须允许本项自身换行承载展开块
const Stack = styled.span`
  display: inline-flex;
  align-items: center;
  gap: var(--aegis-space-xs);
  flex-wrap: wrap;
`;

const StateName = styled.strong`
  color: var(--aegis-warning);
  font-weight: 700;
  white-space: nowrap;
`;

// URL 可选中复制但**不可点击**：`user-select` 保文本，`a` 元素会引入外呼入口（N-08）
const Url = styled.code`
  font-family: var(--font-mono);
  user-select: all;
  color: var(--aegis-ink);
`;

const DRIVER_REPO_URL = "https://github.com/namazso/PawnIO";

/** 🔴 是否处于「传感器通道降级」—— 即需要向用户解释温度等增强项为何不可用。
 *
 *  `monitor` 允许缺省（组件可退化为纯刷新指示，便于复用与测试）。
 *  `state === "ready"` 是唯一「无需解释」的态；其余四态
 *  （starting/coolingDown/disabled/exeMissing）都要给出具体故障形态（N-06）。
 *
 *  抽成纯函数是为可测：渲染行为（默认收起）需要 jsdom，而门禁当前是 node 环境
 *  （`vitest.config.ts` 注：加 jsdom 属新依赖，须走 ADR-003 登记），
 *  故判据层用单测钉住，渲染层如实声明未覆盖。
 */
export function isChannelDegraded(monitor?: MonitorStatus): boolean {
  return monitor !== undefined && monitor.state !== "ready";
}

export function LiveDot({
  live,
  monitor,
}: {
  live: boolean;
  /** 🔴 可选：缺失时组件退化为原行为（纯刷新指示），便于独立复用与测试。 */
  monitor?: MonitorStatus;
}) {
  const { t } = useTranslation("overview");
  const [open, setOpen] = useState(false);
  const detailId = useId();
  const degraded = isChannelDegraded(monitor);

  return (
    <Stack>
      <Wrap role="status" aria-live="polite">
        <Dot live={live} degraded={degraded} aria-hidden="true" />
        {live ? t("live.on") : t("live.paused")}
        {degraded && monitor ? (
          <Toggle
            type="button"
            aria-expanded={open}
            aria-controls={detailId}
            /* 悬停即给状态名+原因：不用点开也能知道「为什么降级」 */
            title={`${t(`monitor.state.${monitor.state}`)} · ${
              monitor.reason ?? t("monitor.reason.unknown")
            }`}
            onClick={() => setOpen((v) => !v)}
          >
            {t("live.degraded")}
          </Toggle>
        ) : null}
      </Wrap>
      {degraded && monitor && open ? (
        <Detail id={detailId}>
          <StateName>{t(`monitor.state.${monitor.state}`)}</StateName>
          <span>{monitor.reason ?? t("monitor.reason.unknown")}</span>
          {!monitor.driverInstalled ? (
            <span>
              {t("monitor.driver.guide")} <Url>{DRIVER_REPO_URL}</Url>
            </span>
          ) : null}
        </Detail>
      ) : null}
    </Stack>
  );
}
