// 规则编辑弹窗（新建/ 编辑共用）。
//
// 🔴 **两条硬约束**（布局稿 §二 + 规格 F-FB-21）：
//   ① 卷影/定时开关**恒可见、禁用、就地说明原因** —— 不隐藏。
//      F-FB-21 明确要求用户能理解「为什么勾了仍无效」；隐藏开关等于
//      让用户以为功能不存在，而它其实只是形态待定。
//   ② 来源全不存在时，保存会 reject `DRAFT_CONFIRM_REQUIRED`（F-FB-04）——
//      那**不是失败**，是后端在要求用户确认。必须弹确认再带 confirmedDraft 重试。
//
// 🔴 零裸令牌：颜色/间距/圆角/字号全部 `var(--aegis-*)`。

import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import styled from "@emotion/styled";
import { open as openDialog } from "@tauri-apps/plugin-dialog";
import { AppButton } from "@/ui/components/app-button";
import { useConfirmDialog } from "@/ui/components/confirm-dialog";
import { asBackupError, useSaveRule } from "../hooks/use-backup";
import { emptyRule } from "@/types/backup";
import type { RuleView } from "@/types/backup";

const Overlay = styled.div`
  position: fixed;
  inset: 0;
  background: color-mix(in srgb, #000 62%, transparent);
  display: flex;
  align-items: center;
  justify-content: center;
  /* z-index 高于壳层一切：弹窗是全局最上层（04分册 §三） */
  z-index: 1400;
`;

const Dialog = styled.div`
  width: 640px;
  max-width: calc(100vw - 48px);
  max-height: calc(100vh - 96px);
  overflow-y: auto;
  background: var(--aegis-canvas);
  border: 1px solid var(--aegis-hairline);
  border-radius: var(--aegis-radius-sm);
  padding: var(--aegis-space-lg);
  display: flex;
  flex-direction: column;
  gap: var(--aegis-space-md);
`;

const Title = styled.h2`
  margin: 0;
  font-size: var(--aegis-type-h3-size);
  font-weight: 700;
  color: var(--aegis-ink);
`;

const Field = styled.label`
  display: flex;
  flex-direction: column;
  gap: var(--aegis-space-xs);
  font-size: var(--aegis-type-caption-size);
  color: var(--aegis-mute);
`;

const Input = styled.input`
  /* h=36（规范 §三 input 行）。原 32 是页内自写值 ⇒ 全站输入框高度漂到 28/31/32/36 四种
     （2026-10-06 UI 审查 M1）。边框本就是 border-interactive，无需改。 */
  height: 36px;
  padding: 0 var(--aegis-space-sm);
  font: inherit;
  font-size: var(--aegis-type-body-size);
  color: var(--aegis-ink);
  background: var(--aegis-surface-inset);
  border: 1px solid var(--aegis-border-interactive);
  border-radius: var(--aegis-radius-sm);

  &:focus-visible {
    outline: 2px solid var(--aegis-focus-ring);
    outline-offset: 2px;
  }
`;

const TextArea = styled.textarea`
  min-height: 64px;
  padding: var(--aegis-space-xs) var(--aegis-space-sm);
  font: inherit;
  font-size: var(--aegis-type-body-size);
  color: var(--aegis-ink);
  background: var(--aegis-surface-inset);
  border: 1px solid var(--aegis-border-interactive);
  border-radius: var(--aegis-radius-sm);
  resize: vertical;

  &:focus-visible {
    outline: 2px solid var(--aegis-focus-ring);
    outline-offset: 2px;
  }
`;

const Row = styled.div`
  display: flex;
  gap: var(--aegis-space-sm);
  flex-wrap: wrap;
  align-items: center;
`;

const SourceList = styled.ul`
  list-style: none;
  margin: 0;
  padding: 0;
  display: flex;
  flex-direction: column;
  gap: 2px;
`;

const SourceItem = styled.li`
  display: flex;
  align-items: center;
  gap: var(--aegis-space-sm);
  font-size: var(--aegis-type-caption-size);
  color: var(--aegis-ink);
  background: var(--aegis-surface-inset);
  border-radius: var(--aegis-radius-sm);
  padding: var(--aegis-space-xs) var(--aegis-space-sm);
`;

const SourcePath = styled.span`
  flex: 1 1 auto;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  font-family: var(--font-mono);
`;

/** 🔴 复选项行：标签 + 说明 + 控件。说明文字是「禁用原因」的载体，不能省。 */
const CheckRow = styled.div`
  display: flex;
  align-items: flex-start;
  gap: var(--aegis-space-sm);
  font-size: var(--aegis-type-body-size);
  color: var(--aegis-ink);
`;

const CheckHint = styled.div`
  font-size: var(--aegis-type-caption-size);
  color: var(--aegis-mute);
`;

const WarnBox = styled.div`
  border: 1px solid color-mix(in srgb, var(--aegis-warning) 42%, transparent);
  background: color-mix(in srgb, var(--aegis-warning) 10%, transparent);
  border-radius: var(--aegis-radius-sm);
  padding: var(--aegis-space-sm) var(--aegis-space-md);
  font-size: var(--aegis-type-caption-size);
  color: var(--aegis-warning);
`;

const ErrorBox = styled.div`
  font-size: var(--aegis-type-caption-size);
  color: var(--aegis-error);
`;

const Footer = styled.div`
  display: flex;
  justify-content: flex-end;
  gap: var(--aegis-space-sm);
`;

export function RuleEditorDialog({
  open,
  rule,
  onClose,
}: {
  open: boolean;
  rule: RuleView | null;
  onClose: () => void;
}) {
  const { t } = useTranslation("backup");
  /* 🔴 确认/输入承载件（§三 dialog 行；UI 审查 B3）：本文件原有两处原生框 ——
     `window.prompt`（手动加来源路径）与 `window.confirm`（草稿保存确认）—— 均已改调它。 */
  const { ask, dialog } = useConfirmDialog();
  const save = useSaveRule();
  const [draft, setDraft] = useState<RuleView>(emptyRule());
  const [keepText, setKeepText] = useState("7");
  const [exclusionText, setExclusionText] = useState("");
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    if (!open) return;
    const base = rule ?? emptyRule();
    setDraft(base);
    setKeepText(String(base.keepCount));
    setExclusionText(base.exclusions.join("\n"));
    setError(null);
    save.reset();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [open, rule]);

  if (!open) return null;

  const patch = (next: Partial<RuleView>): void => setDraft((prev) => ({ ...prev, ...next }));

  const addSources = async (directory: boolean): Promise<void> => {
    const picked = await openDialog({
      directory,
      multiple: true,
      title: t(directory ? "editor.addFolder" : "editor.addFile", { ns: "backup" }),
    });
    if (picked === null) return;
    const list = Array.isArray(picked) ? picked : [picked];
    // 🔴 F-FB-02：去重 + 去首尾空白（后端也会做一次，前端做是为了即时反馈）
    const merged = [...draft.sources];
    for (const path of list) {
      const clean = path.trim();
      if (clean !== "" && !merged.includes(clean)) merged.push(clean);
    }
    patch({ sources: merged });
  };

  const addManual = (): void => {
    /* 原生 window.prompt 下线（B3）：换成带输入框的确认对话框。
       空值不给「添加」（禁用即阻断，§三 dialog）；重复路径仍丢弃 ——
       旧实现是 trim 后静默丢弃，用户看不到"为什么没加上"，现在至少有输入框可自查。 */
    ask({
      title: t("editor.manualTitle"),
      confirmLabel: t("dialog.add"),
      cancelLabel: t("dialog.cancel"),
      input: { placeholder: t("dialog.pathPlaceholder") },
      onConfirm: (v) => {
        if (!draft.sources.includes(v)) patch({ sources: [...draft.sources, v] });
      },
    });
  };

  const submit = (confirmedDraft: boolean): void => {
    const payload: RuleView = {
      ...draft,
      // 🔴 保留份数以输入框为准；GFS 打开时后端会按 1 处理并忽略此值
      keepCount: Math.max(1, Math.min(100, Number.parseInt(keepText, 10) || 1)),
      exclusions: exclusionText
        .split("\n")
        .map((line) => line.trim())
        .filter((line) => line !== ""),
    };
    save.mutate(
      { rule: payload, confirmedDraft },
      {
        onSuccess: onClose,
        onError: (e) => {
          const parsed = asBackupError(e);
          // 🔴 F-FB-04：这条 reject 是「请确认按草稿存」，不是失败 ⇒ 弹确认后重试
          if (parsed.code === "DRAFT_CONFIRM_REQUIRED") {
            const missing = parsed.message.split("：")[1] ?? "";
            /* 原生 confirm 下线（B3）。这条**既非失败也非破坏性操作**（只是"仍按草稿保存"，
               之后可随时再编辑）⇒ 用 primary 而非 danger，不把普通确认染成警告色。 */
            ask({
              title: t("editor.draftConfirmTitle"),
              body: t("editor.draftConfirmBody", { list: missing }),
              confirmLabel: t("dialog.saveDraft"),
              cancelLabel: t("dialog.cancel"),
              tone: "primary",
              onConfirm: () => submit(true),
            });
            return;
          }
          setError(parsed.message);
        },
      },
    );
  };

  return (
    <Overlay
      role="dialog"
      aria-modal="true"
      aria-label={t(rule ? "editor.titleEdit" : "editor.titleNew", { ns: "backup" })}
      onClick={(e) => {
        if (e.target === e.currentTarget) onClose();
      }}
    >
      <Dialog>
        <Title>{t(rule ? "editor.titleEdit" : "editor.titleNew", { ns: "backup" })}</Title>

        {/* 确认/输入承载件（B3）。Chakra AlertDialog 自带 Portal（挂到 document.body），
            写在这里只是就近声明 —— 不占本弹窗布局流，z 序自然在本弹窗之上。 */}
        {dialog}

        <Field>
          {t("editor.name", { ns: "backup" })}
          <Input
            value={draft.name}
            placeholder={t("editor.namePlaceholder", { ns: "backup" })}
            onChange={(e) => patch({ name: e.target.value })}
          />
        </Field>

        <Field>
          {t("editor.note", { ns: "backup" })}
          <Input
            value={draft.note}
            placeholder={t("editor.notePlaceholder", { ns: "backup" })}
            onChange={(e) => patch({ note: e.target.value })}
          />
        </Field>

        {/* ── 来源 ── */}
        <Field>
          {t("editor.sources", { ns: "backup" })}
          <Row>
            <AppButton variant="secondary" size="sm" onClick={() => void addSources(true)}>
              {t("editor.addFolder", { ns: "backup" })}
            </AppButton>
            <AppButton variant="secondary" size="sm" onClick={() => void addSources(false)}>
              {t("editor.addFile", { ns: "backup" })}
            </AppButton>
            <AppButton variant="ghost" size="sm" onClick={addManual}>
              {t("editor.addManual", { ns: "backup" })}
            </AppButton>
          </Row>
          {draft.sources.length === 0 ? (
            <CheckHint>{t("editor.sourcesEmpty", { ns: "backup" })}</CheckHint>
          ) : (
            <SourceList>
              {draft.sources.map((source) => (
                <SourceItem key={source}>
                  <SourcePath title={source}>{source}</SourcePath>
                  <AppButton
                    variant="ghost"
                    size="sm"
                    onClick={() => patch({ sources: draft.sources.filter((s) => s !== source) })}
                  >
                    {t("editor.remove", { ns: "backup" })}
                  </AppButton>
                </SourceItem>
              ))}
            </SourceList>
          )}
        </Field>

        {/* ── 落点 ── */}
        <Field>
          {t("editor.target", { ns: "backup" })}
          <Row>
            <label>
              <input
                type="radio"
                name="target"
                checked={draft.customRoot.trim() === ""}
                onChange={() => patch({ customRoot: "" })}
              />{" "}
              {t("editor.targetGlobal", { ns: "backup" })}
            </label>
            <label>
              <input
                type="radio"
                name="target"
                checked={draft.customRoot.trim() !== ""}
                onChange={() => {
                  // 🔴 选「自定义」但还没选目录时给个可编辑的默认值，
                  //    而不是留空 —— 留空会被后端判成「用全局根」，
                  //    用户以为选了自定义却存成了全局。
                  if (draft.customRoot.trim() === "") {
                    patch({ customRoot: "D:\\AegisBackup" });
                  }
                }}
              />{" "}
              {t("editor.targetCustom", { ns: "backup" })}
            </label>
            {draft.customRoot.trim() !== "" && (
              <>
                <Input
                  value={draft.customRoot}
                  onChange={(e) => patch({ customRoot: e.target.value })}
                />
                <AppButton
                  variant="secondary"
                  size="sm"
                  onClick={() => {
                    // 🔴 用 `void` 包裹而非 async onClick：onClick 的返回值被 React 丢弃，
                    //    而 async 函数返回Promise ⇒ 触发 no-misused-promises
                    //    （且未处理的 reject 会被静默吞掉）。
                    void (async () => {
                      const picked = await openDialog({
                        directory: true,
                        title: t("editor.targetChoose", { ns: "backup" }),
                      });
                      if (typeof picked === "string") patch({ customRoot: picked });
                    })();
                  }}
                >
                  {t("editor.targetChoose", { ns: "backup" })}
                </AppButton>
              </>
            )}
          </Row>
        </Field>

        {/* ── 保留与排除 ── */}
        <Field>
          {t("editor.retention", { ns: "backup" })}
          <Row>
            <Input
              type="number"
              min={1}
              max={100}
              value={keepText}
              style={{ width: 96 }}
              onChange={(e) => setKeepText(e.target.value)}
            />
            <CheckHint>{t("editor.keepCount", { ns: "backup" })}</CheckHint>
          </Row>
          <TextArea
            value={exclusionText}
            placeholder={t("editor.exclusionsHint", { ns: "backup" })}
            onChange={(e) => setExclusionText(e.target.value)}
          />
        </Field>

        {/* ── 可选能力：恒可见 + 禁用 + 就地说明（F-FB-21）── */}
        <Field>
          {t("editor.capabilities", { ns: "backup" })}
          <CheckRow>
            <input type="checkbox" disabled checked={false} onChange={() => undefined} />
            <div>
              <div>{t("editor.useShadow", { ns: "backup" })}</div>
              {/* 🔴 禁用原因就在旁边 —— 不写这一行就成了「无理由不给用」 */}
              <CheckHint>{t("rule.shadowUnavailable", { ns: "backup" })}</CheckHint>
            </div>
          </CheckRow>
          <CheckRow>
            <input type="checkbox" disabled checked={false} onChange={() => undefined} />
            <div>
              <div>{t("editor.useSchedule", { ns: "backup" })}</div>
              <CheckHint>{t("rule.scheduleUndecided", { ns: "backup" })}</CheckHint>
            </div>
          </CheckRow>
        </Field>

        {draft.sources.length === 0 && (
          <WarnBox>{t("editor.draftWarning", { ns: "backup" })}</WarnBox>
        )}
        {error !== null && <ErrorBox>{error}</ErrorBox>}

        <Footer>
          <AppButton variant="ghost" size="sm" onClick={onClose}>
            {t("editor.cancel", { ns: "backup" })}
          </AppButton>
          <AppButton
            variant="primary"
            size="sm"
            disabled={save.isPending}
            onClick={() => submit(false)}
          >
            {t(save.isPending ? "editor.saving" : "editor.save", { ns: "backup" })}
          </AppButton>
        </Footer>
      </Dialog>
    </Overlay>
  );
}
