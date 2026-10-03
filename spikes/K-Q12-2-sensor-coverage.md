# K-Q12-2 — 本机传感器覆盖三态实测（空面板文案的依据）

- 日期：2026-10-03｜执行：Qoder（M-01 片1 附）
- 条件：🔴 **未安装 PawnIO**（`HKLM\...\Uninstall\PawnIO` 实测"系统找不到指定的注册表项或值"）+ **非管理员**（普通用户令牌）
  → 这正是 Aegis 的默认交付形态（Q-12=B 但零分发驱动，见 `DEPENDENCIES.md §四`）。
- 取数方式：`Aegis/monitor/bin/Release/net48/AegisMonitor.exe` stdin 写 `{"cmd":"read"}`，解析 stdout 行 JSON。
- 体量：单次 64 条传感器 / ~10.2 KB；首次 548ms，稳态 ~12ms（数字见 `K-Q12-1-monitor-build.md §四`）。

## 一、三态表（本机硬件 = i7-12700H + RTX 3060 Laptop + 32G）

| 硬件类型 | 出现的传感器类型 | 态 | 概览用途 |
|---|---|---|---|
| `Cpu`（12th Gen Intel Core i7-12700H） | `Load` ×22（含 **`CPU Total`**、`CPU Core Max`、逐核逐线程）、`Power` ×4 | 🟢 占用可读 | F-01 CPU 卡大数字取 `Load/CPU Total`；R-07 回退链的第二级 |
| `Cpu` | **`Temperature` 0 条** | 🔴 本机不可读 | CPU 卡温度徽章 → 「未检测」；印证规格 §七 **L-01（K-001：部分 Win11 平台 CPU 温度不可用）** |
| `Memory`（`Total Memory`） | `Data` ×2（`Memory Used` 11.84 GB / `Memory Available` 19.96 GB）、`Load` ×1（`Memory` 37.24%） | 🟢 可读 | F-01 RAM 卡：占用大数字取 `Load/Memory`，容量取 `Data` |
| `Memory`（`Virtual Memory`） | 同上四类 | 🟢 可读 | 🔴 **不得用这张**——虚拟内存不等于物理内存容量，聚合必须按 `hardware == "Total Memory"` 收窄 |
| `GpuNvidia`（RTX 3060 Laptop） | `Temperature` ×2（**`GPU Core` 56℃**、`GPU Hot Spot` 59.75℃）、`Load` ×19（**`GPU Core` 1%**、`GPU Memory` 18.1%、`D3D 3D`、`GPU Bus`…）、`Clock` ×2、`Power`、`Voltage`、`Throughput` ×2、`SmallData` ×5 | 🟢 可读（LHM 经 NVAPI/NVML，不依赖内核驱动） | F-01 GPU 卡：`Load/GPU Core` + `Temperature/GPU Core`；F-02 显存/频率卡同批可用 |
| `GpuIntel` / `GpuAmd` | 未出现 | ⚪ 本机无该硬件 | 混合显卡口径沿用母本：存在 `GpuNvidia` 时跳过 `GpuIntel`（`NexBox/sensor.rs:336`） |
| `Motherboard` / `SuperIO` | **未出现**（`IsMotherboardEnabled=true` 仍无输出） | 🔴 非管理员不可读 | 主板温度/风扇徽章 → 「未检测」 |
| `HardDisk` / `Hdd` / `Ssd` | **未出现**（`IsStorageEnabled=true` 仍无输出） | 🔴 非管理员不可读 | F-06 磁盘健康面板走**空面板 + 原因文案**（规格要求"空态必写原因"）：`未检测到存储设备（需管理员权限）` |
| `Fan` 类传感器 | 全硬件 0 条 | 🔴 本机不可读 | 风扇相关字段一律「未检测」，不显示 0 |

## 二、由此固化的聚合规则（落 `services/src/overview/sensor_bridge.rs`，单点实现）

1. **CPU 占用**：`hardwareType == "Cpu"` 且 `sensorType == "Load"` 且 `name ∈ {CPU Total, Total}`（母本同口径 `sensor.rs:257`）。
2. **CPU 温度**：`sensorType == Temperature` 且 `hardwareType ∈ {Cpu, SuperIO, Motherboard}`
   且 `name ∈ {Core (Tctl/Tdie), CPU Package, Tctl, Core, CPU, CPU Core, CPU Temperature}`，取首个命中（母本 `sensor.rs:288-299`）。
3. **GPU**：先按 `hardware`（设备名）分组；组内 `Load` 优先 `GPU Core`，回退 `D3D 3D`/`GPU`/`D3D Usage`/`Core`；
   `Temperature` 优先 `GPU Core`，回退 `GPU`/`Core`/`GPU Temperature`（母本 `sensor.rs:340-350`）。
4. **RAM**：`hardwareType == Memory` 且 **`hardware == "Total Memory"`**；占用取 `sensorType == Load`，容量取 `Memory Used` / `Memory Available`。
5. 任何字段取不到 → `None`，由 DTO 层渲染成「未检测」+ 原因，🔴 **禁止填 0 或留空**（R-08 状态诚实红线）。

## 三、与批准方案「无 PawnIO 降级矩阵」的对账

| 矩阵预期 | 本机实测 | 一致 |
|---|---|---|
| CPU 占用、RAM 占用、磁盘容量 ✅（sysinfo 侧） | sysinfo 三项俱在；sidecar 亦给到 CPU/RAM 占用（双源，取用优先级见下） | ✅ |
| **GPU 占用 ✅（sysinfo 侧）** | 🔴 **该格不成立**：`sysinfo` 0.30.13 无任何 GPU 采集面（`Components`/`Networks` 均非 GPU），**GPU 负载与温度只能来自 sidecar**（本机经 NVAPI/NVML，不需内核驱动）。无 sidecar 时显卡卡按 **R-07 原文"负载与温度皆无则不出现"** 整卡缺席（页面剩 3 卡，`auto-fit` 网格不留空槽），🔴 不是"降级为仅型号"；已按实测更正 `plans/grand-pine-perch.md §二` 降级矩阵 | ❌ 矩阵需更正 |
| CPU 温度 ❌「未检测」 | 0 条 CPU Temperature 传感器 | ✅ |
| SMART / 磁盘剩余寿命 ❌ | `HardDisk` 硬件整体缺席 | ✅ |
| GPU 温度/风扇（N 卡）✅ NVML | `GpuNvidia` 温度+负载俱在 | ✅ |
| F-06 存储健康 = 空面板 + 原因文案 | 无存储传感器 | ✅ |

> 🟡 风扇（Fan）在本机连 GPU 侧也没给出（RTX 3060 Laptop 常见无风扇 tachometer），故"风扇字段"按实测归入不可读；
> 若后续在其它机型上出现 `SensorType == Fan`，聚合规则 5 自动生效，无需改代码。

## 四、片1-c 落地后新增的取用优先级（双源同字段时）

同一字段 sidecar 与 sysinfo 都可能给（CPU 占用、RAM 占用），裁定口径落在 `badges.rs::pick_big_value`（R-07 单源）：

1. **占用类优先 sysinfo**：它每个 2s 节拍自更新，不依赖 sidecar 存活；
2. **温度/传感器负载只认 sidecar**：sysinfo 无温度面；
3. 两者都缺 → 大数字回退到温度顶位，再缺 → `null` 渲染成破折号，🔴 不填 0。

## 五、待补测（依赖片1-c 的桥状态机）

> 🟡 片1-c 已把该状态机落进 `services/src/overview/sensor_bridge.rs` 并由单测钉住（`force_exe_missing_for_tests()` 走"5s 快败"的结构性上界，测试里**不真起** sidecar）；下面两条是**真机**动作，尚未实测，不得记为已验证。

- 🔲 把 `AegisMonitor.exe` 改名后启动主程序：验证 **5s 快败 / 30s 冷却 / 连败 5 次禁用** 与四卡 L-08 降级（卡不消失、卡高不变）。
- 🔲 管理员启动同产物：确认 `HardDisk` / `Motherboard` 是否补齐，据此定 F-06 是否走"提权后重采"分支。
