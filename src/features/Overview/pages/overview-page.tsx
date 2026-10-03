import { useTranslation } from "react-i18next";
import styled from "@emotion/styled";
import { PageHeader } from "@/ui/components/page-header";
import { AppButton } from "@/ui/components/app-button";
import { LiveDot } from "@/ui/components/live-dot";
import { StatCard } from "@/ui/components/stat-card";
import { ErrorStateBlock, LoadingSkeleton } from "@/ui/components/state-block";
import { ChannelStatus } from "../components/channel-status";
import { useManualRefresh, useOverviewLive } from "../hooks/use-overview-live";
import { formatUptime, joinHeader } from "../lib/format";

// 本机概览页（M-01 片1）：页头条 + 实时概览四卡 + 传感器通道状态行。
// 布局母本 = Source/SystemToolkit 的 OverviewView.xaml（单列垂直流 + 顶部四卡等宽一排），
// 已按批准方案的六分区线框落地第 1 分区；其余分区随片2~片4 逐片追加在同一列流里。

const Page = styled.div`
  max-width: 1280px;
  margin: 0 auto;
  display: flex;
  flex-direction: column;
  gap: var(--aegis-space-lg);
`;

const HeadTools = styled.div`
  display: flex;
  align-items: center;
  gap: var(--aegis-space-md);
`;

// 四卡一排（母本 UniformGrid Columns=4 的等价物）：等宽、随行拉伸，
// 显卡按 R-01/R-07 可能是 0~n 张，故用 auto-fit 而非硬写四列——卡数变化时不留空槽。
const CardGrid = styled.div`
  display: grid;
  grid-template-columns: repeat(auto-fit, minmax(228px, 1fr));
  gap: var(--aegis-space-md);
`;

export function OverviewPage() {
  const { t } = useTranslation("overview");
  const { live, data, isPending, isError, error, refetch, isFetching } = useOverviewLive();
  const refreshAll = useManualRefresh();

  const uptimeText = data
    ? formatUptime(data.header.uptimeSecs, {
        day: t("uptime.day"),
        hour: t("uptime.hour"),
        minute: t("uptime.minute"),
        belowMinute: t("uptime.belowMinute"),
      })
    : null;

  // F-12：任一组成项缺失时该段自然省略，不留空分隔符
  const sub = joinHeader([
    data?.header.computerName,
    data?.header.osName,
    uptimeText ? t("uptime.prefix", { value: uptimeText }) : null,
  ]);

  // R-13：本拍在途（含后端闸门）时刷新按钮禁用并给原因，重复点击被吸收
  const busy = isFetching || (data?.inFlight ?? false);

  return (
    <Page>
      <PageHeader
        title={t("page.title")}
        sub={sub === "" ? undefined : sub}
        actions={
          <HeadTools>
            <LiveDot live={live} />
            <AppButton
              variant="secondary"
              size="sm"
              type="button"
              disabled={busy}
              title={busy ? t("refresh.busy") : t("refresh.hint")}
              onClick={refreshAll}
            >
              {t("refresh.label")}
            </AppButton>
          </HeadTools>
        }
      />

      {isPending ? <LoadingSkeleton rows={4} /> : null}

      {isError ? (
        <ErrorStateBlock
          title={t("state.error.title")}
          body={error instanceof Error ? error.message : String(error)}
          onRetry={() => void refetch()}
          onRetryLabel={t("refresh.label")}
        />
      ) : null}

      {data ? (
        <>
          <CardGrid>
            {data.cards.map((card) => (
              <StatCard key={`${card.kind}-${card.index}`} card={card} />
            ))}
          </CardGrid>
          <ChannelStatus monitor={data.monitor} />
        </>
      ) : null}
    </Page>
  );
}
