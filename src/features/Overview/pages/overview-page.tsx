import { useMemo } from "react";
import { useTranslation } from "react-i18next";
import styled from "@emotion/styled";
import { useRegisterBrand } from "@/shell/brand-bar";
import { AppButton } from "@/ui/components/app-button";
import { LiveDot } from "@/ui/components/live-dot";
import { StatCard, StatCardSlot } from "@/ui/components/stat-card";
import { SectionTitle } from "@/ui/components/section-title";
import { ErrorStateBlock } from "@/ui/components/state-block";
import { FailureDetail, failureDetail, failureReason } from "@/ui/components/failure-detail";
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
  /* 🔴 2026-10-06：与 file-backup-page 同口径补上 width: 100%。
     没有它，下面的 margin: 0 auto 产生的 auto 外边距会**吃掉**容器剩余宽度
     （flex 列里 auto 外边距优先于 align-items: stretch），整页塌到内容宽并居中
     —— 实测本页内容只有 525px 宽、左边落在 x=360，破坏「内容左缘 x=104」契约。 */
  width: 100%;
  margin: 0 auto;
  display: flex;
  flex-direction: column;
  /* 🔴 2026-10-04：间距节奏的**唯一**来源。
     原先 SectionTitle 自带 margin: 16px 0 12px，而本容器已有 gap: 16px
     —— flex 的 gap 与子元素 margin **相加不抵消**，分区标题上方实际是 32px、
     下方 28px，而普通内容之间只有 16px ⇒ 标题比它所属的内容更贴近上一区，
     「层级不清」由此产生。现把 margin 归零（见 section-title.tsx），
     **全部垂直节奏由这一处 gap 决定**，改一处即全局生效。 */
  gap: var(--aegis-space-lg);
`;

/* 🔴 2026-10-06 删除 `HeadTools` 包装层：它唯一的作用是声明 `gap: md`，而间距的
   唯一声明处是壳层 `Acts`（规范 §三 brand-bar 行）。留着它就是同一排两套节奏。
   动作槽现在直接传 Fragment ⇒ LiveDot 与两枚按钮成为 Acts 的项。 */

// 四卡一排（母本 UniformGrid Columns=4 的等价物）：等宽、随行拉伸，
// 显卡按 R-01/R-07 可能是 0~n 张，故用 auto-fit 而非硬写四列——卡数变化时不留空槽。
// 🔴 2026-10-04：原 `CardGrid` 已被下面的 `StaggerGrid` 取代（网格规则完全相同，
// 额外承担一次性进场编排），故删除而非保留两份几乎一样的网格声明。

/**
 * 🟢 分区进场：一次性编排，不是每段各自散落的淡入（2026-10-04）。
 *
 * 🔴 为什么只在「首屏」播、且只播一次：慢变量（软件清单、磁盘健康）要数秒才到，
 * 用户会**分段**看着页面填满。若每段进场都播一次淡入，等于每隔 2 秒有一次
 * 视觉跳动 —— 那是"持续动效"的另一种形式，正是规范 §8.3 要避免的。
 * 故：仅四卡（首屏第一帧就有）参与进场，其余分区**直接出现**。
 *
 * 🔴 为什么用 `transform` + `opacity` 而非 `max-height`：这两个属性走合成层，
 * 不触发布局；`animation-fill-mode: backwards` 让元素在**延迟期间**就位，
 * 避免「先出现在最终位置、然后又跳回起点」的双重闪动。
 */
const StaggerGrid = styled.div`
  display: grid;
  grid-template-columns: repeat(auto-fit, minmax(228px, 1fr));
  gap: var(--aegis-space-md);

  > * {
    animation: aegis-card-enter var(--aegis-motion-duration-normal)
      var(--aegis-motion-easing-decelerate) backwards;
    /* 逐卡 40ms 递进：四张卡在 120ms 内依次落位，读起来是「一次动作」而非四个独立动画。
       🔴 用 nth-of-type 而非 nth-child：emotion 在 SSR 时对 nth-child 报不安全
       （子元素类型不同则序号会错位，而这里子元素全是 StatCard 的 section），
       nth-of-type 按**元素类型**计数，语义更稳。 */
    &:nth-of-type(1) {
      animation-delay: 0ms;
    }
    &:nth-of-type(2) {
      animation-delay: 40ms;
    }
    &:nth-of-type(3) {
      animation-delay: 80ms;
    }
    &:nth-of-type(4) {
      animation-delay: 120ms;
    }
  }

  @keyframes aegis-card-enter {
    from {
      opacity: 0;
      transform: translateY(6px);
    }
    to {
      opacity: 1;
      transform: none;
    }
  }

  /* 规范 §8.3：reduced-motion 下**完全不播**。位移 6px 与淡入都是纯装饰，
     四卡内容不受影响，且本项目已恢复全局兜底（同时限时长与次数）作为双保险。 */
  @media (prefers-reduced-motion: reduce) {
    > * {
      animation: none;
    }
  }
`;

// 母本 `UniformGrid Columns="2"`（OverviewView.xaml 第 251/317 行）的等价物：
// 详情卡按 kind 分流后逐行落位，同类多设备（如多显卡）自然并排，不留空槽。
// （实现在下方 `DetailGrid`。）
/**
 * 首拍在途的**轻量**占位：只一行小字，不是骨架屏。
 *
 * 🔴 为什么不显示骨架：本页是启动首屏，整页骨架会让应用看起来"卡住了"；
 * 而四卡区域此时本来就没有内容可画（真实数据要等硬件枚举），画占位条反而是假信息。
 *
 * 🟢 2026-10-04 补「进度感」：这一行是用户在首屏**唯一**能看到的"正在采集"信号，
 * 但原先它是纯静态文字，2~6 秒的硬件枚举期间界面完全无反馈。
 * 现给它一个 1.6s 的呼吸（opacity 0.45↔1）—— 循环周期刻意**长于**数据节拍，
 * 使��"系统在忙"读起来是缓慢稳定的，而非与 2 秒心跳赛跑的躁动。
 */
const BootLine = styled.p`
  font-size: var(--aegis-type-caption-size);
  /* 🔴 文字一律 mute 系：stone 在 §六 #6 已被降级为 **UI-only**（图标/边框，3.64:1 不足正文 4.5）。
     本行是首屏正文级的「正在采集」提示 ⇒ 用 mute（canvas 上 6.60:1 ✅）。2026-10-06 UI 审查 M12。
     ⚠️ 本注释在模板字符串内，禁写反引号（会报 TS1005）。 */
  color: var(--aegis-mute);
  margin: 0;
  animation: aegis-boot-breathe 1.6s var(--aegis-motion-easing-standard) infinite;

  @keyframes aegis-boot-breathe {
    0%,
    100% {
      opacity: 1;
    }
    50% {
      opacity: 0.45;
    }
  }

  /* 规范 §8.3：reduced-motion 下停帧。文字照常显示（信息不丢），只是不再呼吸。 */
  @media (prefers-reduced-motion: reduce) {
    animation: none;
  }
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
  /* 🟢 反馈补全（2026-10-04）：原先只有 hover 底色，缺按下态与键盘焦点态。
     焦点环由全局 :focus-visible 提供（2px accent-strong，浅底合规），
     这里**不**写 outline: none —— 规范 §八「禁 outline:none 且无替代焦点指示」。 */
  transition: background-color var(--aegis-motion-duration-fast) var(--aegis-motion-easing-standard);

  &:hover {
    background: color-mix(in srgb, var(--aegis-accent) 8%, transparent);
  }
  &:active {
    background: color-mix(in srgb, var(--aegis-accent) 16%, transparent);
  }

  @media (prefers-reduced-motion: reduce) {
    transition: none;
  }
`;

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
      {/* 🔴 主线只到「为什么」为止（UI 审查 D7）：后端命令名收进下面的折叠件，
          它同时仍是报障时唯一的定位手段，故是收起来而不是删掉。 */}
      <SectionErrorText>{`${sectionTitle}：${failureReason(error)}`}</SectionErrorText>
      <FailureDetail detail={failureDetail(error)} />
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

/* 🔴 2026-10-05 用户裁决删除「构建时刻标记」（原 `BUILD_STAMP` + `BuildStamp`）。
   连带删除 `vite.config.ts` 的 `define: { VITE_BUILD_STAMP }`。
   保留原因说明备查：它是开发期诊断件（仅 `import.meta.env.DEV` 渲染），
   用于「一眼自证当前窗口跑的是哪次构建」；注释里本就写明「正式发布前连同 define 一并删除」，
   现在提前执行。删 `define` 时**必须**连本段一起删 —— 否则 `as string` 断言会把
   `undefined` 渲染成字面量字符串。*/

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

  /* 品牌区注册（2026-10-05 垂直迁移第 7 条）：主标题归壳层统一渲染（由路由从
     modules.ts 派生），页面只上报**副标题**与**动作槽**。
     🔴 actions 必须 memo —— useRegisterBrand 的依赖数组含它，
        每次渲染新建的 React 元素会让 BrandBar 每帧重渲染（渲染风暴）。 */
  const brandActions = useMemo(
    () => (
      <>
        <LiveDot live={live} />
        {/* 🔴 2026-10-05 用户裁决：**「导出报告」移到「立即刷新」右边**。
            原来的顺序是「导出 → 刷新」，primary 按钮紧邻右边界；调换后
            secondary 的「立即刷新」成为最右项。
            ⚠️ 原注释说右缘"留 20（padding-right）"—— 该值早在 2026-10-05 就被推翻
               （`Bar` 现为 `padding-right: 0`，动作槽右缘 1260 与卡片描边同一条竖线）。 */}
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
      </>
    ),
    [live, exportReport.exporting, exportReport.onClick, t, busy, refreshAll],
  );
  useRegisterBrand("/overview", { sub: sub === "" ? undefined : sub, actions: brandActions });

  return (
    <Page>
      {/* 🔴 2026-10-05 用户裁决删除「构建时刻标记」（`BUILD_STAMP`，形如 `19:33:04`）。
          它是开发期诊断件（仅 dev 渲染），本就该在发布前删掉 —— 现在提前执行。
          🔴 `vite.config.ts` 的 `define: { VITE_BUILD_STAMP }` **同步删除**，
             否则留着死代码。*/}

      {/* 🔴 F-12 / 用户 2026-10-04 裁决「启动后第一眼不能是加载态」：
          本机概览是程序启动后**第一个看见的页面**，故**不整页挂骨架屏**。
          首拍在途时只让各分区自然缺席（`data === undefined` 时不渲染卡片区），
          页头与操作按钮立即可用——用户看到的是「标题 + 空分区」而非整页骨架。
          真失败仍走下面的 ErrorStateBlock（明确显示，不静默）。*/}
      {isPending && !data ? <BootPlaceholder /> : null}

      {isError ? (
        <>
          <ErrorStateBlock
            title={t("state.error.title")}
            /* 🔴 主线只说「为什么」，命令名进折叠（UI 审查 D7）。 */
            body={failureReason(error)}
            detail={failureDetail(error)}
            onRetry={() => void refetch()}
            onRetryLabel={t("refresh.label")}
            /* 🔴 2026-10-06 批次⑤：工具栏已有主 CTA（导出报告 = primary），
               失败态里的重试再出 primary 就是两个同权重主按钮争抢，且文案同为「立即刷新」。
               降级为 secondary：重试仍可用，视觉让位给工具栏。 */
            retryVariant="secondary"
          />
          {/* 🔴 失败态不塌陷（UI 审查 m2，实测下方空白约 700px）：四卡的**轮廓**照常在场，
              只是内部是静止占位条。读屏用 aria-hidden 跳过它们 —— 失败信息已由上面的
              ErrorStateBlock（aria-live=assertive）说完了，占位卡对读屏是纯噪音。 */}
          <StaggerGrid aria-hidden="true">
            {[0, 1, 2, 3].map((i) => (
              <StatCardSlot key={i} />
            ))}
          </StaggerGrid>
        </>
      ) : null}

      {data ? (
        <>
          <StaggerGrid>
            {data.cards.map((card) => (
              <StatCard key={`${card.kind}-${card.index}`} card={card} />
            ))}
          </StaggerGrid>
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
