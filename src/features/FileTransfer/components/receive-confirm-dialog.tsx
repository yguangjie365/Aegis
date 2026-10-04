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
  background: rgba(31, 36, 48, 0.28);
  display: flex;
  align-items: center;
  justify-content: center;
  z-index: 41;
`;

const Box = styled.div`
  width: 452px;
  background: var(--aegis-canvas);
  border: 1px solid var(--aegis-border-strong);
  border-radius: var(--aegis-radius-sm);
  box-shadow: 0 10px 34px rgba(31, 36, 48, 0.2);
  overflow: hidden;
`;

const Head = styled.div`
  display: flex;
  align-items: baseline;
  gap: var(--aegis-space-sm);
  padding: var(--aegis-space-md) var(--aegis-space-lg) 0;
  font-weight: 700;
  font-size: 13.5px;
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
  font-size: 12.5px;
  word-break: break-all;
`;

const Mono = styled.span`
  font-family: var(--font-mono);
`;

const Conflict = styled.div`
  border: 1px solid var(--aegis-border-strong);
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

const Seg = styled.button<{ on: boolean }>`
  border: 1px solid ${(p) => (p.on ? "var(--aegis-accent)" : "var(--aegis-border-strong)")};
  background: ${(p) => (p.on ? "var(--aegis-accent)" : "var(--aegis-surface-soft)")};
  color: ${(p) => (p.on ? "#fff" : "var(--aegis-ink)")};
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
  color: #b3402a;
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
  font-size: 11.5px;
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
            <Key>{t("recvConfirm.fileName")}</Key>
            <Value>{pending.fileName}</Value>
            <Key>{t("recvConfirm.fileSize")}</Key>
            <Value>{formatBytes(pending.fileSize)}</Value>
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
