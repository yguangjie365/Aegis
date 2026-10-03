// 实时概览取数（M-01 片1：F-01 / F-08 / F-10 / F-12）。
//
// 🔴 节拍只由 `refetchInterval` 承载，且其值随 `live` 在 `2000 | false` 间切换：
// 切到 false 时 TanStack Query 会**清掉**定时器，不留任何常驻 tick（R-19「不存在全局常驻定时器」）。
// 组件卸载（离开本页）同样自动清表——不需要额外的"停表"代码路径。

import { useEffect, useRef } from "react";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import { REFRESH_INTERVAL_MS, usePageLiveness } from "@/lib/timing/use-tick";
import { fetchLiveSample } from "@/lib/tauri/overview";

export const OVERVIEW_LIVE_KEY = ["overview", "live"] as const;

export function useOverviewLive() {
  const live = usePageLiveness();
  const queryClient = useQueryClient();
  const query = useQuery({
    queryKey: OVERVIEW_LIVE_KEY,
    queryFn: fetchLiveSample,
    refetchInterval: live ? REFRESH_INTERVAL_MS : false,
    // 数据本身 2 秒就换代，没有必要再判新鲜度；关掉焦点重取避免与节拍重复打点
    refetchOnWindowFocus: false,
    staleTime: REFRESH_INTERVAL_MS,
  });

  // F-08 预期结果：暂停后再回页面「直接恢复并取新值」——节拍只负责下一拍，
  // 回到前台那一瞬间要主动补一次，否则用户会先盯着最长 2 秒的旧值。
  const wasLive = useRef(live);
  useEffect(() => {
    const resumed = live && !wasLive.current;
    wasLive.current = live;
    if (resumed) void queryClient.invalidateQueries({ queryKey: OVERVIEW_LIVE_KEY });
  }, [live, queryClient]);

  return { live, ...query };
}

/** F-10 手动全量刷新：片1 覆盖面 = 实时三路（硬件/系统/软件的慢变量随片2~片4 接入同一入口） */
export function useManualRefresh() {
  const queryClient = useQueryClient();
  return () => void queryClient.invalidateQueries({ queryKey: ["overview"] });
}
