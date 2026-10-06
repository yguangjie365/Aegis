// 运行日志面板（F-FB-48）。
//
// 母本形态：页底常驻一块深底等宽的运行日志，带「信息 / 成功 / 错误」三个分级过滤
// 与「清空日志」。规格 §3.1 要求它是**可折叠**的 ⇒ 在母本基础上加折叠，
// 但**默认展开**：日志是这一页主要的反馈面，默认折起来会让「失败为什么失败」无迹可寻。
//
// 🔴 **自动滚到底**：时间序是旧→新、新条目追加在末尾；不滚的话用户看不到刚发生的事
//（母本也是这个行为 —— 它的日志区永远停在最后一行）。
//
// 🔴 颜色只走令牌（规范 §一），分级三色取 `error` / `success` / `mute`，
// 不新造色值 —— 否则换主题时日志会与其它面板的语义色漂成两种。

import { useEffect, useMemo, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import styled from "@emotion/styled";
import { ChevronDown, ChevronRight } from "lucide-react";
import { Panel } from "@/ui/components/panel";
import { formatClock } from "../lib/format";
import { useLogStore, type LogLevel } from "../store/log-store";

/** 过滤档：三级别 + 全部。 */
type Filter = LogLevel | "all";

/**
 * 运行日志面板的容器。
 *
 * 🔴 2026-10-06 角色互换（用户裁决「快照区高度固定 = 标题行 + 7 条内容行」）：
 *   原来快照区 `flex: 1 1 0` 吃剩余、日志 `flex: 0 0 auto` 按内容高；
 *   现在**快照区按内容高（固定预算）**，日志吃剩下的空间。
 *
 * 🔴 **本面板的 flex-grow 实际由 `file-backup-page.tsx` 的 Column 通配规则决定**
 *   （`> section { flex: 1 1 auto }`，特异性 (0,1,1) 高于本组件类名的 (0,1,0)）。
 *   ⇒ 下面写 `flex: 1 1 auto` 是**如实复述**而不是争夺：本组件从来抢不过那条规则，
 *     此前写 `0 0 auto` 时被静默覆盖 —— 这正是文件里挂了很久的
 *     🟡「空日志实测 301px / 推算只该 ~100px，多出来的高度找不到根因」的**答案**：
 *     不是 Head 变高、也不是 padding 偏大，而是本面板一直在**长满右栏剩余高度**。
 *     2026-10-06 定位（快照区固定后剩余空间变小，空隙才显眼起来）。
 *   ⚠️ 与 RuleFooterActions 同一处坑：要改回去得用 `data-fixed` 从那条通配规则里排除，
 *     在本组件内加更高特异性硬压是另一种脏法。
 */
const Shell = styled(Panel)`
  flex: 1 1 auto;
  min-height: 0;
  max-height: 45%;
  overflow: hidden;
  margin-top: var(--aegis-space-md);
  /* 本面板自己也是 column flex：Head 恒定、Body 吸收全部剩余并可滚。 */
  display: flex;
  flex-direction: column;
`;

export function LogPanel() {
  const { t } = useTranslation("backup");
  const entries = useLogStore((s) => s.entries);
  const clear = useLogStore((s) => s.clear);
  const [collapsed, setCollapsed] = useState(false);
  const [filter, setFilter] = useState<Filter>("all");
  const bodyRef = useRef<HTMLDivElement | null>(null);

  const counts = useMemo(() => {
    let info = 0;
    let success = 0;
    let error = 0;
    for (const entry of entries) {
      if (entry.level === "error") error += 1;
      else if (entry.level === "success") success += 1;
      else info += 1;
    }
    return { info, success, error, all: entries.length };
  }, [entries]);

  const shown = useMemo(
    () => (filter === "all" ? entries : entries.filter((entry) => entry.level === filter)),
    [entries, filter],
  );

  useEffect(() => {
    const node = bodyRef.current;
    // 折叠时 body 不在 DOM 里 ⇒ 只在展开态滚（否则 ref 为 null，等于白跑）。
    if (node !== null) node.scrollTop = node.scrollHeight;
  }, [shown.length, collapsed]);

  const tabs: { key: Filter; label: string; count: number }[] = [
    { key: "all", label: t("log.filter.all"), count: counts.all },
    { key: "info", label: t("log.filter.info"), count: counts.info },
    { key: "success", label: t("log.filter.success"), count: counts.success },
    { key: "error", label: t("log.filter.error"), count: counts.error },
  ];

  return (
    <Shell as="section" aria-label={t("log.title")}>
      <Head>
        <Toggle type="button" onClick={() => setCollapsed((v) => !v)} aria-expanded={!collapsed}>
          {collapsed ? (
            <ChevronRight size={16} strokeWidth={1.5} aria-hidden="true" />
          ) : (
            <ChevronDown size={16} strokeWidth={1.5} aria-hidden="true" />
          )}
          <Title>{t("log.title")}</Title>
        </Toggle>

        <Tabs>
          {tabs.map((tab) => (
            <Tab
              key={tab.key}
              type="button"
              $active={filter === tab.key}
              onClick={() => setFilter(tab.key)}
            >
              {tab.label}
              <Count $inverted={filter === tab.key}>{formatCountInline(tab.count)}</Count>
            </Tab>
          ))}
        </Tabs>

        <Spacer />

        <ClearButton type="button" onClick={clear} disabled={entries.length === 0}>
          {t("log.clear")}
        </ClearButton>
      </Head>

      {!collapsed && (
        <Body ref={bodyRef}>
          {shown.map((entry) => (
            <Line key={entry.id} $level={entry.level}>
              <Time>{formatClock(entry.at)}</Time>
              {entry.text}
            </Line>
          ))}
        </Body>
      )}
    </Shell>
  );
}

/** 标签上的计数：三位数以内原样，超过折成 `99+`（避免窄标签被数字撑开）。 */
function formatCountInline(count: number): string {
  return count > 99 ? "99+" : String(count);
}

const Head = styled.div`
  display: flex;
  align-items: center;
  gap: var(--aegis-space-sm);
  flex-wrap: wrap;
  /* 头一行恒定：可收缩的只有下面的日志正文。 */
  flex: none;
`;

const Toggle = styled.button`
  display: inline-flex;
  align-items: center;
  gap: var(--aegis-space-xs);
  background: none;
  border: none;
  padding: 0;
  cursor: pointer;
  color: var(--aegis-ink);
`;

const Title = styled.h3`
  font-size: var(--aegis-type-h3-size);
  font-weight: 700;
  line-height: 1.4;
  margin: 0;
  color: var(--aegis-ink);
`;

const Tabs = styled.div`
  display: inline-flex;
  align-items: center;
  gap: var(--aegis-space-xs);
  margin-left: var(--aegis-space-md);
`;

const Tab = styled.button<{ $active: boolean }>`
  display: inline-flex;
  align-items: center;
  gap: var(--aegis-space-xs);
  padding: 2px var(--aegis-space-sm);
  border-radius: var(--aegis-radius-sm);
  /* 🔴 规范 §三 tabs 要求「激活态与常态**反色**」，但它给的字面是「ink 底 + 白字」。
     🔴🔴 本项目是 **Nvidia.Dark 单主题**（单主题 colorMode 恒 dark）⇒
         ink 在深色下是**浅色**（实测 rgb(232,232,232)），照搬「ink 底 + on-dark 白字」
 *     会得到**白底白字**（实测 activeTabBg=232 / activeTabColor=255，两者皆实测确认）。
   *     正确做法是取**反色**而非固定某色：底用 ink（= 常态文字色）、
   *     字用 canvas（= 与 ink 成对的内容画布色）⇒ 浅色主题得「深底白字」、
   *     深色主题得「浅底深字」，两边都可读（规范 §一.2 双色模）。
   *     🔴 本注释刻意不含反引号：它在 emotion 模板串里，反引号会终止模板串。 */
  border: 1px solid ${(p) => (p.$active ? "var(--aegis-ink)" : "var(--aegis-hairline)")};
  background: ${(p) => (p.$active ? "var(--aegis-ink)" : "transparent")};
  color: ${(p) => (p.$active ? "var(--aegis-canvas)" : "var(--aegis-ink)")};
  font-size: var(--aegis-type-caption-size);
  cursor: pointer;
  transition: border-color var(--aegis-motion-duration-fast) var(--aegis-motion-easing-standard);

  &:hover {
    border-color: var(--aegis-border-interactive);
  }

  &:focus-visible {
    outline: 2px solid var(--aegis-focus-ring);
    outline-offset: 2px;
  }
`;

const Count = styled.span<{ $inverted: boolean }>`
  /* 激活态是反色底 ⇒ 计数跟着用成对色，否则深底深字 / 浅底浅字都读不出来。
     🔴 用 canvas 而不是 on-dark：on-dark 是「深底上的文字色」（本项目深色主题下≈白），
     而激活底是 ink（≈浅灰 232）⇒ 配 on-dark 就是白底白字。 */
  color: ${(p) => (p.$inverted ? "var(--aegis-canvas)" : "var(--aegis-mute)")};
  opacity: ${(p) => (p.$inverted ? 0.75 : 1)};
  font-variant-numeric: tabular-nums;
`;

const Spacer = styled.div`
  flex: 1;
`;

const ClearButton = styled.button`
  padding: 2px var(--aegis-space-sm);
  border-radius: var(--aegis-radius-sm);
  border: 1px solid var(--aegis-hairline);
  background: transparent;
  color: var(--aegis-mute);
  font-size: var(--aegis-type-caption-size);
  cursor: pointer;

  &:hover:not(:disabled) {
    color: var(--aegis-error);
    border-color: var(--aegis-error);
  }

  &:disabled {
    cursor: default;
    opacity: 0.5;
  }
`;

const Body = styled.div`
  margin-top: var(--aegis-space-sm);
  padding: var(--aegis-space-sm);
  background: var(--aegis-surface-inset);
  border-radius: var(--aegis-radius-sm);
  overflow-y: auto;
  /* 🔴 吃满 Shell 剩余高度、同时可收缩（2026-10-06 重算）：
     ① min-height 0：缺它时收缩落在 Shell 自己身上，正文会被 Shell 的 overflow hidden
        裁掉 —— 最后几条日志既看不见也滚不到；
     ② flex-grow 1：删掉空态提示文字后正文自然高只剩上下 padding（16px），
        而 Shell 被 Column 的通配规则撑到右栏剩余高度，中间会留一条孤儿空白；
        让正文长满，inset 底纹直接铺到卡片底部。
     🟡 这里原有「max-height 120px」已删：那是旧配比（快照区吃剩余）下调出来的，
        目的是「别让日志比表格高」。现在快照区高度固定、日志只吃剩下的，
        这条线失去意义，留着反而制造上面说的孤儿空白。
        唯一的天花板收在 Shell 的「max-height 45%」—— 一个数，不再两处打架。
     ⚠️ 本注释内不得出现反引号，也不得出现星号斜杠的连写（会提前终止注释）。 */
  flex: 1 1 auto;
  min-height: 0;
  display: flex;
  flex-direction: column;
  gap: 2px;
  font-family: var(--font-mono);
  font-size: var(--aegis-type-caption-size);
  line-height: 1.6;
`;

const Line = styled.div<{ $level: LogLevel }>`
  color: ${(p) =>
    p.$level === "error"
      ? "var(--aegis-error)"
      : p.$level === "success"
        ? "var(--aegis-success)"
        : "var(--aegis-mute)"};
  /* 路径很长 ⇒ 允许在任意位置断行，避免横向滚动条 */
  word-break: break-all;
`;

const Time = styled.span`
  color: var(--aegis-mute);
  opacity: 0.7;
  margin-right: var(--aegis-space-xs);
`;
