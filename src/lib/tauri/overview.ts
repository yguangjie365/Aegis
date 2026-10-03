// overview 域前端门面（M-01 片1）：四条只读采集命令。
// 🔴 业务代码不得裸调 tauriInvoke —— 一律经本文件（01 专题「前端不触引擎」白名单侧）。
//
// 命令面契约（services/src/overview/commands.rs）：入参只有 CmdEnvelope（由 invoke.ts 自动包），
// 采集失败**不 reject** 而是返回带 reason 的降级载荷（R-10/R-11 分区独立容错）；
// 故此处的 `Promise` 只在「入参级错误」或「后端未接入」时 reject，界面不得把它当数据缺失渲染。

import { invoke } from "./invoke";
import type { LogModule } from "@/lib/logging";
import type {
  HardwareDetailDto,
  LiveSample,
  MonitorStatus,
  NetEndpoints,
  SnapshotDto,
  TopProcesses,
} from "@/types/overview";

const OVERVIEW: LogModule = "Overview";

export async function fetchLiveSample(): Promise<LiveSample> {
  return invoke<LiveSample>("overview_live_sample", OVERVIEW, "overviewLiveSample", null);
}

export async function fetchTopProcesses(): Promise<TopProcesses> {
  return invoke<TopProcesses>("overview_top_processes", OVERVIEW, "overviewTopProcesses", null);
}

export async function fetchNetEndpoints(): Promise<NetEndpoints> {
  return invoke<NetEndpoints>("overview_net_endpoints", OVERVIEW, "overviewNetEndpoints", null);
}

export async function fetchMonitorStatus(): Promise<MonitorStatus> {
  return invoke<MonitorStatus>("overview_monitor_status", OVERVIEW, "overviewMonitorStatus", null);
}

/** F-02 静态硬件详情（片2-b/c/d 产出，含系统信息两面板） */
export async function fetchStaticInfo(): Promise<HardwareDetailDto> {
  return invoke<HardwareDetailDto>(
    "overview_static_info",
    OVERVIEW,
    "overviewStaticInfo",
    null
  );
}

/** F-09 读快照（缺失/损坏/超龄返回 null） */
export async function readSnapshot(): Promise<SnapshotDto | null> {
  return invoke<SnapshotDto | null>(
    "overview_snapshot_read",
    OVERVIEW,
    "overviewSnapshotRead",
    null
  );
}

/** F-09 写快照 */
export async function writeSnapshot(payload: HardwareDetailDto): Promise<void> {
  return invoke<void>(
    "overview_snapshot_write",
    OVERVIEW,
    "overviewSnapshotWrite",
    payload
  );
}
