/**
 * 设置页 · 文件备份（全局默认值配置区）
 * ============================================================================
 * 2026-10-06 由文件备份页工具栏的「备份设置」弹窗迁入（用户裁决）。迁移依据不是
 * 「顺手整理」，而是两条既有口径本就判它归设置页：
 * - `settings-page.tsx` 的归属纪律：设置页只放「凭据 / 全局开关」，功能页只放该功能
 *   自己的操作，**同一件事不两处都做** ⇒ 备份页那个按钮一并删除，且不留跳转入口。
 * - 功能规格 `06-文件备份.md` §设置项 把这几项称作**全局设置项**。
 *
 * 🔴 数值钳制（R-19）：一律由后端 `clamped()` 二次钳制，前端 `min`/`max` 只做
 *   **输入侧**限幅，不自行「修正」用户填的值（那会让用户以为自己填了别的数）。
 *   并发上限 8 来自后端 `CONCURRENCY_MAX`。
 * 🟡 F-FB-29 要求校验强度「三档必须写清代价」—— 现状只靠选项文案本身
 *   （「全量读回比对」/「抽样比对 N 项」/「关闭」）承载，**迁移前后一样**，
 *   未补齐。要补需新增三条文案，属独立改动，不在本次移动范围内。
 */
import { Box, HStack, Input, Text, VStack, useColorModeValue } from "@chakra-ui/react";
import { AlertTriangle, FolderCog, Loader2 } from "lucide-react";
import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { open as openDialog } from "@tauri-apps/plugin-dialog";

import { AppButton } from "@/ui/components/app-button";
import { FailureDetail, failureDetail, failureReason } from "@/ui/components/failure-detail";
import {
  asBackupError,
  useBackupSettings,
  useSaveBackupSettings,
} from "@/features/FileBackup/hooks/use-backup";
import type { SettingsView } from "@/types/backup";

/** 与 `SteamApiKeySection` 同一套深浅分支取值（浅色包交付时只改第一个参数）。 */
export function BackupSettingsSection() {
  const { t } = useTranslation("backup");
  const query = useBackupSettings();
  const save = useSaveBackupSettings();
  const [draft, setDraft] = useState<SettingsView | null>(null);
  const [dirty, setDirty] = useState(false);
  const [flash, setFlash] = useState<{ kind: "ok" | "err"; text: string } | null>(null);

  const cardBg = useColorModeValue("white", "aegis.surface-soft");
  const borderColor = useColorModeValue("gray.200", "aegis.hairline");
  const subText = useColorModeValue("gray.600", "aegis.mute");

  /* 回源即覆盖草稿，但**用户正在编辑时不覆盖**（dirty）—— 否则保存成功后的
     invalidate 重取会把刚填的内容刷回服务端值，读起来像「没保存」。 */
  useEffect(() => {
    if (query.data && !dirty) setDraft(query.data);
  }, [query.data, dirty]);

  const patch = (next: Partial<SettingsView>): void => {
    setDirty(true);
    setDraft((prev) => (prev === null ? prev : { ...prev, ...next }));
  };

  const verifyHint = (sample: number): string =>
    sample === -1
      ? t("settings.verifyOff")
      : sample === 0
        ? t("settings.verifyFull")
        : t("settings.verifySampleN", { count: sample });

  const saveSettings = (): void => {
    if (draft === null) return;
    save.mutate(draft, {
      onSuccess: () => {
        setDirty(false);
        setFlash({ kind: "ok", text: t("settings.saved") });
      },
      onError: (e) => setFlash({ kind: "err", text: asBackupError(e).message }),
    });
  };

  return (
    <Box
      bg={cardBg}
      border="1px solid"
      borderColor={borderColor}
      borderRadius="var(--aegis-radius-sm)"
      p="lg"
    >
      <HStack spacing="sm" mb={1}>
        <FolderCog size={16} strokeWidth={1.5} color="var(--aegis-accent)" aria-hidden />
        <Text fontSize="14" fontWeight={700}>
          {t("settings.title")}
        </Text>
      </HStack>
      <Text fontSize="12" color={subText} mb="md">
        {t("settings.sectionNote")}
      </Text>

      {query.isPending && (
        <HStack spacing="xs" color={subText} fontSize="12">
          <Loader2 size={14} strokeWidth={1.5} className="spin" aria-hidden />
          {t("settings.loading")}
        </HStack>
      )}

      {query.isError && (
        <HStack
          spacing="sm"
          wrap="wrap"
          fontSize="12"
          color="var(--aegis-error)"
          p="sm"
          border="1px solid"
          borderColor="var(--aegis-error)"
          borderRadius="var(--aegis-radius-sm)"
        >
          <AlertTriangle size={14} strokeWidth={1.5} aria-hidden />
          {/* 🔴 主线只说「后端未运行」；命令名 backup_get_settings 收进折叠（UI 审查 D7）。
              wrap 是给展开后的详情行留的：那行 flex-basis 100%，不换行会挤扁本行的重试按钮。 */}
          <Text flex="1">{failureReason(query.error)}</Text>
          <FailureDetail detail={failureDetail(query.error)} />
          <AppButton variant="ghost" size="sm" onClick={() => void query.refetch()}>
            {t("errors.retry")}
          </AppButton>
        </HStack>
      )}

      {draft && (
        <VStack spacing="md" align="stretch">
          <VStack spacing="xs" align="stretch">
            <Text fontSize="12" color={subText}>
              {t("settings.globalRoot")}
            </Text>
            <HStack spacing="sm">
              <Input
                size="sm"
                value={draft.globalRoot}
                placeholder={t("settings.globalRootEmpty")}
                onChange={(e) => patch({ globalRoot: e.target.value })}
              />
              {/* AppButton 是 styled.button，不吃 Chakra 的 shorthand prop，
                  故收缩用外层 Box 给。 */}
              <Box flexShrink={0}>
                <AppButton
                  variant="secondary"
                  size="sm"
                  onClick={() => {
                    // 🔴 `void` 包裹：见 restore-dialog 同处注释
                    void (async () => {
                      const picked = await openDialog({
                        directory: true,
                        title: t("settings.choose"),
                      });
                      if (typeof picked === "string") patch({ globalRoot: picked });
                    })();
                  }}
                >
                  {t("settings.choose")}
                </AppButton>
              </Box>
            </HStack>
          </VStack>

          <HStack spacing="md" align="start" flexWrap="wrap">
            <VStack spacing="xs" align="stretch" flex="1" minW="160px">
              <Text fontSize="12" color={subText}>
                {t("settings.defaultKeepCount")}
              </Text>
              <Input
                size="sm"
                type="number"
                min={1}
                max={100}
                value={draft.defaultKeepCount}
                onChange={(e) =>
                  patch({ defaultKeepCount: Number.parseInt(e.target.value, 10) || 1 })
                }
              />
            </VStack>
            <VStack spacing="xs" align="stretch" flex="1" minW="160px">
              <Text fontSize="12" color={subText}>
                {t("settings.concurrency")}
              </Text>
              <Input
                size="sm"
                type="number"
                min={1}
                max={8}
                value={draft.concurrency}
                onChange={(e) => patch({ concurrency: Number.parseInt(e.target.value, 10) || 1 })}
              />
            </VStack>
          </HStack>

          <VStack spacing="xs" align="stretch">
            <Text fontSize="12" color={subText}>
              {t("settings.verifySample")}
            </Text>
            <HStack spacing="sm" flexWrap="wrap" as="fieldset">
              {(
                [
                  [-1, "settings.verifyOff"],
                  [0, "settings.verifyFull"],
                  [200, "settings.verifySampleN"],
                ] as [number, string][]
              ).map(([value, key]) => (
                <HStack key={value} spacing="xs" as="label" cursor="pointer">
                  <input
                    type="radio"
                    name="backupVerify"
                    checked={value === 200 ? draft.verifySample > 0 : draft.verifySample === value}
                    onChange={() => patch({ verifySample: value })}
                  />
                  <Text fontSize="12">{t(key, value === 200 ? { count: 200 } : undefined)}</Text>
                </HStack>
              ))}
            </HStack>
            <Text fontSize="11" color={subText}>
              {verifyHint(draft.verifySample)}
            </Text>
          </VStack>

          <VStack spacing="xs" align="stretch">
            <Text fontSize="12" color={subText}>
              {t("settings.defaultPolicy")}
            </Text>
            <HStack spacing="sm" flexWrap="wrap" as="fieldset">
              {(
                [
                  ["rename", "restore.policyRename"],
                  ["overwrite", "restore.policyOverwrite"],
                  ["skip", "restore.policySkip"],
                ] as [SettingsView["defaultConflictPolicy"], string][]
              ).map(([value, key]) => (
                <HStack key={value} spacing="xs" as="label" cursor="pointer">
                  <input
                    type="radio"
                    name="backupDefaultPolicy"
                    checked={draft.defaultConflictPolicy === value}
                    onChange={() => patch({ defaultConflictPolicy: value })}
                  />
                  <Text fontSize="12">{t(key)}</Text>
                </HStack>
              ))}
            </HStack>
          </VStack>

          <HStack spacing="sm" justify="flex-end">
            {flash && (
              <Text
                fontSize="12"
                color={flash.kind === "ok" ? "var(--aegis-success)" : "var(--aegis-error)"}
                fontWeight={700}
                marginRight="auto"
              >
                {flash.text}
              </Text>
            )}
            <AppButton variant="primary" size="sm" disabled={save.isPending} onClick={saveSettings}>
              {t("settings.save")}
            </AppButton>
          </HStack>
        </VStack>
      )}
    </Box>
  );
}

export default BackupSettingsSection;
