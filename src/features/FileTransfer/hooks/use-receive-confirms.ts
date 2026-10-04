// 接收确认的查询与裁决（M-05 片1；F-FT-14 / R-8）。
//
// 🔴 轮询而非事件：与设备列表同口径（本批不引入事件通道）。
//    弹窗的时延要求本就不高（超时 30 秒），2 秒节拍足够；
//    代价是倒计时按 2 秒跳——已登记"1 秒节拍封装"的欠账。

import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { REFRESH_INTERVAL_MS, usePageLiveness } from "@/lib/timing/use-tick";
import { BackendUnavailableError } from "@/lib/tauri/invoke";
import { confirmReceive, listPendingConfirms } from "@/lib/tauri/transfer";
import type { PerTransferPolicy } from "@/types/transfer";

export const RECEIVE_CONFIRMS_KEY = ["transfer", "pendingConfirms"] as const;

/**
 * 轮询待决的接收确认。
 *
 * 🔴 服务未运行时后端返回**空表**而不是报错（与设备列表同口径）——
 *    「没有待决」与「服务没跑」对界面都是"没有弹窗"，不该让调用方处理两种空。
 */
export function useReceiveConfirms() {
  const live = usePageLiveness();
  return useQuery({
    queryKey: RECEIVE_CONFIRMS_KEY,
    queryFn: listPendingConfirms,
    enabled: live,
    refetchInterval: live ? REFRESH_INTERVAL_MS : false,
    // 🔴 不重试"后端不可用"（不在 Tauri 运行时内）——重试只会把同一句话刷四遍
    retry: (count, error) => !(error instanceof BackendUnavailableError) && count < 1,
  });
}

/**
 * 提交一次裁决。
 *
 * 🔴 失败**不是**可以忽略的：`CONFIRM_GONE` 表示"你点慢了，那条已按拒绝处理"，
 *    界面必须如实说，而不是显示"已接收"。
 */
export function useResolveReceiveConfirm() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: (input: { id: string; accept: boolean; policy: PerTransferPolicy | null }) =>
      confirmReceive(input.id, input.accept, input.policy ?? undefined),
    onSettled: () => {
      // 成功或失败都重取：成功要让这条从列表消失，失败要让界面看到真实待决
      void queryClient.invalidateQueries({ queryKey: RECEIVE_CONFIRMS_KEY });
    },
  });
}
