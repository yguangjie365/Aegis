import { useCallback, useRef, useState } from "react";
import { AlertDialog, AlertDialogContent, AlertDialogOverlay, Input } from "@chakra-ui/react";
import styled from "@emotion/styled";
import { AppButton } from "./app-button";

/* ═══════════════════════════════════════════════════════════════════════════
   确认对话框 = 危险操作红线的**唯一承载件**（规范 §三 dialog 行 + §八 8.1）
   ═══════════════════════════════════════════════════════════════════════════
   来历（2026-10-06 UI 审查 B3）：FileBackup 的删除快照 / 删除规则 / 覆盖导入 /
   草稿确认 / 手动加路径共 **5 处**走的是浏览器原生 `window.confirm` 与 `window.prompt`。
   原生框的问题不是"不好看"，而是三条硬缺陷：
     ① 不可令牌化 —— 遮罩、字号、按钮变体全在浏览器手里，深色包里嵌一块系统白框；
     ② 焦点与朗读不受控 —— 无法保证「初始焦点落取消」「关闭后归还触发控件」；
     ③ 遮罩下方的页面内容**仍可 Tab 到**（没有焦点圈定），键盘用户能走进被遮住的界面。
   §三 把 dialog 列为「功能规格红线承载件」（预览→确认→执行→记录），原生框承担不了这个角色。

   🔴 为什么建在 Chakra `AlertDialog` 上，而不是手写焦点陷阱：
     · 仓库已有这套语汇（复刻域 SteamPage 用的就是它），不另立第三套实现；
     · 焦点圈定 / Esc / 关闭归还触发控件 / `role=alertdialog` + `aria-modal` 是库测过的行为。
       本仓 §八 8.4 的 `A11ySemanticsGuard` **尚未实装**（见规范同日状态更正），
       手写陷阱一旦漏一条，没有任何机器会拦 —— 这正是「把可达性交给库」的理由。
     · 布局仍由本文件的 styled 层控制（Chakra 只出机制，不出观感），
       观感一律走 §二 令牌，不写裸值。
   ═══════════════════════════════════════════════════════════════════════════ */

export interface ConfirmRequest {
  /** 标题（§三：h3 档） */
  title: string;
  /** 正文（§三：body 档）。承担「预览」这一步 —— 必须带上操作对象（路径 / 规则名 / 缺项清单）。
   *  输入型（替代 window.prompt）可省，标题已足够说明要输入什么。 */
  body?: React.ReactNode;
  confirmLabel: string;
  cancelLabel: string;
  /**
   * `danger` = 不可撤销操作（实心 `error-fill` + 白字，§三 button-danger：实心只留给最终执行步）；
   * `primary` = 普通确认（实心绿）。缺省 primary。
   */
  tone?: "danger" | "primary";
  /** 传了即「输入型」（替代 `window.prompt`）；确认时回传 trim 后的值，空值不给确认（禁用即阻断）。 */
  input?: { placeholder?: string };
  onConfirm: (value: string) => void;
}

const Head = styled.h2`
  margin: 0;
  padding: var(--aegis-space-lg) var(--aegis-space-lg) 0;
  font-size: var(--aegis-type-h3-size);
  font-weight: var(--aegis-type-h3-weight);
  line-height: var(--aegis-type-h3-leading);
  color: var(--aegis-ink);
`;

const Body = styled.div`
  padding: var(--aegis-space-sm) var(--aegis-space-lg) var(--aegis-space-lg);
  font-size: var(--aegis-type-body-size);
  font-weight: var(--aegis-type-body-weight);
  line-height: var(--aegis-type-body-leading);
  color: var(--aegis-ink);
  /* 原生 confirm 的文案里带 \n（如导入说明的那三条要点）⇒ 不写 pre-line 会被压成一行 */
  white-space: pre-line;
`;

const Field = styled(Input)`
  /* 🔴 焦点环读 focus-ring 语义键：Chakra Input 的默认焦点色是 blue.300，
     不在 §二 25 色契约内（UI 审查 M1 记的那条「控件走 Chakra 默认板」）。
     这里先在本承载件内纠正；表单原语（Field）落地后由原语统一接管。 */
  &:focus-visible {
    border-color: var(--aegis-focus-ring);
    box-shadow: 0 0 0 1px var(--aegis-focus-ring);
  }
`;

const Foot = styled.div`
  display: flex;
  justify-content: flex-end;
  align-items: center;
  gap: var(--aegis-space-md);
  padding: 0 var(--aegis-space-lg) var(--aegis-space-lg);
`;

/** Chakra 只负责机制（层叠 / 焦点 / Esc），观感全在这份 sx 里，值一律取令牌。 */
const CONTENT_SX = {
  w: "100%",
  maxW: "452px",
  p: 0,
  bg: "var(--aegis-canvas)",
  border: "1px solid var(--aegis-border-interactive)",
  borderRadius: "var(--aegis-radius-sm)",
} as const;

const OVERLAY_SX = {
  /* §三 dialog 行：遮罩恒 rgba(0,0,0,.55)。Chakra 默认黑 alpha 档不在此值，且换包不跟随。 */
  bg: "rgba(0, 0, 0, 0.55)",
} as const;

/**
 * 用法：页面里 `const { ask, dialog } = useConfirmDialog();`，
 * 把 `if (!window.confirm(text)) { ...动作 }` 改写成
 * `ask({ title, body, confirmLabel, cancelLabel, tone, onConfirm: () => { ...动作 } })`，
 * 并在 JSX 末尾渲染一次 `{dialog}`。
 */
export function useConfirmDialog(): {
  ask: (req: ConfirmRequest) => void;
  dialog: React.ReactNode;
} {
  const [req, setReq] = useState<ConfirmRequest | null>(null);
  const [value, setValue] = useState("");
  /* 🔴 leastDestructiveRef = 取消按钮：§八 8.1「危险确认对话框初始焦点落**取消**按钮」。
     库会同时把它作为关闭后的焦点归还目标（returnFocusOnClose 默认开）。 */
  const cancelRef = useRef<HTMLButtonElement | null>(null);

  const ask = useCallback((next: ConfirmRequest) => {
    setValue("");
    setReq(next);
  }, []);

  const close = useCallback(() => setReq(null), []);

  const runConfirm = (): void => {
    if (!req) return;
    const v = value.trim();
    if (req.input && v === "") return; // 禁用即阻断的兜底（按钮同条件 disabled）
    req.onConfirm(v);
    setReq(null);
  };

  const emptyInput = Boolean(req?.input) && value.trim() === "";

  return {
    ask,
    dialog: req ? (
      <AlertDialog
        isOpen
        onClose={close}
        leastDestructiveRef={cancelRef}
        /* Esc = 取消（等价点击取消）；🔴 禁 Esc 直通执行步 —— 故 closeOnEsc 只映射到 close。 */
        closeOnEsc
        /* 点遮罩不关：危险确认必须显式选择，误触空白处不算"取消"以外的任何事。 */
        closeOnOverlayClick={false}
        isCentered
      >
        <AlertDialogOverlay sx={OVERLAY_SX}>
          {/* 🔴 阴影走 Chakra 的 `shadow` **prop** 而不是 sx.boxShadow —— 实测 sx 会被
              ModalContent 自带的 shadow 覆盖（渲染取证量到的是 Chakra 默认的
              `0 0 0 1px rgba(255,255,255,.05)`）。值仍取 §二 elevation 的 sticky 档。 */}
          <AlertDialogContent aria-label={req.title} shadow="sticky" sx={CONTENT_SX}>
            <Head>{req.title}</Head>
            <Body>
              {req.body}
              {req.input ? (
                <Field
                  mt="var(--aegis-space-md)"
                  size="md"
                  value={value}
                  placeholder={req.input.placeholder}
                  onChange={(e) => setValue(e.target.value)}
                  onKeyDown={(e) => {
                    if (e.key === "Enter") {
                      e.preventDefault();
                      runConfirm();
                    }
                  }}
                />
              ) : null}
            </Body>
            <Foot>
              <AppButton ref={cancelRef} variant="secondary" size="md" onClick={close}>
                {req.cancelLabel}
              </AppButton>
              <AppButton
                variant={req.tone === "danger" ? "danger" : "primary"}
                size="md"
                disabled={emptyInput}
                onClick={runConfirm}
              >
                {req.confirmLabel}
              </AppButton>
            </Foot>
          </AlertDialogContent>
        </AlertDialogOverlay>
      </AlertDialog>
    ) : null,
  };
}
