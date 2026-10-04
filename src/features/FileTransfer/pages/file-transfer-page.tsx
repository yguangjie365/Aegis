import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import styled from "@emotion/styled";
import { AppButton } from "@/ui/components/app-button";
import { PageHeader } from "@/ui/components/page-header";
import { log, newCorrelationId } from "@/lib/logging";
import { useTransferDrop } from "../hooks/use-transfer-drop";
import {
  usePickFilesToSend,
  useRefreshTransferPeers,
  useSaveTransferSettings,
  useSendTransferFile,
  useTransferPeers,
  useTransferServiceControl,
  useTransferSettings,
  useTransferStatus,
} from "../hooks/use-transfer";
import { asTransferError } from "@/lib/tauri/transfer";
import type { KnownPeer, TransferPeer, TransferSettings } from "@/types/transfer";
import { formatCountdown, isExpiringSoon } from "../lib/countdown";
import type { DropRowRect } from "../lib/drop-target";
import {
  NEW_DEVICE_NAME,
  addKnownPeer,
  createKnownPeerRow,
  isValidPort,
  isSamePeer,
  removeKnownPeer,
} from "../lib/known-peers";
import { validate, sizeLine } from "../lib/text-metrics";

/**
 * 文件互传页（M-05 片1：电脑互传通道）。
 *
 * 规格：`Docs/功能规格/05-文件互传.md`；页面结构见其 §三 3.1。
 * 布局母本：`Source/SystemToolkit/src/SystemToolkit.Modules.FileTransfer/FileTransferView.xaml`（③仅布局参照，零 XAML 复制）。
 * 线框经用户批准（2026-10-04）后编码，满足「UI 先审后写」。
 *
 * 🟠 本批只落**页面骨架与空态**：服务启停、设备发现、传输任务、历史均待后端命令面
 * （协议与引擎随 `crates/services/src/transfer/` 下一批落地）。**不接任何假数据**——
 * 空态一律按规格 R-29「空态与失败态必带原因」写实，宁可为空也不摆占位数字。
 *
 * 🔴 节奏档位照参照件（行内 4 / 行距 8 / 组距 12 / 卡距 14→本工程 `space-lg`）。
 * 控件高度用既有 `AppButton` 的 `md`(36) / `sm`(28) 两档——与线框图的 32/26 有微差，
 * 以「复用既有组件」优先（DP-1），差异已登记变更记录待用户目验。
 */

const Wrap = styled.div`
  display: flex;
  flex-direction: column;
  gap: var(--aegis-space-md);
`;

const Tabs = styled.div`
  display: flex;
  gap: var(--aegis-space-xs);
  border-bottom: 1px solid var(--aegis-hairline);
`;

const Tab = styled.button<{ active?: boolean }>`
  appearance: none;
  border: 0;
  background: none;
  font: inherit;
  font-size: 14px;
  padding: 9px var(--aegis-space-lg);
  margin-bottom: -1px;
  cursor: pointer;
  color: ${(p) => (p.active ? "var(--aegis-ink)" : "var(--aegis-mute)")};
  font-weight: ${(p) => (p.active ? 700 : 400)};
  border-bottom: 2px solid ${(p) => (p.active ? "var(--aegis-accent)" : "transparent")};
  &:disabled {
    color: var(--aegis-disabled);
    cursor: not-allowed;
  }
`;

const Card = styled.section`
  background: var(--aegis-surface-soft);
  border: 1px solid var(--aegis-hairline);
  border-radius: var(--aegis-radius-sm);
  padding: var(--aegis-space-lg);
`;

const CardHead = styled.div`
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: var(--aegis-space-md);
  flex-wrap: wrap;
  margin-bottom: var(--aegis-space-md);
`;

const CardTitle = styled.h2`
  margin: 0;
  font-size: 17px;
  font-weight: 700;
  line-height: 1.4;
`;

const Acts = styled.div`
  display: flex;
  align-items: center;
  gap: var(--aegis-space-sm);
`;

const Cols = styled.div`
  display: grid;
  /* 🔴 必须 minmax(0,1fr)：1fr 实为 minmax(auto,1fr) 带 min-content 下限，
     左卡编辑行的最小宽度会把右卡挤成一条竖排文字（线框阶段实测踩到）。 */
  grid-template-columns: repeat(2, minmax(0, 1fr));
  gap: var(--aegis-space-lg);
  align-items: start;
`;

const Form = styled.div`
  display: grid;
  grid-template-columns: auto 1fr auto auto;
  align-items: center;
  gap: var(--aegis-space-sm);
`;

const Label = styled.span`
  font-size: 14px;
  white-space: nowrap;
`;

const Input = styled.input`
  height: 32px;
  width: 100%;
  padding: 0 10px;
  font: inherit;
  font-size: 14px;
  color: var(--aegis-ink);
  background: var(--aegis-surface-inset);
  border: 1px solid var(--aegis-hairline);
  border-radius: var(--aegis-radius-sm);
`;

const MonoInput = styled(Input)`
  font-family: var(--font-mono);
  font-size: 13px;
`;

const PortInput = styled(MonoInput)`
  width: 92px;
`;

const Select = styled.select`
  height: 32px;
  padding: 0 var(--aegis-space-sm);
  font: inherit;
  font-size: 14px;
  color: var(--aegis-ink);
  background: var(--aegis-surface-inset);
  border: 1px solid var(--aegis-hairline);
  border-radius: var(--aegis-radius-sm);
`;

const Hint = styled.span<{ span?: boolean }>`
  font-size: 12px;
  color: var(--aegis-mute);
  grid-column: ${(p) => (p.span ? "2 / -1" : "auto")};
`;

const CheckRow = styled.div`
  display: flex;
  gap: var(--aegis-space-lg);
  flex-wrap: wrap;
  grid-column: 2 / -1;
`;

const Check = styled.label`
  display: inline-flex;
  align-items: center;
  gap: var(--aegis-space-sm);
  font-size: 14px;
  white-space: nowrap;
`;

const List = styled.div`
  background: var(--aegis-canvas);
  border: 1px solid var(--aegis-hairline);
  border-radius: var(--aegis-radius-sm);
  min-height: 96px;
`;

const Empty = styled.div`
  display: flex;
  flex-direction: column;
  justify-content: center;
  align-items: flex-start;
  gap: 2px;
  min-height: inherit;
  padding: 0 10px;
`;

const EmptyTitle = styled.span`
  font-size: 14px;
`;

const EmptyHint = styled.span`
  font-size: 11px;
  color: var(--aegis-mute);
`;

const EditRow = styled.div`
  display: flex;
  align-items: center;
  gap: var(--aegis-space-sm);
  margin-top: var(--aegis-space-md);
  flex-wrap: wrap;
`;

const TextArea = styled.textarea`
  width: 100%;
  min-height: 64px;
  max-height: 160px;
  padding: 8px 10px;
  font: inherit;
  font-size: 14px;
  line-height: 1.5;
  color: var(--aegis-ink);
  background: var(--aegis-surface-inset);
  border: 1px solid var(--aegis-hairline);
  border-radius: var(--aegis-radius-sm);
  resize: vertical;
`;

const PriceRow = styled.div`
  display: flex;
  align-items: center;
  gap: var(--aegis-space-md);
  margin-top: var(--aegis-space-sm);
`;

const SectionDivider = styled.div`
  display: flex;
  gap: var(--aegis-space-sm);
  border-top: 1px solid var(--aegis-hairline);
  margin-top: var(--aegis-space-md);
  padding-top: var(--aegis-space-md);
`;

const Board = styled.div`
  background: var(--aegis-canvas);
  border: 1px solid var(--aegis-hairline);
  border-radius: var(--aegis-radius-sm);
  min-height: 90px;
`;

const Head = styled.div`
  display: flex;
  gap: var(--aegis-space-sm);
  padding: 6px 10px;
  background: var(--aegis-surface-inset);
  border-bottom: 1px solid var(--aegis-hairline);
  font-size: 12px;
  font-weight: 700;
  letter-spacing: 0.04em;
  color: var(--aegis-mute);
`;

const Filters = styled.div`
  display: grid;
  grid-template-columns: auto auto minmax(0, 1fr);
  gap: var(--aegis-space-sm);
  margin-bottom: var(--aegis-space-sm);
`;

const LogBar = styled.div`
  display: flex;
  align-items: center;
  gap: var(--aegis-space-md);
  padding-top: var(--aegis-space-sm);
  border-top: 1px solid var(--aegis-hairline);
`;

const LogToggle = styled.button`
  appearance: none;
  border: 0;
  background: none;
  font: inherit;
  font-size: 14px;
  font-weight: 700;
  color: var(--aegis-ink);
  cursor: pointer;
  padding: 0;
`;

const LogBox = styled.div`
  background: var(--aegis-chrome-dark);
  color: var(--aegis-on-dark-mute);
  border-radius: var(--aegis-radius-sm);
  padding: 6px 10px;
  max-height: 112px;
  overflow: auto;
  font-family: var(--font-mono);
  font-size: 13px;
  line-height: 1.6;
`;

const Badge = styled.span<{ on?: boolean }>`
  display: inline-flex;
  align-items: center;
  height: 24px;
  padding: 0 10px;
  border-radius: var(--aegis-radius-full);
  font-size: 12px;
  font-weight: 700;
  white-space: nowrap;
  background: ${(p) => (p.on ? "#eaf6e0" : "#fdecec")};
  color: ${(p) => (p.on ? "#2d6200" : "#8f1212")};
`;

export function FileTransferPage() {
  const { t } = useTranslation("fileTransfer");

  // ── 后端状态（M-05 片1 命令面，2026-10-04 接线） ──
  // 🔴 不做乐观更新：`serviceRunning` 一律取自后端快照（R-08「状态诚实」）。
  const statusQuery = useTransferStatus();
  const settingsQuery = useTransferSettings();
  const peersQuery = useTransferPeers();
  const control = useTransferServiceControl();
  const saveSettings = useSaveTransferSettings();
  const refreshPeers = useRefreshTransferPeers();

  const status = statusQuery.data;
  const serviceRunning = status?.running ?? false;
  const loaded = settingsQuery.data;
  const persisted = loaded?.settings;

  // 表单是本机编辑副本；设置载入成功后灌一次，此后由用户编辑驱动
  const [receiveDir, setReceiveDir] = useState("");
  const [tcpPort, setTcpPort] = useState("18889");
  const [udpPort, setUdpPort] = useState("18888");
  const [policy, setPolicy] = useState("rename");
  const [knownPeerOnly, setKnownPeerOnly] = useState(true);
  // 🔴 Q-15 裁决（2026-10-04，用户）：接收前询问**默认开启**，全链路统一。
  // 该初值是"载入前"的占位；载入后由后端设置覆盖（后端默认同样为开）。
  const [askBeforeReceive, setAskBeforeReceive] = useState(true);
  const [text, setText] = useState("");
  const [logOpen, setLogOpen] = useState(true);
  const [seeded, setSeeded] = useState(false);

  useEffect(() => {
    if (seeded || persisted === undefined) return;
    setReceiveDir(persisted.receiveDirectory ?? "");
    setTcpPort(String(persisted.transferPort));
    setUdpPort(String(persisted.discoveryPort));
    setPolicy(persisted.conflictPolicy);
    setKnownPeerOnly(persisted.requireKnownPeer);
    setAskBeforeReceive(persisted.requireReceiveConfirmation);
    setSeeded(true);
  }, [persisted, seeded]);

  /** 提交一次设置变更（F-FT-59：合法即自动保存，无需"保存"按钮）。 */
  const commit = useCallback(
    (patch: Partial<TransferSettings>) => {
      if (persisted === undefined) return;
      saveSettings.mutate({ ...persisted, ...patch });
    },
    [persisted, saveSettings],
  );

  const textValidation = useMemo(() => validate(text), [text]);

  /** 服务动作的失败文案（R-28：失败必须可见，不静默）。 */
  const actionError =
    control.start.error !== null
      ? t("service.startFailed", { reason: asTransferError(control.start.error).message })
      : control.stop.error !== null
        ? t("service.stopFailed", { reason: asTransferError(control.stop.error).message })
        : null;

  /** 停止"不完整"时如实提示（R-28：不得宣称已停止）。 */
  const stopIncomplete =
    control.stop.data !== undefined && !control.stop.data.allStopped
      ? t("service.stopIncomplete", { reason: control.stop.data.message })
      : null;

  // 🔴 `?? []` 必须包在 useMemo 里：否则每次渲染都造一个新数组，
  // 使下面依赖 `peers` 的 useCallback 每轮都变（react-hooks/exhaustive-deps 是 error 级）
  const peers = useMemo(() => peersQuery.data ?? [], [peersQuery.data]);
  const peerError = peersQuery.error !== null ? asTransferError(peersQuery.error).message : null;

  // ── 目标选择与发送（F-FT-11/12；R-7：目标不许猜测） ──
  // 按钮路径用"所选行"、拖放路径用"落点命中哪一行"——两条路径都不猜。
  const [selectedPeerId, setSelectedPeerId] = useState<string | null>(null);
  const [notice, setNotice] = useState<string | null>(null);
  const send = useSendTransferFile();
  const pickFiles = usePickFilesToSend();

  // 设备行的 DOM 引用（拖放命中测试用；每次事件现取，避免与列表变化脱节）
  const rowRefs = useRef(new Map<string, HTMLElement>());

  const collectRows = useCallback(
    (): DropRowRect[] =>
      peers
        .map((peer) => {
          const element = rowRefs.current.get(peer.deviceId);
          if (element === undefined) return null;
          const rect = element.getBoundingClientRect();
          return {
            id: peer.deviceId,
            left: rect.left,
            top: rect.top,
            right: rect.right,
            bottom: rect.bottom,
          };
        })
        .filter((row): row is DropRowRect => row !== null),
    [peers],
  );

  /** 把一批路径发给某个设备（F-FT-11：每个文件一条"入队发送"日志）。 */
  const sendPathsTo = useCallback(
    (rowId: string, paths: string[]): void => {
      const peer = peers.find((candidate) => candidate.deviceId === rowId);
      if (peer === undefined) {
        // 命中了已消失的行（列表刚刷新）——如实提示，不静默丢弃
        setNotice(t("send.targetMissing"));
        return;
      }
      const peerLabel = `${peer.ip}:${peer.port}`;
      for (const path of paths) {
        const name = path.split(/[\\/]/).pop() ?? path;
        // F-FT-11 的原文日志："入队发送：{文件名} → {ip}:{端口}"
        log({
          level: "info",
          module: "Transfer",
          action: "transferSendFile",
          result: "Success",
          correlationId: newCorrelationId(),
        });
        setNotice(t("send.queued", { name, peer: peerLabel }));
        send.mutate({ ip: peer.ip, port: peer.port, path });
      }
    },
    [peers, send, t],
  );

  const drop = useTransferDrop({
    rows: collectRows,
    onDropOnRow: sendPathsTo,
    // 🔴 R-7 的原文：不猜测目标，要求用户重新拖到行上
    onDropOutside: () => setNotice(t("send.dropOutside")),
  });

  /** 按钮路径：发给"所选设备"。没选就明确提示，不静默。 */
  const sendPickedToSelection = useCallback(async (): Promise<void> => {
    if (selectedPeerId === null) {
      setNotice(t("send.needSelection"));
      return;
    }
    const paths = await pickFiles();
    if (paths.length === 0) return; // 用户取消：零副作用
    sendPathsTo(selectedPeerId, paths);
  }, [pickFiles, selectedPeerId, sendPathsTo, t]);

  // ── 已知设备清单（F-FT-10） ──
  // 🔴 这一栏是**单机自测**的入口：自己的宣告被按 R-6 过滤掉，故"发现的设备"里永远没有本机；
  // 想在同一台机器上验一遍收发，就得手动加一条 127.0.0.1:端口。
  const knownPeers = useMemo(() => persisted?.knownPeers ?? [], [persisted]);
  const [selectedKnownIndex, setSelectedKnownIndex] = useState<number | null>(null);
  const selectedKnown = selectedKnownIndex === null ? undefined : knownPeers[selectedKnownIndex];
  const [knownDraft, setKnownDraft] = useState({ name: "", ip: "", port: "18889" });
  const [knownPortError, setKnownPortError] = useState<string | null>(null);

  /** 发给一条已知设备（按钮路径；与拖放路径互不混淆）。 */
  const sendKnownPeer = useCallback(
    async (peer: KnownPeer): Promise<void> => {
      const paths = await pickFiles();
      if (paths.length === 0) return;
      for (const path of paths) {
        const name = path.split(/[\\/]/).pop() ?? path;
        log({
          level: "info",
          module: "Transfer",
          action: "transferSendFile",
          result: "Success",
          correlationId: newCorrelationId(),
        });
        setNotice(t("send.queued", { name, peer: `${peer.ip}:${peer.port}` }));
        send.mutate({ ip: peer.ip, port: peer.port, path });
      }
    },
    [pickFiles, send, t],
  );

  /** 「加入已知」（F-FT-10：按 IP+端口去重；重复加入只提示"已加入"）。 */
  const addDiscoveredToKnown = useCallback(
    (peer: TransferPeer): void => {
      if (persisted === undefined) return;
      const candidate: KnownPeer = {
        name: peer.deviceName,
        ip: peer.ip,
        port: peer.port,
      };
      const { list, added } = addKnownPeer(persisted.knownPeers, candidate);
      if (!added) {
        setNotice(t("discovered.added"));
        return;
      }
      // F-FT-10 的原文日志
      log({
        level: "info",
        module: "Transfer",
        action: "transferAddKnownPeer",
        result: "Success",
        correlationId: newCorrelationId(),
      });
      setNotice(
        t("known.addedLog", { name: candidate.name, ip: candidate.ip, port: candidate.port }),
      );
      commit({ knownPeers: list });
    },
    [commit, persisted, t],
  );

  return (
    <Wrap>
      <PageHeader
        title={t("page.title")}
        sub={t("page.sub", {
          devices: peers.length,
          tasks: 0, // 任务视图随传输任务批接入；此处不摆占位数字（R-29）
        })}
        actions={
          <Badge on={serviceRunning}>
            {statusQuery.isPending
              ? t("page.status.stopped")
              : serviceRunning
                ? t("page.status.desktopRunning")
                : t("page.status.stopped")}
          </Badge>
        }
      />

      {/* F-FT-01：双页签。片1 只交付「电脑互传」，手机页签随之禁用并标注去向——
          不做成"点了没反应"的死页签（用户裁决 2026-10-04：先交付电脑互传）。 */}
      <Tabs role="tablist">
        <Tab role="tab" aria-selected active>
          {t("page.tabs.desktop")}
        </Tab>
        <Tab role="tab" aria-selected={false} disabled title={`${t("page.tabs.mobile")}（片2）`}>
          {t("page.tabs.mobile")}
        </Tab>
      </Tabs>

      {/* ① 本机与服务（F-FT-06 端口与目录信息 / F-FT-59 设置持久化） */}
      <Card>
        <CardHead>
          <CardTitle>{t("local.title")}</CardTitle>
          <Acts>
            {/* 🔴 启停是**后端定论**：按钮在动作在飞时禁用，失败一律显示原因（R-28） */}
            <AppButton
              variant={serviceRunning ? "secondary" : "primary"}
              size="sm"
              disabled={control.busy || settingsQuery.isPending}
              onClick={() => {
                if (serviceRunning) {
                  control.stop.mutate();
                } else {
                  control.start.mutate();
                }
              }}
            >
              {control.busy
                ? serviceRunning
                  ? t("page.status.stopping")
                  : t("page.status.starting")
                : serviceRunning
                  ? t("local.stop")
                  : t("local.start")}
            </AppButton>
          </Acts>
        </CardHead>
        {/* 失败与"停止不完整"都必须可见（禁止静默失败 / R-28） */}
        {actionError !== null && <Hint span>{actionError}</Hint>}
        {stopIncomplete !== null && <Hint span>{stopIncomplete}</Hint>}
        {loaded?.corrupt === true && <Hint span>{t("settings.corrupt")}</Hint>}
        {loaded?.fileMissing === true && <Hint span>{t("settings.fileMissing")}</Hint>}
        {loaded !== undefined && loaded.repairs.length > 0 && (
          <Hint span>{t("settings.repairs", { detail: loaded.repairs.join("；") })}</Hint>
        )}
        <Form>
          <Label>{t("local.deviceId")}</Label>
          <span style={{ fontFamily: "var(--font-mono)", fontSize: 13 }}>
            {status?.deviceId ?? "—"}
          </span>
          <Label>{t("pairing.label")}</Label>
          <span style={{ fontFamily: "var(--font-mono)", fontSize: 13 }}>
            {status?.pairingCode ?? t("pairing.placeholder")}
          </span>
          {/* F-FT-38：服务未启动时说明"启动后生成"；运行中给出轮换剩余（本轮 2 秒粒度） */}
          <Hint span>
            {status?.pairingCode == null
              ? t("pairing.notStarted")
              : isExpiringSoon(status.pairingExpiresInMs)
                ? t("pairing.expiringSoon")
                : t("pairing.expiresIn", {
                    time: formatCountdown(status.pairingExpiresInMs),
                  })}
          </Hint>

          <Label>{t("local.receiveDir")}</Label>
          <MonoInput
            value={receiveDir}
            placeholder={t("local.receiveDirMissing")}
            onChange={(e) => setReceiveDir(e.target.value)}
            onBlur={() => commit({ receiveDirectory: receiveDir === "" ? null : receiveDir })}
          />
          <AppButton variant="secondary" size="sm">
            {t("local.browse")}
          </AppButton>
          <AppButton variant="secondary" size="sm">
            {t("local.open")}
          </AppButton>
          <Hint span>{t("space.unknown")}</Hint>

          <Label>{t("local.tcpPort")}</Label>
          <PortInput
            value={tcpPort}
            onChange={(e) => setTcpPort(e.target.value)}
            onBlur={() => {
              const parsed = Number.parseInt(tcpPort, 10);
              // 🔴 端口非法就**不提交**：后端 `validate_ports` 会拒，但让用户先改到合法值
              // 比"提交后被拒再回退"体验好；合法值才落库（F-FT-10 的"非法输入只给行内错误"同取向）
              if (Number.isInteger(parsed) && parsed >= 1024 && parsed <= 65535) {
                commit({ transferPort: parsed });
              }
            }}
          />
          <Hint>{t("local.tcpPortTooltip")}</Hint>
          <span />
          <Label>{t("local.udpPort")}</Label>
          <PortInput
            value={udpPort}
            onChange={(e) => setUdpPort(e.target.value)}
            onBlur={() => {
              const parsed = Number.parseInt(udpPort, 10);
              if (Number.isInteger(parsed) && parsed >= 1024 && parsed <= 65535) {
                commit({ discoveryPort: parsed });
              }
            }}
          />
          <Hint>{t("local.udpPortTooltip")}</Hint>
          <span />
          {/* F-FT-06：端口生效时机只描述当前事实 */}
          <Hint span>
            {serviceRunning ? t("local.portHintRunning") : t("local.portHintStopped")}
          </Hint>

          <Label>{t("local.conflict")}</Label>
          <Select
            value={policy}
            onChange={(e) => {
              setPolicy(e.target.value);
              // 离散控件：选定即提交（F-FT-59"合法即自动保存"）
              commit({ conflictPolicy: e.target.value });
            }}
          >
            <option value="rename">{t("conflict.rename")}</option>
            <option value="ask">{t("conflict.ask")}</option>
            <option value="skip">{t("conflict.skip")}</option>
            <option value="overwrite">{t("conflict.overwrite")}</option>
          </Select>
          <Hint>{t("local.conflictShared")}</Hint>
          <span />
          <Hint span>{t("local.conflictMobileNote")}</Hint>

          <span />
          <CheckRow>
            <Check>
              <input
                type="checkbox"
                checked={knownPeerOnly}
                onChange={(e) => {
                  setKnownPeerOnly(e.target.checked);
                  commit({ requireKnownPeer: e.target.checked });
                }}
              />
              {t("local.requireKnownPeer")}
            </Check>
            <Check>
              <input
                type="checkbox"
                checked={askBeforeReceive}
                onChange={(e) => {
                  setAskBeforeReceive(e.target.checked);
                  commit({ requireReceiveConfirmation: e.target.checked });
                }}
              />
              {t("local.requireConfirm")}
            </Check>
          </CheckRow>
        </Form>
      </Card>

      {/* ② 已知设备 / 局域网设备（F-FT-07~10） */}
      <Cols>
        <Card>
          <CardHead>
            <CardTitle>{t("known.title")}</CardTitle>
            <Acts>
              {/* F-FT-10：默认命名「新设备」并**自动选中该行** */}
              <AppButton
                variant="secondary"
                size="sm"
                onClick={() => {
                  if (persisted === undefined) return;
                  const { list, index } = createKnownPeerRow(persisted.knownPeers);
                  setSelectedKnownIndex(index);
                  setKnownDraft({ name: NEW_DEVICE_NAME, ip: "", port: "18889" });
                  setKnownPortError(null);
                  commit({ knownPeers: list });
                }}
              >
                {t("known.add")}
              </AppButton>
              <AppButton
                variant="secondary"
                size="sm"
                disabled={selectedKnown === undefined}
                onClick={() => {
                  if (persisted === undefined || selectedKnown === undefined) return;
                  setSelectedKnownIndex(null);
                  commit({ knownPeers: removeKnownPeer(persisted.knownPeers, selectedKnown) });
                }}
              >
                {t("known.remove")}
              </AppButton>
              <AppButton
                variant="secondary"
                size="sm"
                disabled={selectedKnown === undefined}
                onClick={() => {
                  if (persisted === undefined || selectedKnown === undefined) return;
                  // 🔴 端口非法 => 行内报错、**不落库**（F-FT-10）
                  if (!isValidPort(knownDraft.port)) {
                    setKnownPortError(t("known.portError"));
                    return;
                  }
                  setKnownPortError(null);
                  const updated = persisted.knownPeers.map((peer) =>
                    isSamePeer(peer, selectedKnown)
                      ? {
                          name: knownDraft.name,
                          ip: knownDraft.ip.trim(),
                          port: Number.parseInt(knownDraft.port, 10),
                        }
                      : peer,
                  );
                  commit({ knownPeers: updated });
                }}
              >
                {t("known.save")}
              </AppButton>
            </Acts>
          </CardHead>
          <List>
            {knownPeers.length === 0 ? (
              <Empty>
                <EmptyTitle>{t("known.emptyTitle")}</EmptyTitle>
                <EmptyHint>{t("known.emptyHint")}</EmptyHint>
              </Empty>
            ) : (
              knownPeers.map((peer, index) => (
                <EditRow
                  key={`${peer.ip}:${peer.port}:${index}`}
                  onClick={() => {
                    setSelectedKnownIndex(index);
                    setKnownDraft({ name: peer.name, ip: peer.ip, port: String(peer.port) });
                    setKnownPortError(null);
                  }}
                  style={{
                    cursor: "pointer",
                    outline:
                      selectedKnownIndex === index
                        ? "1px solid var(--aegis-border-strong, rgba(127,127,127,0.5))"
                        : undefined,
                  }}
                >
                  <span style={{ fontSize: 13 }}>{peer.name}</span>
                  <MonoInput readOnly value={`${peer.ip}:${peer.port}`} style={{ width: 178 }} />
                  {/* F-FT-10：「发送文件」按行发给该设备 */}
                  <AppButton
                    variant="secondary"
                    size="sm"
                    style={{ marginLeft: "auto" }}
                    disabled={send.isPending}
                    onClick={(e) => {
                      e.stopPropagation(); // 别把"发送"当成"选中"
                      void sendKnownPeer(peer);
                    }}
                  >
                    {t("known.sendFile")}
                  </AppButton>
                </EditRow>
              ))
            )}
          </List>
          <EditRow>
            <Label>{t("known.name")}</Label>
            <Input
              style={{ width: 118 }}
              value={knownDraft.name}
              disabled={selectedKnown === undefined}
              onChange={(e) => setKnownDraft({ ...knownDraft, name: e.target.value })}
            />
            <Label>{t("known.ip")}</Label>
            <MonoInput
              style={{ width: 112 }}
              value={knownDraft.ip}
              disabled={selectedKnown === undefined}
              onChange={(e) => setKnownDraft({ ...knownDraft, ip: e.target.value })}
            />
            <Label>{t("known.port")}</Label>
            <PortInput
              style={{ width: 78 }}
              value={knownDraft.port}
              disabled={selectedKnown === undefined}
              onChange={(e) => setKnownDraft({ ...knownDraft, port: e.target.value })}
            />
            <AppButton
              variant="primary"
              size="sm"
              style={{ marginLeft: "auto" }}
              disabled={selectedKnown === undefined || send.isPending}
              onClick={() => {
                if (selectedKnown === undefined) return;
                if (!isValidPort(knownDraft.port)) {
                  setKnownPortError(t("known.portError"));
                  return;
                }
                setKnownPortError(null);
                void sendKnownPeer(selectedKnown);
              }}
            >
              {t("known.sendFile")}
            </AppButton>
          </EditRow>
          {/* F-FT-10：非法输入只给**行内**错误 */}
          {knownPortError !== null && <Hint span>{knownPortError}</Hint>}
        </Card>

        <Card>
          <CardHead>
            <CardTitle>{t("discovered.title")}</CardTitle>
            <Acts>
              {/* F-FT-08：刷新 = 立即重列当前快照，**不触发任何主动探测** */}
              <Hint>
                {peersQuery.dataUpdatedAt > 0
                  ? t("discovered.lastRefresh", {
                      time: new Date(peersQuery.dataUpdatedAt).toLocaleTimeString(),
                    })
                  : t("discovered.neverRefreshed")}
              </Hint>
              <AppButton variant="secondary" size="sm" onClick={refreshPeers}>
                {t("discovered.refresh")}
              </AppButton>
            </Acts>
          </CardHead>
          {peerError !== null && (
            <Hint span>{t("discovered.loadFailed", { reason: peerError })}</Hint>
          )}
          <List>
            {peers.length === 0 ? (
              // 🔴 F-FT-08：空态**永远带原因**，不给一张无解释的空表
              <Empty>
                <EmptyTitle>{t("discovered.emptyTitle")}</EmptyTitle>
                <EmptyHint>
                  {serviceRunning ? t("discovered.emptyHint") : t("discovered.emptyNotStarted")}
                </EmptyHint>
              </Empty>
            ) : (
              peers.map((peer) => (
                <EditRow
                  key={peer.deviceId}
                  ref={(element) => {
                    // 拖放命中测试要按行取 DOM 矩形；卸载时清掉，否则会命中已消失的行
                    if (element === null) rowRefs.current.delete(peer.deviceId);
                    else rowRefs.current.set(peer.deviceId, element);
                  }}
                  onClick={() => setSelectedPeerId(peer.deviceId)}
                  style={{
                    cursor: "pointer",
                    // 拖拽高亮：只在命中行变化时才变（over 每秒 238 次，见拖放钩子）
                    background:
                      drop.hoveredRowId === peer.deviceId
                        ? "var(--aegis-surface-hover, rgba(127,127,127,0.12))"
                        : undefined,
                    outline:
                      selectedPeerId === peer.deviceId
                        ? "1px solid var(--aegis-border-strong, rgba(127,127,127,0.5))"
                        : undefined,
                  }}
                >
                  <Label>{t("discovered.online")}</Label>
                  <span style={{ fontSize: 13 }}>{peer.deviceName}</span>
                  <MonoInput readOnly value={`${peer.ip}:${peer.port}`} style={{ width: 178 }} />
                  <AppButton
                    variant="secondary"
                    size="sm"
                    style={{ marginLeft: "auto" }}
                    onClick={(e) => {
                      e.stopPropagation(); // 别把"加入已知"当成"选中"
                      addDiscoveredToKnown(peer);
                    }}
                  >
                    {t("discovered.addToKnown")}
                  </AppButton>
                </EditRow>
              ))
            )}
          </List>
          <EditRow>
            {/* F-FT-08「发送文件到所选设备」：按钮路径用**所选行**（拖放路径用落点，见 R-7） */}
            <AppButton
              variant="primary"
              size="sm"
              disabled={selectedPeerId === null || send.isPending}
              onClick={() => void sendPickedToSelection()}
            >
              {t("discovered.sendToSelected")}
            </AppButton>
            <Hint>{t("discovered.dragHint")}</Hint>
          </EditRow>
          {/* 发送与目标选择的提示都在这里（R-7 的"请拖到目标设备行上"、入队回执、失败原因） */}
          {notice !== null && <Hint span>{notice}</Hint>}
          {send.error !== null && (
            <Hint span>{t("send.failed", { reason: asTransferError(send.error).message })}</Hint>
          )}
        </Card>
      </Cols>

      {/* ③ 文本 / 文件 / 剪贴板（F-FT-32~36） */}
      <Card>
        <CardHead>
          <CardTitle>{t("text.title")}</CardTitle>
          <Acts>
            <AppButton variant="secondary" size="sm">
              {t("text.paste")}
            </AppButton>
          </Acts>
        </CardHead>
        <TextArea
          value={text}
          placeholder={t("text.placeholder")}
          onChange={(e) => setText(e.target.value)}
        />
        <PriceRow>
          <Hint>{sizeLine(textValidation)}</Hint>
          <AppButton variant="primary" size="sm" style={{ marginLeft: "auto" }}>
            {t("text.send")}
          </AppButton>
        </PriceRow>
        <SectionDivider>
          {/* F-FT-36 / F-FT-51：依赖 Web 通道，属片2 → 禁用并标注去向，而非点击报错 */}
          <AppButton variant="secondary" size="sm" disabled title="片2 交付">
            {t("text.sendToMobile")}
          </AppButton>
          <AppButton variant="secondary" size="sm" disabled title="片2 交付">
            {t("text.sendFileToMobile")}
          </AppButton>
        </SectionDivider>
      </Card>

      {/* ④ 传输任务（F-FT-25~30） */}
      <Card>
        <CardHead>
          <CardTitle>{t("tasks.title")}</CardTitle>
          <Acts>
            <AppButton variant="secondary" size="sm">
              {t("tasks.cancelSelected")}
            </AppButton>
          </Acts>
        </CardHead>
        <Board>
          <Empty>
            <EmptyTitle>{t("tasks.emptyTitle")}</EmptyTitle>
            <EmptyHint>{t("tasks.emptyHint")}</EmptyHint>
          </Empty>
        </Board>
      </Card>

      {/* ⑤ 传输历史（F-FT-54~58） */}
      <Card>
        <CardHead>
          <CardTitle>{t("history.title")}</CardTitle>
          <Acts>
            <AppButton variant="secondary" size="sm">
              {t("history.export")}
            </AppButton>
            <AppButton variant="secondary" size="sm">
              {t("history.clear")}
            </AppButton>
          </Acts>
        </CardHead>
        <Filters>
          <Select defaultValue="all">
            <option value="all">{t("history.allDirections")}</option>
            <option value="send">{t("history.sendOnly")}</option>
            <option value="receive">{t("history.receiveOnly")}</option>
          </Select>
          <Select defaultValue="all">
            <option value="all">{t("history.allStatuses")}</option>
          </Select>
          <Input placeholder={t("history.search")} />
        </Filters>
        <Board>
          <Head>
            <span style={{ width: 130 }}>{t("history.col.time")}</span>
            <span style={{ width: 70 }}>{t("history.col.direction")}</span>
            <span style={{ width: 50 }}>{t("history.col.kind")}</span>
            <span style={{ flex: 1 }}>{t("history.col.name")}</span>
            <span style={{ width: 130 }}>{t("history.col.peer")}</span>
            <span style={{ width: 80 }}>{t("history.col.size")}</span>
            <span style={{ width: 60 }}>{t("history.col.status")}</span>
          </Head>
          <Empty>
            <EmptyTitle>{t("history.emptyTitle")}</EmptyTitle>
            <EmptyHint>{t("history.emptyHint")}</EmptyHint>
          </Empty>
        </Board>
      </Card>

      {/* 底部共享操作日志（F-FT-01：两通道写同一条，跨页签连续可追） */}
      <div>
        <LogBar>
          <LogToggle onClick={() => setLogOpen((v) => !v)}>
            {logOpen ? "▾" : "▸"} {t("log.collapsed")}
          </LogToggle>
          <Hint>{t("log.lines", { count: 0 })}</Hint>
          <AppButton variant="ghost" size="sm" style={{ marginLeft: "auto" }}>
            {t("log.clear")}
          </AppButton>
        </LogBar>
        {logOpen ? <LogBox /> : null}
      </div>
    </Wrap>
  );
}
