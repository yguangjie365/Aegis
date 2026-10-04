// 文件互传取数与动作（M-05 片1：服务启停 / 设置 / 设备列表）。
//
// 🔴 节拍只由 `refetchInterval` 承载，值随 `usePageLiveness()` 在 `2000 | false` 间切换：
// 切到 false 时 TanStack Query 会**清掉**定时器（R-19「不存在全局常驻定时器」），
// 离开本页（组件卸载）同样自然停表——不需要额外的"停表"代码路径。
//
// 🔴 本域**不使用乐观更新**：服务启停与设置保存都由后端定论
// （R-28 的 `allStopped`、F-FT-59 的原子写）。乐观地把 `running` 改成 true
// 会在后端失败时给用户一个"看起来起来了"的假象——正是 R-08「状态诚实」要避免的。

import { useCallback } from "react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { useTranslation } from "react-i18next";
import { open } from "@tauri-apps/plugin-dialog";
import { BackendUnavailableError } from "@/lib/tauri/invoke";
import { REFRESH_INTERVAL_MS, usePageLiveness } from "@/lib/timing/use-tick";
import {
  asTransferError,
  fetchTransferStatus,
  listTransferPeers,
  loadTransferSettings,
  saveTransferSettings,
  sendTransferFile,
  startTransferService,
  stopTransferService,
} from "@/lib/tauri/transfer";
import type { TransferSettings } from "@/types/transfer";

export const TRANSFER_STATUS_KEY = ["transfer", "status"] as const;
export const TRANSFER_SETTINGS_KEY = ["transfer", "settings"] as const;
export const TRANSFER_PEERS_KEY = ["transfer", "peers"] as const;

/**
 * 本域统一的 retry 策略（与 overview 同口径）。
 *
 * 🔴 `BackendUnavailableError` 是**永久性错误**（进程不在 Tauri 运行时内），
 * 重试再多次也必然失败；默认 `retry: 3` 只会让用户盯着骨架屏白等。
 */
const RETRY_STRATEGY = (failureCount: number, error: unknown): boolean => {
  if (error instanceof BackendUnavailableError) return false;
  return failureCount < 3;
};

/**
 * 服务状态（F-FT-02/06/08）。
 *
 * 轮询理由：未启动 → 启动、启动 → 停止这些跳变必须让界面跟上；
 * 运行中时还要把配对码与剩余时间带出来（F-FT-38）。
 * 🟠 剩余时间目前按 2 秒粒度刷新；F-FT-38 要的"秒级"倒计时需要一个 1 秒 tick 封装
 * （本仓尚无 `useInterval`），属独立改动，已登记。
 */
export function useTransferStatus() {
  const live = usePageLiveness();
  return useQuery({
    queryKey: TRANSFER_STATUS_KEY,
    queryFn: fetchTransferStatus,
    retry: RETRY_STRATEGY,
    refetchInterval: live ? REFRESH_INTERVAL_MS : false,
    refetchOnWindowFocus: false,
    staleTime: REFRESH_INTERVAL_MS,
  });
}

/** 设置（F-FT-59）：只在挂载时取一次（慢变量，改由保存动作失效后回源）。 */
export function useTransferSettings() {
  return useQuery({
    queryKey: TRANSFER_SETTINGS_KEY,
    queryFn: loadTransferSettings,
    retry: RETRY_STRATEGY,
    refetchOnWindowFocus: false,
    staleTime: Infinity,
  });
}

/**
 * 在线设备快照（F-FT-07/08）。
 *
 * 🔴 未启动时后端回**空数组**（不是错误）⇒ 界面渲染空态而非失败态。
 * 与 F-FT-08「空态永远带原因」配套：原因由 `useTransferStatus().data?.running` 给出。
 */
export function useTransferPeers() {
  const live = usePageLiveness();
  return useQuery({
    queryKey: TRANSFER_PEERS_KEY,
    queryFn: listTransferPeers,
    retry: RETRY_STRATEGY,
    refetchInterval: live ? REFRESH_INTERVAL_MS : false,
    refetchOnWindowFocus: false,
    staleTime: REFRESH_INTERVAL_MS,
  });
}

/** 启动 / 停止（两个 mutation 共享失效逻辑）。 */
export function useTransferServiceControl() {
  const queryClient = useQueryClient();

  const invalidateAll = (): void => {
    void queryClient.invalidateQueries({ queryKey: TRANSFER_STATUS_KEY });
    void queryClient.invalidateQueries({ queryKey: TRANSFER_PEERS_KEY });
  };

  const start = useMutation({
    mutationFn: startTransferService,
    onSettled: invalidateAll,
  });

  const stop = useMutation({
    mutationFn: stopTransferService,
    onSettled: invalidateAll,
  });

  return {
    start,
    stop,
    /** 任一动作在飞——两个按钮都该禁用（避免并发启停） */
    busy: start.isPending || stop.isPending,
  };
}

/**
 * 保存设置（F-FT-59：原子写）。
 *
 * 🔴 `onSuccess` 里**回源**而不是把入参当新缓存：命令边界对"非持久字段"（如配对开关）
 * 有自己的处置，磁盘契约的一致性由后端说了算，前端不该替它拍板。
 */
export function useSaveTransferSettings() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: (settings: TransferSettings) => saveTransferSettings(settings),
    onSuccess: () => {
      void queryClient.invalidateQueries({ queryKey: TRANSFER_SETTINGS_KEY });
    },
  });
}

/** 手动刷新设备列表（F-FT-08：**立即重列当前快照**，不触发任何主动探测）。 */
export function useRefreshTransferPeers() {
  const queryClient = useQueryClient();
  return () => void queryClient.invalidateQueries({ queryKey: TRANSFER_PEERS_KEY });
}

/**
 * 发送一个文件（F-FT-11/12）。
 *
 * 🔴 不用 `useMutation` 的 `onSuccess` 做"任务入队"之类的后续动作：
 * 片1 的任务视图尚未落地，发送结论由调用方按返回的 `outcome` / 抛出的原因码直接呈现。
 */
export function useSendTransferFile() {
  return useMutation({ mutationFn: sendTransferFile });
}

/**
 * 选择要发送的文件（F-FT-12：支持多选）。
 *
 * 🔴 **用户取消返回空数组而不是报错**：取消是正常路径，不是失败（R-31 的精神）。
 * 返回的路径是系统给的**原生绝对路径**，由后端直接读盘。
 */
export function usePickFilesToSend() {
  const { t } = useTranslation("fileTransfer");
  return useCallback(async (): Promise<string[]> => {
    const picked = await open({ multiple: true, title: t("send.pickTitle") });
    if (picked === null) return [];
    return Array.isArray(picked) ? picked : [picked];
  }, [t]);
}

export { asTransferError };
