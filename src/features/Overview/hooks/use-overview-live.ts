// 实时概览取数（M-01 片1：F-01 / F-08 / F-10 / F-12）。
//
// 🔴 节拍只由 `refetchInterval` 承载，且其值随 `live` 在 `2000 | false` 间切换：
// 切到 false 时 TanStack Query 会**清掉**定时器，不留任何常驻 tick（R-19「不存在全局常驻定时器」）。
// 组件卸载（离开本页）同样自动清表——不需要额外的"停表"代码路径。

import { useEffect, useRef } from "react";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import type { HardwareDetailDto, LiveSample } from "@/types/overview";
import { BackendUnavailableError } from "@/lib/tauri/invoke";
// 🔴 2026-10-07：本文件的降级路径原本只用 console.warn 留痕 ⇒ 不进八字段 JSONL，
//   用户报障时日志里查不到。改走 `log()`（它内部已镜像到 console，无需再手写）。
import { log } from "@/lib/logging";
import { REFRESH_INTERVAL_MS, usePageLiveness } from "@/lib/timing/use-tick";
import {
  fetchLiveSample,
  fetchTopProcesses,
  fetchNetEndpoints,
  fetchSoftwareList,
  fetchStaticInfo,
  readSnapshot,
  writeSnapshot,
} from "@/lib/tauri/overview";

export const OVERVIEW_LIVE_KEY = ["overview", "live"] as const;
export const OVERVIEW_PROCESSES_KEY = ["overview", "processes"] as const;
export const OVERVIEW_ENDPOINTS_KEY = ["overview", "endpoints"] as const;
export const OVERVIEW_SOFTWARE_KEY = ["overview", "software"] as const;
/** F-02/F-03 静态上行（慢变量）：详情区、快照回写与 F-11 导出报告共用这一份缓存 */
export const OVERVIEW_STATIC_KEY = ["overview", "static"] as const;

/**
 * 本域统一的 retry 策略（M-01 收尾）。
 *
 * 🔴 `BackendUnavailableError` 是**永久性错误**：进程不在 Tauri 运行时内，重试再多次也必然失败。
 * 而 TanStack Query 的默认 `retry: 3` 会让用户盯着骨架屏等完 3 次重试才看到错误信息。
 * 故命中该错误一律`retry: false`，让失败立即可见（这与「禁止假数据静默兜底」同一取向：
 * 失败要**明确显示**，不能拖成一段空白等待）。
 *
 * 其余错误（网络抖动、后端瞬时失败）维持默认重试 —— 那些重试是有意义的。
 */
const RETRY_STRATEGY = (failureCount: number, error: unknown): boolean => {
  if (error instanceof BackendUnavailableError) return false;
  return failureCount < 3;
};

export function useOverviewLive() {
  const live = usePageLiveness();
  const queryClient = useQueryClient();
  const query = useQuery({
    queryKey: OVERVIEW_LIVE_KEY,
    queryFn: fetchLiveSample,
    retry: RETRY_STRATEGY,
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

/** F-04 实时进程榜（2 秒心跳同步更新） */
export function useTopProcesses() {
  const live = usePageLiveness();
  return useQuery({
    queryKey: OVERVIEW_PROCESSES_KEY,
    queryFn: fetchTopProcesses,
    retry: RETRY_STRATEGY,
    refetchInterval: live ? REFRESH_INTERVAL_MS : false,
    refetchOnWindowFocus: false,
    staleTime: REFRESH_INTERVAL_MS,
  });
}

/** F-05 网络端点面板（2 秒心跳同步更新） */
export function useNetEndpoints() {
  const live = usePageLiveness();
  return useQuery({
    queryKey: OVERVIEW_ENDPOINTS_KEY,
    queryFn: fetchNetEndpoints,
    retry: RETRY_STRATEGY,
    refetchInterval: live ? REFRESH_INTERVAL_MS : false,
    refetchOnWindowFocus: false,
    staleTime: REFRESH_INTERVAL_MS,
  });
}

/** F-07 已安装软件清单（慢变量，不进 2 秒节拍，只在挂载时取一次） */
export function useSoftwareList() {
  return useQuery({
    queryKey: OVERVIEW_SOFTWARE_KEY,
    queryFn: fetchSoftwareList,
    retry: RETRY_STRATEGY,
    refetchOnWindowFocus: false,
    staleTime: Infinity, // 慢变量：除非手动刷新，否则不重新取
  });
}

/**
 * F-02 + F-03 静态上行（硬件详情卡 / 系统信息卡）。
 *
 * 🔴 为什么先读快照（2026-10-04 收尾补齐）：`overview_static_info` 要跑 WMI 硬件枚举与
 * SMART 读取，**冷启动耗时以秒计**；而这批是慢变量（换驱动 / 插硬盘才会变）。后端
 * `overview_snapshot_read` 已提供带指纹校验 + 30 天超龄淘汰的缓存（`snapshot.rs`），
 * 却一直没被前端调用 —— 于是每次进页面都白等一遍全量采集。
 *
 * 🔴 命中快照后**仍要回源一次**（`revalidateAfterMs` 的存在理由）：若命中就直接返回，
 * 这条query 的 `dataUpdatedAt` 永远是快照的落盘时刻，`staleTime` 到点后refetch 会
 * 重复走「读快照 → 又拿到同一份 → 再也不采」的死循环，硬件数据将永久冻结。
 * 故命中快照时先给界面立刻可读的内容，再排一次**无条件**的后台回源把数据刷新到当下。
 */
export function useStaticInfo() {
  const queryClient = useQueryClient();
  const query = useQuery({
    queryKey: OVERVIEW_STATIC_KEY,
    queryFn: fetchStaticInfo,
    retry: RETRY_STRATEGY,
    refetchOnWindowFocus: false,
    // 慢变量：默认不按节拍取，仅靠手动刷新与下面的快照快路径驱动
    staleTime: STATIC_STALE_MS,
  });

  // 采集成功即回写快照，供下次冷启动走快路径。
  // 🔴 后端 `write_snapshot` 自带指纹校验与 30 天超龄淘汰，前端无需重复判断新鲜度。
  // 写失败只记日志：快照是优化手段，写不进去最多下次多等一次全量采集，不该影响界面。
  const payload = query.data;
  useEffect(() => {
    if (payload === undefined) return;
    // 🔴 四卡随详情一起落盘：`queryClient` 是唯一能同时拿到两者的入口
    // （静态 hook 本身看不到实时卡的最新一拍）。
    void writeSnapshot(
      payload,
      queryClient.getQueryData<LiveSample>(OVERVIEW_LIVE_KEY)?.cards,
    ).catch((error: unknown) => {
      // 🔴 快照是优化手段，写失败不阻断界面 —— 但**必须留痕**：
      // 静默 catch 会让「快照一直没生效 / 一直没更新」这类问题完全无从排查。
      log({
        level: "warn",
        module: "Overview",
        action: "snapshot-write",
        result: "Failed",
        exception: error instanceof Error ? error.message : String(error),
      });
    });
  }, [payload, queryClient]);

  return query;
}

/**
 * 冷启动快路径：把快照内容**预置**进详情区缓存（`initialData` 语义等价物），
 * 使详情区在 `overview_static_info` 尚未返回时就能出内容。
 *
 * 与 `useStaticInfo` 的关系：两者共用 `OVERVIEW_STATIC_KEY`，因此**同一条 query**——
 * 这里写入的快照就是详情区立刻可读的那份；`useStaticInfo` 随后正常回源并覆盖它。
 * 页面已在后台采集耗时很短，故本hook 不显示任何 loading 态，只在快照与采集都还没有时留空。
 */
export function useSnapshotWarmup() {
  const queryClient = useQueryClient();

  useEffect(() => {
    const hasStatic =
      queryClient.getQueryData<HardwareDetailDto>(OVERVIEW_STATIC_KEY) !== undefined;
    const hasLive = queryClient.getQueryData<LiveSample>(OVERVIEW_LIVE_KEY) !== undefined;
    if (hasStatic && hasLive) return; // 同一会话内回访，缓存已有，不必预热
    let cancelled = false;
    void (async () => {
      try {
        const snapshot = await readSnapshot();
        if (cancelled || snapshot === null) return; // 无快照 = 首装 / 超龄 / 指纹不符，走正常采集

        // 🔴 四卡：预置**上一拍读数**（后端 `Snapshot.cards`），让启动首屏立刻有内容
        // ——本页是程序启动后第一个看见的页面，不能是加载态（用户 2026-10-04 裁决）。
        // 空数组则不预置：四卡自然缺席，而不是画出空壳。
        if (!hasLive && snapshot.cards.length > 0) {
          // 🔴 快照只存四卡（`Snapshot.cards`），故其余字段按"尚未知"占位：
          //   · header：空串→ 页面用 `joinHeader` 自动省略空项，不会显示成 " · " （F-12）；
          //     `uptimeSecs: null` 而非 `0` —— 0 会被 `formatUptime` 折成「不足 1 分钟」，
          //     凭空显示成「刚开机」，而真实开机时长可能数天（R-08；审查严重 3）。
          //   · monitor：`exeMissing` + 有理由 → ChannelStatus 如实显示"通道未就绪"，
          //     真实状态由紧随其后的 invalidate 回源立即覆盖（不在此伪造 ready）。
          //   · inFlight：false —— 🔴 不能填 true：若回源**失败**（真错误），
          //     按钮会因 `busy = isFetching || inFlight` 而永久禁用，用户只能重启程序。
          //     取false 让按钮在回源期间仍可点（重复点击由 TanStack 去重）。
          const placeholder: LiveSample = {
            correlationId: "snapshot-warmup",
            header: { computerName: "", osName: "", uptimeSecs: null },
            cards: snapshot.cards,
            monitor: {
              state: "exeMissing",
              reason: null,
              sidecarPid: null,
              driverInstalled: false,
              driverVersion: null,
            },
            inFlight: false,
            diskHealth: [],
          };
          queryClient.setQueryData<LiveSample>(OVERVIEW_LIVE_KEY, placeholder);
        }

        if (!hasStatic) {
          queryClient.setQueryData<HardwareDetailDto>(OVERVIEW_STATIC_KEY, snapshot.payload);
        }
        // 🔴 关键：预置快照后**必须**回源一次，否则数据被快照永久冻结。
        // invalidate 会重跑 queryFn，拿到的是当下真实值。
        void queryClient.invalidateQueries({ queryKey: OVERVIEW_STATIC_KEY });
        void queryClient.invalidateQueries({ queryKey: OVERVIEW_LIVE_KEY });
      } catch (error) {
        // 🔴 快照读失败（含文件损坏）不是错误路径，仍退化为「无快照」走正常采集；
        // 但必须留痕，否则「快照一直没命中」无从排查。
        log({
          level: "warn",
          module: "Overview",
          action: "snapshot-read",
          result: "Failed",
          exception: error instanceof Error ? error.message : String(error),
        });
      }
    })();
    return () => {
      cancelled = true;
    };
  }, [queryClient]);
}

/** 静态上行视为"足够新"的时长：只作为回源节拍，不作为有效性判据（有效性由后端快照指纹管） */
const STATIC_STALE_MS = 5 * 60 * 1000;

/** F-10 手动全量刷新：片1 覆盖面 = 实时三路（硬件/系统/软件的慢变量随片2~片4 接入同一入口） */
export function useManualRefresh() {
  const queryClient = useQueryClient();
  return () => void queryClient.invalidateQueries({ queryKey: ["overview"] });
}
