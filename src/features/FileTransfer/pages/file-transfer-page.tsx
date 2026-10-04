import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import styled from "@emotion/styled";
import { AppButton } from "@/ui/components/app-button";
import { PageHeader } from "@/ui/components/page-header";
import { log, newCorrelationId } from "@/lib/logging";
import { PairCodeDialog } from "../components/pair-code-dialog";
import { ReceiveConfirmDialog, formatBytes } from "../components/receive-confirm-dialog";
import { useReceiveConfirms, useResolveReceiveConfirm } from "../hooks/use-receive-confirms";
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
  useTransferTasks,
  useCancelTransferTask,
  useExportTransferHistory,
  useTransferHistory,
  useSendText,
  useReadClipboard,
  useSetTaskPaused,
} from "../hooks/use-transfer";
import type { TransferTaskState } from "@/types/transfer";
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

/** 挂起中的发送请求（配对码通过后才真正发出）。 */
interface PendingSend {
  ip: string;
  port: number;
  /** 对端展示串，形如 `书房台式机（192.168.1.24:18889）` */
  label: string;
  paths: string[];
}

/** 速度（字节/秒）→ 人读串；判不出来就"—"（F-FT-25 允许）。 */
function formatSpeed(bytesPerSecond: number | null): string {
  // 🔴 用 `== null`：同时挡住 null 与 undefined（接口漂移时不要把界面变成 NaN MB/s）
  if (bytesPerSecond == null || bytesPerSecond <= 0) return "—";
  if (bytesPerSecond >= 1024 * 1024) return `${(bytesPerSecond / (1024 * 1024)).toFixed(1)} MB/s`;
  if (bytesPerSecond >= 1024) return `${(bytesPerSecond / 1024).toFixed(0)} KB/s`;
  return `${bytesPerSecond} B/s`;
}

/** 剩余毫秒 → hh:mm:ss；判不出来就"—"（F-FT-25 允许）。 */
function formatDuration(ms: number | null): string {
  if (ms == null || !Number.isFinite(ms) || ms < 0) return "—";
  const total = Math.floor(ms / 1000);
  const hh = String(Math.floor(total / 3600)).padStart(2, "0");
  const mm = String(Math.floor((total % 3600) / 60)).padStart(2, "0");
  const ss = String(total % 60).padStart(2, "0");
  return `${hh}:${mm}:${ss}`;
}
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

  // 任务台账（F-FT-25/30）：只读快照。后端服务未跑时返回空表 ⇒ 界面显示空态而非失败态。
  const tasksQuery = useTransferTasks();
  const tasks = useMemo(() => tasksQuery.data ?? [], [tasksQuery.data]);
  // 「取消所选任务」需要"选中一行"——与设备列表同口径（点行即选中）
  const [selectedTaskId, setSelectedTaskId] = useState<string | null>(null);
  const cancelTask = useCancelTransferTask();
  // F-FT-26：暂停/恢复。🔴 这里记的是**本端最近一次被受理的动作**——
  // "已暂停（对端）"要等协议帧接上才能知道（现为待办，见变更记录）。
  const setTaskPaused = useSetTaskPaused();
  const [pausedLocally, setPausedLocally] = useState<Record<string, boolean>>({});
  const selectedTask = tasks.find((task) => task.id === selectedTaskId) ?? null;
  /** 取消"没取消到"时的如实提示（`cancelled: false` 不是错误，见钩子注释）。 */
  const cancelNotice = cancelTask.data === false ? t("tasks.cancelMissed") : null;

  // 传输历史（F-FT-54/55）：只读快照 + 三个筛选（全在本地过滤，不打后端）
  const historyQuery = useTransferHistory();
  const history = useMemo(() => historyQuery.data ?? [], [historyQuery.data]);
  const exportHistory = useExportTransferHistory();
  const [historyDirection, setHistoryDirection] = useState<"all" | "send" | "receive">("all");
  const [historyStatus, setHistoryStatus] = useState<"all" | TransferTaskState>("all");
  const [historySearch, setHistorySearch] = useState("");
  const visibleHistory = useMemo(() => {
    const needle = historySearch.trim().toLowerCase();
    return history.filter((entry) => {
      if (historyDirection !== "all" && entry.direction !== historyDirection) return false;
      if (historyStatus !== "all" && entry.state !== historyStatus) return false;
      if (needle === "") return true;
      // F-FT-55：按文件名或对端 IP/端口搜索
      return (
        entry.fileName.toLowerCase().includes(needle) ||
        entry.peerLabel.toLowerCase().includes(needle)
      );
    });
  }, [history, historyDirection, historyStatus, historySearch]);

  // ── 目标选择与发送（F-FT-11/12；R-7：目标不许猜测） ──
  // 按钮路径用"所选行"、拖放路径用"落点命中哪一行"——两条路径都不猜。
  const [selectedPeerId, setSelectedPeerId] = useState<string | null>(null);
  const [notice, setNotice] = useState<string | null>(null);
  const send = useSendTransferFile();
  // 文本通道（F-FT-33）：读剪贴板 + 发送。🔴 发送后**不清空输入框**（便于改一改再发），
  // 故下面任何一处都不得调用 `setText("")`。
  const pasteFromClipboard = useReadClipboard();
  const sendTextMutation = useSendText();
  const selectedPeer = useMemo(
    () => peers.find((candidate) => candidate.deviceId === selectedPeerId) ?? null,
    [peers, selectedPeerId],
  );
  const pickFiles = usePickFilesToSend();

  // ── 接收确认（F-FT-14 / R-8；布局经用户 2026-10-04 确认） ──
  // 🔴 一次只显示一条（用户裁决：多请求**排队**），队列长度显示在标题旁。
  const confirmsQuery = useReceiveConfirms();
  const resolveConfirm = useResolveReceiveConfirm();
  const confirmQueue = useMemo(() => confirmsQuery.data ?? [], [confirmsQuery.data]);
  const currentConfirm = confirmQueue.at(0) ?? null;
  const confirmError =
    resolveConfirm.error !== null ? asTransferError(resolveConfirm.error).message : null;

  // ── 配对码对话框与发送编排（布局经用户 2026-10-04 确认，方案 A） ──
  // 三条发送路径（拖放 / 已知设备行 / 所选设备按钮）**都经 `requestSend`**：
  // 需要码就挂起并弹框，不需要码就直接发。
  const [pendingSend, setPendingSend] = useState<PendingSend | null>(null);
  const [pairError, setPairError] = useState<string | null>(null);
  /** 本次运行期免问的设备 IP。🔴 这只是"少问一次"，**不是**授权——授权在后端 `PairingGuard` */
  const skipPairPrompt = useRef(new Set<string>());
  /** 最近一次真正发过的请求：失败（码错/节流）时用它重新弹框 */
  const lastAttempt = useRef<PendingSend | null>(null);

  const doSend = useCallback(
    (request: PendingSend, code: string | undefined): void => {
      lastAttempt.current = request;
      for (const path of request.paths) {
        const name = path.split(/[\\/]/).pop() ?? path;
        // F-FT-11 的原文日志："入队发送：{文件名} → {ip}:{端口}"
        log({
          level: "info",
          module: "Transfer",
          action: "transferSendFile",
          result: "Success",
          correlationId: newCorrelationId(),
        });
        setNotice(t("send.queued", { name, peer: `${request.ip}:${request.port}` }));
        send.mutate({ ip: request.ip, port: request.port, path, pairCode: code });
      }
    },
    [send, t],
  );

  const requestSend = useCallback(
    (target: { ip: string; port: number; label: string }, paths: string[]): void => {
      if (paths.length === 0) return;
      const request: PendingSend = { ...target, paths };
      if (skipPairPrompt.current.has(target.ip)) {
        doSend(request, undefined);
        return;
      }
      setPairError(null);
      setPendingSend(request);
    },
    [doSend],
  );

  const confirmPair = useCallback(
    (code: string, remember: boolean): void => {
      if (pendingSend === null) return;
      if (remember) skipPairPrompt.current.add(pendingSend.ip);
      setPairError(null);
      const request = pendingSend;
      setPendingSend(null);
      doSend(request, code);
    },
    [pendingSend, doSend],
  );

  // 🔴 码错/被节流时**重新弹框**并把后端的原因显示出来——否则用户点一次失败就没下文，
  //    只能反复点"发送"而不知道要改什么。同时清掉免问记录（那条记录已被证明无效）。
  useEffect(() => {
    const failure = send.error;
    if (failure === null) return;
    const info = asTransferError(failure);
    if (info.code !== "PAIRING_INVALID") return;
    const request = lastAttempt.current;
    if (request === null) return;
    skipPairPrompt.current.delete(request.ip);
    setPairError(info.message);
    setPendingSend(request);
  }, [send.error]);

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
      requestSend(
        { ip: peer.ip, port: peer.port, label: `${peer.deviceName}（${peer.ip}:${peer.port}）` },
        paths,
      );
    },
    [peers, requestSend, t],
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
      requestSend(
        { ip: peer.ip, port: peer.port, label: `${peer.name}（${peer.ip}:${peer.port}）` },
        paths,
      );
    },
    [pickFiles, requestSend],
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
          {/* 🔴 码错/节流的失败由对话框就地显示（见下方 `PairCodeDialog` 的 error），
              此处只在**没有**对话框时兜底，避免同一句话出现两遍 */}
          {send.error !== null && pendingSend === null && (
            <Hint span>{t("send.failed", { reason: asTransferError(send.error).message })}</Hint>
          )}
        </Card>
      </Cols>

      {/* ③ 文本 / 文件 / 剪贴板（F-FT-32~36） */}
      <Card>
        <CardHead>
          <CardTitle>{t("text.title")}</CardTitle>
          <Acts>
            <AppButton
              variant="secondary"
              size="sm"
              disabled={pasteFromClipboard.isPending}
              onClick={() =>
                pasteFromClipboard.mutate(undefined, {
                  onSuccess: (value) => setText(value),
                })
              }
            >
              {t("text.paste")}
            </AppButton>
          </Acts>
        </CardHead>
        {/* 读剪贴板的三档失败分开说（F-FT-33）：被占用 / 里面没有文本 / 操作失败，
            用户能做的事各不相同 */}
        {pasteFromClipboard.error !== null && (
          <Hint span>{asTransferError(pasteFromClipboard.error).message}</Hint>
        )}
        <TextArea
          value={text}
          placeholder={t("text.placeholder")}
          onChange={(e) => setText(e.target.value)}
        />
        <PriceRow>
          <Hint>{sizeLine(textValidation)}</Hint>
          <AppButton
            variant="primary"
            size="sm"
            style={{ marginLeft: "auto" }}
            // 🔴 空文本后端会拒（F-FT-32），这里先挡住；超限不在前端拦，
            //    由后端回 TEXT_TOO_LONG 并给出"N KB / 上限 256 KB"的原话。
            disabled={text.trim() === "" || selectedPeer === null || sendTextMutation.isPending}
            onClick={() => {
              if (selectedPeer === null) return;
              sendTextMutation.mutate({
                peer: `${selectedPeer.ip}:${selectedPeer.port}`,
                text,
              });
            }}
          >
            {t("text.send")}
          </AppButton>
        </PriceRow>
        {/* 🔴 结果措辞按 F-FT-34：`delivered` 的含义是**对方剪贴板写入成功**，
            绝不写成"已发送"——那会在对端没写进去时骗人。 */}
        {sendTextMutation.error !== null && (
          <Hint span>
            {t("text.sendFailed", { reason: asTransferError(sendTextMutation.error).message })}
          </Hint>
        )}
        {sendTextMutation.data !== undefined &&
          (sendTextMutation.data.delivered ? (
            <Hint span>{t("text.delivered")}</Hint>
          ) : (
            <Hint span>
              {t("text.notDelivered", {
                reason:
                  sendTextMutation.data.reasonText ||
                  sendTextMutation.data.reasonCode ||
                  t("text.unknownReason"),
              })}
            </Hint>
          ))}
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
            {/* 🔴 没选任务就禁用，而不是点了没反应（禁止静默失败） */}
            <AppButton
              variant="secondary"
              size="sm"
              disabled={selectedTask === null || cancelTask.isPending}
              onClick={() => {
                if (selectedTask === null) return;
                cancelTask.mutate(selectedTask.id);
              }}
            >
              {t("tasks.cancelSelected")}
            </AppButton>
            {/* 🔴 F-FT-26：暂停 ⇄ 继续，看的是**该任务**当前是否被本端暂停 */}
            <AppButton
              variant="secondary"
              size="sm"
              style={{ marginLeft: 8 }}
              disabled={selectedTask === null || setTaskPaused.isPending}
              onClick={() => {
                if (selectedTask === null) return;
                const paused = pausedLocally[selectedTask.id] === true;
                setTaskPaused.mutate(
                  { id: selectedTask.id, pause: !paused },
                  {
                    onSuccess: (outcome) => {
                      // 🔴 R-14：未被受理要说清楚，不能点了没反应
                      if (outcome.accepted) {
                        setPausedLocally((prev) => ({ ...prev, [selectedTask.id]: !paused }));
                      }
                    },
                  },
                );
              }}
            >
              {t(
                selectedTask !== null && pausedLocally[selectedTask.id]
                  ? "tasks.pauseToggleResume"
                  : "tasks.pauseTogglePause",
              )}
            </AppButton>
          </Acts>
        </CardHead>
        {cancelTask.error !== null && (
          <Hint span>
            {t("send.failed", { reason: asTransferError(cancelTask.error).message })}
          </Hint>
        )}
        {cancelNotice !== null && <Hint span>{cancelNotice}</Hint>}
        {/* 🔴 R-14 原文的措辞：未被受理必须说明"任务当前状态" */}
        {setTaskPaused.data !== undefined && setTaskPaused.data.accepted === false && (
          <Hint span>{t("tasks.pauseRejected", { state: setTaskPaused.data.state })}</Hint>
        )}
        <Board>
          {tasks.length === 0 ? (
            <Empty>
              <EmptyTitle>{t("tasks.emptyTitle")}</EmptyTitle>
              <EmptyHint>{t("tasks.emptyHint")}</EmptyHint>
            </Empty>
          ) : (
            tasks.map((task) => (
              <EditRow
                key={task.id}
                onClick={() => setSelectedTaskId(task.id)}
                style={{
                  cursor: "pointer",
                  outline:
                    selectedTaskId === task.id
                      ? "1px solid var(--aegis-border-strong, rgba(127,127,127,0.5))"
                      : undefined,
                }}
              >
                {/* 方向：发送 ↑ / 接收 ↓（F-FT-30 两种方向共用一张表） */}
                <span style={{ width: 14, textAlign: "center" }}>
                  {task.direction === "send" ? "↑" : "↓"}
                </span>
                <span style={{ fontSize: 13, flex: 1, minWidth: 0 }}>{task.fileName}</span>
                <MonoInput readOnly value={task.peerLabel} style={{ width: 150 }} />
                {/* 进度由后端算好的 percent 直接显示，界面不再算一遍（避免口径漂移） */}
                <span style={{ width: 44, textAlign: "right", fontSize: 12 }}>{task.percent}%</span>
                {/* 🔴 F-FT-25：「速度 · 剩余 hh:mm:ss」。
                    两者都由**后端**算好（含"暂停时长不计入速度"，见 tasks.rs 的 `speed_bps`）——
                    界面只做格式化，不再算一遍，避免两处口径漂移。
                    任一项判不出来就显示"—"，**不编数字**。 */}
                <span
                  style={{
                    width: 128,
                    fontSize: 12,
                    fontFamily: "var(--font-mono)",
                    color: "var(--aegis-mute)",
                  }}
                >
                  {formatSpeed(task.speedBps)} · {formatDuration(task.remainingMs)}
                </span>
                <span style={{ width: 58, fontSize: 12 }}>{t(`tasks.state.${task.state}`)}</span>
                {/* 🔴 失败/跳过/取消必须带原因码——R-29：失败态不摆无解释的红字 */}
                {task.reasonCode !== null && (
                  <span
                    style={{
                      fontFamily: "var(--font-mono)",
                      fontSize: 11.5,
                      color: "var(--aegis-mute)",
                    }}
                  >
                    {task.reasonCode}
                  </span>
                )}
              </EditRow>
            ))
          )}
        </Board>
      </Card>

      {/* ⑤ 传输历史（F-FT-54~58） */}
      <Card>
        <CardHead>
          <CardTitle>{t("history.title")}</CardTitle>
          <Acts>
            <AppButton
              variant="secondary"
              size="sm"
              disabled={exportHistory.isPending || history.length === 0}
              // 🔴 F-FT-56：导出的是**当前筛选后的行**（所见即所得），提示语照规格原文
              title={t("history.exportHint")}
              onClick={() => exportHistory.mutate(visibleHistory.map((entry) => entry.id))}
            >
              {t("history.export")}
            </AppButton>
            <AppButton variant="secondary" size="sm">
              {t("history.clear")}
            </AppButton>
          </Acts>
        </CardHead>
        {/* 🔴 成功话术按规格原文："✅ 已导出 {N} 条历史：{路径}" */}
        {exportHistory.data != null && (
          <Hint span>
            {t("history.exported", {
              count: exportHistory.data.count,
              path: exportHistory.data.path,
            })}
          </Hint>
        )}
        {exportHistory.error !== null && (
          <Hint span>
            {t("history.exportFailed", { reason: asTransferError(exportHistory.error).message })}
          </Hint>
        )}
        <Filters>
          <Select
            value={historyDirection}
            onChange={(e) => setHistoryDirection(e.target.value as "all" | "send" | "receive")}
          >
            <option value="all">{t("history.allDirections")}</option>
            <option value="send">{t("history.sendOnly")}</option>
            <option value="receive">{t("history.receiveOnly")}</option>
          </Select>
          <Select
            value={historyStatus}
            onChange={(e) => setHistoryStatus(e.target.value as "all" | TransferTaskState)}
          >
            <option value="all">{t("history.allStatuses")}</option>
            <option value="completed">{t("tasks.state.completed")}</option>
            <option value="skipped">{t("tasks.state.skipped")}</option>
            <option value="failed">{t("tasks.state.failed")}</option>
            <option value="cancelled">{t("tasks.state.cancelled")}</option>
          </Select>
          <Input
            placeholder={t("history.search")}
            value={historySearch}
            onChange={(e) => setHistorySearch(e.target.value)}
          />
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
          {visibleHistory.length === 0 ? (
            <Empty>
              {/* 🔴 空态区分"确实没有"与"筛掉了"——两者对用户的意思完全不同 */}
              <EmptyTitle>
                {history.length === 0 ? t("history.emptyTitle") : t("history.noMatch")}
              </EmptyTitle>
              <EmptyHint>
                {history.length === 0 ? t("history.emptyHint") : t("history.noMatchHint")}
              </EmptyHint>
            </Empty>
          ) : (
            visibleHistory.map((entry) => (
              <EditRow key={entry.id}>
                <span style={{ width: 130, fontSize: 12 }}>{entry.finishedAtText}</span>
                <span style={{ width: 70, fontSize: 12 }}>
                  {t(`history.dir.${entry.direction}`)}
                </span>
                <span style={{ width: 50, fontSize: 12 }}>
                  {/* 🔴 按 kind 如实渲染：文本条目不许显示成"文件" */}
                  {t(entry.kind === "text" ? "history.kindText" : "history.kindFile")}
                </span>
                <span style={{ flex: 1, fontSize: 13, minWidth: 0 }}>{entry.fileName}</span>
                <MonoInput readOnly value={entry.peerLabel} style={{ width: 130 }} />
                <span style={{ width: 80, fontSize: 12, textAlign: "right" }}>
                  {formatBytes(entry.fileSize)}
                </span>
                <span style={{ width: 60, fontSize: 12 }}>{t(`tasks.state.${entry.state}`)}</span>
                {/* 🔴 失败/跳过/取消必须带原因——R-29：不摆无解释的红字 */}
                {entry.reasonCode !== null && (
                  <span
                    style={{
                      fontFamily: "var(--font-mono)",
                      fontSize: 11.5,
                      color: "var(--aegis-mute)",
                    }}
                    title={entry.reasonText}
                  >
                    {entry.reasonCode}
                  </span>
                )}
              </EditRow>
            ))
          )}
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
      {/* 配对码对话框（布局经用户 2026-10-04 确认，方案 A：发送前弹、带免码勾选、多文件只问一次） */}
      <PairCodeDialog
        open={pendingSend !== null}
        peerLabel={pendingSend?.label ?? ""}
        busy={send.isPending}
        error={pairError}
        sendingCount={pendingSend?.paths.length ?? 0}
        onCancel={() => {
          setPendingSend(null);
          setPairError(null);
        }}
        onConfirm={confirmPair}
      />
      {/* 接收确认弹窗（F-FT-14；布局经用户确认，含"回车不默认拒绝、拒绝需点击"） */}
      <ReceiveConfirmDialog
        pending={currentConfirm}
        queuedCount={Math.max(0, confirmQueue.length - 1)}
        busy={resolveConfirm.isPending}
        error={confirmError}
        onResolve={(accept, policy) => {
          if (currentConfirm === null) return;
          resolveConfirm.mutate({ id: currentConfirm.id, accept, policy });
        }}
      />
    </Wrap>
  );
}
