import styled from "@emotion/styled";

/**
 * section-title：分区标题（页面级 `h2`）。
 *
 * 🔴 2026-10-04 重做的理由 —— 原实现是「accent 色2px 下划线 + 16px 上边距」，
 * 与本页另外两个真问题叠在一起，把 6个分区**全部压成同一视觉重量**：
 *
 * ① **下划线承担不了分区层级**。`--aegis-accent` 在白底仅 **2.41:1**（非文本门槛 3:1），
 *    拿它当唯一分区记号，等于让最弱的元素去标最重要的结构；且 6 条等长等色的绿线
 *    平铺下来，扫视时���条都一样重 —— 这就是「层级不清」的根源。
 *    现改为**字重 + 留白**承担层级（规范 §一.3：层次靠发丝线与字重），
 *    下划线降级为 hairline（1.61:1，装饰线本就豁免对比度门槛）。
 *
 * ② **上边距与 `Page` 的 gap 叠加**。`Page` 是 flex + `gap: 16px`，而本组件
 *    原有 `margin: 16px 0 12px` —— margin 不会被 gap 抵消，两者相加得** 32px**，
 *    而上一个分区的内容到下个标题只有 16px ⇒ 分区头比它所属的内容更贴近上一区。
 *    现改为 `margin: 0`（间距交给 `Page` 的 gap 统一管），**一处管节奏**。
 *
 * ③ 标题字重从 600 改 **700**：600 在 CJK 回退字体上不存在（同panel.tsx 头注），
 *    且分区标题本就该是页面里第二重的字重。
 *
 * 🟢 保留的语义：仍用 `h2`（分区级），卡内标题用 `h3` —— 大纲层级不变。
 */
const Title = styled.h2`
  font-size: var(--aegis-type-h2-size);
  font-weight: 700;
  line-height: 1.25;
  /* margin 归零：分区节奏统一由 Page 的 gap 决定，避免与 gap 叠加成双倍间距 */
  margin: 0;
  padding-bottom: var(--aegis-space-xs);
  color: var(--aegis-ink);
  border-bottom: 1px solid var(--aegis-hairline);
`;

interface SectionTitleProps {
  children: string;
}

export function SectionTitle({ children }: SectionTitleProps) {
  return <Title>{children}</Title>;
}
