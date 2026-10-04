import { useTranslation } from "react-i18next";
import styled from "@emotion/styled";
import { PageHeader } from "@/ui/components/page-header";
import { AppButton } from "@/ui/components/app-button";
import { LiveDot } from "@/ui/components/live-dot";
import { StatCard } from "@/ui/components/stat-card";
import { SectionTitle } from "@/ui/components/section-title";
import { ErrorStateBlock } from "@/ui/components/state-block";
import { DiskHealthPanel } from "@/ui/components/disk-health-panel";
import { DualProcessTables } from "@/ui/components/process-table";
import { NetEndpointsPanel } from "@/ui/components/net-endpoints-panel";
import { SoftwareListPanel } from "@/ui/components/software-list-panel";
import { ChannelStatus } from "../components/channel-status";
import { KeyValueCard } from "@/ui/components/key-value-card";
import {
  useManualRefresh,
  useOverviewLive,
  useTopProcesses,
  useNetEndpoints,
  useSoftwareList,
  useStaticInfo,
  useSnapshotWarmup,
} from "../hooks/use-overview-live";
import { splitStaticCards } from "../lib/export-report";
import { useExportReport } from "../hooks/use-export-report";
import { formatUptime, joinHeader } from "../lib/format";

// 本机概览页（M-01 片1+片2）：页头条 + 实时概览四卡 + 传感器通道状态行 + 硬件详情区 + 系统信息区。
// 布局母本 = Source/SystemToolkit 的 OverviewView.xaml（单列垂直流 + 顶部四卡等宽一排），
// 已按批准方案的六分区线框落地第 1-3 分区；其余分区随片3~片4 逐片追加在同一列流里。

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

// 母本 `UniformGrid Columns="2"`（OverviewView.xaml 第 251/317 行）的等价物：
// 详情卡按 kind 分流后逐行落位，同类多设备（如多显卡）自然并排，不留空槽。
/**
 * 🟢 构建时刻标记样式：等宽小字 + 极低对比，视觉上不干扰但随时可查。
 * 放在页头下方而非嵌入标题行，避免与 `joinHeader` 的副标题串在一起。
 */
const BuildStamp = styled.code`
  align-self: flex-start;
  font-family: var(--font-mono);
  font-size: 10px;
  line-height: 1.4;
  color: var(--aegis-stone);
  user-select: text;
`;

const BootLine = styled.p`
  font-size: var(--aegis-type-caption-size);
  color: var(--aegis-stone);
  margin: 0;
`;

/**
 * 分区级失败态（2026-10-04 审查严重 4）。
 *
 * 🔴 为什么不复用 `ErrorStateBlock`：那个是整页级的大号居中框（`padding: xxl`），
 * 出现在页面中段的某个分区里会显得像整页崩了。此处改为**行内**一条：
 * 带分区名 + 具体原因 + 重试按钮，位置就落在该分区本该出现的地方。
 *
 * 🔴 存在的理由：此前四个慢变量 query 只解构 `data`，`isError` 从未被读取 ——
 * 采集失败时 `data === undefined`，分区**整段静默消失**，用户无法区分
 * 「本机没这块硬件」与「采集失败了」。本项目三态纪律要求失败必须显式且带原因。
 */
const SectionErrorFrame = styled.div`
  display: flex;
  align-items: center;
  gap: var(--aegis-space-sm);
  flex-wrap: wrap;
  padding: var(--aegis-space-md) var(--aegis-space-lg);
  border: 1px solid var(--aegis-hairline);
  border-left: 3px solid var(--aegis-error);
  border-radius: var(--aegis-radius-sm);
  background: var(--aegis-surface-soft);
  font-size: var(--aegis-type-caption-size);
  color: var(--aegis-mute-on-soft);
`;

const SectionErrorText = styled.span`
  flex: 1 1 240px;
  min-width: 0;
`;

const SectionRetry = styled.button`
  flex: none;
  border: 1px solid var(--aegis-accent-strong);
  background: transparent;
  color: var(--aegis-accent-strong);
  font: inherit;
  font-size: var(--aegis-type-caption-size);
  font-weight: 700;
  height: 24px;
  padding: 0 var(--aegis-space-md);
  border-radius: var(--aegis-radius-sm);
  cursor: pointer;

  &:hover {
    background: color-mix(in srgb, var(--aegis-accent) 8%, transparent);
  }
`;

/** 把 TanStack 的 `error` 归一成一句可读文本（`BackendUnavailableError` 等都是 Error 子类） */
function errorText(error: unknown): string {
  if (error instanceof Error) return error.message;
  return String(error);
}

/** 分区失败态：`isError` 为真时替代该分区的正常内容 */
function SectionError({
  isError,
  error,
  sectionTitle,
  retryLabel,
  onRetry,
}: {
  isError: boolean;
  error: unknown;
  sectionTitle: string;
  retryLabel: string;
  onRetry: () => void;
}) {
  if (!isError) return null;
  return (
    <SectionErrorFrame role="alert">
      <SectionErrorText>{`${sectionTitle}：${errorText(error)}`}</SectionErrorText>
      <SectionRetry type="button" onClick={onRetry}>
        {retryLabel}
      </SectionRetry>
    </SectionErrorFrame>
  );
}

/**
 * 母本「网络与存储健康」区的`UniformGrid Columns="2"`（`OverviewView.xaml` 第 417行）
 * 的等价物：网络端点与磁盘健康两个面板并排。
 * 与 `DetailGrid` 同样用**固定两列**而非 auto-fit（母本是硬 `Columns=2`）。
 */
const HealthGrid = styled.div`
  display: grid;
  grid-template-columns: repeat(2, minmax(0, 1fr));
  gap: var(--aegis-space-md);
  align-items: start;
`;

/**
 * 母本 `UniformGrid Columns="2"`（`OverviewView.xaml` 第 249-252 行）的等价物。
 *
 * 🔴 用**固定两列**而非 `auto-fit`：母本是硬 `Columns=2`；`auto-fit` 会按可用宽度
 * 算出三列/四列（实测 1280 窗口下 `minmax(320px,1fr)` 排成三列），与母本不符。
 * 三张卡时第三张落到第二行左侧，右侧留白 —— 母本 `UniformGrid` 也是这个行为。
 */
const DetailGrid = styled.div`
  display: grid;
  grid-template-columns: repeat(2, minmax(0, 1fr));
  gap: var(--aegis-space-md);
  align-items: start;
`;

/**
 * 首拍在途的**轻量**占位：只一行小字，不是骨架屏。
 *
 * 🔴 为什么不显示骨架：本页是启动首屏，整页骨架会让应用看起来"卡住了"；
 * 而四卡区域此时本来就没有内容可画（真实数据要等硬件枚举），画占位条反而是假信息。
 */
function BootPlaceholder() {
  const { t } = useTranslation("overview");
  return (
    <BootLine role="status" aria-live="polite">
      {t("state.boot.collecting")}
    </BootLine>
  );
}

/**
 * 🟢 构建时刻（由 `vite.config.ts` 的 `define` 在打包时注入为字面量，dev 下每次重启刷新）。
 *
 * 🔴 `import.meta.env.DEV` 守卫是**必需**的，不是洁癖：`define` 一旦被移除而本段未同步移除，
 * `as string` 的类型断言会把 `undefined` 变成字面量字符串直接渲染到界面上
 * （类型系统在这里帮不上忙 —— `as` 就是断言）。开发期诊断标记不得进入正式界面。
 */
const BUILD_STAMP = import.meta.env.DEV ? (import.meta.env.VITE_BUILD_STAMP as string) : null;

export function OverviewPage() {
  const { t } = useTranslation("overview");
  const { live, data, isPending, isError, error, refetch, isFetching } = useOverviewLive();
  // 🔴 四条慢变量 query 必须接出 `isError` / `error` / `refetch`：
  // 只解构 `data` 时，采集失败与「本机无此数据」在界面上**完全一样**（分区消失）。
  const processesQuery = useTopProcesses();
  const endpointsQuery = useNetEndpoints();
  const softwareQuery = useSoftwareList();
  const staticQuery = useStaticInfo();
  const processesData = processesQuery.data;
  const endpointsData = endpointsQuery.data;
  const softwareData = softwareQuery.data;
  const staticData = staticQuery.data;
  // 冷启动快路径：命中快照则详情区立即可读（不等数秒的WMI 采集）
  useSnapshotWarmup();
  const refreshAll = useManualRefresh();

  // F-02 / F-03 共用一份上行，按 kind 分流（与 F-11 导出报告走同一个 splitStaticCards，
  // 避免界面与报告各写一套分流而漂移）
  const { detailCards, systemCards } = splitStaticCards(staticData?.cards ?? []);

  // 🔴 `formatUptime(null)` 返回 null（尚未知）→ 该段由 `joinHeader` 整段省略，
  // 而不是显示成「运行 不足 1 分钟」这种凭空捏造的观测结论。
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
  // F-11：导出按钮**不禁用**——规格要求"点了没反应"必须改成明确提示，故按钮常可点、闸门在处理器里
  const exportReport = useExportReport(data, busy);

  return (
    <Page>
      <PageHeader
        title={t("page.title")}
        sub={sub === "" ? undefined : sub}
        actions={
          <HeadTools>
            <LiveDot live={live} />
            <AppButton
              variant="primary"
              size="sm"
              type="button"
              disabled={exportReport.exporting}
              title={t("report.hint")}
              onClick={exportReport.onClick}
            >
              {t(exportReport.exporting ? "report.exporting" : "report.button")}
            </AppButton>
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

      {/* 🟢 构建时刻标记（开发期诊断，2026-10-04）：连续三次「界面不是我改的那版」后加入。
          连续出现时可用它一眼自证当前窗口跑的是哪次构建，不必靠猜 exe 时间戳。
          🔴 仅 dev 渲染（见 BUILD_STAMP 处的守卫说明）；正式发布前连同
          `vite.config.ts` 的 `define` 一并删除。*/}
      {BUILD_STAMP ? <BuildStamp title={`构建 ${BUILD_STAMP}`}>{BUILD_STAMP}</BuildStamp> : null}

      {/* 🔴 F-12 / 用户 2026-10-04 裁决「启动后第一眼不能是加载态」：
          本机概览是程序启动后**第一个看见的页面**，故**不整页挂骨架屏**。
          首拍在途时只让各分区自然缺席（`data === undefined` 时不渲染卡片区），
          页头与操作按钮立即可用——用户看到的是「标题 + 空分区」而非整页骨架。
          真失败仍走下面的 ErrorStateBlock（明确显示，不静默）。*/}
      {isPending && !data ? <BootPlaceholder /> : null}

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

          {/* F-02 硬件详情区（母本「详细信息 / DETAILS」分区，Columns=2） */}
          {detailCards.length > 0 && (
            <>
              <SectionTitle>{t("detail.sectionTitle")}</SectionTitle>
              <DetailGrid>
                {detailCards.map((card, idx) => (
                  <KeyValueCard key={`${card.kind}-${card.index}-${idx}`} card={card} />
                ))}
              </DetailGrid>
            </>
          )}

          {/* F-03 系统信息区（母本「系统信息 / SYSTEM INFORMATION」分区，Columns=2） */}
          {systemCards.length > 0 && (
            <>
              <SectionTitle>{t("systemInfo.sectionTitle")}</SectionTitle>
              <DetailGrid>
                {systemCards.map((card, idx) => (
                  <KeyValueCard key={`sys-${card.kind}-${idx}`} card={card} />
                ))}
              </DetailGrid>
            </>
          )}

          {/* 🔴 F-02/F-03 采集失败（静态上行）——此前只判`data` 真值，
              失败时两区一起静默消失，与「本机没这些硬件」无法区分。 */}
          <SectionError
            isError={staticQuery.isError}
            error={staticQuery.error}
            sectionTitle={t("detail.sectionTitle")}
            retryLabel={t("refresh.label")}
            onRetry={() => void staticQuery.refetch()}
          />

          {/* F-04 进程榜（CPU/内存双榜单，2 秒心跳更新） */}
          {processesData && (
            <>
              <SectionTitle>{t("processes.sectionTitle")}</SectionTitle>
              <DualProcessTables data={processesData} />
            </>
          )}

          <SectionError
            isError={processesQuery.isError}
            error={processesQuery.error}
            sectionTitle={t("processes.sectionTitle")}
            retryLabel={t("refresh.label")}
            onRetry={() => void processesQuery.refetch()}
          />

          {/* F-05 网络端点 + F-06 磁盘健康 —— 母本把它们合为**同一个分区**
              「网络与存储健康 / NETWORK & STORAGE HEALTH」，区内 `UniformGrid Columns="2"`
              并排放两个面板（`OverviewView.xaml` 第 411-419行）。
              原实现拆成两个独立分区各带标题，故两者上下堆叠 —— 与母本不符。 */}
          {(endpointsData || (data.diskHealth && data.diskHealth.length > 0)) && (
            <>
              <SectionTitle>{t("health.sectionTitle")}</SectionTitle>
              <HealthGrid>
                {endpointsData ? <NetEndpointsPanel data={endpointsData} /> : null}
                {data.diskHealth && data.diskHealth.length > 0 ? (
                  <DiskHealthPanel rows={data.diskHealth} />
                ) : null}
              </HealthGrid>
            </>
          )}

          {/* 🔴 F-05 端点采集失败——此前同样只判 `data`，失败时该面板无声消失 */}
          <SectionError
            isError={endpointsQuery.isError}
            error={endpointsQuery.error}
            sectionTitle={t("endpoints.title")}
            retryLabel={t("refresh.label")}
            onRetry={() => void endpointsQuery.refetch()}
          />

          {/* F-07 已安装软件清单（慢变量，不进 2 秒节拍） */}
          {softwareData && softwareData.apps.length > 0 && (
            <>
              <SectionTitle>{t("software.sectionTitle")}</SectionTitle>
              <SoftwareListPanel data={softwareData} />
            </>
          )}

          <SectionError
            isError={softwareQuery.isError}
            error={softwareQuery.error}
            sectionTitle={t("software.sectionTitle")}
            retryLabel={t("refresh.label")}
            onRetry={() => void softwareQuery.refetch()}
          />
        </>
      ) : null}
    </Page>
  );
}
