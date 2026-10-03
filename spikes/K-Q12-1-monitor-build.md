# K-Q12-1 — AegisMonitor sidecar 可复现构建取证

- 日期：2026-10-03｜执行：Qoder（M-01 片1-a）
- 结论：🟢 **构建链打通且产物可运行**（下列数字均为本机实测，非推断）；Q-12=B 路线成立，无需走"无 sidecar 降级矩阵"。

## 一、环境实测值

| 项 | 实测 |
|---|---|
| .NET SDK | `8.0.425`（`C:\Program Files\dotNet\sdk`；winget `Microsoft.DotNet.SDK.8` 退出码 0） |
| 运行时 | `Microsoft.NETCore.App 8.0.31 / 10.0.12`、`Microsoft.WindowsDesktop.App 8.0.31 / 10.0.12` |
| net48 targeting pack | **本机未装**（`C:\Program Files (x86)\Reference Assemblies\` 不存在）→ SDK 自动追加 `Microsoft.NETFramework.ReferenceAssemblies[.net48] 1.0.3` 补齐，无需装 Developer Pack |
| NuGet 源 | `api.nuget.org` 实测 302 → `nuget.azure.cn` 镜像；首次 restore 8.14s，还原 21 个包 |

## 二、构建命令（唯一入口 = `tools/build-monitor.ps1`）

```powershell
# 脚本内三步（另加产物存在性校验 + SHA256 打印）
dotnet restore Aegis/monitor/AegisMonitor.csproj
dotnet build   Aegis/monitor/AegisMonitor.csproj -c Release -o Aegis/monitor/bin/Release/net48 --no-restore
```

实测：`已成功生成。0 个警告 0 个错误`。

## 三、产物

| 项 | 值 |
|---|---|
| 路径 | `Aegis/monitor/bin/Release/net48/AegisMonitor.exe` |
| 大小 / SHA256 | 13,312 字节 / `56926f0abf73fd66c34506497a5dd6e657fd3ef68cace57c4ad6fb1f8048c00a` |
| 同目录 dll | 18 个（LHM 传递依赖 + `System.Text.Json` 8.0.0 系） |

包版本（与 NexBox `monitor/obj/project.assets.json` 同值，未追新）：
`librehardwaremonitorlib 0.9.6`、`hidsharp 2.6.4`、`diskinfotoolkit 1.1.2`、`ramspdtoolkit-ndd 1.4.2`、
`blacksharp.core 1.0.4/1.0.7`、`system.text.json 8.0.0`、`system.management 10.0.2`、`system.codedom 10.0.2`、
`system.threading.accesscontrol 10.0.3`、`microsoft.netframework.referenceassemblies(.net48) 1.0.3`。

> 🟢 风险表"`.NET 10` 系包在 SDK 8 下是否兼容"已实测：**可还原、可编译、可运行**（后三个包由 LHM 传递引入，无降级）。
> 🟡 许可档位与分发边界见 `DEPENDENCIES.md §四`；`hidsharp 2.6.4` 仍为**待核**，它随包分发 → 打包前必须核掉，否则从 sidecar 引用中剔除并降级对应字段。

## 四、运行期协议冒烟（同一产物，stdin/stdout 行协议）

| 观测 | 实测 |
|---|---|
| 首条响应延迟 | **548 ms**（含 `Computer.Open()`；预算上限 15s，余量充足） |
| 首条响应 | 10,184 字节 / 64 条传感器 |
| 第二次 `read` | 请求后约 **12 ms** 返回（10,358 字节 / 64 条）→ 稳态读取代价可忽略，2s 节拍可行 |
| `{"cmd":"exit"}` | 退出码 `0`（优雅退出路径可用） |
| stderr | 仅一行 `[AegisMonitor] Computer.Open() 完成` |

## 五、本轮实测发现的两处修正（非照抄母本）

1. **stderr 编码**：中文 Windows 控制台默认 OEM 代码页（GBK），母本 `Console.Error.WriteLine` 落管道后是**非法 UTF-8**；
   宿主按 UTF-8 逐行读会报错并静默丢掉全部诊断（违 R-08）。
   处置双保险：C# 侧 `Console.SetError(new StreamWriter(Console.OpenStandardError(), UTF8 无 BOM){AutoFlush=true})`；
   Rust 侧 `infra::sidecar` 改**按字节 `read_until(b'\n')` + `String::from_utf8_lossy`**。
   验证：修正后 stderr 字节 `e5 ae 8c e6 88 90` = "完成" 的 UTF-8 编码。
2. **构建产物不入版本库**：`monitor/bin/`、`monitor/obj/` 需进 `.gitignore`（否则 18 个 dll 会进暂存区）。
