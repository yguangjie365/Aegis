// 恢复向导弹窗（目标 → 冲突策略 → 只读预演 → 执行）。
//
// 🔴 **预演与执行共用后端同一 `plan`**（`restore::plan`，F-FB-35）——
//   界面**不得**自己估冲突数。预演显示 0 冲突、执行时却覆盖了文件，
//   那就是骗人；F-FB-35 把「同一判据只实现一次并被各处共用」列为一致性红线。
//
// 🔴 **覆盖策略必须额外警告**：覆盖是**不可撤销**地丢掉目标位置现有文件，
//   破坏性操作要二次确认 + 列出对象（R-21）。

import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import styled from "@emotion/styled";
import { open as openDialog } from "@tauri-apps/plugin-dialog";
import { AppButton } from "@/ui/components/app-button";
import { asBackupError, useExecuteRestore, usePlanRestore } from "../hooks/use-backup";
import { formatCount } from "../lib/format";
import { useLogStore } from "../store/log-store";
import type { ConflictPolicy, RestoreOutcome, SnapshotView } from "@/types/backup";

const Overlay = styled.div`
  position: fixed;
  inset: 0;
  background: color-mix(in srgb, #000 62%, transparent);
  display: flex;
  align-items: center;
  justify-content: center;
  z-index: 1400;
`;

const Dialog = styled.div`
  width: 720px;
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

const Section = styled.div`
  display: flex;
  flex-direction: column;
  gap: var(--aegis-space-xs);
`;

const Label = styled.div`
  font-size: var(--aegis-type-caption-size);
  color: var(--aegis-mute);
`;

const Row = styled.div`
  display: flex;
  align-items: center;
  gap: var(--aegis-space-sm);
  flex-wrap: wrap;
  font-size: var(--aegis-type-body-size);
  color: var(--aegis-ink);
`;

const Input = styled.input`
  flex: 1 1 240px;
  /* h=36（§三 input 行；UI 审查 M1 余项，与 rule-editor 同步归位） */
  height: 36px;
  padding: 0 var(--aegis-space-sm);
  font: inherit;
  color: var(--aegis-ink);
  background: var(--aegis-surface-inset);
  border: 1px solid var(--aegis-border-interactive);
  border-radius: var(--aegis-radius-sm);

  &:focus-visible {
    outline: 2px solid var(--aegis-focus-ring);
    outline-offset: 2px;
  }
`;

/** 预演数字块：四项计数 + 落点，一眼看清会发生什么。 */
const Preview = styled.div`
  border: 1px solid var(--aegis-hairline);
  border-radius: var(--aegis-radius-sm);
  background: var(--aegis-surface-soft);
  padding: var(--aegis-space-md);
  display: flex;
  flex-direction: column;
  gap: var(--aegis-space-xs);
  font-size: var(--aegis-type-body-size);
  color: var(--aegis-ink);
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

export function RestoreDialog({
  snapshot,
  onClose,
}: {
  snapshot: SnapshotView | null;
  onClose: () => void;
}) {
  const { t } = useTranslation("backup");
  const plan = usePlanRestore();
  const run = useExecuteRestore();
  // 🔴 预演**不记日志**：它每改一次目标或策略就重算一次，逐次留痕会把日志刷成流水账。
  //   真正需要事后回看的是「执行结果」—— 写了几个、失败几个、跳过几个。
  const logPush = useLogStore((s) => s.push);
  const [custom, setCustom] = useState("");
  const [useCustom, setUseCustom] = useState(false);
  const [policy, setPolicy] = useState<ConflictPolicy>("rename");
  const [result, setResult] = useState<RestoreOutcome | null>(null);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    if (!snapshot) return;
    setCustom("");
    setUseCustom(false);
    setPolicy("rename");
    setResult(null);
    setError(null);
    plan.reset();
    run.reset();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [snapshot]);

  if (!snapshot) return null;

  const runPlan = (
    nextPolicy: ConflictPolicy,
    nextCustom: string,
    nextUseCustom: boolean,
  ): void => {
    plan.mutate(
      {
        ruleId: snapshot.ruleId,
        snapshotDir: snapshot.displayPath,
        customTarget: nextUseCustom ? nextCustom : "",
        policy: nextPolicy,
      },
      {
        onSuccess: () => setError(null),
        onError: (e) => setError(asBackupError(e).message),
      },
    );
  };

  const preview = plan.data;
  const destructive = policy === "overwrite" && (preview?.conflicts ?? 0) > 0;

  return (
    <Overlay
      role="dialog"
      aria-modal="true"
      aria-label={t("restore.title", { ns: "backup" })}
      onClick={(e) => {
        if (e.target === e.currentTarget) onClose();
      }}
    >
      <Dialog>
        <Title>{t("restore.title", { ns: "backup" })}</Title>
        <Label>{t("restore.snapshot", { ns: "backup", name: snapshot.displayPath })}</Label>

        {/* ── ① 目标 ── */}
        <Section>
          <Label>{t("restore.target", { ns: "backup" })}</Label>
          <Row>
            <label>
              <input
                type="radio"
                name="target"
                checked={!useCustom}
                onChange={() => {
                  setUseCustom(false);
                  runPlan(policy, "", false);
                }}
              />{" "}
              {t("restore.targetOriginal", { ns: "backup" })}
            </label>
            <label>
              <input
                type="radio"
                name="target"
                checked={useCustom}
                onChange={() => {
                  setUseCustom(true);
                  runPlan(policy, custom, true);
                }}
              />{" "}
              {t("restore.targetCustom", { ns: "backup" })}
            </label>
            {useCustom && (
              <>
                <Input
                  value={custom}
                  onChange={(e) => setCustom(e.target.value)}
                  onBlur={() => runPlan(policy, custom, true)}
                />
                <AppButton
                  variant="secondary"
                  size="sm"
                  onClick={() => {
                    // 🔴 `void` 包裹：onClick 不消费返回值，async 回调会触发
                    //    no-misused-promises，且未处理的 reject 会被静默吞掉
                    void (async () => {
                      const picked = await openDialog({
                        directory: true,
                        title: t("editor.targetChoose", { ns: "backup" }),
                      });
                      if (typeof picked === "string") {
                        setCustom(picked);
                        runPlan(policy, picked, true);
                      }
                    })();
                  }}
                >
                  {t("editor.targetChoose", { ns: "backup" })}
                </AppButton>
              </>
            )}
          </Row>
        </Section>

        {/* ── ② 冲突策略 ── */}
        <Section>
          <Label>{t("restore.policy", { ns: "backup" })}</Label>
          <Row>
            {(
              [
                ["rename", "restore.policyRename"],
                ["overwrite", "restore.policyOverwrite"],
                ["skip", "restore.policySkip"],
              ] as [ConflictPolicy, string][]
            ).map(([value, key]) => (
              <label key={value}>
                <input
                  type="radio"
                  name="policy"
                  checked={policy === value}
                  onChange={() => {
                    setPolicy(value);
                    runPlan(value, custom, useCustom);
                  }}
                />{" "}
                {t(key, { ns: "backup" })}
              </label>
            ))}
          </Row>
          {destructive && (
            <WarnBox>{t("restore.policyWarningOverwrite", { ns: "backup" })}</WarnBox>
          )}
        </Section>

        {/* ── ③ 只读预演 ── */}
        <Section>
          <Label>{t("restore.preview", { ns: "backup" })}</Label>
          {plan.isPending && <Label>{t("restore.previewPending", { ns: "backup" })}</Label>}
          {error !== null && <ErrorBox>{error}</ErrorBox>}
          {preview && (
            <Preview>
              <div>
                {t("restore.previewTotal", { ns: "backup", count: formatCount(preview.total) })}
              </div>
              <div>
                {t("restore.previewConflicts", {
                  ns: "backup",
                  count: formatCount(preview.conflicts),
                })}
              </div>
              <div>
                {t("restore.previewRejected", {
                  ns: "backup",
                  count: formatCount(preview.guardRejected),
                })}
              </div>
              <div>
                {t("restore.previewFresh", {
                  ns: "backup",
                  count: formatCount(preview.freshWrites),
                })}
              </div>
              <Label>{t("restore.previewTarget", { ns: "backup", path: preview.target })}</Label>
            </Preview>
          )}
        </Section>

        {/* ── ④ 执行结果 ── */}
        {result && (
          <Preview>
            <strong>{t("restore.resultTitle", { ns: "backup" })}</strong>
            <div>
              {t("restore.resultSucceeded", { ns: "backup", count: formatCount(result.succeeded) })}
            </div>
            {result.failed > 0 && (
              <div style={{ color: "var(--aegis-error)" }}>
                {t("restore.resultFailed", { ns: "backup", count: formatCount(result.failed) })}
              </div>
            )}
            {result.verifyMismatched > 0 && (
              <div style={{ color: "var(--aegis-warning)" }}>
                {t("restore.resultMismatched", {
                  ns: "backup",
                  count: formatCount(result.verifyMismatched),
                })}
              </div>
            )}
            {result.skipped > 0 && (
              <div>
                {t("restore.resultSkipped", {
                  ns: "backup",
                  count: formatCount(result.skipped),
                })}
              </div>
            )}
            <div>
              {t("restore.resultEmptyDirs", {
                ns: "backup",
                count: formatCount(result.emptyDirsRebuilt),
              })}
            </div>
            {result.failures.slice(0, 10).map((f, i) => (
              <ErrorBox key={`${f.code}-${i}`}>
                {f.text}
                {f.path && ` · ${f.path}`}
              </ErrorBox>
            ))}
            {result.skipReasons.slice(0, 10).map((f, i) => (
              <Label key={`skip-${f.code}-${i}`}>
                {f.text}
                {f.path && ` · ${f.path}`}
              </Label>
            ))}
          </Preview>
        )}
        {run.isError && <ErrorBox>{asBackupError(run.error).message}</ErrorBox>}

        <Footer>
          <AppButton variant="ghost" size="sm" onClick={onClose}>
            {t(result ? "restore.close" : "editor.cancel", { ns: "backup" })}
          </AppButton>
          {!result && (
            <AppButton
              variant={destructive ? "danger" : "primary"}
              size="sm"
              // 🔴 没有预演就不许执行：用户必须在知道会发生什么之后才点确认
              disabled={!preview || preview.total === 0 || run.isPending}
              onClick={() => {
                run.mutate(
                  {
                    ruleId: snapshot.ruleId,
                    snapshotDir: snapshot.displayPath,
                    customTarget: useCustom ? custom : "",
                    policy,
                  },
                  {
                    onSuccess: (outcome) => {
                      setResult(outcome);
                      // 🔴 写入失败**或**读回校验不符 ⇒ 整体结论为不成功（F-FB-36），
                      //   两者合并计数并落 error；只报 succeeded 会把「写了但内容不对」显示成成功。
                      const bad = outcome.failed + outcome.verifyMismatched;
                      logPush(
                        bad > 0 ? "error" : "success",
                        t("log.restoreDone", {
                          ok: formatCount(outcome.succeeded),
                          failed: formatCount(bad),
                          skipped: formatCount(outcome.skipped),
                        }),
                      );
                    },
                    onError: (e) => {
                      const parsed = asBackupError(e);
                      setError(parsed.message);
                      logPush("error", t("log.restoreFailed", { message: parsed.message }));
                    },
                  },
                );
              }}
            >
              {t(run.isPending ? "restore.running" : "restore.run", { ns: "backup" })}
            </AppButton>
          )}
        </Footer>
      </Dialog>
    </Overlay>
  );
}
