// 文件备份取数与动作（M-06）。
//
// 🔴 **不轮询**：备份域的规则/快照都是**用户操作驱动**的慢变量
//（规则只在编辑时变、快照只在备份/裁剪后变），2 秒节拍纯属浪费。
//   例外是「备份正在跑」期间——但那时用户在等一个 Promise，不需要轮询。
//   ⇒ 全部走 `staleTime: Infinity` + 动作后 `invalidate`（同 transfer 的 settings 口径）。
//
// 🔴 **进度区为什么是「阶段未知」**：后端 `commands.rs` 的 `progress` 回调目前是
//   空实现（事件名须走 contracts 注册表，W0-07 生成链未落地）。
//   在那之前，界面只能说「进行中」而**不能编造百分比**——
//   假进度条比没有进度条更糟：用户会以为快完了而不敢做别的事。
//   接线后：把 `useBackupRun` 的 `stage` 换成 `ProgressView` 消费即可。

import { useCallback } from "react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { BackendUnavailableError } from "@/lib/tauri/invoke";
import {
  asBackupError,
  cancelBackup,
  deleteRule,
  deleteSnapshot,
  executeRestore,
  exportRules,
  fetchRules,
  fetchSettings,
  importRules,
  listSnapshots,
  openSnapshotDir,
  planRestore,
  reorderRules,
  saveRule,
  saveSettings,
  setRuleEnabled,
  startBackup,
  verifySnapshot,
} from "@/lib/tauri/backup";
import type { ConflictPolicy, RuleView, SettingsView } from "@/types/backup";

export const BACKUP_RULES_KEY = ["backup", "rules"] as const;
export const BACKUP_SETTINGS_KEY = ["backup", "settings"] as const;
export const BACKUP_SNAPSHOTS_KEY = ["backup", "snapshots"] as const;

/**
 * 本域统一的 retry 策略（与 transfer / overview 同口径）。
 *
 * 🔴 `BackendUnavailableError` 是**永久性错误**（进程不在 Tauri 运行时内），
 * 重试再多次也必然失败；默认 `retry: 3` 只会让用户盯骨架屏白等。
 */
const RETRY_STRATEGY = (failureCount: number, error: unknown): boolean => {
  if (error instanceof BackendUnavailableError) return false;
  return failureCount < 2;
};

/** 规则列表 + 页头汇总（F-FB-01）。 */
export function useBackupRules() {
  return useQuery({
    queryKey: BACKUP_RULES_KEY,
    queryFn: fetchRules,
    retry: RETRY_STRATEGY,
    refetchOnWindowFocus: false,
    staleTime: Infinity,
  });
}

/** 快照列表（F-FB-26）。`ruleId` 为空时**不发起请求**（不选规则就没快照）。 */
export function useSnapshots(ruleId: string | null) {
  return useQuery({
    queryKey: [...BACKUP_SNAPSHOTS_KEY, ruleId],
    queryFn: () => listSnapshots(ruleId as string),
    enabled: ruleId !== null && ruleId !== "",
    retry: RETRY_STRATEGY,
    refetchOnWindowFocus: false,
    staleTime: Infinity,
  });
}

/** 设置（F-FB-24/29/34）。 */
export function useBackupSettings() {
  return useQuery({
    queryKey: BACKUP_SETTINGS_KEY,
    queryFn: fetchSettings,
    retry: RETRY_STRATEGY,
    refetchOnWindowFocus: false,
    staleTime: Infinity,
  });
}

/**
 * 保存规则（F-FB-02/04/05）。
 *
 * 🔴 `DRAFT_CONFIRM_REQUIRED` **不是失败**：那是后端在要求「来源全不存在，
 * 请用户确认按草稿存」（F-FB-04）。故 `onError` 把它识别出来并回传调用方，
 * 由界面弹确认后带 `confirmedDraft=true` 重试 —— 不能弹「保存失败」。
 */
export function useSaveRule() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: ({ rule, confirmedDraft }: { rule: RuleView; confirmedDraft: boolean }) =>
      saveRule(rule, confirmedDraft),
    onSuccess: () => {
      void queryClient.invalidateQueries({ queryKey: BACKUP_RULES_KEY });
    },
  });
}

/** 删除规则（F-FB-06）。🔴 **快照不连带删除**，界面必须说清。 */
export function useDeleteRule() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: (rule: RuleView) => deleteRule(rule),
    onSettled: () => {
      void queryClient.invalidateQueries({ queryKey: BACKUP_RULES_KEY });
    },
  });
}

/**
 * 立即备份（F-FB-16）。
 *
 * 🔴 这是**长任务**：Promise 在后端跑完前不 resolve。用户点「取消」走的是
 * 另一个命令（`cancelBackup`），不能中断这个 await。
 * 结束后规则与快照都要重取（快照数变了、可能裁剪了旧的）。
 */
export function useStartBackup() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: ({ ruleId, label }: { ruleId: string; label?: string }) =>
      startBackup(ruleId, label ?? ""),
    onSettled: () => {
      void queryClient.invalidateQueries({ queryKey: BACKUP_RULES_KEY });
      void queryClient.invalidateQueries({ queryKey: BACKUP_SNAPSHOTS_KEY });
    },
  });
}

/**
 * 请求取消（F-FB-19）。
 *
 * 🔴 reject（`NOT_RUNNING`）**不是失败**：那表示「你点慢了，它刚跑完」。
 * 界面应据此刷新列表，而不是弹「取消失败」。
 */
export function useCancelBackup() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: () => cancelBackup(),
    onSettled: () => {
      void queryClient.invalidateQueries({ queryKey: BACKUP_SNAPSHOTS_KEY });
    },
  });
}

/**
 * 手动校验快照（F-FB-28）。
 *
 * 🔴 校验结论会写回快照的 `info.json` ⇒ 成功后必须让**该规则**的快照列表重取，
 * 否则表格里的校验徽章仍显示旧结论（用户会以为校验没生效）。
 */
export function useVerifySnapshot() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: ({ ruleId, snapshotDir }: { ruleId: string; snapshotDir: string }) =>
      verifySnapshot(ruleId, snapshotDir),
    onSuccess: (_data, variables) => {
      void queryClient.invalidateQueries({
        queryKey: [...BACKUP_SNAPSHOTS_KEY, variables.ruleId],
      });
    },
  });
}

/**
 * 打开快照目录（F-FB-27）。只读动作 ⇒ 不需要失效任何缓存。
 *
 * 失败（越界 / 不存在 / 无权限）由后端分级上报，调用方负责把它写进操作日志。
 */
export function useOpenSnapshotDir() {
  return useMutation({
    mutationFn: ({ ruleId, snapshotDir }: { ruleId: string; snapshotDir: string }) =>
      openSnapshotDir(ruleId, snapshotDir),
  });
}

/**
 * 切换规则启用 / 停用（F-FB-07）。
 *
 * 🔴 用 `onSettled` 而非 `onSuccess` 失效缓存：F-FB-07 要求
 * 「落盘失败则界面不落地显示」—— 失败时重取一次列表，开关就弹回磁盘上的真实值，
 * 而不会停在用户点过的那个位置。
 */
export function useSetRuleEnabled() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: ({ ruleId, enabled }: { ruleId: string; enabled: boolean }) =>
      setRuleEnabled(ruleId, enabled),
    onSettled: () => {
      void queryClient.invalidateQueries({ queryKey: BACKUP_RULES_KEY });
    },
  });
}

/** 导出规则（F-FB-14）。只写文件、不改规则集合 ⇒ 无需失效缓存。 */
export function useExportRules() {
  return useMutation({
    mutationFn: ({ targetPath }: { targetPath: string }) => exportRules(targetPath),
  });
}

/**
 * 导入规则（F-FB-15，**合并式**）。
 *
 * 🔴 用 `onSettled`：导入后规则集合一定变了（无论覆盖还是新增），
 *   失败时也必须重取 —— 否则界面会停留在「导入前」的样子且没有任何解释。
 */
export function useImportRules() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: ({ sourcePath }: { sourcePath: string }) => importRules(sourcePath),
    onSettled: () => {
      void queryClient.invalidateQueries({ queryKey: BACKUP_RULES_KEY });
    },
  });
}

/**
 * 重排规则顺序（F-FB-08）。
 *
 * 🔴 `onSettled`（而非 `onSuccess`）：落盘失败时列表重取 ⇒ 顺序**自动回退**，
 *   满足规格「持久化失败时界面顺序回退」，且绝不出现「界面变了、磁盘没变」。
 */
export function useReorderRules() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: ({ orderedIds }: { orderedIds: string[] }) => reorderRules(orderedIds),
    onSettled: () => {
      void queryClient.invalidateQueries({ queryKey: BACKUP_RULES_KEY });
    },
  });
}

/** 删除快照（F-FB-30）。 */
export function useDeleteSnapshot() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: ({ ruleId, snapshotDir }: { ruleId: string; snapshotDir: string }) =>
      deleteSnapshot(ruleId, snapshotDir),
    onSettled: (_data, _error, variables) => {
      // 🔴 TanStack Query v5 的回调签名是 (data, error, variables, context) ——
      //    第 2 位是 error、**第 3 位**才是 variables。写成 (_d, variables)
      //    会把 Error 当变量用，`variables.ruleId` 直接编译不过。
      void queryClient.invalidateQueries({
        queryKey: [...BACKUP_SNAPSHOTS_KEY, variables.ruleId],
      });
      void queryClient.invalidateQueries({ queryKey: BACKUP_RULES_KEY });
    },
  });
}

/**
 * 恢复预演（F-FB-35，**只读**）。
 *
 * 🔴 与执行共用后端同一 `plan`，界面**不得**自己估冲突数 ——
 * 预演显示 0 冲突、执行时却覆盖了文件，那就是骗人。
 */
export function usePlanRestore() {
  return useMutation({
    mutationFn: ({
      ruleId,
      snapshotDir,
      customTarget,
      policy,
    }: {
      ruleId: string;
      snapshotDir: string;
      customTarget: string;
      policy: ConflictPolicy | "";
    }) => planRestore(ruleId, snapshotDir, customTarget, policy),
  });
}

/** 执行恢复（F-FB-36）。 */
export function useExecuteRestore() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: ({
      ruleId,
      snapshotDir,
      customTarget,
      policy,
    }: {
      ruleId: string;
      snapshotDir: string;
      customTarget: string;
      policy: ConflictPolicy | "";
    }) => executeRestore(ruleId, snapshotDir, customTarget, policy),
    onSettled: (_data, _error, variables) => {
      void queryClient.invalidateQueries({ queryKey: BACKUP_RULES_KEY });
      void queryClient.invalidateQueries({
        queryKey: [...BACKUP_SNAPSHOTS_KEY, variables.ruleId],
      });
    },
  });
}

/** 保存设置（后端原子写 + 数值钳制，R-19）。 */
export function useSaveBackupSettings() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: (settings: SettingsView) => saveSettings(settings),
    onSuccess: () => {
      void queryClient.invalidateQueries({ queryKey: BACKUP_SETTINGS_KEY });
      void queryClient.invalidateQueries({ queryKey: BACKUP_RULES_KEY });
    },
  });
}

/** 手动刷新规则（页头「刷新」按钮）。 */
export function useRefreshRules() {
  const queryClient = useQueryClient();
  return useCallback(async (): Promise<void> => {
    await queryClient.invalidateQueries({ queryKey: BACKUP_RULES_KEY });
  }, [queryClient]);
}

export { asBackupError };
