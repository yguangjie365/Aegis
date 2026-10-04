import { useMemo, useState } from "react";
import { useTranslation } from "react-i18next";
import styled from "@emotion/styled";
import { AppButton } from "@/ui/components/app-button";
import { PageHeader } from "@/ui/components/page-header";
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

  // 🟠 本批无后端命令面：服务恒为"未启动"。接命令后由真实运行态驱动（F-FT-02/03）。
  const serviceRunning = false;
  const [receiveDir, setReceiveDir] = useState("");
  const [tcpPort, setTcpPort] = useState("18889");
  const [udpPort, setUdpPort] = useState("18888");
  const [policy, setPolicy] = useState("rename");
  const [knownPeerOnly, setKnownPeerOnly] = useState(true);
  // 🔴 Q-15 裁决（2026-10-04，用户）：接收前询问**默认开启**，全链路统一。
  const [askBeforeReceive, setAskBeforeReceive] = useState(true);
  const [text, setText] = useState("");
  const [logOpen, setLogOpen] = useState(true);

  const textValidation = useMemo(() => validate(text), [text]);

  return (
    <Wrap>
      <PageHeader
        title={t("page.title")}
        sub={t("page.sub", { devices: 0, tasks: 0 })}
        actions={
          <Badge on={serviceRunning}>
            {serviceRunning ? t("page.status.desktopRunning") : t("page.status.stopped")}
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
            <AppButton variant="secondary" size="sm">
              {serviceRunning ? t("local.stop") : t("local.start")}
            </AppButton>
          </Acts>
        </CardHead>
        <Form>
          <Label>{t("local.deviceId")}</Label>
          <span style={{ fontFamily: "var(--font-mono)", fontSize: 13 }}>—</span>
          <span />
          <span />

          <Label>{t("local.receiveDir")}</Label>
          <MonoInput
            value={receiveDir}
            placeholder={t("local.receiveDirMissing")}
            onChange={(e) => setReceiveDir(e.target.value)}
          />
          <AppButton variant="secondary" size="sm">
            {t("local.browse")}
          </AppButton>
          <AppButton variant="secondary" size="sm">
            {t("local.open")}
          </AppButton>
          <Hint span>{t("space.unknown")}</Hint>

          <Label>{t("local.tcpPort")}</Label>
          <PortInput value={tcpPort} onChange={(e) => setTcpPort(e.target.value)} />
          <Hint>{t("local.tcpPortTooltip")}</Hint>
          <span />
          <Label>{t("local.udpPort")}</Label>
          <PortInput value={udpPort} onChange={(e) => setUdpPort(e.target.value)} />
          <Hint>{t("local.udpPortTooltip")}</Hint>
          <span />
          {/* F-FT-06：端口生效时机只描述当前事实 */}
          <Hint span>{t("local.portHintStopped")}</Hint>

          <Label>{t("local.conflict")}</Label>
          <Select value={policy} onChange={(e) => setPolicy(e.target.value)}>
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
                onChange={(e) => setKnownPeerOnly(e.target.checked)}
              />
              {t("local.requireKnownPeer")}
            </Check>
            <Check>
              <input
                type="checkbox"
                checked={askBeforeReceive}
                onChange={(e) => setAskBeforeReceive(e.target.checked)}
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
              <AppButton variant="secondary" size="sm">
                {t("known.add")}
              </AppButton>
              <AppButton variant="secondary" size="sm">
                {t("known.remove")}
              </AppButton>
              <AppButton variant="secondary" size="sm">
                {t("known.save")}
              </AppButton>
            </Acts>
          </CardHead>
          <List>
            <Empty>
              <EmptyTitle>{t("known.emptyTitle")}</EmptyTitle>
              <EmptyHint>{t("known.emptyHint")}</EmptyHint>
            </Empty>
          </List>
          <EditRow>
            <Label>{t("known.name")}</Label>
            <Input style={{ width: 118 }} />
            <Label>{t("known.ip")}</Label>
            <MonoInput style={{ width: 112 }} />
            <Label>{t("known.port")}</Label>
            <PortInput style={{ width: 78 }} />
            <AppButton variant="primary" size="sm" style={{ marginLeft: "auto" }}>
              {t("known.sendFile")}
            </AppButton>
          </EditRow>
        </Card>

        <Card>
          <CardHead>
            <CardTitle>{t("discovered.title")}</CardTitle>
            <Acts>
              <Hint>{t("discovered.neverRefreshed")}</Hint>
              <AppButton variant="secondary" size="sm">
                {t("discovered.refresh")}
              </AppButton>
            </Acts>
          </CardHead>
          <List>
            <Empty>
              <EmptyTitle>{t("discovered.emptyTitle")}</EmptyTitle>
              <EmptyHint>{t("discovered.emptyHint")}</EmptyHint>
            </Empty>
          </List>
          <EditRow>
            <AppButton variant="secondary" size="sm">
              {t("discovered.sendToSelected")}
            </AppButton>
            <Hint>{t("discovered.dragHint")}</Hint>
          </EditRow>
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
