# Overview —本机概览（M-01）

## 一、参照关系（读代码前先看这段，避免找错母本）

| 参照工程 | 角色 | 具体引用点 |
|---|---|---|
| **SystemToolkit** | **视觉与布局母本（本模块唯一母本）** | `src/SystemToolkit.Modules.Overview/OverviewView.xaml`（546 行）。分区骨架、`UniformGrid Columns=4/2` 栅格、键值表 `Width="120"` + `*` 两列、每区「中文标题 + 全大写英文副标」。另复用其 `OverviewReportBuilder.cs` 的报告章节顺序（③ 思路复用，文本组装在本仓前端，原因见 `lib/export-report.ts` 头注）。 |
| **NexBox** | **逻辑与组件参照（仅限同功能文件）** | `src/pages/HardwarePage.tsx`（1116 行）、`HardwareModelCard.tsx`、`HardwareDetailModal.tsx` —— 提供「硬件信息怎么采集、怎么分卡、卡里放哪些键」的领域口径。⚠️ **其「卡片 + 点击弹 Modal」形态未被采用**（用户2026-10-04 裁决取 SystemToolkit 的分区平铺），原因是本仓 `DetailCardDto` 契约与已备好的 `KeyValueCard`（平铺专用）都是按分区卡设计的。NexBox 页头的 6 个按钮（远程监控/装驱动/复制信息/清数据/导出）**未移植**，本模块页头保持「LIVE + 导出报告 + 立即刷新」。 |

> 🔴 NexBox **没有** overview 模块，不要在 `src/features/` 里找。本模块的对应物是 `HardwarePage.tsx`。

## 二、功能分区（对齐母本六分区，2026-10-04 全部接通）

| # | 分区 | 规格号 | 后端命令 | 前端 |
|---|---|---|---|---|
| ① | 实时概览四卡 | F-01 | `overview_live_sample` | `StatCard` × n（`auto-fit` 网格） |
| ② | 传感器通道状态 | F-08 | 同上（`monitor` 段） | `ChannelStatus` |
| ③ | **详细信息**（硬件详情，两列） | F-02 | `overview_static_info` | `KeyValueCard` × n（`DetailGrid`） |
| ④ | **系统信息**（两列） | F-03 | 同上（按 `kind` 分流） | `KeyValueCard` × n |
| ⑤ | 实时占用进程（CPU/内存双榜） | F-04 | `overview_top_processes` | `DualProcessTables` |
| ⑥ | 网络端点 / 磁盘健康 / 已安装软件 | F-05/06/07 | `overview_net_endpoints` / 同live / `overview_software_list` | `NetEndpointsPanel` / `DiskHealthPanel` / `SoftwareListPanel` |

另有两个非分区能力：**F-09 快照**（`read/write_snapshot`）与**F-11 导出报告**。

## 三、两条数据流（易混，务必分清）

| | 实时组 | 静态组 |
|---|---|---|
| 分区 | ①②⑤⑥网络端点 | ③④ + F-09 快照 + F-11 报告 |
| hook | `useOverviewLive` / `useTopProcesses` / `useNetEndpoints` | `useStaticInfo`（+ `useSnapshotWarmup` 快路径） |
| 节奏 | 2 秒心跳（`REFRESH_INTERVAL_MS`），随 `live` 切`false` 即停表 | 慢变量，不进节拍 |
| 缓存键 | `OVERVIEW_LIVE_KEY` 等 | `OVERVIEW_STATIC_KEY`（**详情区与 F-11 导出报告共用同一份**） |

**F-02 与 F-03 共用一次上行**，靠 `splitStaticCards()`（`lib/export-report.ts` 导出）按 `SYSTEM_INFO_KINDS`（`os` / `userRegion`）分流。界面与报告**共用这一个函数**，避免两处各写一套分流而漂移。

## 四、关键设计决策

- **F-09 快照冷启动快路径**：`useSnapshotWarmup` 挂载时读快照（带指纹校验 + 30 天超龄淘汰，后端 `snapshot.rs`）并预置进缓存，让详情区不等数秒的 WMI 采集；随后 `useStaticInfo` 回源覆盖。采集成功即 `writeSnapshot` 回写。
  🔴 **命中快照后必须回源一次**，否则 `dataUpdatedAt` 永远停在快照落盘时刻，数据被永久冻结。
- **失败要立即可见**：本域统一 `RETRY_STRATEGY` —— 命中 `BackendUnavailableError`（浏览器预览模式，永久性错误）不重试，其余错误维持默认重试 3 次。否则用户要盯骨架屏等完3 次重试才看到必然失败的错误。

## 五、约定

- 🔴 **业务代码不得裸调 `tauriInvoke`**，一律经 `@/lib/tauri/overview`（本仓「前端不触引擎」白名单侧）。
- 🔴 **文案单一来源**：`KvRowDto.key` 只带 i18n 键，中文只存在于 `src/locales/*/overview.json`；`DETAIL_KIND_I18N_PREFIX` 是 kind → 文案前缀的**唯一**映射表，界面与导出报告共用。
- 🔴 **禁假数据兜底**：后端未接入时显式报错（`BackendUnavailableError`），空态/占位不得伪装成后端返回。
- 测试：`lib/format.test.ts` + `lib/export-report.test.ts`（21 例），改动后须保持全绿。