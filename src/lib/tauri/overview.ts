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
  StatCard,
  LiveSample,
  MonitorStatus,
  NetEndpoints,
  SnapshotDto,
  SoftwareList,
  SensorTable,
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

/** F-02 + F-03 静态上行：硬件详情卡 + 系统信息两面板（按 `kind` 分流；快照与报告同源） */
export async function fetchStaticInfo(): Promise<HardwareDetailDto> {
  return invoke<HardwareDetailDto>("overview_static_info", OVERVIEW, "overviewStaticInfo", null);
}

/** F-09 读快照（缺失/损坏/超龄返回 null） */
export async function readSnapshot(): Promise<SnapshotDto | null> {
  return invoke<SnapshotDto | null>(
    "overview_snapshot_read",
    OVERVIEW,
    "overviewSnapshotRead",
    null,
  );
}

/**
 * F-09 写快照。
 *
 * @param cards F-01 四卡的上一拍读数 —— 供下次冷启动秒显（启动首屏不能是加载态）。
 *   可省略：此时后端写空数组，下次启动四卡自然缺席（而非读到空壳）。
 */
export async function writeSnapshot(
  payload: HardwareDetailDto,
  cards?: readonly StatCard[],
): Promise<void> {
  return invoke<void>("overview_snapshot_write", OVERVIEW, "overviewSnapshotWrite", {
    ...payload,
    cards: cards ?? [],
  });
}

/** F-07 已安装软件清单（慢变量，不进 2 秒节拍） */
export async function fetchSoftwareList(): Promise<SoftwareList> {
  return invoke<SoftwareList>("overview_software_list", OVERVIEW, "overviewSoftwareList", null);
}

/** F-11 全量传感器逐条读数（报告"传感器快照"节；空数组 = 本路未读到，整节省略） */
export async function fetchSensorTable(): Promise<SensorTable> {
  return invoke<SensorTable>("overview_sensor_table", OVERVIEW, "overviewSensorTable", null);
}

/**
 * F-11 写出报告文件（Markdown 文本由前端组装，后端只做原子落盘）。
 *
 * 🔴 只在用户于另存为对话框里选定路径后才被调用；用户取消 = 本函数不执行 = 零副作用。
 */
export async function writeReportFile(path: string, content: string): Promise<void> {
  return invoke<void>("overview_report_write", OVERVIEW, "overviewReportWrite", {
    path,
    content,
  });
}
