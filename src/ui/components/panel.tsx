/**
 * panel.tsx — 面板与表格的**唯一**容器语汇（2026-10-04 视觉统一）
 *
 * 🔴 为什么要抽这个文件：统一前，同一页面上并存**两套互斥的卡片语汇**——
 *   ① `StatCard.Frame`：canvas 底 + 1px hairline 描边 + **无阴影**（规范 §一.3 的正解）
 *   ② `KeyValueCard` / `ProcessTable` / `NetEndpointsPanel` / `DiskHealthPanel` /
 *      `SoftwareListPanel`：surface-soft 底 + **无描边** + `box-shadow: elevation-sticky`
 * 五处面板用的是 ②，而 ② 直接违反两条已生效条款：
 *   · §一.3「层次靠发丝线与字重，**不靠阴影**（阴影仅允许粘滞条 1 处）」——
 *     `elevation-sticky` 的定义原文就是「唯一阴影：粘滞工具条/弹层」，
 *     被借给 5 个内容面板 = 阴影额度超发5 倍。
 *   · §一.4「正文只用 400/700 两档」—— 五处都写了 `font-weight: 600`，
 *     而 CJK 回退字体（微软雅黑/苹方）**没有 600 字重**，浏览器只能合成或
 *     就近取 400/700 ⇒ 同一页面上「看起来一样粗」的标题实际粗细不一。
 *
 * 故统一到 ①：canvas 底 + hairline 描边 + 无阴影，四卡与所有面板同属一套语汇。
 * 分区层次改由 §标题的**字重与留白**承担（见 section-title.tsx），不再靠阴影。
 */

import styled from "@emotion/styled";

/** 面板容器：canvas 底 + hairline 描边，无阴影（规范 §一.3）。四卡与内容面板共用。 */
export const Panel = styled.section`
  background: var(--aegis-canvas);
  border: 1px solid var(--aegis-hairline);
  border-radius: var(--aegis-radius-sm);
  padding: var(--aegis-space-lg);
`;

/**
 * 面板标题：`h3`（卡内标题低于页面级分区标题 `h2`，否则一屏多个 h2 会让读屏大纲失真）。
 *
 * 🔴 字重用 **700** 而非 600：规范 §一.4 只允许 400/700 两档，且 600 在 CJK 回退字体上
 * 不存在（见本文件头注）。此前五处面板各写一遍 600，是「层级不清」的隐形来源 ——
 * 视觉上都在强调，读屏与实际字重却对不上。
 */
export const PanelTitle = styled.h3`
  font-size: var(--aegis-type-h3-size);
  font-weight: 700;
  line-height: 1.4;
  margin: 0 0 var(--aegis-space-md) 0;
  color: var(--aegis-ink);
`;

/** 面板内的次级说明文字（复用各面板原先各自写的 Summary / Empty / Placeholder）。 */
export const PanelNote = styled.p`
  font-size: var(--aegis-type-body-size);
  line-height: 1.5;
  color: var(--aegis-mute);
  margin: 0;
`;

/**
 * 采集失败原因（各面板的 `Reason`）—— 统一到 warning 令牌。
 *
 * 空态必写「为什么是空的」（R-08），故这是**功能性**文字而非装饰，样式三面板必须一致。
 */
export const PanelReason = styled.p`
  font-size: var(--aegis-type-caption-size);
  line-height: 1.5;
  color: var(--aegis-warning);
  margin: var(--aegis-space-sm) 0 0 0;
`;

/* ══ 表格语汇 ═══════════════════════════════════════════════════════════════
   统一前 process-table / net-endpoints-panel / software-list-panel **各写一份**
   完全相同的 Table/Thead/Th/Tbody/Td（约 40 行 × 3）—— 改一次列宽要改三处，
   漏一处就与另两处漂移。 */

export const DataTable = styled.table`
  width: 100%;
  border-collapse: collapse;
  font-size: var(--aegis-type-body-size);
  /* 🔴 固定列宽：软件清单的表头声明了 300/150/240/120/100px，若用 auto 布局，
     内容会把列撑到与声明不同的宽度，「版本/发布商」两列因内容长度差异而错位。 */
  table-layout: fixed;
`;

/**
 * 表头行：`1px` 而非 `2px` 描边 —— 2px 在密集表格里会压过数据本身。
 *
 * 🔴 2026-10-06 规范对齐（用户裁决「改共享组件，全站生效」）：行高对齐规范 §三
 *   `table` 的 **36px**。此前 padding 是 `space-xs`(4) ⇒ 行高仅 29px，
 *   密集表格读起来发挤。改成 `space-sm`(8) 上下 + body 行高 21 ≈ **37px**。
 *   ⚠️ 规范同条要求的「列头 caption 大写化」在**中文语境不适用**（无 case 之分），
 *   层级改由 `font-weight: 700` + caption 字号承担，此处不写 `text-transform`。
 */
export const Thead = styled.thead`
  border-bottom: 1px solid var(--aegis-border-interactive);
`;

export const Th = styled.th<{ $width?: string; $numeric?: boolean }>`
  text-align: ${(p) => (p.$numeric ? "right" : "left")};
  padding: var(--aegis-space-sm) var(--aegis-space-md);
  color: var(--aegis-mute);
  font-size: var(--aegis-type-caption-size);
  font-weight: 700;
  letter-spacing: 0.02em;
  white-space: nowrap;
  ${(p) => p.$width && `width: ${p.$width};`}
`;

/**
 * 斑马行（规范 §三 `table`：「斑马行 surface-soft」）。
 *
 * 🔴 用 `:nth-of-type(even)` 而不是 `:nth-child(even)`：表体里若混入
 *   `<caption>` 或其他非 `tr` 元素，`nth-child` 的奇偶会错位。
 *   只作用于 `body > tr`（`tbody tr`）⇒ 表头行永不被染。
 */
export const Tbody = styled.tbody`
  tr {
    border-bottom: 1px solid var(--aegis-hairline);

    &:last-child {
      border-bottom: none;
    }
  }

  tr:nth-of-type(even) {
    background: var(--aegis-surface-soft);
  }
`;

export const Td = styled.td<{ $numeric?: boolean; $title?: string }>`
  padding: var(--aegis-space-sm) var(--aegis-space-md);
  color: var(--aegis-ink);
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
  ${(p) => p.$numeric && "text-align: right; font-variant-numeric: tabular-nums;"}
`;

/**
 * 数据行：随节拍更新时给一次**极短**的底色回落，让「哪一行变了」可被余光捕捉。
 *
 * 🔴 为什么只做「回落」不做「扫过」：进程榜每 2 秒一刷，一次「扫过」动画会在
 * 每轮刷新时都亮一次，2 秒一次的高频闪烁比没有动画更干扰。回落只在**值确实变了**
 * 时触发，且 2s 后归零 —— 是「提示刚刚变了」而不是「持续动效」。
 *
 * `@keyframes` 定义在本文件（全局 `theme.ts` 只管 transition/animation 的降级，
 * 不该塞组件级关键帧）。
 */
export const Tr = styled.tr<{ $pulse?: boolean; $selected?: boolean }>`
  /* 时长 1.6s 属组件私有编舞（D2 豁免），但**缓动必须取令牌** ——
     原写裸 ease-out：§二 的 decelerate 档就是 ease-out 的契约化写法（进入/落位先快后慢）。
     2026-10-06 UI 审查 D2。⚠️ 本注释在模板字符串内，禁写反引号（会报 TS1005）。 */
  animation: ${(p) =>
    p.$pulse ? "aegis-row-settle 1.6s var(--aegis-motion-easing-decelerate)" : "none"};

  /* 🔴 规范 §三 table：「**选中行**：左侧 2px accent-strong 竖条 +
   *   border-interactive 描边（不得只靠 8% 底色）」。
   *   用 border-left 而**不用 box-shadow inset** —— 规范 §一.3 明确
   *   「层次靠发丝线与字重，**不靠阴影**」，而内嵌阴影在 token 审计里
   *   与真阴影同源；border 是真边框，且 table-layout: fixed 下
   *   只吃掉首列 2px，不会让列宽跑偏。
   *   🔴 本注释刻意不含反引号：它在 emotion 模板串里，反引号会**终止模板串**
   *      （tsc 报 TS1005）—— 今天已在同一处栽了第三次。 */
  ${(p) =>
    p.$selected &&
    `
    td:first-child {
      border-left: 2px solid var(--aegis-accent-strong);
    }

    &,
    td {
      background: color-mix(in srgb, var(--aegis-accent) 8%, transparent);
    }
  `}

  @keyframes aegis-row-settle {
    from {
      background: color-mix(in srgb, var(--aegis-accent) 9%, transparent);
    }
    to {
      background: transparent;
    }
  }

  /* 规范 §8.3：reduced-motion 关动效但**保留状态变化** —— 底色回落本身就是状态提示，
     关掉它等于把「这行变了」的信息也删了。故改为不动画（瞬时切换），而非 none 整块。 */
  @media (prefers-reduced-motion: reduce) {
    animation: none;
  }
`;
