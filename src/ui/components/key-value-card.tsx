import { useTranslation } from "react-i18next";
import styled from "@emotion/styled";
import type { DetailCardDto, DetailKind } from "@/types/overview";

const Card = styled.div`
  background: var(--aegis-surface-soft);
  border-radius: var(--aegis-radius-sm);
  padding: var(--aegis-space-lg);
  box-shadow: var(--aegis-elevation-sticky);
`;

const Title = styled.h3`
  font-size: var(--aegis-type-h3-size);
  font-weight: 600;
  margin: 0 0 var(--aegis-space-md) 0;
  color: var(--aegis-ink);
`;

/**
 * 母本 `OverviewView.xaml:288-296` 的行布局：`ColumnDefinition Width="120"` + `Width="*"`，
 * 值用等宽字体（`Font_Mono`）并右对齐。
 *🔴 值用等宽是有原因的：容量「476.9 GB」、主频「2.3 GHz」这类数字右对齐后小数点才能成列，
 * 比例字体下各位数字宽度不等，视觉上会歪。
 */
const Row = styled.div`
  display: grid;
  grid-template-columns: 120px minmax(0, 1fr);
  align-items: baseline;
  gap: var(--aegis-space-sm);
  padding: var(--aegis-space-xs) 0;
  border-bottom: 1px solid var(--aegis-hairline);

  &:last-child {
    border-bottom: none;
  }
`;

/**
 * 键列。
 *
 * 🔴 `min-width: 0` 是关键护栏：grid 项默认 `min-width: auto`，内容过长时会**撑破轨道**
 * 把值列挤到零宽并溢出卡片（用户截图中`detail.cpu.detail.` 曾把键列顶满整行）。
 * 置0 后由下面的 `overflow: hidden` 正常截断。
 *
 * 正常情况下键是中文短词（「型号」「核心 / 线程」），120px（母本 `ColumnDefinition Width="120"`）
 * 足够；只有 i18n 键解析失败露出裸键时才会截断 —— 那正是应该被看见的异常。
 */
const Key = styled.span`
  color: var(--aegis-mute);
  font-size: var(--aegis-type-body-size);
  font-weight: 600; /* 母本 SemiBold：CJK 回退字体无Medium 权重，500 会被降为 Regular 显细 */
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
`;

const Value = styled.span<{ $missing?: boolean }>`
  color: ${(props) => (props.$missing ? "var(--aegis-stone)" : "var(--aegis-ink)")};
  font-size: var(--aegis-type-body-size);
  font-weight: 600;
  font-family: var(--font-mono); /* 母本 MonoText：数字右对齐才能成列 */
  text-align: right;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
`;

const Reason = styled.p`
  color: var(--aegis-warning);
  font-size: var(--aegis-type-caption-size);
  margin: var(--aegis-space-sm) 0 0 0;
`;

// 🔴 详情卡 kind → i18n 文案前缀的**唯一**映射表：界面（本组件）与 F-11 导出报告
// （features/Overview/lib/export-report.ts）共用同一份，避免两处各写一份中文键而漂移。
//
// 🔴 键类型是 `Record<DetailKind, string>` 而非 `Record<string, string>`：
// 后者让新增类目时漏改此表也不报错（tsc 仍绿），运行时退化成裸键 `detail.xxx.title`。
// 收窄后新增类目**必然**在此编译失败，逼着同步 `locales/*/overview.json`。
export const DETAIL_KIND_I18N_PREFIX: Record<DetailKind, string> = {
  cpu: "detail.cpu",
  gpu: "detail.gpu",
  storage: "detail.storage",
  memory: "detail.memory",
  board: "detail.board",
  bios: "detail.bios",
  network: "detail.network",
  audio: "detail.audio",
  display: "detail.display",
  os: "detail.os",
  userRegion: "detail.userRegion",
};

/** F-02 硬件详情 vs F-03 系统信息 的分流依据（后端同装一个 DTO，见 commands.rs 头注）。 */
export const SYSTEM_INFO_KINDS: ReadonlySet<DetailKind> = new Set<DetailKind>(["os", "userRegion"]);

interface KeyValueCardProps {
  card: DetailCardDto;
}

export function KeyValueCard({ card }: KeyValueCardProps) {
  const { t } = useTranslation("overview");

  // 🔴 `row.key` 是后端给的**完整** i18n 键（如 `detail.cpu.model`，见
  // `hardware/cpu.rs` 的 `kv("detail.cpu.model", …)`），**不得**再拼 `prefix` ——
  // 拼一次会得到 `detail.cpu.detail.cpu.model`，词典查不到就把裸键原样渲染到界面
  // （2026-10-04 用户截图满屏 `detail.cpu.detail.cpu.model` 即此因）。
  // 与 `lib/export-report.ts` 的 `buildReport` 同款修法：两处必须同改，
  // 否则变成「界面中文、报告里是裸键」。
  // `prefix` 仅用于卡标题 —— 卡标题的键按 kind 推导，与行键无关。
  // 🔴 `kind` 已是联合类型，此处**不需要** `??` 兜底：留兜底等于把「新增类目忘了配文案」
  // 重新变成静默裸键，那正是本项目踩过多次的坑。
  const prefix = DETAIL_KIND_I18N_PREFIX[card.kind];

  return (
    <Card>
      <Title>{t(`${prefix}.title`)}</Title>
      {card.rows.map((row) => {
        const label = t(row.key);
        const isMissing = row.value == null || row.value === "";
        const displayValue = isMissing ? t("badge.undetected") : row.value;

        return (
          <Row key={row.key}>
            <Key>{label}</Key>
            {/* 母本同行有 TextTrimming + ToolTip：长值（型号/序列号）截断后可悬停看全文 */}
            <Value $missing={isMissing} title={isMissing ? undefined : (displayValue ?? undefined)}>
              {displayValue}
            </Value>
          </Row>
        );
      })}
      {card.reason ? <Reason>{card.reason}</Reason> : null}
    </Card>
  );
}
