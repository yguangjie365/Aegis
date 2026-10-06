// 接收确认弹窗（M-05 片1）。布局经用户 2026-10-04 确认，含三点修订：
//   ① 冲突区三个选项（自动改名 / 覆盖 / 跳过）**都要**
//   ② **回车不默认拒绝**——拒绝须鼠标点击（🔴 与 Esc 不同：Esc 是"我要关掉这个弹窗"的
//      明确意图，回车是"我手刚在键盘上"的误触；R-8 的 Esc/关闭/超时三条仍按拒绝）
//   ③ 多请求排队**接受**（一次显示一条，标题旁标注还剩几条）
//
// 规格：F-FT-14（弹窗内容）/ F-FT-18（默认开启，Q-15 裁决）/ R-8（四条路径全部按拒绝）。
//
// 🔴 三条渲染纪律：
// 1. `availableBytes === null` 时**绝不显示 "0 B"**——那是"空间不足"，与"查不到"是两回事
//    （R-12 的保守分支：无法判定则放行但记录）。
// 2. `nameConflict === false` 时**不渲染冲突区**（F-FT-14 原文"仅当存在同名文件时"）。
// 3. 倒计时归零**不清空弹窗**，而是切到"已超时"态——否则用户点了「接收」却什么都没发生，
//    会以为界面坏了。

import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import styled from "@emotion/styled";
import { AppButton } from "@/ui/components/app-button";
import type { PendingConfirm, PerTransferPolicy } from "@/types/transfer";

const Scrim = styled.div`
  position: fixed;
  inset: 0;
  /* 遮罩恒 rgba(0,0,0,.55)（§三 dialog 行）。原 31,36,48/.28 是契约外色值：
     偏蓝、且 28% 太浅，深色界面上压不住底层内容（UI 审查 M6）。 */
  background: rgba(0, 0, 0, 0.55);
  display: flex;
  align-items: center;
  justify-content: center;
  z-index: 41;
`;

const Box = styled.div`
  width: 452px;
  background: var(--aegis-canvas);
  /* 🔴 2026-10-06 批次③：原 var(--aegis-border-strong) 从未定义 —— 未定义属性令 border
     在 computed-value 阶段失效，静默回退 currentColor（边框随文字色漂移、不参与换包）。
     同文件 Conflict / Seg 两处同口径修正。 */
  border: 1px solid var(--aegis-border-interactive);
  border-radius: var(--aegis-radius-sm);
  /* §二 elevation 只有三档，弹层允许的那一档就是 sticky（§一 #3 唯一阴影处）。
     原「0 10px 34px rgba(31,36,48,.2)」是契约外第四档（UI 审查 M6）。
     ⚠️ 本注释在模板字符串内，禁写反引号（TS1005）。 */
  box-shadow: var(--aegis-elevation-sticky);
  overflow: hidden;
`;

const Head = styled.div`
  display: flex;
  align-items: baseline;
  gap: var(--aegis-space-sm);
  padding: var(--aegis-space-md) var(--aegis-space-lg) 0;
  font-weight: 700;
  /* 同 pair-code：半档值 13.5 → body-strong 14/700（M6） */
  font-size: var(--aegis-type-body-size);
`;

const Count = styled.span`
  margin-left: auto;
  font-weight: 400;
  font-size: 12px;
  color: var(--aegis-mute);
  font-variant-numeric: tabular-nums;
`;

const Body = styled.div`
  padding: var(--aegis-space-sm) var(--aegis-space-lg) var(--aegis-space-md);
  font-size: 13px;
  line-height: 1.6;
`;

const Meta = styled.dl`
  display: grid;
  grid-template-columns: 76px 1fr;
  gap: 3px 10px;
  margin: var(--aegis-space-xs) 0 var(--aegis-space-md);
`;

const Key = styled.dt`
  color: var(--aegis-mute);
  font-size: 12px;
  margin: 0;
`;

const Value = styled.dd`
  margin: 0;
  /* 12.5 → caption 12：这里是文件名/大小一类的**元数据**，正是 caption 档的职责（M6） */
  font-size: var(--aegis-type-caption-size);
  word-break: break-all;
`;

const Mono = styled.span`
  font-family: var(--font-mono);
`;

const Conflict = styled.div`
  border: 1px solid var(--aegis-border-interactive);
  background: var(--aegis-surface-inset);
  border-radius: var(--aegis-radius-sm);
  padding: var(--aegis-space-sm) 10px;
  margin: var(--aegis-space-sm) 0 2px;
`;

const Segs = styled.div`
  display: flex;
  gap: var(--aegis-space-sm);
  align-items: center;
  margin-top: 6px;
  font-size: 12px;
  flex-wrap: wrap;
`;

/** 🔴 R-9：「当前策略下将会发生的动作」——必须与策略选择一起出现，不能只给选项不说后果。 */
const Action = styled.div`
  margin-top: 6px;
  font-size: 12px;
  line-height: 1.5;
  color: var(--aegis-mute);
`;

const Seg = styled.button<{ on: boolean }>`
  border: 1px solid ${(p) => (p.on ? "var(--aegis-accent)" : "var(--aegis-border-interactive)")};
  background: ${(p) => (p.on ? "var(--aegis-accent)" : "var(--aegis-surface-soft)")};
  /* 🔴 绿底必须黑字（规范 §六 #1，本规范对源语系的第一号强制修正）：
     accent 上白字仅 2.41:1 ❌，黑字（accent-on）8.71:1 ✅。原先这里写死白色字面量，
     等于把选中态做成全屏最难读的一块 —— 2026-10-06 UI 审查 M7。
     ⚠️ 本注释在模板字符串内，禁写反引号（会报 TS1005，本项目已踩多次）。 */
  color: ${(p) => (p.on ? "var(--aegis-accent-on)" : "var(--aegis-ink)")};
  border-radius: var(--aegis-radius-sm);
  padding: 2px 9px;
  font-size: 12px;
  cursor: pointer;
`;

const Warn = styled.div`
  color: var(--aegis-mute);
  font-size: 12px;
  margin-top: var(--aegis-space-sm);
`;

const ErrorLine = styled.div`
  /* 错误**文字**恒走 error 令牌（Dark 包 #ff5c5c，canvas 上 6.14:1）。
     原写死 #b3402a 实算仅 3.26:1，12px 文字不达 4.5:1（§六、UI 审查 M6）。 */
  color: var(--aegis-error);
  font-size: 12px;
  margin-top: var(--aegis-space-sm);
`;

const Foot = styled.div`
  display: flex;
  justify-content: flex-end;
  gap: var(--aegis-space-sm);
  padding: var(--aegis-space-sm) var(--aegis-space-lg);
  background: var(--aegis-surface-soft);
  border-top: 1px solid var(--aegis-hairline);
`;

const Hint = styled.div`
  color: var(--aegis-mute);
  /* 11.5 → caption 12，**不降到 micro 11**：§二 明写 micro「禁用于正文」，
     而这一行是给用户的操作提示（正文性），不是图表刻度（M6）。 */
  font-size: var(--aegis-type-caption-size);
  text-align: right;
  margin-right: auto;
  align-self: center;
`;

export interface ReceiveConfirmDialogProps {
  /** 当前要显示的待决请求；`null` = 不显示 */
  pending: PendingConfirm | null;
  /** 队列里**还有**几条（多请求排队；0 表示这是最后一条） */
  queuedCount: number;
  /** 正在提交裁决 */
  busy: boolean;
  /** 后端回传的失败文案（如 `CONFIRM_GONE`）；无错为 `null` */
  error: string | null;
  /** 用户裁决。`policy` 仅在 `accept` 为真且存在冲突时有意义 */
  onResolve: (accept: boolean, policy: PerTransferPolicy | null) => void;
}

/**
 * 接收确认弹窗。
 *
 * 🔴 **不绑定回车的默认动作**（用户裁决②）：键盘唯一的作用是 Esc = 拒绝。
 *    两个动作都需要显式点击，避免误触把一次接收拒掉或收下。
 */
export function ReceiveConfirmDialog({
  pending,
  queuedCount,
  busy,
  error,
  onResolve,
}: ReceiveConfirmDialogProps): React.JSX.Element | null {
  const { t } = useTranslation("fileTransfer");
  const [policy, setPolicy] = useState<PerTransferPolicy>("rename");

  // 每次换一条请求就把策略重置为设置里的默认（由调用方通过 pending 变化驱动）
  useEffect(() => {
    setPolicy("rename");
  }, [pending?.id]);

  // Esc = 拒绝（R-8 四条路径之一）。🔴 只处理 Esc，**不处理 Enter**。
  useEffect(() => {
    if (pending === null) return;
    const onKey = (event: KeyboardEvent): void => {
      if (event.key === "Escape" && !busy) onResolve(false, null);
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [pending, busy, onResolve]);

  if (pending === null) return null;

  const seconds = Math.max(0, Math.ceil(pending.expiresInMs / 1000));
  const expired = seconds <= 0 || error !== null;
  const conflict = pending.nameConflict;

  return (
    <Scrim
      role="presentation"
      onMouseDown={(e) => e.target === e.currentTarget && !busy && onResolve(false, null)}
    >
      <Box role="dialog" aria-modal="true" aria-label={t("recvConfirm.title")}>
        <Head>
          <span>{t("recvConfirm.title")}</span>
          {/* 倒计时是 R-8 的可见化：超时 = 拒绝，用户必须看得见还剩多久 */}
          {!expired && <Count>{t("recvConfirm.countdown", { seconds })}</Count>}
          {queuedCount > 0 && !expired && (
            <Count>{t("recvConfirm.queued", { count: queuedCount })}</Count>
          )}
        </Head>
        <Body>
          <Meta>
            <Key>{t("recvConfirm.from")}</Key>
            <Value>
              <Mono>{pending.peerLabel}</Mono>
            </Value>
            {/* 🔴 文本传输的"文件名"其实是条目标签，措辞要按类型走 */}
            <Key>{t(pending.kind === "text" ? "recvConfirm.content" : "recvConfirm.fileName")}</Key>
            <Value>{pending.fileName}</Value>
            <Key>{t(pending.kind === "text" ? "recvConfirm.size" : "recvConfirm.fileSize")}</Key>
            <Value>{formatBytes(pending.fileSize)}</Value>
            {/* 🔴 文本与磁盘无关：绝不在文本传输里显示"保存到/磁盘剩余"——
                那是在用一个不存在的约束干扰"要不要覆盖剪贴板"这个不可逆的决定。 */}
            {pending.kind === "text" ? (
              <>
                <Key>{t("recvConfirm.writeTo")}</Key>
                <Value>{t("recvConfirm.clipboardOverwrite")}</Value>
              </>
            ) : (
              <>
                <Key>{t("recvConfirm.saveTo")}</Key>
                <Value>
                  <Mono>{pending.saveDirectory === "" ? "—" : pending.saveDirectory}</Mono>
                </Value>
                <Key>{t("recvConfirm.diskFree")}</Key>
                {/* 🔴 无法判定 ≠ 0：显示 "0 B" 会让人以为空间不够 */}
                <Value>
                  {pending.availableBytes === null
                    ? t("recvConfirm.diskUnknown")
                    : formatBytes(pending.availableBytes)}
                </Value>
              </>
            )}
          </Meta>

          {conflict && !expired && (
            <Conflict>
              <b>{t("recvConfirm.conflictTitle")}</b>
              <Segs>
                <span>{t("recvConfirm.conflictHow")}</span>
                {(["rename", "overwrite", "skip"] as const).map((option) => (
                  <Seg
                    key={option}
                    type="button"
                    on={policy === option}
                    disabled={busy}
                    onClick={() => setPolicy(option)}
                  >
                    {t(`conflict.${option}`)}
                  </Seg>
                ))}
              </Segs>
              {/* 🔴 R-9 原文：冲突区必须写明"**当前策略下将会发生的动作**"，
                  且"覆盖类后果必须写明**原文件已不可恢复**"。
                  本区只在**已存在同名文件**时出现（F-FT-14：仅必要时），故不打扰常规接收。
                  🔴 键段是 **conflict**（顶层段，与 recvConfirm 平级），不是 recvConfirm：
                  早期误写成 `recvConfirm.action*`，而 i18next 缺键时**返回键名本身且不报错**，
                  结果是这行"唯一告知不可恢复性"的文案向用户显示成 `recvConfirm.actionOverwrite`
                  这样的原始 key —— 恰恰是最不该出错的一行。
                  与同段既有的 `rename/ask/skip/overwrite` 一致（`file-transfer-page.tsx` 在用）。 */}
              <Action>
                {policy === "overwrite"
                  ? t("conflict.actionOverwrite")
                  : t(policy === "skip" ? "conflict.actionSkip" : "conflict.actionRename")}
              </Action>
            </Conflict>
          )}

          {pending.availableBytes === null && !expired && (
            <Warn>{t("recvConfirm.diskUnknownNote")}</Warn>
          )}

          {error !== null && <ErrorLine>{error}</ErrorLine>}
        </Body>
        <Foot>
          {expired ? (
            <>
              {/* 已超时：只给一个"知道了"，因为这条请求**已经按拒绝处理完了** */}
              <AppButton variant="primary" size="sm" onClick={() => onResolve(false, null)}>
                {t("recvConfirm.ok")}
              </AppButton>
            </>
          ) : (
            <>
              <Hint>{t("recvConfirm.hint")}</Hint>
              <AppButton
                variant="secondary"
                size="sm"
                disabled={busy}
                onClick={() => onResolve(false, null)}
              >
                {t("recvConfirm.reject")}
              </AppButton>
              <AppButton
                variant="primary"
                size="sm"
                disabled={busy}
                onClick={() => onResolve(true, conflict ? policy : null)}
              >
                {t("recvConfirm.accept")}
              </AppButton>
            </>
          )}
        </Foot>
      </Box>
    </Scrim>
  );
}

/**
 * 人类可读的字节数（F-FT-14 要求弹窗展示大小）。
 *
 * 用 1024 进制并标 KB/MB/GB（与界面其余部分的"128.4 MB"一致）。
 */
export function formatBytes(bytes: number): string {
  if (!Number.isFinite(bytes) || bytes < 0) return "—";
  if (bytes < 1024) return `${bytes} B`;
  const units = ["KB", "MB", "GB", "TB"];
  let value = bytes / 1024;
  let unit = 0;
  while (value >= 1024 && unit < units.length - 1) {
    value /= 1024;
    unit += 1;
  }
  return `${value.toFixed(1)} ${units[unit]}`;
}
