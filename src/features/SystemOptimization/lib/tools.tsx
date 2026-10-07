/**
 * 系统优化模块 · 工具登记表（单一事实源）
 * ============================================================================
 * 复刻基线：NexBox `BuiltinToolsPage.tsx`（23 项）与 `OptimizePage.tsx`（14 项）
 * 的 `defaultTools` 数组。NexBox 把数组内联在页面里；Aegis 抽出到本文件，
 * 原因有二（**唯一的两处结构性适配，逻辑与取值逐项未改**）：
 *   1. 占位页需要按路由 id 反查工具名/状态，故需要一份可被页面外引用的表；
 *   2. 后续批次迁入子页时，状态（pending → ported / needsElevation）在登记表
 *      里单点维护，避免 37 处散落改状态。
 *
 * 🔴 路由前缀改写（用户 2026-10-07 裁决「改名并完整搬入」）：
 *   NexBox 顶层扁平路由 `/display-filter`、`/optimize/storage-clean`
 *   → Aegis 统一收敛到本模块命名空间 `/system-optimization/tools/<id>`，
 *   避免与 Aegis 既有模块路由（`/app-manager` = 软件管理、`/net-manager`…）撞车。
 *   `/app-manager`、`/network-optimizer`、`/disk-health` 等重叠项因此**完整保留**，
 *   不复用也不覆盖 Aegis 现有模块。
 *
 * `requiresElevation` 标记来源：**仅标注已在源码中实测到管理员判定者**
 * （`is_admin` / `isAdmin` 字面命中，grep 复核），未命中者留空，待各批次
 * 迁入时按实际后端实现补齐 —— 不做推测性标注。
 */
import {
  Palette,
  Crosshair,
  Layout,
  Cpu,
  Zap,
  Monitor,
  HardDrive,
  Volume2,
  MousePointerClick,
  Gauge,
  Braces,
  ShieldCheck,
  FlaskConical,
  DownloadCloud,
  Video,
  Boxes,
  Clock,
  ScanLine,
  ScrollText,
  MemoryStick,
  Trash2,
  List,
  Settings2,
  Network,
  MousePointer2,
  Download,
  Gamepad2,
  Database,
} from "lucide-react";
import type { ViewItem } from "../types/view-types";

// 复刻页面从本模块取 ViewItem 类型（两页沿用 NexBox 的 `ViewItem` 标注），
// 故在此 re-export，避免各页跨目录引 types。
export type { ViewItem };

/** 工具在本模块内的落地状态（占位页据此如实展示，不假装可用） */
export type ToolStatus =
  | "pending" // 批次 0：仅入口，子页未迁入
  | "ported" // 对应批次已迁入并接线
  | "blockedBySpec" // 🔴 规格明令不搬入（见下方逐条登记），保留卡片占位但**不提供功能**
  | "needsElevation"; // 子页已迁入，但操作需管理员权限（提权通道 W0-08 未落地 ⇒ 操作区禁用）

/**
 * 🔴 需澄清的管辖边界（2026-10-07）
 * ------------------------------------------------------------------
 * `Docs/功能规格/08-游戏管理.md:17`「明确不做」栏记载 **用户 2026-10-03 裁决**：
 * 「游戏辅助一律不移植」——自瞄（`crosshair.rs` / `crosshair_hold.rs`）、
 * 自动点击（`autoclicker.rs`）、游戏进程优化（`game_process_optimize.rs` /
 * `game_fps.rs` / `game_filter.rs`）**均不搬入**。
 *
 * 🔴 **该裁决是否及于本模块，尚未澄清**（不臆断）：
 *   - 该裁决的上下文是 08 册（游戏管理模块，基线 = NexBox 游戏模块换皮），记述的是
 *     NexBox 侧 11 个后端文件 + 6 个前端页面不搬入本工程；
 *   - 本模块（SystemOptimization）基线 = NexBox「内置工具 + 优化」两模块，
 *     其中落到上述清单的只有 3 项（crosshair / autoclicker / game-process-optimize）；
 *   - 清单里的三角洲族（delta_force.rs 等 + DeltaForcePage 等 3 页）**不在本模块的
 *     37 个工具内**（两个入口页未链接它们）。
 *
 * 处置：三项保留卡片并标 `blockedBySpec`，占位页明示原因，**不搬入、不删除**；
 * 是否推翻/延伸 2026-10-03 裁决由用户裁定（未裁定前不动代码、不改规格册）。
 */
export interface ToolEntry extends ViewItem {
  status: ToolStatus;
  /** 源码实测含管理员判定的工具；未实测者不标（勿推测） */
  requiresElevation?: boolean;
}

/** 全部工具 id → 路由的统一派生（单一事实源：path 由 id 派生，不手写） */
export function toolPath(id: string): string {
  return `/system-optimization/tools/${id}`;
}

const base = (t: Omit<ToolEntry, "path">): ToolEntry => ({
  ...t,
  path: toolPath(t.id),
});

/** 内置工具（复刻 BuiltinToolsPage 的 23 项，顺序与取值逐项一致） */
export const builtinTools: ToolEntry[] = [
  base({
    id: "display-filter",
    icon: Palette,
    titleKey: "sidebar.displayFilter",
    descriptionKey: "builtinTools.displayFilterDesc",
    color: "#98DDD0",
    status: "pending",
  }),
  base({
    id: "crosshair",
    icon: Crosshair,
    titleKey: "sidebar.crosshair",
    descriptionKey: "builtinTools.crosshairDesc",
    color: "#FF6B9D",
    // 🔴 规格 08 §明确不做：自瞄（crosshair.rs / crosshair_hold.rs）不搬入
    status: "blockedBySpec",
  }),
  base({
    id: "overlay-panel",
    icon: Layout,
    titleKey: "sidebar.overlayPanel",
    descriptionKey: "builtinTools.overlayPanelDesc",
    color: "#9B59B6",
    status: "pending",
  }),
  base({
    id: "gpu-rename",
    icon: Cpu,
    titleKey: "sidebar.gpuRename",
    descriptionKey: "builtinTools.gpuRenameDesc",
    color: "#F39C12",
    status: "pending",
    requiresElevation: true,
  }),
  base({
    id: "resolution-converter",
    icon: Monitor,
    titleKey: "sidebar.resolutionConverter",
    descriptionKey: "builtinTools.resolutionConverterDesc",
    color: "#4A90E2",
    status: "pending",
  }),
  base({
    id: "dlss-preset",
    icon: Zap,
    titleKey: "sidebar.dlssPreset",
    descriptionKey: "builtinTools.dlssPresetDesc",
    color: "#76B900",
    status: "pending",
  }),
  base({
    id: "disk-health",
    icon: HardDrive,
    titleKey: "sidebar.diskHealth",
    descriptionKey: "builtinTools.diskHealthDesc",
    color: "#00B4D8",
    status: "pending",
  }),
  base({
    id: "nvidia-driver",
    titleKey: "sidebar.nvidiaDriver",
    descriptionKey: "builtinTools.nvidiaDriverDesc",
    icon: LogoNvidia,
    color: "#76B900",
    status: "pending",
  }),
  base({
    id: "audio-eq",
    icon: Volume2,
    titleKey: "sidebar.audioEq",
    descriptionKey: "builtinTools.audioEqDesc",
    color: "#E74C3C",
    status: "pending",
  }),
  // 🔴 NexBox 原文对「商店版构建」隐藏「NVIDIA 驱动下载」项（其商店政策 10.1.5）。
  //   Aegis 无 MSIX 商店版 ⇒ 该分支恒为 false，故本表**保留全部 23 项**，
  //   不搬运无意义的条件分支（差异登记见 README「与 NexBox 的差异」）。
  base({
    id: "nvidia-driver-download",
    icon: LogoNvidia,
    titleKey: "sidebar.nvidiaDriverDownload",
    descriptionKey: "builtinTools.nvidiaDriverDownloadDesc",
    color: "#76B900",
    status: "pending",
  }),
  base({
    id: "autoclicker",
    icon: MousePointerClick,
    titleKey: "sidebar.autoclicker",
    descriptionKey: "builtinTools.autoclickerDesc",
    color: "#FF8C00",
    // 🔴 规格 08 §明确不做：自动点击（autoclicker.rs）不搬入
    status: "blockedBySpec",
  }),
  base({
    id: "speedtest",
    icon: Gauge,
    titleKey: "sidebar.speedtest",
    descriptionKey: "builtinTools.speedtestDesc",
    color: "#00B4D8",
    status: "pending",
  }),
  base({
    id: "runtime-repair",
    icon: Braces,
    titleKey: "sidebar.runtimeRepair",
    descriptionKey: "builtinTools.runtimeRepairDesc",
    color: "#4A90E2",
    status: "pending",
  }),
  base({
    id: "vtx-virtualization",
    icon: ShieldCheck,
    titleKey: "sidebar.vtxVirtualization",
    descriptionKey: "builtinTools.vtxVirtualizationDesc",
    color: "#38A169",
    status: "pending",
    requiresElevation: true,
  }),
  base({
    id: "hidden-features",
    icon: FlaskConical,
    titleKey: "sidebar.hiddenFeatures",
    descriptionKey: "builtinTools.hiddenFeaturesDesc",
    color: "#E67E22",
    beta: true,
    status: "pending",
    requiresElevation: true,
  }),
  base({
    id: "context-menu",
    icon: MousePointer2,
    titleKey: "sidebar.contextMenu",
    descriptionKey: "builtinTools.contextMenuDesc",
    color: "#3B82F6",
    status: "pending",
  }),
  base({
    id: "download-accelerator",
    icon: DownloadCloud,
    titleKey: "sidebar.downloadAccelerator",
    descriptionKey: "builtinTools.downloadAcceleratorDesc",
    color: "#00C896",
    status: "pending",
  }),
  base({
    id: "nvidia-recording",
    icon: Video,
    titleKey: "sidebar.nvidiaRecording",
    descriptionKey: "builtinTools.nvidiaRecordingDesc",
    color: "#76B900",
    status: "pending",
  }),
  base({
    id: "vac-repair",
    icon: LogoCs,
    titleKey: "sidebar.vacRepair",
    descriptionKey: "builtinTools.vacRepairDesc",
    color: "#66C0F4",
    status: "pending",
    requiresElevation: true,
  }),
  base({
    id: "app-manager",
    icon: Boxes,
    titleKey: "sidebar.appManager",
    descriptionKey: "builtinTools.appManagerDesc",
    color: "#4A90E2",
    status: "pending",
  }),
  base({
    id: "time-sync",
    icon: Clock,
    titleKey: "sidebar.timeSync",
    descriptionKey: "builtinTools.timeSyncDesc",
    color: "#00B4D8",
    status: "pending",
    requiresElevation: true,
  }),
  base({
    id: "screen-test",
    icon: ScanLine,
    titleKey: "sidebar.screenTest",
    descriptionKey: "builtinTools.screenTestDesc",
    color: "#38B2AC",
    status: "pending",
  }),
  base({
    id: "bsod-log",
    icon: ScrollText,
    titleKey: "sidebar.bsodLog",
    descriptionKey: "builtinTools.bsodLogDesc",
    color: "#EF4444",
    beta: true,
    status: "pending",
  }),
];

/** 系统优化（复刻 OptimizePage 的 14 项，顺序与取值逐项一致） */
export const optimizeTools: ToolEntry[] = [
  base({
    id: "storage-clean",
    icon: HardDrive,
    titleKey: "storageClean.title",
    descriptionKey: "storageClean.description",
    color: "#3182CE",
    status: "pending",
  }),
  base({
    id: "memory-cleanup",
    icon: MemoryStick,
    titleKey: "optimization.memoryCleanup.title",
    descriptionKey: "optimization.memoryCleanup.description",
    color: "#38A169",
    status: "pending",
  }),
  base({
    id: "ace-optimize",
    icon: Gauge,
    titleKey: "optimization.aceOptimize.title",
    descriptionKey: "optimization.aceOptimize.description",
    color: "#DD6B20",
    status: "pending",
  }),
  base({
    id: "memory-limit",
    icon: Cpu,
    titleKey: "optimization.memoryLimit.title",
    descriptionKey: "optimization.memoryLimit.description",
    color: "#FF6B9D",
    status: "pending",
    requiresElevation: true,
  }),
  base({
    id: "shader-cache",
    icon: Trash2,
    titleKey: "shaderCache.title",
    descriptionKey: "builtinTools.shaderCacheDesc",
    color: "#EF4444",
    status: "pending",
  }),
  base({
    id: "power-management",
    icon: Zap,
    titleKey: "optimization.powerManagement.title",
    descriptionKey: "optimization.powerManagement.description",
    color: "#F6AD55",
    status: "pending",
    requiresElevation: true,
  }),
  base({
    id: "startup-manager",
    icon: List,
    titleKey: "optimization.startupManager.title",
    descriptionKey: "optimization.startupManager.description",
    color: "#805AD5",
    status: "pending",
    requiresElevation: true,
  }),
  base({
    id: "system-optimizer",
    icon: Settings2,
    titleKey: "systemOptimizer.pageTitle",
    descriptionKey: "systemOptimizer.pageDesc",
    color: "#667EEA",
    status: "pending",
    requiresElevation: true,
  }),
  base({
    id: "network-optimizer",
    icon: Network,
    titleKey: "networkOptimize.pageTitle",
    descriptionKey: "networkOptimize.pageDesc",
    color: "#38A169",
    status: "pending",
    requiresElevation: true,
  }),
  base({
    id: "peripheral-optimize",
    icon: MousePointer2,
    titleKey: "peripheralOptimize.pageTitle",
    descriptionKey: "peripheralOptimize.pageDesc",
    color: "#E53E3E",
    status: "pending",
    requiresElevation: true,
  }),
  base({
    id: "windows-update",
    icon: Download,
    titleKey: "windowsUpdate.pageTitle",
    descriptionKey: "windowsUpdate.pageDesc",
    color: "#E53E3E",
    status: "pending",
  }),
  base({
    id: "cpu-scheduler",
    icon: Cpu,
    titleKey: "optimization.cpuScheduler.title",
    descriptionKey: "optimization.cpuScheduler.description",
    color: "#3182CE",
    beta: true,
    status: "pending",
    requiresElevation: true,
  }),
  base({
    id: "game-process-optimize",
    icon: Gamepad2,
    titleKey: "optimization.gameProcessOptimize.title",
    descriptionKey: "optimization.gameProcessOptimize.description",
    color: "#8B5CF6",
    // 🔴 规格 08 §明确不做：游戏进程优化（game_process_optimize.rs / game_fps.rs /
    //   game_filter.rs）不搬入
    status: "blockedBySpec",
  }),
  base({
    id: "pso-cache",
    icon: Database,
    titleKey: "psoCache.title",
    descriptionKey: "builtinTools.psoCacheDesc",
    color: "#0EA5E9",
    status: "pending",
  }),
];

/** 全部工具（占位页按 id 反查用） */
export const allTools: ToolEntry[] = [...builtinTools, ...optimizeTools];

export function findTool(id: string): ToolEntry | undefined {
  return allTools.find((tool) => tool.id === id);
}

// ── 品牌图标（逐字复刻 BuiltinToolsPage.tsx:36-43 的两个 Logo 组件）────────────
import nvidiaLogoImg from "../assets/nvidia.png";
import csLogoImg from "../assets/cs-logo.png";

function LogoNvidia({ size = 24 }: { size?: number }) {
  return (
    <img
      src={nvidiaLogoImg}
      width={size}
      height={size}
      style={{ objectFit: "contain" }}
      alt="NVIDIA"
    />
  );
}

function LogoCs({ size = 24 }: { size?: number }) {
  return (
    <img src={csLogoImg} width={size} height={size} style={{ objectFit: "contain" }} alt="CS:GO" />
  );
}
