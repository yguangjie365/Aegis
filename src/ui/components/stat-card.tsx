import { useTranslation } from "react-i18next";
import styled from "@emotion/styled";
import { SegBar } from "./seg-bar";
import { TempBadgeChip } from "./temp-badge";
import type { StatCard as StatCardData } from "@/types/overview";

// stat-card：F-01 实时概览卡（母本 SystemToolkit OverviewView 四卡语系）。
// 🔴 本组件不做任何"要不要显示"的判断（R-07/R-08/R-09 已在 Rust 侧判完），
// 只按 DTO 的 null / 枚举渲染；单位后缀也跟随 `valueSource`，不得自己认定大数字必是百分比。

/** hover 两段的时长（ms）。🔴 动效暂无令牌维度（规范 §二 五维不含动效、§五 禁新增维度），
 *  故就地声明并集中在此，便于将来随「动效令牌」一次性收口。 */
const HOVER_BORDER_MS = 120;
const HOVER_BAR_MS = 140;
/** 进入用的减速曲线，与 `shell/nav-rail` 的 `--ind-draw` 同一支 —— 避免全站出现两种"进入感"。 */
const EASE_ENTER = "cubic-bezier(0.32, 0.72, 0.24, 1)";

/**
 * 🔴 hover 反馈走**「定位」语汇，不是「可点击」语汇**（2026-10-04 用户裁决方案 B）。
 *
 * 四卡**不可点击**（`pages/overview-page.tsx` 只渲染 `<StatCard card />`，无任何 handler），
 * 因此不得使用上浮 / 阴影这类"我能点"的表达 —— 那会让用户去点、点了没反应，正撞 R-13 红线
 * （"点了没反应必须改成明确提示"）。故只用两种"鼠标在此"的信号：
 *
 * ① 边框 `hairline #cccccc` → `border-interactive #8f8f8f`：非文本对比 **1.61:1 → 3.23:1**（过 3:1）。
 * ② 左侧 2px 竖条：复用规范 §三 `table`「选中行 = 2px 竖条」的既有语汇。
 *    颜色用 `accent-strong #5a8d00`（浅底 **4.00:1**）而**不是** `accent #76b900`
 *    —— 后者在白底仅 **2.41:1**，达不到非文本 3:1（规范 §六 #8 的裁定）。
 *
 * 🔴 竖条用绝对定位 + `scaleY` ⇒ **不推挤内容、不触发布局**；无新色值、无阴影（§一.3 层次不靠阴影）。
 */
const Frame = styled.section`
  position: relative;
  overflow: hidden;
  display: flex;
  flex-direction: column;
  gap: var(--aegis-space-sm);
  padding: var(--aegis-space-lg);
  border: 1px solid var(--aegis-hairline);
  border-radius: var(--aegis-radius-sm);
  background: var(--aegis-canvas);
  transition: border-color ${HOVER_BORDER_MS}ms ${EASE_ENTER};

  &::before {
    content: "";
    position: absolute;
    left: 0;
    top: 0;
    bottom: 0;
    width: 2px;
    background: var(--aegis-accent-strong);
    transform: scaleY(0);
    transform-origin: 50% 50%;
    transition: transform ${HOVER_BAR_MS}ms ${EASE_ENTER};
  }

  &:hover {
    border-color: var(--aegis-border-interactive);
  }
  &:hover::before {
    transform: scaleY(1);
  }

  /* 规范 §8.3：reduced-motion 关过渡但**保留状态变化** —— 边框色与竖条仍照常切换，只是不再渐变 */
  @media (prefers-reduced-motion: reduce) {
    transition: none;
    &::before {
      transition: none;
    }
  }
`;

/**
 * 🔴 固定高度（2026-10-04 截图修复「硬件名称不在一条线上」）。
 *
 * 根因：本行内容是**条件性**的 —— `TempBadgeChip` 在 `measured` 时渲染 chip、
 * 在 `notApplicable` 时**整个返回 null**（R-09：隐藏而非显示占位文字）。
 * 于是 CPU/显卡卡（温度已测）有 chip、内存/存储卡可能没有，Head 高度不同，
 * 其后的NameRow / Model / 使用区**逐卡错位**。
 *
 * `min-height` 只防抖动、不保证对齐，故这里给**固定高度**并把徽章垂直居中：
 * 行高不再随内容变化，四卡的名称行必然落在同一基线上（L-08 卡高恒定）。
 */
const Head = styled.div`
  display: flex;
  align-items: center;
  justify-content: flex-end;
  gap: var(--aegis-space-sm);
  height: 22px; /* 与 TempBadgeChip 的 height 20px 同档，留 2px 呼吸 */
  flex: none;
`;

// 🔴 卡内标题用 h3 而非 h2：卡片是 `section`，其标题应低于页面级分区标题（h2），
// 否则一屏里出现多个 h2 会让读屏的大纲层级失真。
const Title = styled.h3`
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

/** 母本「名称」区：`NameZh` + `NameEn` 同行并列（英文缩写更小、下沉对齐基线）。 */
const NameRow = styled.div`
  display: flex;
  align-items: baseline;
  gap: 6px;
  height: 21px; /* 固定而非 min-height：保证四卡名称行严格同基线 */
  flex: none;
  overflow: hidden;
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

// 母本「使用区」是一行两列：`UsageLabel` 靠左（小号灰）与 `UsageText` 靠右（大号粗体），
// 见 OverviewView.xaml 第 217-219 行。原实现把大数字单独竖排一行，与母本不一致。
const UsageRow = styled.div`
  display: flex;
  align-items: baseline;
  justify-content: space-between;
  gap: var(--aegis-space-sm);
  /* 🔴 固定高度（2026-10-04 截图）：存储卡的「F: 剩余 346.6 GB」多出一行 detail 时，
     若不锁高，该卡的大数字行会向下漂，四卡的使用区不再同基线。 */
  min-height: 36px;
  flex: none;
`;

const UsageLabel = styled.span`
  font-size: 12px;
  line-height: 1.4;
  color: var(--aegis-mute);
`;

const BigValue = styled.div`
  font-size: 32px;
  font-weight: 700;
  line-height: 1.1;
  font-variant-numeric: tabular-nums;
  color: var(--aegis-ink);
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

// 🔴 单源：R-07 三级回退的"顶位的是温度还是占用"判定，界面（本组件）与 F-11 导出报告共用。
// 二者都必须走这里，否则会出现"界面上是 88℃、报告里写成 88%"这类凭空造数（R-08）。

/** 占用说明的 i18n 键：温度顶位时改用温度文案，不再谎称"使用率"。 */
export function statCaptionKey(card: StatCardData): string {
  return card.valueSource === "temperature"
    ? "card.caption.temperature"
    : `card.caption.${card.kind}`;
}

/** 大数字的单位后缀（温度顶位为 ℃，其余为 %）。 */
export function statValueUnit(card: StatCardData): string {
  return card.valueSource === "temperature" ? "℃" : "%";
}

export function StatCard({ card }: { card: StatCardData }) {
  const { t } = useTranslation("overview");
  return (
    <Frame aria-label={t(`card.title.${card.kind}`)}>
      {/* 头部只留温度徽章（母本该位置是图标 + 徽章；图标留待 F-01 图标资源接入）。
          🔴 内存/硬盘温度徽章由后端 `build_badge` 供给，未检测时出「未检测」虚线chip、
          无温度来源时整个徽章不渲染（R-08/R-09），两者都不会挤占卡内固定高度。 */}
      <Head>
        <TempBadgeChip badge={card.badge} />
      </Head>
      {/* 母本「名称」区：`NameZh` + `NameEn` 同行（OverviewView.xaml 第 212-213 行），
          `Model` 单独一行（第 215 行）。原来把中文名与英文缩写分置标题行两侧，与母本不一致。 */}
      <NameRow>
        <Title>{t(`card.title.${card.kind}`)}</Title>
        <Abbr>{t(`card.abbr.${card.kind}`)}</Abbr>
      </NameRow>
      <Model>{card.label}</Model>
      <UsageRow>
        <UsageLabel>{t(statCaptionKey(card))}</UsageLabel>
        <BigValue>
          {card.bigValue === null ? "—" : card.bigValue}
          {card.bigValue !== null ? <Unit>{statValueUnit(card)}</Unit> : null}
        </BigValue>
      </UsageRow>
      <SegBar percent={card.barPercent} label={t(statCaptionKey(card))} level={card.segLevel} />
      <Detail>{card.detail}</Detail>
      {card.degradeReason ? <Degrade>{card.degradeReason}</Degrade> : null}
    </Frame>
  );
}
