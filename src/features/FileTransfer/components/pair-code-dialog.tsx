// 配对码输入对话框（M-05 片1；布局经用户 2026-10-04 确认，方案 A）。
//
// 规格依据：F-FT-13（默认要求配对；本次运行期首个带有效码的地址之后免码）、
// R-2（6 位、大小写不敏感、一次性、比较不泄露时序）、R-5（连续失败触发节流）。
//
// 🔴 三处刻意设计：
// 1. **不自己判码**：一律把用户输入交给后端，由后端裁决（前端判等于把契约抄成两份）。
// 2. **`type="text"` 而不是 `password`**：配对码是要念给旁边人听的短码，
//    遮住反而让用户无法核对输入（R-2 也没把它列为秘密；它是**一次性**的）。
// 3. **异常输入在本地只做"够不够 6 位"这一件事**，其余交给后端 —— 少一处口径漂移。

import { useEffect, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import { AppButton } from "@/ui/components/app-button";
import styled from "@emotion/styled";

/** 配对码长度（R-2；与后端 `pairing::CODE_LENGTH` 同值）。 */
export const PAIR_CODE_LENGTH = 6;

const Scrim = styled.div`
  position: fixed;
  inset: 0;
  /* 遮罩恒 rgba(0,0,0,.55)（§三 dialog 行）。原 31,36,48/.28 是契约外色值（UI 审查 M6）。 */
  background: rgba(0, 0, 0, 0.55);
  display: flex;
  align-items: center;
  justify-content: center;
  z-index: 40;
`;

const Box = styled.div`
  width: 412px;
  background: var(--aegis-canvas);
  border: 1px solid var(--aegis-border-interactive);
  border-radius: var(--aegis-radius-sm);
  /* 弹层阴影走 elevation 契约里唯一允许的那一档 sticky（§二 / §一 #3）。
     原 0 10px 34px rgba(31,36,48,.2) 是契约外第四档（UI 审查 M6）。 */
  box-shadow: var(--aegis-elevation-sticky);
  overflow: hidden;
`;

const Head = styled.div`
  padding: var(--aegis-space-md) var(--aegis-space-lg) 0;
  font-weight: 700;
  /* 13.5 不在 §二 12 档阶梯上（半档值）。按**语义**归位而非数值就近：
     这是一行加粗分区标题 ⇒ body-strong（14/700）。UI 审查 M6。 */
  font-size: var(--aegis-type-body-size);
`;

const Body = styled.div`
  padding: var(--aegis-space-sm) var(--aegis-space-lg) var(--aegis-space-md);
  font-size: 13px;
  line-height: 1.6;
`;

const CodeInput = styled.input`
  width: 168px;
  letter-spacing: 0.34em;
  font-family: var(--font-mono);
  /* 19 不在阶梯上 ⇒ 归 h2（20）。配对码是全屏唯一需要被读准的一串字符，
     取**上一档**而不是下一档（§一 #5 密度让位于可读性，此处只此一处理由成立）。 */
  font-size: var(--aegis-type-h2-size);
  padding: 7px 10px;
  /* 🔴 2026-10-06 批次③：原写 var(--aegis-border-strong) —— 该令牌**从未定义**。
     未定义的自定义属性令 border 在 computed-value 阶段失效，浏览器静默回退到
     currentColor（即文字色），故边框"看起来有"但实际随文字色漂移、不参与换包。 */
  border: 1px solid var(--aegis-border-interactive);
  border-radius: var(--aegis-radius-sm);
  background: var(--aegis-surface-soft);
  color: var(--aegis-ink);
  text-align: center;
  text-transform: uppercase;

  &:disabled {
    color: var(--aegis-mute);
  }
`;

const Remember = styled.label`
  display: flex;
  align-items: center;
  gap: var(--aegis-space-xs);
  color: var(--aegis-mute);
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

const ErrorLine = styled.div`
  /* 错误文字恒走 error 令牌；原写死 #b3402a 在 canvas 上仅 3.26:1，不达 4.5（§六、M6）。 */
  color: var(--aegis-error);
  font-size: 12px;
  margin-top: var(--aegis-space-sm);
`;

const BusyLine = styled.div`
  color: var(--aegis-mute);
  font-size: 12px;
  margin-top: var(--aegis-space-sm);
`;

/** 去掉空白与常见全角字符（用户常从聊天软件复制，会带空格/全角）。 */
export function normalizePairCode(raw: string): string {
  return raw
    .replace(/[\s\u3000]/g, "")
    .replace(/[０-９Ａ-Ｚａ-ｚ]/g, (ch) => String.fromCharCode(ch.charCodeAt(0) - 0xfee0))
    .toUpperCase();
}

export interface PairCodeDialogProps {
  open: boolean;
  /** 对端展示串，形如 `书房台式机（192.168.1.24:18889）` */
  peerLabel: string;
  /** 正在发送（按钮与输入框都禁用） */
  busy: boolean;
  /** 后端回传的失败文案（`PAIRING_INVALID` 等）；无错为 `null` */
  error: string | null;
  /** 正在发送的文件数（多文件时提示"正在发送 N 个文件…"） */
  sendingCount: number;
  onCancel: () => void;
  /** 用户确认：`code` 已归一化；`remember` = 本次运行期不再询问该设备 */
  onConfirm: (code: string, remember: boolean) => void;
}

/**
 * 配对码对话框。
 *
 * 🔴 挂载即聚焦输入框、回车即提交、Esc 即取消——三条都是"不用鼠标也能走完"的最低要求。
 * 焦点圈定在框内由 `Scrim` 捕获点击 + 输入框 autoFocus 近似实现（本仓暂无焦点陷阱组件）。
 */
export function PairCodeDialog({
  open,
  peerLabel,
  busy,
  error,
  sendingCount,
  onCancel,
  onConfirm,
}: PairCodeDialogProps): React.JSX.Element | null {
  const { t } = useTranslation("fileTransfer");
  const [raw, setRaw] = useState("");
  const [remember, setRemember] = useState(true);
  const inputRef = useRef<HTMLInputElement>(null);

  // 每次打开都重置：上一次输错的码不该留在框里（用户会以为已经改过了）
  useEffect(() => {
    if (!open) return;
    setRaw("");
    setRemember(true);
    inputRef.current?.focus();
  }, [open]);

  useEffect(() => {
    if (!open) return;
    const onKey = (event: KeyboardEvent): void => {
      if (event.key === "Escape" && !busy) onCancel();
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [open, busy, onCancel]);

  if (!open) return null;

  const code = normalizePairCode(raw);
  const ready = code.length === PAIR_CODE_LENGTH && !busy;

  return (
    <Scrim
      role="presentation"
      onMouseDown={(e) => e.target === e.currentTarget && !busy && onCancel()}
    >
      <Box role="dialog" aria-modal="true" aria-label={t("pairDialog.title")}>
        <Head>{t("pairDialog.title")}</Head>
        <Body>
          <p>{t("pairDialog.prompt", { peer: peerLabel })}</p>
          <CodeInput
            ref={inputRef}
            type="text"
            inputMode="text"
            autoCapitalize="characters"
            autoComplete="off"
            spellCheck={false}
            maxLength={PAIR_CODE_LENGTH + 4 /* 容忍粘贴时带的空格 */}
            value={raw}
            disabled={busy}
            onChange={(e) => setRaw(e.target.value)}
            onKeyDown={(e) => {
              if (e.key === "Enter" && ready) onConfirm(code, remember);
            }}
          />
          <Remember>
            <input
              type="checkbox"
              checked={remember}
              disabled={busy}
              onChange={(e) => setRemember(e.target.checked)}
            />
            {t("pairDialog.remember")}
          </Remember>
          {busy && <BusyLine>{t("pairDialog.sending", { count: sendingCount })}</BusyLine>}
          {/* 🔴 失败原因来自后端（禁止前端自行解释原因码） */}
          {error !== null && !busy && <ErrorLine>{error}</ErrorLine>}
        </Body>
        <Foot>
          <AppButton variant="secondary" size="sm" disabled={busy} onClick={onCancel}>
            {t("pairDialog.cancel")}
          </AppButton>
          <AppButton
            variant="primary"
            size="sm"
            disabled={!ready}
            onClick={() => onConfirm(code, remember)}
          >
            {error === null ? t("pairDialog.send") : t("pairDialog.retry")}
          </AppButton>
        </Foot>
      </Box>
    </Scrim>
  );
}
