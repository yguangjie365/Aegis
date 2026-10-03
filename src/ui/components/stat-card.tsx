import { useTranslation } from "react-i18next";
import styled from "@emotion/styled";
import { SegBar } from "./seg-bar";
import { TempBadgeChip } from "./temp-badge";
import type { StatCard as StatCardData } from "@/types/overview";

// stat-card：F-01 实时概览卡（母本 SystemToolkit OverviewView 四卡语系）。
// 🔴 本组件不做任何"要不要显示"的判断（R-07/R-08/R-09 已在 Rust 侧判完），
// 只按 DTO 的 null / 枚举渲染；单位后缀也跟随 `valueSource`，不得自己认定大数字必是百分比。

const Frame = styled.section`
  display: flex;
  flex-direction: column;
  gap: var(--aegis-space-sm);
  padding: var(--aegis-space-lg);
  border: 1px solid var(--aegis-hairline);
  border-radius: var(--aegis-rounded-sm);
  background: var(--aegis-canvas);
`;

const Head = styled.div`
  display: flex;
  align-items: center;
  gap: var(--aegis-space-sm);
`;

const Title = styled.h2`
  font-size: 15px;
  font-weight: 700;
  line-height: 1.25;
`;

const Abbr = styled.span`
  font-size: 12px;
  font-weight: 700;
  letter-spacing: 0.04em;
  color: var(--aegis-mute);
`;

const Spacer = styled.span`
  margin-left: auto;
`;

const Model = styled.div`
  font-size: 13px;
  line-height: 1.4;
  color: var(--aegis-ink);
  /* 型号长短不一（R-21 精简后仍可能换行）：固定两行高，卡高不随文案跳动（L-08） */
  min-height: 36px;
  display: -webkit-box;
  -webkit-line-clamp: 2;
  -webkit-box-orient: vertical;
  overflow: hidden;
`;

const BigValue = styled.div`
  font-size: 32px;
  font-weight: 700;
  line-height: 1.1;
  font-variant-numeric: tabular-nums;
`;

const Unit = styled.span`
  font-size: 15px;
  font-weight: 700;
  margin-left: 2px;
  color: var(--aegis-mute);
`;

const Detail = styled.div`
  font-size: 12px;
  line-height: 1.4;
  color: var(--aegis-mute);
  min-height: 17px;
`;

const Degrade = styled.div`
  font-size: 12px;
  line-height: 1.4;
  color: var(--aegis-warning);
`;

export function StatCard({ card }: { card: StatCardData }) {
  const { t } = useTranslation("overview");
  // R-07 三级回退：温度顶位时大数字不是百分比——单位与说明都要跟着换，
  // 否则会把 88℃ 显示成"88%"这种凭空造数（R-08 状态诚实红线）。
  const isTemperature = card.valueSource === "temperature";
  const captionKey = isTemperature ? "card.caption.temperature" : `card.caption.${card.kind}`;
  return (
    <Frame aria-label={t(`card.title.${card.kind}`)}>
      <Head>
        <Title>{t(`card.title.${card.kind}`)}</Title>
        <Abbr>{t(`card.abbr.${card.kind}`)}</Abbr>
        <Spacer />
        <TempBadgeChip badge={card.badge} />
      </Head>
      <Model>{card.label}</Model>
      <BigValue>
        {card.bigValue === null ? "—" : card.bigValue}
        {card.bigValue !== null ? <Unit>{isTemperature ? "℃" : "%"}</Unit> : null}
      </BigValue>
      <SegBar percent={card.barPercent} label={t(captionKey)} />
      <Detail>{card.detail}</Detail>
      {card.degradeReason ? <Degrade>{card.degradeReason}</Degrade> : null}
    </Frame>
  );
}
