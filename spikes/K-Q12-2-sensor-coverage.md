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
- 🔲 管理员启动同产物：确认 `HardDisk` / `Motherboard` 是否补齐，据此定 F-06 是否走"提权后重采"分支。🔴 需 UAC 交互，交用户执行。

## 六、片4 F-11 前的复测（同日 23:20，非管理员 + 未装 PawnIO）

取数方式与 §一 相同（stdin `{"cmd":"read"}`，stdout 一行 JSON）。**结论未变，数字随负载浮动**：

- 体量：64 条传感器 / 10,261 B；硬件面仍只有 `Cpu` ×26、`Memory` ×6、`GpuNvidia` ×32，**`HardDisk` / `Motherboard` 仍 0 条**，`Fan` 类仍 0 条 → §一 的三态判定与 F-06「空面板 + 原因」口径继续成立。
- CPU 面本次见 `CPU Total` 23.09%、`CPU Core Max` 66.67%，`Temperature` 仍 0 条（L-01 再次印证）。
- GPU 面本次见 `GPU Core` 60℃ / `GPU Hot Spot` 64.69℃ / `GPU Core` 负载 3%，与 §一 同构。
- Memory 面见 `Total Memory` 42.95% 与 `Virtual Memory` 42.43% **两条并存** → 聚合规则 4 的 `hardware == "Total Memory"` 收窄不是可选优化，缺它就把虚拟内存当物理内存占用展示（R-08 违规）。

## 七、2026-10-07 复诊：为何 SystemToolkit 能读到盘温而本项目读不到（结论：非代码缺陷）

用户报「SystemToolkit 可以读取硬盘温度」。逐项取证后**结论是：本项目没有缺陷，两侧走的是同一条路、同一份 LHM 0.9.6，差别在运行环境而非代码**。

| 项 | SystemToolkit | 本项目 | 是否构成差异 |
|---|---|---|---|
| LHM 配置 | `IsStorageEnabled = true`（`HardwareSensorProbe.cs:120`） | `IsStorageEnabled = true`（`monitor/Program.cs:38`） | ❌ **完全相同** |
| LHM 版本 | 0.9.6 | 0.9.6 | ❌ 相同 |
| 是否自带内核驱动 | ❌ 0.9.6 起已移除 WinRing0 | ❌ 同 | ❌ 相同 |
| 是否自动装 PawnIO | ❌ 不装 | ❌ 不装（`DEPENDENCIES.md` §四「零分发零安装」） | ❌ 相同 |
| **运行权限 / 环境** | 🔴 **基线 2026-09-05 真机复现：管理员运行 + HVCI/阻止列表全关 + 未装 PawnIO → 温度节点整体缺席；装上后齐全** | 同上条件 ⇒ 同样缺席 | ✅ **唯一差异来源** |

基线原文（`HardwareSensorProbe.cs:422-430`）：

> LHM 0.9.6 起内置 WinRing0 已移除，CPU 温度经 PawnIO 内核驱动读取
> （真机复现：**管理员运行 + HVCI/阻止列表全关 + 未装 PawnIO → 温度节点整体缺席；装上后齐全**）

⇒ **盘温与 CPU 温度同源**（都走内核驱动读 SuperIO/SMBus），不是两个独立能力。故「Aegis 读不到盘温」与「Aegis 读不到 CPU 温度」（`:14` 早已实测 `Temperature` 0 条）是**同一个原因**，不是两个 bug。

🔴 **本项目的展示面已如实呈现**：`locales/zh-CN/overview.json:193` 的 F-06 空态即「未检测到存储设备（需管理员权限）」，与 §一 的三态判定一致 —— 无数据不填0、不留空（R-08）。

⚠️ **§五 的第2 条待补测至今未做**（`:60`，需 UAC 交互）：**管理员启动同产物，确认 `HardDisk` 是否补齐**。在它完成前，「非管理员读不到盘温」仍属**未经本项目实测复核**的推断（依据来自基线复现 + 本项目 10-03 非管理员实测），不能记为已验证。

## 七·补、2026-10-07 真机复测：⚠️ **推翻了本节上文两处推断**

起因：用户再次报「SystemToolkit 能读盘温」。本轮**不再靠代码推理**，直接起sidecar 副本取真实回包（脚本 `.worknotes/probe-monitor.cjs`，复制 `bin/Release/net48` 到临时目录以避开运行中 Aegis 对 exe 的文件锁）。

### 实测结果（与 §一/§六 两次独立记录完全一致）

| 项 | 实测值 |
|---|---|
| stderr | 仅 `Computer.Open() 完成`（**无任何异常**） |
| 回包 | 1 行 / 10,224 B / **64 条传感器** |
| 硬件族分布 | `Cpu` 26、`Memory` 6、`GpuNvidia` 32 —— **仅 3 族** |
| 硬件名 | i7-12700H、`Total Memory`、`Virtual Memory`、RTX 3060 Laptop —— **仅 4 个** |
| Temperature 条目 | **仅 2 条**，全在 GPU（`GPU Core` 60℃ / `GPU Hot Spot` 64.66℃） |
| **存储族（Ssd/Hdd/HardDisk/NVMe/Disk）** |🔴 **0 条** |
| CPU 温度 / Motherboard / SuperIO | 🔴 **各 0 条** |

### ⚠️ 修正一：「Glob 找不到 exe」是我误判

上文写「`AegisMonitor.exe` 在工作区不存在（Glob 零命中）」—— **错误**。实测产物存在（13,312 B，mtime 2026-10-03 12:12），且**运行时确有 sidecar 存活**（`aegis` PID 9852 于 06:11:37 启动 → `AegisMonitor` PID 21732 于 06:11:38 拉起，通道是**活的**）。
根因：**Glob 遵循 `.gitignore`，而 `bin/` 被排除** ⇒「工具没列出」被我当成「文件不存在」。**这与本会话早前「grep 输出被截断 ⇒ 据此断言不存在」是同一类错误：把「工具没显示」当「没有」。**

### ⚠️ 修正二：「盘温与 CPU 温度同源、都是权限问题」——推断不足

上文由「CPU 温度 0 条」+「基线说未装 PawnIO → 温度节点整体缺席」推出「盘温同为权限所致」。**本轮实测不支持这个推论**，理由是**故障形态不符**：

- 若仅是内核驱动缺失/权限不足，LHM 的典型行为是**硬件节点仍被枚举、只是传感器为空**；
- 实测是**连硬件节点都不存在**（存储族 0 个硬件名，`Motherboard`/`SuperIO` 也 0），且 `Computer.Open()` **未报任何异常**。
⇒ 更符合「LHM 在当前环境下**根本没枚举到**这些硬件」，而非「枚举到了但读不到传感器」。**两者处置不同**（前者改环境/驱动，后者改代码），不可混为一谈。

### 🔴 修正三（2026-10-07 第二次取证）：**环境侧全部到位**，故「权限」也未被证实

用 `.worknotes/probe-monitor.cjs` 增采环境事实（非管理员令牌下）：

| 环境项 | 实测值 | 含义 |
|---|---|---|
| HVCI `Enabled` | `0` | 内核代码完整性**未开启**（不拦驱动） |
| 内核阻止列表 | `<不存在>` | 未配置 |
| **PawnIO 已安装** | `DisplayName : PawnIO`，`ImagePath : \...\DriverStore\FileRepository\pawnio.inf_amd64_a72a2f969b8b7496\PawnIO.sys` | ✅ **已装**（与 §一 的「未安装」记录**相反**—— 那次确实没装，此间装上了） |
| PawnIO 服务 `Start` | `3`（手动） | 由 LHM 按需加载 |
| **此刻已加载驱动** | 🔴 **`PawnIO` + `JYWinRing0_130` 均在运行**（208 个驱动中命中 2 个） | ✅ **驱动此刻确实在跑** |
| WinRing0 内置驱动服务 | `<不存在>` | 符合 0.9.6 起的预期 |

⇒ **驱动已加载、策略未拦截**，那么「权限不足」只剩一种可能形态：**LHM 在非管理员令牌下主动跳过需要管理员的 SuperIO/SMBus 硬件**（表现为节点根本不出现，而非节点出现但传感器空）。
🔴 **这仍是推断**，未证实 —— 唯一能证实它的动作是**以管理员身份跑同一脚本**，对比存储族是否出现。

## 七·终、2026-10-07 管理员对照：**真因是判据白名单缺`Storage`**

管理员令牌下跑 `.worknotes/probe-monitor.cjs`，**64 条 → 205 条传感器**，硬件族从 3 族变 4 族：

| 硬件族 | 非管理员 | 🔴 管理员 |
|---|---|---|
| `Cpu` | 26 | 87 |
| `Memory` | 6 | 48 |
| `GpuNvidia` | 32 | 32 |
| **`Storage`** | **0** | 🔴 **38** |

**结论（两层，须分开看）**：

###① 权限确实决定「能否枚举到」（非管理员 = 整族缺席）

CPU/主板/存储/内存温度全部依赖 SuperIO/SMBus 通道，**非管理员令牌下 LHM 连硬件节点都不枚举**（不是「节点在、传感器空」）。这解释了 §一/§六 全部现象，且与基线 `HardwareSensorProbe.cs:422-430` 的真机复现一致。

### ② 🔴 但**本仓有一个独立的真实缺陷**，它让管理员也读不到盘温

`matching.rs` 的存储族白名单原为 `["HardDisk", "Hdd", "Ssd"]`，而 **LHM 0.9.6 真机给的是 `hardwareType: "Storage"`** —— **一个都不匹配**（基线用的是 LHM `HardwareType.Storage` 枚举，`HardwareSensorProbe.cs:174`）。

⇒ `is_storage_family` 恒 `false` ⇒ `has_storage_sensor` 恒 `false`、`storage_temperature_of` 恒 `None` ⇒ **即便管理员下回包已摆着 `Composite Temperature` 45/44°C，存储卡徽章仍显示「未检测」**。

⇒ **这才是「SystemToolkit 能读到、Aegis 读不到」的直接原因**（权限只解释「非管理员时没有」，解释不了「管理员时仍没有」）。

**已修**：`STORAGE_FAMILIES` 补 `Storage` 并改为**小写包含**匹配（等值匹配要求穷举 LHM 全部取值，而那份清单本就不完整）；`metrics.rs` 的 `family_of` 同步补 `contains("storage")`。回归测试用**真机回包原样截取**（`REAL_ADMIN_REPLY`），且已做**阳性对照**（白名单回退 ⇒ 该测试 FAILED ⇒ 还原 PASSED）。

### 🔴 元教训（本轮最有价值的一条）

此前 12 条传感器夹具**全部是照抄 `NexBox` 母本源码臆造的形态** —— `hardwareType` 写 `"Ssd"`、阈值名写 `"Critical Comp. Temperature"`、内存写 `"DIMM1"`。而真机是`"Storage"` / `"Critical Temperature"` / `"DIMM #0"`（且 hardware 是模组型号 `SK Hynix - HMCG78MEBSA253N (#0)` 而非 `Total Memory`）。

⇒ **臆造形态的夹具让 85°C 阈值缺陷与`Storage` 白名单缺陷双双测不出来，而测试全绿。**
⇒ 纪律：**传感器类判据的夹具必须抄真机回包，不得从上游源码推演。** 今天的两个缺陷同源于此。

### 顺带修正 §一/§六 的环境记录

- §一 记「**未安装** PawnIO」，现已**安装**且 `PawnIO` + `JYWinRing0_130` **此刻均在运行**（208 个驱动中命中 2 个）；HVCI `Enabled=0`、阻止列表不存在。⇒ 该条历史记录的**环境前提已变**，其结论（非管理员下三类硬件缺席）仍成立。
- **本仓聚合判据的其余部分仍未被真机数据检验**：管理员下`Memory` 48 条里有 `Thermal Sensor High Limit 55°C` / `Critical High Limit 85°C`，正是阈值型条目 —— 这类形态在既有夹具里**从未出现过**（旧夹具只造了 `DIMM1 High Limit` 一条）。`DIMM #0` / `DIMM #2` 与 `Total Memory` 分属不同硬件名，**内存负载判据是否受影响待复核**。

### 内存负载判据复核（2026-10-07，结论：三条判据均正确，但有一条是推导而非实测）

管理员回包的**硬件名全表**（已留痕）显示 `Memory` 族下**同时存在 4 个硬件名**：

```
Virtual Memory   [Memory]      ← 虚拟内存，🔴 绝不可当物理占用
Total Memory     [Memory]      ← 物理内存合计，✅ 负载正确来源
SK Hynix - HMCG78MEBSA253N (#0) [Memory]   ← 物理模组（管理员下新增）
SK Hynix - HMCG78MEBSA253N (#2) [Memory]   ← 物理模组（管理员下新增）
```

复核三条判据，**全部正确且是刻意的**：

| 判据 | 结论 | 依据 |
|---|---|---|
| `memory_load_of` 收窄 `hardware == "Total Memory"` | ✅ 正确 | 严格等值 ⇒ 虚拟内存与物理模组**都不会被误取** |
| `memory_temperature_of` **不**收窄硬件名 | ✅ 正确且必要 | TSOD 只挂在**模组**节点上；`Total Memory` 是虚拟合计节点、不给温度。若一并收窄，本机内存温度会永远读不到 |
| `Data` 条目（`Memory Used`/`Available`） | ⚪ **生产代码不消费** | 内存容量与占用走 `sysinfo`（`R-07` 一级优先），传感器侧仅用于诊断 |

⇒ **两条判据刻意不对称是正确的**，已各钉一条测试（`memory_temperature_may_come_from_a_module_even_though_load_may_not`）。

🔴 **一条夹具是「推导」而非「抄真机」，须登记在案**：`memory_load_fixture()` 里`Load` 条目的**归属**（`Total Memory` 有 `Load`、`Virtual Memory` 有 `Load`）来自 §一 的10-03 非管理员实测 + 硬件名全表**推导**，**不是**管理员回包的真实数据 —— 当时取证脚本只列了 `Temperature` 条目，未列非温度条目。
若真机证伪（管理员下 `Total Memory` 不再给 `Load`），该夹具即失效，须重采。已把取证脚本补上「全部非温度条目」输出，**下次管理员跑一次即可闭环**。

⇒ **待办（需管理员跑一次）**：`node .worknotes\probe-monitor.cjs`（脚本已补非温度条目输出）⇒ 核对 `Total Memory` 下的 `Load` 与 `Data` 是否仍在。

1. **sidecar 每条读数自带 `unit`**（℃ / % / W / MHz / GB / MB / B/s / V 实测均出现）。
   单位是**数据不是文案**，故 `overview_sensor_table` 直接透传（`SensorReading.unit` → `SensorRowDto.unit`），
   🔴 不在本仓重抄一份 15 项单位表（那份表的唯一 owner 在 sidecar `Program.cs::GetUnit`）。
2. 本机 64 条覆盖 8 种 `sensorType`：`Load` `Temperature` `Power` `SmallData` `Data` `Clock` `Throughput` `Voltage`
   → `locales/zh-CN/overview.json` 的 `sensor.type.*` 已**全部覆盖这 8 种**，本机导出不漏译名；
   未知类型（如他机上出现的 `Fan` / `Control`）按"词典无此键则原样透出 sidecar 串"处理，🔴 不臆造译法。
