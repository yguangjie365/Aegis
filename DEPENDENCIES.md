# DEPENDENCIES — 依赖与许可证登记表（唯一清单）

> 口径事实源：[ADR-003 §二.2](../Docs/开发文档/04-决策/ADR-003-第三方代码引入与许可证规范.md)（档位表见其 §三，本表不复述）。
> 引入五步（02 分册 §6.2）缺一即违规；`NOTICE` 由本表生成。
> 版本一律记 **lockfile 实测值**（钉版纪律见 TASKS §五 "W0-01/W0-02 收口"条目；基线 = NexBox 实测）。
> 🔴 **基线条口的唯一例外**：**tauri 全家桶**（`tauri` / `tauri-build` / `tauri-plugin-*`）以 **Aegis lockfile 实测值为基线**，不再逐版对齐 NexBox；详见 [ADR-005](../Docs/开发文档/04-决策/ADR-005-依赖基线与钉版口径.md)（2026-10-04 用户拍板）。
> 本表 2026-10-02 建表时一次性补登 W0 期已引入的直接依赖（登记日期如实标注）。

## 一、Rust 直接依赖（`src-tauri/Cargo.lock` 实测）

| 名称 | 版本（lock 实测） | 许可证 | 引入方式 | 用途 | 档位 | 登记日期 | 变更行 |
|---|---|---|---|---|---|---|---|
| tauri | 2.12.1 | MIT OR Apache-2.0 | crate 引用 | 应用宿主（features: protocol-asset, tray-icon） | 🟢 | 2026-10-02（补登 W0-01） | 建表补登；**2026-10-03 修正漂移**——`Cargo.toml` 声明 `"2.10"` 属 caret 语义，lock 实测已解析到 **2.12.1**（原登记写 2.11.1 与实测不符）。🔴 该漂移正是 `tauri dev` 报 "Found version mismatched Tauri packages" 的根因：Rust 2.12.1 vs npm `@tauri-apps/api` 2.11.1 |
| tauri-build | 2.7.1 | MIT OR Apache-2.0 | crate（build-dep） | 构建期代码生成 | 🟢 | 2026-10-02（补登 W0-01） | 建表补登；**2026-10-03 修正漂移**（原登记 2.6.1，lock 实测 2.7.1） |
| tauri-plugin-single-instance | 2.4.2 | MIT OR Apache-2.0 | crate 引用 | 单实例守卫 | 🟢 | 2026-10-02（补登 W0-01） | 建表补登 |
| serde | 1.0.x | MIT OR Apache-2.0 | crate 引用 | 序列化基座 | 🟢 | 2026-10-02（补登 W0-01） | 建表补登 |
| serde_json | 1.0.x | MIT OR Apache-2.0 | crate 引用 | JSON（缓存/IPC 载荷） | 🟢 | 2026-10-02（补登 W0-01） | 建表补登 |
| tauri-plugin-dialog | 2.7.1 | MIT OR Apache-2.0 | crate 引用 | 原生目录选择框（MU-1 导入文件夹） | 🟢 | 2026-10-02 | MU-1 新增；连带 tauri-plugin-fs 2.5.1（同许可，钉回 NexBox 基线） |
| tauri-plugin-store | 2.5.0 | MIT OR Apache-2.0 | crate 引用（NexBox `lib.rs:312` 注册 `plugin-store`，本侧 `src-tauri/Cargo.toml` 与 `crates/services/Cargo.toml` 双声明） | 音乐/设置持久化 Store（主题、播放历史、`cookie_<provider>` 等键值落地） | 🟢 | 2026-10-03 | **建表时漏登 → 2026-10-03 补登并统一版本**：`cargo update -p tauri-plugin-store` **2.4.5 → 2.5.0**，对齐 npm `@tauri-apps/plugin-store` 实测 2.5.0（此前 Rust 2.4.5 vs npm `^2.5.0` 反向不匹配，属同一"版本不匹配 Tauri 包"问题的另一半）。🔴 **偏离 NexBox 基线 +1 次版本**（NexBox lock 实测 `tauri-plugin-store` 2.4.3 / npm `^2.4.3`）：非主动追新，而是为消除 Rust↔npm 双向错配、且 npm 侧 `^2.5.0` 已先落地；mismatch 修复后遗留的"全家桶是否零偏差"口径见 §一 表下注。**当前 Rust 2.5.0 ↔ npm 2.5.0 已对齐**，`cargo check` 0 error、`cargo test --workspace` 198 passed（24 + 174） |
| lofty | 0.21.1 | MIT OR Apache-2.0 | crate 引用 | 音频标签读取（MU-1 元数据/内嵌封面歌词） | 🟢 | 2026-10-02 | MU-1 新增（NexBox 同版实测） |
| image | 0.25.10 | MIT OR Apache-2.0 | crate 引用（default-features=false，仅 png/jpeg/ico） | 封面压缩缩放（MU-1） | 🟢 | 2026-10-02 | MU-1 新增（NexBox 同版实测） |
| keyring | 3.6.3 | MIT OR Apache-2.0 | crate 引用（features: windows-native） | 凭据安全存储（MU-4.1，Windows 侧走 Credential Manager） | 🟢 | 2026-10-02 | MU-4 新增；🔴 **NexBox 无此依赖**（其用明文 `music-cookies.json`，触本工程红线，故属有意偏离，见 §三"Aegis 自建凭据加密层"行）。Windows 后端单条 blob 上限 2560 **字节**，超长凭据分片方案见 [02 专题设计 §5.3](../Docs/开发文档/02-专题设计/02-安全与特权通道.md) |
| tracing | 0.1.44 | MIT | crate 引用 | MU-4 结构化日志（八字段 JSONL 的字段载体） | 🟢 | 2026-10-02 | MU-4 新增（NexBox 同版实测 0.1.44） |
| axum | 0.7.9 | MIT | crate 引用 | 本地音频/封面反代服务（MU-4 `proxy.rs`） | 🟢 | 2026-10-02 | MU-4 新增（NexBox 同版实测 0.7.9） |
| reqwest | 0.12.28 | MIT OR Apache-2.0 | crate 引用（features: stream, json） | 上游平台 HTTP 客户端 + 反代透传 | 🟢 | 2026-10-02 | MU-4 新增；lock 内另存 0.11.27/0.13.3 为传递依赖，我方直接依赖取 0.12.28（NexBox 同版实测） |
| futures-util | 0.3.34 | MIT OR Apache-2.0 | crate 引用 | 流式字节转发（`Stream` 适配） | 🟢 | 2026-10-02 | MU-4 新增；🟡 **高于 NexBox 基线**（其 lock 0.3.32，本侧解析到 0.3.34，semver 兼容位漂移，非主动跳版本） |
| tokio | 1.53.1 | MIT | crate 引用（features: net, sync） | 反代运行时与凭据读写异步任务 | 🟡 | 2026-10-02 | MU-4 新增；🟡 **高于 NexBox 基线**（其 lock 1.52.3 → 本侧 1.53.1，次版本漂移；`Cargo.toml` 声明 `"1"` 未钉次版本，随整体依赖对账收口） |
| md-5 | 0.10.6 | MIT OR Apache-2.0 | crate 引用 | 网易云 weapi `digestConfig`（MU-4.6/4.7） | 🟢 | 2026-10-02 | MU-4 新增（NexBox 同版实测） |
| aes | 0.8.4 | MIT OR Apache-2.0 | crate 引用 | 网易云 EAPI AES-128-CBC（MU-4.6/4.7） | 🟢 | 2026-10-02 | MU-4 新增（NexBox 同版实测） |
| cbc | 0.1.2 | MIT OR Apache-2.0 | crate 引用（features: block-padding） | 同上分组模式 | 🟢 | 2026-10-02 | MU-4 新增（NexBox 同版实测） |
| num-bigint | 0.4.8 | MIT OR Apache-2.0 | crate 引用 | EAPI RSA（模幂） | 🟢 | 2026-10-02 | MU-4 新增（NexBox 同版实测） |
| hex | 0.4.3 | MIT OR Apache-2.0 | crate 引用 | 签名/摘要编解码 | 🟢 | 2026-10-02 | MU-4 新增（NexBox 同版实测） |
| chrono | 0.4.45 | MIT OR Apache-2.0 | crate 引用 | 时间戳与登录态有效期 | 🟢 | 2026-10-02 | MU-4 新增；🟡 **高于 NexBox 基线**（其 lock 0.4.44 → 本侧 0.4.45，补丁位漂移，semver 兼容） |
| rand | 0.8.8 | MIT OR Apache-2.0 | crate 引用 | weapi `secKey` 随机串 | 🟢 | 2026-10-02 | MU-4 新增；🟡 **高于 NexBox 基线**（其 0.8 线 lock 0.8.6 → 本侧 0.8.8，补丁位漂移；NexBox 另有 0.9.4 为传递依赖，我方声明钉 `"0.8"`） |
| base64 | 0.22.1 | MIT OR Apache-2.0 | crate 引用 | weapi/QQ 签名编解码 | 🟢 | 2026-10-02 | MU-4 新增；🔴 lock 内三版本共存（0.21.7 / 0.22.1 / 0.23.1 均为传递依赖链），**我方直接依赖为 `base64 = "0.22"`，解析到 0.22.1**（NexBox 亦含 0.22.1） |
| urlencoding | 2.1.3 | MIT | crate 引用 | 查询串与回调参数编码 | 🟢 | 2026-10-02 | MU-4 新增（NexBox 同版实测） |
| sha1 | 0.10.7 | MIT OR Apache-2.0 | crate 引用 | QQ 音乐 `signKey` 签名（MU-4.8） | 🟢 | 2026-10-02 | MU-4 新增；🟡 **高于 NexBox 基线**（其 lock 0.10.6 → 本侧 0.10.7，补丁位漂移） |
| regex | 1.13.1 | MIT OR Apache-2.0 | crate 引用 | QQ 歌词/评论文本解析 | 🟡 | 2026-10-02 | MU-4 新增；🟡 **高于 NexBox 基线**（其 lock 1.12.3 → 本侧 1.13.1，次版本漂移；`Cargo.toml` 声明 `"1"`，随整体依赖对账收口） |
| sysinfo | 0.30.13 | MIT OR Apache-2.0 | crate 引用 | 本机概览占用采集（CPU/内存/磁盘容量、进程榜，M-01 片1） | 🟢 | 2026-10-03 | OV-1 新增；版本钉 NexBox lockfile 实测 0.30.13（其 `hardware.rs` 同源用法）。🔴 移植时踩坑已登记：Windows 进程 CPU 值为"占一个核"口径，整机% 须先除核数再钳 0–100 |
| winreg | 0.52.0 | MIT OR Apache-2.0 | crate 引用（`infra`，`cfg(windows)`） | PawnIO 只读注册表检测（`registry.rs`，M-01 片1） | 🟢 | 2026-10-03 | OV-1 新增；钉 NexBox `cargo.toml` 实测 0.52。🟡 lock 内另存 0.55.0 为传递依赖，我方直接依赖为 `0.52.0` |
| windows-sys | 0.59.0 | MIT OR Apache-2.0 | crate 引用（`infra`，features: `Win32_Foundation` / `Win32_NetworkManagement_IpHelper` / `Win32_Networking_WinSock`） | `GetExtendedTcpTable` 官方绑定（`net_tcp_table.rs`，M-01 片1） | 🟢 | 2026-10-03 | OV-1 新增；版本与 features 子集钉 NexBox 实测（其 `Cargo.toml` 声明 `windows-sys = "0.59"` 且同含 IpHelper/WinSock，用于 `game_ping.rs`、`overlay_panel.rs` 的 `GetIfTable2`）。🔴 非手写 FFI，满足 02 分册 KN-04"FFI 只在 infra"与禁裸绑定红线。🟡 lock 内 0.45/0.48/0.52/0.60.2 均为传递依赖链 |
| wmi | 0.14.5 | MIT OR Apache-2.0 | crate 引用（🔴 **`crates/infra`** 的 `[target.'cfg(windows)'.dependencies]`，default features = `chrono`；`services` 经 `infra::wmi` 使用） | 本机概览片2 静态硬件采集：`Win32_Processor` / `Win32_BaseBoard` / `Win32_BIOS` / `Win32_PhysicalMemory` / `Win32_DiskDrive` / `Win32_SoundDevice` / `Win32_NetworkAdapter` / `WmiMonitorID`（F-02 八类详情卡） | 🟢 | 2026-10-03 | OV-2 新增；版本钉 NexBox 实测（`Source/NexBox/src-tauri/Cargo.toml:47` 声明 `"0.14"`，其 `Cargo.lock` = 0.14.5，checksum `7787dacd…8e71`）——🔴 同版零偏差。传递依赖逐查见下方「片2-a 传递依赖逐查」；🔴 **落点更正（片2-b，2026-10-03）**：本行原记 `services`，违反 [04 分册 §一](../Docs/开发文档/03-开发规范/04-文件与目录规范.md) 准入表"FFI 只出现在 infra 与 elevated-helper"（WMI 走 COM），已迁至 `crates/infra` |
| nvml-wrapper | 0.10.0 | MIT OR Apache-2.0 | crate 引用（🔴 **`crates/infra`** 的 `[target.'cfg(windows)'.dependencies]`，**default features 为空**，未开 `legacy-functions` / `serde`；`services` 经 `infra::nvml` 使用） | NVIDIA GPU **显存总量 + 驱动版本**两项静态规格（F-02 显卡卡，M-01 片2）。🔴 温度/负载/显存已用**不用本 crate**，只走 sidecar——收口口径见 §五 | 🟢 | 2026-10-03 | OV-2 新增；🔴 **不是新增技术路线**：NexBox `Cargo.toml:55` 已带 `nvml-wrapper = "0.10"`（lock 0.10.0，checksum `0c9bff0a…38c5`），母本实现即 `hardware.rs:741 get_nvidia_gpus_with_nvml()`（取 `name/memory_info/temperature/utilization_rates/sys_driver_version`），本工程属②档移植而非自造。🔴 **分发面**：经 `libloading 0.8.9` **运行时动态加载用户机器上随 NVIDIA 驱动已装好的 `nvml.dll`**，Aegis 包内🟰**零 NVIDIA 二进制/零 `nvml.h`**（取证见 §五）；🔴 **落点更正（片2-b）**：原记 `services`，因 NVML 走 `libloading` C 互操作属 FFI，按 04 分册 §一 准入表迁入 `crates/infra` |

> 🟡 **钉版偏差如实申报**：上表 6 个 crate（tokio / regex / futures-util / chrono / rand / sha1）实测版本**高于 NexBox lockfile 基线**，均为 `Cargo.toml` 声明区间内的依赖解析漂移，非本工程主动跳版本。🔴 tauri 全家桶（含 `tauri-plugin-*`）**除 `tauri-plugin-store` 外零偏差**，符合"全家桶锁 NexBox"红线。⚠️ **唯一已知偏差（2026-10-03 由我引入，为修 mismatch 而主动升级）**：`tauri-plugin-store` Rust **2.4.5 → 2.5.0**（NexBox 基线 2.4.3，即 +1 次版本），背景与收口口径见该行"变更行"。✅ **收口：用户 2026-10-04 拍板 = A「维持 2.5.0 双对齐 + 整体抬基线」**，已立 [ADR-005](../Docs/开发文档/04-决策/ADR-005-依赖基线与钉版口径.md)：`tauri-plugin-store` 的 **Aegis 基线钉 2.5.0**（同时抬 NexBox 侧同步口径：今后 p2p 对账以 2.5.0 为对齐目标），并籍此把"全家桶基线 = NexBox 实测"放宽为"**tauri 全家桶以 Aegis lock 实测为基线 + 其余 crate 仍落 NexBox 同 semver 区间**"。处置同批生效：Rust 2.5.0 ↔ npm 2.5.0 双对齐、`cargo check` 0 error、`cargo test --workspace` 198 passed。收口动作 = 把 MU-4 这批声明补成 NexBox 实测精确版本或整体抬基线，**须用户拍板**（见 TASKS §三 Q-23）。许可证档位本身全部落在 ADR-003 允许档（MIT / Apache-2.0 / MIT OR Apache-2.0），无许可风险。

内部 crate（`contracts` / `infra` / `services` / `elevated-helper` / `worker`，path 引用）随仓库整体 GPL-3.0，不占本表。

### 片2-a 传递依赖逐查（2026-10-03，M-01 概览批；脚本 `.worknotes/dep-audit.mjs` 可重放）

口径 = ADR-003 §三 逐档判定，🔴 不看包名看实物：许可证字符串从**本地 cargo 解包源的 `Cargo.toml`** 读取（`~/.cargo/registry/src/<repo>/<pkg>-<ver>/Cargo.toml`），并同时列出包内实际存在的 LICENSE 文件名，非凭记忆、非查网页。

实测口径两处（🔴 均为**实物解包读 `Cargo.toml` 的 license 字段**，非凭记忆）：

- **落点迁移后重测（片2-b，2026-10-03，脚本 `.worknotes/dep_closure_infra.py` 可重放）**：`cargo tree -p infra -e normal` 取 `wmi` + `nvml-wrapper` 两条子树，按 **(name, version) 去重后闭包 48 个包**，其中 **34 个已在 HEAD `Cargo.lock` 内**，**新引入 14 个** —— 与下方 14 行表**逐条一致**，落点由 `services` 迁到 `infra` 不改变许可结论。
- 迁移前的初测（脚本 `.worknotes/dep-audit.mjs`）曾报"39 个包 / 25 已在 HEAD / 新引入 14"：**新引入面同为 14 个**，前两项数字偏小是因为 `cargo tree -p services` 里这些包多数已挂在音乐/系统侧父节点下、子树被去重未展开。🔴 以本节的 48/34/14 为准。

新引入的 **14 个**全部落 🟢 档：

| 包 | 版本 | license 字段（实测） | 包内 LICENSE 文件 |
|---|---|---|---|
| wmi | 0.14.5 | MIT OR Apache-2.0 | LICENSE-APACHE / LICENSE-MIT |
| futures | 0.3.34 | MIT OR Apache-2.0 | LICENSE-APACHE / LICENSE-MIT |
| windows | 0.59.0 | MIT OR Apache-2.0 | license-apache-2.0 / license-mit |
| windows-core | 0.59.0 | MIT OR Apache-2.0 | 同上 |
| windows-implement | 0.59.0 | MIT OR Apache-2.0 | 同上 |
| windows-strings | 0.3.1 | MIT OR Apache-2.0 | 同上 |
| nvml-wrapper | 0.10.0 | MIT OR Apache-2.0 | 🟡 包内无 LICENSE 文件（crates.io 元数据字段为准） |
| nvml-wrapper-sys | 0.8.0 | MIT OR Apache-2.0 | 🟡 同上 |
| libloading | 0.8.9 | **ISC** | LICENSE |
| static_assertions | 1.1.0 | MIT OR Apache-2.0 | LICENSE-APACHE / LICENSE-MIT |
| wrapcenum-derive | 0.4.1 | MIT/Apache-2.0 | LICENSE-APACHE / LICENSE-MIT |
| darling | 0.20.11 | MIT | LICENSE |
| darling_core | 0.20.11 | MIT | LICENSE |
| darling_macro | 0.20.11 | MIT | LICENSE |

🟢 `ISC` 与 `MIT/Apache-2.0`（斜杠写法）均在 ADR-003 §三 🟢 白名单内（`libloading 0.8.9` 与 `wrapcenum-derive` 是这两例）。🔴 NOTICE：两个 nvml 包**发布物内不含 LICENSE 文件**，`NOTICE` 汇总时须从上游仓库取文本随包分发（发布前义务见 §五），不能只凭 `Cargo.toml` 字段就认为"声明已随包"。

🟡 **一条待用户确认项（不阻塞开发，阻塞发布）**：`nvml-wrapper-sys` 的 `src/bindings.rs` 是 bindgen 0.68.1 从 NVIDIA SDK 头文件 `nvml.h` 生成的**符号名与常量声明**。实测取证：生成脚本带 `--no-doc-comments`（见该包 `gen_bindings.sh`），解包物里**没有 `nvml.h`、没有 NVIDIA 版权/许可文本**，只有 API 形状常量。按"宽松许可 crate + 零 NVIDIA 材料分发"我判 🟢 可引入（与 `windows` crate 转写 MS 头文件的业界同例，且 NexBox 基线已同样携带）；但"头文件派生的常量面是否构成 NVIDIA 许可材料"属法律解释，🔴 **发布前请用户或法务确认一次**，本表按我的 🟢 判定先行、结论若变再回改。

## 二、npm 直接依赖（`package-lock.json` 实测）

| 名称 | 版本（lock 实测） | 许可证 | 引入方式 | 用途 | 档位 | 登记日期 | 变更行 |
|---|---|---|---|---|---|---|---|
| @tauri-apps/api | 2.12.1（exact） | MIT OR Apache-2.0 | npm 引用 | 前端宿主通道（invoke/event/window） | 🟢 | 2026-10-02（补登 W0） | 建表补登；**2026-10-03 升级 2.11.1 → 2.12.1**（exact）。理由：`Cargo.toml` 声明 `tauri = "2.10"` 是 caret 语义，lock 实测已到 **2.12.1**，而 npm 侧钉在 2.11.1 → `tauri dev` 报 `Found version mismatched Tauri packages (tauri v2.12.1 : @tauri-apps/api v2.11.1)`。按「npm 对齐 Rust 实际版本」处置，取与 lock 完全一致的 2.12.1（用户 2026-10-03 裁决） |
| @tauri-apps/cli | 2.12.1（exact, dev） | MIT OR Apache-2.0 | npm（dev） | 构建 CLI | 🟢 | 2026-10-02（补登 W0） | 建表补登；**2026-10-03 升级 2.11.4 → 2.12.1**（exact）。与 `@tauri-apps/api` 同批对齐 Rust `tauri` lock 实测 2.12.1（官方要求 CLI 与 runtime 同版本） |
| @tauri-apps/plugin-dialog | 2.7.3（exact） | MIT OR Apache-2.0 | npm 引用 | 目录选择器 JS 侧（MU-1） | 🟢 | 2026-10-02 | MU-1 新增 |
| @tauri-apps/plugin-store | 2.5.0（exact） | MIT OR Apache-2.0 | npm 引用 | 设置持久化（主题/播放历史/音乐凭据键值）JS 侧 | 🟢 | 2026-10-03 | **建表时漏登 → 补登**：`node_modules` 实测 2.5.0，与 Rust `tauri-plugin-store` 2.5.0 对齐（NexBox 基线 `^2.4.3`）。🔴 同一持久化通道跨 Rust/npm 两侧，任何一侧单改都会触发 `Found version mismatched Tauri packages`，故两侧必须同批升 |
| react / react-dom | ^19.0 | MIT | npm 引用 | UI 框架 | 🟢 | 2026-10-02（补登 W0） | 建表补登 |
| react-router-dom | ^7.1.5 | MIT | npm 引用 | 路由 | 🟢 | 2026-10-02（补登 W0） | 建表补登 |
| @chakra-ui/react | ^2.10.9 | MIT | npm 引用 | UI 组件底座 | 🟢 | 2026-10-02（补登 W0） | 建表补登 |
| @emotion/react / @emotion/styled | ^11 | MIT | npm 引用 | 样式引擎（令牌层载体） | 🟢 | 2026-10-02（补登 W0） | 建表补登 |
| @tanstack/react-query | ^5.66 | MIT | npm 引用 | 数据获取状态 | 🟢 | 2026-10-02（补登 W0） | 建表补登 |
| zustand | ^5.0 | MIT | npm 引用 | 外壳状态（导航折叠等） | 🟢 | 2026-10-02（补登 W0） | 建表补登 |
| framer-motion | ^11.18 | MIT | npm 引用 | 动效 | 🟢 | 2026-10-02（补登 W0） | 建表补登 |
| i18next / react-i18next | ^26 / ^17 | MIT | npm 引用 | 国际化 zh-CN | 🟢 | 2026-10-02（补登 W0） | 建表补登 |
| lucide-react | ^0.487 | ISC | npm 引用 | 图标语系（UI 规范 §七） | 🟢 | 2026-10-02（补登 W0） | 建表补登 |
| @fontsource/inter / noto-sans-sc / jetbrains-mono | ^5.x | OFL-1.1 | npm 引用（本地打包，禁 CDN） | 字体（UI 规范 §七 已批口径） | ⚠️ 见注 | 2026-10-02（补登 W0） | OFL-1.1 未列入 ADR-003 §三 档位表；字体选型系用户已批（TASKS §五 主界面框架落地条目），待 ADR-003 评审时补档 |
| typescript | ^5.8.2（dev） | Apache-2.0 | npm（dev） | 类型检查 | 🟢 | 2026-10-02（补登 W0） | 建表补登 |
| vite / @vitejs/plugin-react | ^6 / ^4（dev） | MIT | npm（dev） | 构建 | 🟢 | 2026-10-02（补登 W0） | 建表补登 |
| @types/node / @types/react / @types/react-dom | ^22 / ^19（dev） | MIT | npm（dev） | 类型桩 | 🟢 | 2026-10-02（补登 W0） | 建表补登 |
| husky | 9.1.7（dev；`^9.1.7` 钉自 NexBox `package.json` 实测） | MIT | npm（dev） | pre-commit 钩子承载（四条腿全量判定，见下行 eslint/prettier） | 🟢 | 2026-10-02 | 用户 2026-10-02 拍板补装；许可落 ADR-003 允许档（MIT），`core.hooksPath=.husky/_` 仅改本仓库局部配置。🟢 原"缺 Prettier/ESLint 腿"缺口已于同日补齐（commit `2a4a562`）；钩子红绿双向已实测：ESLint 违规→退出 1、Prettier 差异→退出 1、`src-tauri` 可达模块格式违规→退出 1、未暂存 Rust 改动→跳过该腿。 探针教训：孤儿 `.rs`（`lib.rs` 无 `pub mod`）`cargo fmt` 不遍历，钩子拦不住，测腿必须用可达模块 |
| eslint | 9.39.5（dev；`^9.39.5` 钉自 NexBox lockfile 实测，其 `package.json` 声明 `^9.22.0`） | MIT | npm（dev） | 提交前检查点之一：`eslint . --max-warnings 0` | 🟢 | 2026-10-02 | 用户授权补装（"装依赖→补配置→全量格式化单独 commit→钩子加腿→登记"）。🔴 上游状态：9.39.5 为 `maintenance` 分支末版（`latest` 已转 10.11.0），装时报 deprecated 告警；按"逐包钉 NexBox 实测"纪律不擅自跳大版本，升级窗口随 NexBox 侧对账再议 |
| @eslint/js | 9.39.5（dev） | MIT | npm（dev） | flat config 的 `js.configs.recommended` 底座 | 🟢 | 2026-10-02 | 同上批 |
| typescript-eslint | 8.71.0（dev；NexBox lockfile 实测 8.70.0，`^8` 范围内） | MIT | npm（dev） | `recommendedTypeChecked` + 项目服务解析（`projectService`，覆盖 tsconfig 外的 `vite.config.ts`） | 🟢 | 2026-10-02 | 同上批；`no-explicit-any` 按编码规范 §一.1 显式设 error |
| eslint-plugin-react-hooks | 5.2.0（dev；钉 NexBox lockfile 实测同版） | MIT | npm（dev） | `rules-of-hooks` + `exhaustive-deps`（后者规范 §三 要求 error 且禁整文件禁用） | 🟢 | 2026-10-02 | 同上批 |
| prettier | 3.9.6（dev；钉 NexBox lockfile 实测同版，其 `package.json` 声明 `^3.5.3`） | MIT | npm（dev） | 提交前检查点之一：`prettier --check .` | 🟢 | 2026-10-02 | 同上批。`.prettierrc` 四字段照抄编码规范 §四（与 NexBox 一致）；`.prettierignore` 排除 `**/*.md`：Prettier 按最宽单元格重排中文表格，任何一次增删行都牵连整表抖动，与登记表窄列可读口径冲突 |
| `.gitattributes`（非依赖，登记于此） | — | — | 仓库配置 | 行尾口径固定：`* text=auto eol=lf`，`*.png/*.ico/*.icns binary` | 🟢 | 2026-10-02 | 用户拍板补上；覆盖本机 `core.autocrlf=true`。加后 `git ls-files --eol` 抽查 index/worktree 均 lf，无重规范化抖动 |
| @dnd-kit/core | 6.3.1（钉 NexBox `package.json` `^6.3.1`） | MIT | npm 引用 | 播放队列拖拽排序（F-MU-13 交互复用，PlayQueueDrawer） | 🟢 | 2026-10-02 | MU-2 曾裁"剔除 dnd-kit 不引依赖"；2026-10-02 用户下达音乐模块**完全复用铁律**后随 NexBox `PlayQueueDrawer` 移植解禁，版本按"逐包钉 NexBox 实测"纪律 |
| @dnd-kit/sortable | 10.0.0（钉 NexBox `^10.0.0`） | MIT | npm 引用 | 同上（useSortable / verticalListSortingStrategy） | 🟢 | 2026-10-02 | 同上批 |
| @dnd-kit/utilities | 3.2.2（钉 NexBox `^3.2.2`） | MIT | npm 引用 | 同上（CSS.Transform.toString） | 🟢 | 2026-10-02 | 同上批 |
| vitest | 5.0.3（dev，`^5.0.3`） | MIT | npm（dev） | 前端单测运行器（M-01 片1 引入，`npm test` = `vitest run`，并作为 pre-commit 第 4 条命令） | 🟢 | 2026-10-03 | OV-1 新增。🔴 **NexBox 无此依赖**（`grep -c vitest Source/NexBox/package.json` = 0）：本行不属"钉 NexBox 实测"纪律覆盖范围，系用户 2026-10-03 拍板"片1 一并引入 vitest"后按 npm 解析版本引入。连带新增 15 个传递包（`@vitest/mocker`/`@vitest/spy` 5.0.3、`chai` 6.3.0、`tinybench` 6.2.0、`tinyexec` 1.3.1、`magic-string` 1.4.2、`obug` 2.2.1、`std-env` 4.3.0、`why-is-node-running` 3.2.1、`assertion-error` 2.0.1、`expect-type` 1.4.0、`es-module-lexer` 2.3.2、`estree-walker` 3.0.3、`@types/chai` 5.2.3、`@types/deep-eql` 4.0.2），逐个查 lock 实测许可证均为 MIT 或 Apache-2.0，无 🟡/🔴 档。🟡 `vitest.config.ts` 采 `environment: "node"`（本片只需纯函数用例）；**未预埋 jsdom / @testing-library**——组件级用例开工时须按 ADR-003 五步另行登记 |

## 三、移植代码登记（ADR-003 §二.5）

| 来源 | 许可证 | 引入方式 | 落点 | 备注 |
|---|---|---|---|---|
| NexBox `src-tauri/src/music.rs` | GPL-3.0（与本工程同向） | 源码移植改造（非包引用） | `crates/services/src/music/meta.rs`、`scan.rs` | 文件头已注明来源；同向 copyleft 并入按 GPL-3.0 授权成立 |
| NexBox `src/components/VirtualList.tsx` | GPL-3.0（同向） | 源码移植（逻辑零改动，文案色值改令牌变量） | `src/features/MusicManager/components/virtual-list.tsx` | MU-1 前端片，2026-10-02 |
| NexBox `src/pages/MusicPage.tsx` 本地曲库区段（LocalSongRow/左卡两视图） | GPL-3.0（同向） | 源码移植改造（数据流改接 Aegis 命令面；播放类按钮留 MU-2） | `src/features/MusicManager/components/local-song-row.tsx`、`pages/music-page.tsx` | MU-1 前端片，2026-10-02 |
| NexBox `src/stores/music-store.ts` 本地切片 | GPL-3.0（同向） | 逻辑移植改造（localStorage 持久化删除，曲库单源在 Rust 侧） | `src/features/MusicManager/store/music-store.ts` | MU-1 前端片，2026-10-02 |
| NexBox `src/stores/music-store.ts` 播放切片（淡变引擎/playSong/togglePlay/队列历史/模式循环） | GPL-3.0（同向） | 逻辑移植改造（按 07 册 v1.2 偏离修正：三模式无 heartbeat、淡变 0.2s/0.25s、音量 0–100、R-11 移除当前曲不打断、清空队列同清历史；`convertFileSrc` 直读替代中转） | `src/features/MusicManager/store/playback-store.ts` | MU-2 片，2026-10-02 |
| NexBox `src/pages/MusicPage.tsx` 播放条/进度区段（PlayerBar/PlayerProgress） | GPL-3.0（同向） | 源码移植改造（色值令牌化；三态音量图标；R-13 失败显式上报；R-14 拖动让位） | `src/features/MusicManager/components/player-bar.tsx` | MU-2 片，2026-10-02 |
| NexBox `src/pages/MusicPage.tsx` QueuePanel 区段 | GPL-3.0（同向） | 源码移植改造（Chakra Drawer 承载🟡视觉偏离留 UI 优化批；无拖拽排序——规格未要求） | `src/features/MusicManager/components/queue-panel.tsx` | MU-2 片，2026-10-02 |
| NexBox `src/lib/karaoke-lyrics.ts` | GPL-3.0（同向） | 源码移植（parseYrc/parseLrcEnhanced/finalizeLineDurations/findActiveIndex/getLineProgress/calculateScrollOffset 逻辑零改动）+ 规格补写（`LYRIC_REWIND_S=0.5` 只作用歌词路径、活跃行前瞻 `+0.05s`、零宽/不可见字符清洗、LRC 译文就近挂靠 `matchTranslation`——NexBox 仅精确匹配） | `src/features/MusicManager/lib/karaoke-lyrics.ts` | MU-3 片，2026-10-02 |
| NexBox `src/components/KaraokeLyricLine.tsx` | GPL-3.0（同向） | 源码移植改造（RAF 逐字填充/溢出检测/双层渐变掩膜保留；去 store 耦合改 props；歌词时间回拨 0.5s 在此接入） | `src/features/MusicManager/components/karaoke-lyric-line.tsx` | MU-3 片，2026-10-02 |
| NexBox `src/components/KaraokeLyricsView.tsx` | GPL-3.0（同向） | 源码移植改造（自动滚动/防跳窗阈值 0.5s、2.0s、-1s 保留；歌词定位统一减 0.5s；字号 12–28 与 A−/A+ 属 F-MU-24 全屏播放器=MU-6，本批面板固定 18px） | `src/features/MusicManager/components/karaoke-lyrics-view.tsx` | MU-3 片，2026-10-02 |
| NexBox 歌词装载切片（歌词状态 + 代际守卫） | GPL-3.0（同向） | 逻辑移植改造（Aegis 新建独立 store：`loadSeq` 代际丢弃过期结果、装载期保留旧词防闪烁、四态原因文案 notFound/loadFailed/backendUnavailable、无词曲走 `setNoLyric` 免 IPC） | `src/features/MusicManager/store/lyrics-store.ts`、`components/lyrics-panel.tsx` | MU-3 片，2026-10-02（内嵌歌词回退 `read_embedded_lyric` 为规格补写，NexBox 无此级） |
| NexBox `src-tauri/src/music_api/netease.rs` | GPL-3.0（同向） | 源码移植改造（命令面改接 `CmdEnvelope`；凭据由 NexBox 明文 JSON 改 keyring 读取；`song_url` 保持 NexBox 三参签名，降级链走 `models.rs::quality_candidates_from`，与 QQ 的四参形态不同） | `crates/services/src/music/netease.rs` | MU-4 片，2026-10-02 |
| NexBox `src-tauri/src/music_api/qqmusic.rs` | GPL-3.0（同向） | 源码移植改造（端点/签名/降级策略零改动；🔴 协议级字面量 `tmeAppID:"qqmusic"`、`qqmusic_uin`/`qqmusic_key`、`*.stream.qqmusic.qq.com` 一律保留；**唯一有意偏离**=出站 `provider` 字段由 `"qqmusic"` 改规范值 `"qq"`，词表单源见 `auth::MusicPlatform::{from_slug,slug}`（Q-20 A 档）；音质模板恢复 NexBox 同源 5 档 `hires/lossless/exhigh/standard/aac`） | `crates/services/src/music/qqmusic.rs` | MU-4 片，2026-10-02；MU-4 收口批（同日）改 provider 出站 |
| NexBox `src-tauri/src/music_api/{models,cookie,crypto}.rs` | GPL-3.0（同向） | 源码移植改造（统一模型 + cookie 解析与登录位判定 + weapi/EAPI 加密；`normalize_quality` 保留） | `crates/services/src/music/{models,cookie,crypto}.rs` | MU-4 片，2026-10-02 |
| NexBox `src-tauri/src/music_api/audio_proxy.rs` | GPL-3.0（同向） | 源码移植改造（axum 本地反代 `/audio`(`/cover`)，Range/Referer/CORS 透传，解决在线曲 CORS 与封面防盗链） | `crates/services/src/music/proxy.rs` | MU-4 片，2026-10-02 |
| NexBox `src-tauri/src/music_api/mod.rs`「登录窗口 - 多平台」段 | GPL-3.0（同向） | 源码移植**拆分**：纯逻辑（cookie 优先级表/域名判定/弹窗完成判据/回跳清洗）下沉 `services::music::login_window`；宿主窗口编排留 app crate `src/music_login.rs`（🔴 已登记 [04 分册 §一](../Docs/开发文档/03-开发规范/04-文件与目录规范.md) 薄转发例外，新增窗口命令须同步该条）。🔴 关键对齐：同一 user-data-dir 下**全窗口 `additionalBrowserArgs` 必须逐字节一致**，参数口径与主窗口 `tauri.conf.json` 同源 | `crates/services/src/music/login_window.rs`、`src-tauri/src/music_login.rs`、`tauri.conf.json` | MU-4.15 片，2026-10-02（临时 `diag()` 诊断已在收口批全部删除） |
| NexBox `src/pages/MusicPage.tsx` 在线区段 + `src/stores/music-store.ts` 在线切片 | GPL-3.0（同向） | 源码/逻辑移植改造（搜索·歌单·推荐·榜单·登录态；🔴 凭据存储与词表归一按本工程红线重做；`playable` 不预拦截、由取地址结果裁定；平台切换重置 + 过期响应守卫为规格补写） | `src/features/MusicManager/online/`、`store/online-store.ts`、`types/music.ts` | MU-4.4~4.14 片，2026-10-02；🟡 UI 仍为"Aegis 适配版"，完整复刻留 P4（Q-22 ④→①→③） |
| Aegis 自建（无 NexBox 对应实现）：凭据加密层 | — | 新实现（DP-1 检索结论：NexBox 用明文 JSON `music-cookies.json`，触本工程"禁明文落盘"红线，属🔴 有意偏离而非可移植项；超长凭据自动分片 + `cookie.meta` 提交标记，规避 Credential Manager 单条 2560 字节上限） | `crates/services/src/music/auth.rs` | MU-4.1 片，2026-10-02；实现约束登记见 [02 专题设计 §5.3](../Docs/开发文档/02-专题设计/02-安全与特权通道.md) |
| NexBox `monitor/`（`Program.cs` `SensorReader.cs` `NexBoxMonitor.csproj`，net48 + LibreHardwareMonitorLib 0.9.6） | GPL-3.0（同向） | 源码移植改造（②）：stdio 行协议（`{"cmd":"read"}`/`{"cmd":"exit"}`）、`SensorVisitor`、NaN/Infinity JSON 兜底零改动；🔴 **摘除 PawnIO 分发/安装联动**（改判为"仅只读探测"，见本节下行与 §四）；输出字段按 Aegis DTO 需要裁剪 | `Aegis/monitor/{AegisMonitor.csproj,Program.cs,SensorReader.cs}`、`Aegis/tools/build-monitor.ps1` | M-01 片1（Q-12=B），2026-10-03；构建可复现性见 `spikes/K-Q12-1-monitor-build.md` |
| NexBox `src-tauri/src/sensor.rs`（传感器桥生命周期） | GPL-3.0（同向） | 源码移植改造（②）：15s Open 兜底 / 8s 读超时 / 30s 冷却 / 连败 5 次禁用的参数实测值沿用；🔴 三处有意偏离——① 母本 `WaitForSingleObject` FFI + `std::thread::sleep` 轮询 → 改读线程 + `mpsc::recv_timeout`（02 册 KN-04、clippy.toml 禁令）；② 母本全局 `lazy_static + Mutex<Option<..>>` → 状态收进 `OverviewState`（不留跨域全局可变量）；③ 母本 `find_monitor_exe` 方案G 的 `CARGO_MANIFEST_DIR` 编译期兜底 → 改向上回溯祖先目录。桥与传输原语拆为两文件（`infra::sidecar` 零业务判断） | `crates/infra/src/sidecar.rs`、`crates/services/src/overview/sensor_bridge.rs`（片2-0 起为该域门面，实物在 `sensor_bridge/{bridge,exe,types,matching}.rs`） | M-01 片1，2026-10-03；🟠 同日片2-0 因 500 行硬线拆为子模块，🔴 零行为变更（声明名集合 71/71 无增无丢） |
| NexBox `src-tauri/src/pawnio_driver.rs` **仅 L21-43 / L46-60**（只读注册表检测半区） | GPL-3.0（同向） | 源码移植改造（②）+ 🔴 **整段截断**：母本 L110 起 `Start-Process -Verb RunAs` 静默装驱动、以及随包分发 `resources/binaries/PawnIO_setup.exe`（`installer/pack-payload.ps1:72`）**一律不移植**——PawnIO 上游许可实测 GPL-2.0（=🔴 禁分发档，见 §四），且按需提权红线禁应用自发起 UAC。本工程采零分发/零安装/仅只读检测 + 纯文字引导（用户 2026-10-03 拍板） | `crates/infra/src/registry.rs` | M-01 片1，2026-10-03；🔴 取证口径：`grep -rn "PawnIO_setup" Aegis/` 与 `grep -rn "Verb RunAs" Aegis/src-tauri/crates` 须零命中（本批已实测，仅文档/注释命中） |
| NexBox `src-tauri/src/hardware.rs`（占用与容量采集） | GPL-3.0（同向） | ②改造移植：sysinfo 同库同版（`sysinfo = "0.30"`，lock 实测 0.30.13）的 CPU/内存/`Disks` 取数路径；存储卡口径采"卷（DriveInfo）单一来源、已用率取多卷最大值"（与 SystemToolkit 2026-09-02 起同向，R-06）；🔴 **不移植 `long_os_version`**：母本用 `libloading` + 手写 `RtlGetVersion` FFI 取内核版本，本工程禁手写 Win32 绑定（01 总览 §二），改采 sysinfo 的注册表实现（同读 `HKLM\...\CurrentVersion`，语义等价） | `crates/services/src/overview/live.rs`（片2-0 起为该域门面，实物在 `live/{storage,cards,status,sampling}.rs`） | M-01 片1，2026-10-03；型号精简（R-21）与回退链（R-07）为本域纯函数新实现，见 `badges.rs`；🟠 同日片2-0 因 500 行硬线拆为子模块，🔴 零行为变更（声明名集合 37/37 无增无丢） |
| SystemToolkit `src/SystemToolkit.Core/Utilities/NetAdapterFilter.cs` | GPL-3.0（同向，C#→Rust 重写） | 源码移植改造（②跨栈）：35 条垃圾特征黑名单**逐条照搬、未增未减**，每条保留母本的实机实证（2026-09-02 Killer WLAN + Hyper-V + 蓝牙机型；2026-09-10 火绒 NDIS 过滤驱动）；🔴 全工程唯一实现（R-20），描述与连接名两路都要查 | `crates/services/src/network/visibility.rs` | M-01 片1，2026-10-03；本批修回一处移植写反的幽灵网卡判据（禁用且仅 APIPA 的"干净名"真卡不得当驱动残留隐藏） |
| SystemToolkit `TopProcessSampler`（进程 CPU 差分采样思路） | GPL-3.0（同向） | ③仅思路复用（非代码移植）：两轮差分才出 CPU 值，🔴 拒绝 WMI `PerfProc` 路线（母本实测冷启动 6291ms）；新增口径：sysinfo Windows 进程 CPU 是"占一个核"语义，展示前先除逻辑核数再钳 0–100（R-17），PID 0 不入榜，无计数器上一值一律 `None`（R-08 禁 0% 冒充） | `crates/services/src/overview/processes.rs` | M-01 片1 提前建（原计划片3），2026-10-03 |
| SystemToolkit `TcpConnectionTable`（`GetExtendedTcpTable` + `MIB_TCPTABLE_OWNER_PID`） | GPL-3.0（同向） | ③仅思路复用（C#→Rust，非代码移植）：改走 `windows-sys` 0.59 官方绑定；🔴 刻意**不采本机/远端 IP**（F-05 数据视图只需端点总数 + 进程归属 + 监听端口摘要，隐私面收窄） | `crates/infra/src/net_tcp_table.rs`、`crates/services/src/overview/endpoints.rs` | M-01 片1 提前建（原计划片3），2026-10-03 |
| SystemToolkit `OverviewView.xaml`（547 行视觉树） | GPL-3.0（同向） | ③仅布局参照，🔴 **零代码/零 XAML 复制**：六分区流式结构、四卡 `UniformGrid Columns=4`、键值卡 `120\|*`、固定 5 行表、软件表 `MaxHeight` 有界视口等布局语义转写为 emotion styled + `--aegis-*` 令牌 | `src/features/Overview/pages/overview-page.tsx`、`src/ui/components/{stat-card,seg-bar,temp-badge,live-dot}.tsx` | M-01 片1，2026-10-03；线框经批准（plans/grand-pine-perch.md §一）后编码，满足 UI 先审后写 |
| NexBox `src-tauri/src/app_manager.rs`（「已安装应用扫描」半区：`UNINSTALL_KEY`/`score_text`/`read_reg_str_raw_decode`/`dedupe_key`/`scan_uninstall_key`/`list_installed_apps`） | GPL-3.0（同向） | ③同 API 思路复用（**非逐行移植**，函数签名重做）：`winreg` 三视图扫描（HKLM64 `Uninstall` + HKLM `Wow6432Node` + HKCU）、`SystemComponent=1` 剔除、多编码打分解码（UTF-16LE/UTF-8/GBK 三候选排序取最优，REG_MULTI_SZ 只取首段）、名称+版本跨来源去重、按小写名升序——口径与母本一致。🔴 **两处有意偏离**：① 新增 `is_update_or_tool`（R-16 剔除 Update/Patch/Hotfix/Redistributable/DirectX 等，母本**无此过滤**，实测 `app_manager.rs` 全文零命中 `update/patch/KB`）；② 大小单位：母本 `estimated_size_kb` 原样透传 KB，本工程归一为 `estimated_size_bytes`（展示层 `format_bytes` 单源）。🔴 **整段不移植**：`registry_source` 字段、`parse_display_icon`/`get_app_icons`、`open_app`、`uninstall_app` + `launch_elevated`（F-07 是只读清单；卸载属 F-13，且母本那两处提权触本工程红线） | `crates/services/src/overview/software.rs` | M-01 片4，2026-10-03；真机冒烟 `real_registry_scan_yields_user_apps_without_patches` 实测本机 **51 条**（非空 + 零补丁混入 + 去重后无重 + 行序稳定四项断言） |
| SystemToolkit `src/SystemToolkit.Core/Overview/Services/OverviewReportBuilder.cs`（89 行，Markdown 报告纯函数 `Build`） | GPL-3.0（同向，C#→TS 跨栈重写） | ③仅思路复用（零代码复制）。🟢 沿用的口径：四节顺序（硬件概览→硬件详情→系统概览→传感器快照）、`\| 项目 \| 值 \|` + `\| --- \| --- \|` 表式、`\|` 转义防破表、"传感器探测失败则**整节省略**不输出空表"、统计卡行式 `标题：大数字（副文案） · 说明 百分比`、🔴 R-15 不含已安装软件清单。🔴 **落点反转（本条最重要的偏离）**：母本在 Core 层用 C# 直接拼 Markdown，本工程**不能照此分层**——DTO 的 `KvRowDto.key` 按设计只装 i18n key（文案单源纪律），Rust 侧拼装必然把裸 key 写进用户文件；故拼装在 `src/features/Overview/lib/export-report.ts`（纯函数，vitest 15 例钉住），后端只留两个命令：`overview_sensor_table`（数据上行，`unit` 随读数透传避免重抄单位表）+ `overview_report_write`（仅落盘，走 `infra::fs::atomic_write`）。另有一处口径修正：说明段跟随 `bigValue` 的**来源**而非母本的 `Percent is {}` 门——R-07 温度顶位时没有进度条，报告仍须写明该数是温度而非占用率 | `src/features/Overview/lib/export-report.ts`、`src/features/Overview/hooks/use-export-report.ts`、`crates/services/src/overview/report.rs`（61 行，仅落盘） | M-01 片4，2026-10-03；保存/提示经 `@tauri-apps/plugin-dialog` 2.7.3 `save()`+`message()`（本工程**无** plugin-fs，故落盘必须走后端命令）；取消路径零副作用 |

| SystemToolkit `src/SystemToolkit.Core/FileTransfer/Models/TransferReasonCodes.cs` | GPL-3.0（同向，C#→Rust 重写） | ③仅思路复用（零代码复制）：**16 个原因码逐条照搬、未增未减**，中文说明**照抄母本 `Describe()` 措辞**（界面文案，实现者不得改写）。🔴 做成线协议契约（历史条目按字符串存储，改拼写即破坏兼容），带码表稳定性测试；母本"未知码原样返回不吞"以 `Option` + `display_text` 表达同一语义 | `crates/services/src/transfer/reasons.rs` | M-05 片1，2026-10-04；规格 05 册 F-FT-31 / §五「原因码对照」 |
| SystemToolkit `.../FileTransfer/Models/TransferText.cs` | GPL-3.0（同向，C#→Rust 重写） | ③仅思路复用：`MaxBytes = 256 KiB` 与 `PreviewLength = 120` 两常量同值照搬；"只拒不截"语义保留。🟠 **一处有意偏离**：母本按 C# `string.Length` 即 **UTF-16 码元**计"字"（emoji 算 2），本实现按 **Unicode 标量值**（Rust `chars()`）算（emoji 算 1）——理由是须与前端 `[...s].length` 逐值一致，否则同一串前后端显示两个字数；🔴 上限判据（UTF-8 字节数）不受影响，故偏离只影响展示不影响拒绝。残留：预览按标量值切、不按字形簇（母本亦只防代理对），防字形簇需新依赖 `unicode-segmentation`，本批不做 | `crates/services/src/transfer/text.rs` | M-05 片1，2026-10-04；规格 05 册 F-FT-15/32/35 |
| SystemToolkit `.../FileTransfer/Models/TransferConflictPolicy.cs` + `.../Services/TransferConflictResolver.cs` | GPL-3.0（同向，C#→Rust 重写） | ③仅思路复用：四策略（Rename/Ask/Skip/Overwrite）、` (n)` 后缀**插在扩展名之前**、`Skip` 终态为「已跳过」而非「已完成」、`WritesFile` 语义、"Ask 在无确认闸道时降级为改名"——口径逐条一致。🟠 一处登记：母本契约层默认枚举值实测为 `Rename`（判别值 0），与规格 F-FT-23「默认自动改名」及 house policy「绝不覆盖」三处一致，故 `ConflictPolicy::default()` 取 `Rename`；规格 F-FT-17 那句"默认策略为『每次询问』时…"是 Ask 模式**弹窗内的默认选项**，非出厂策略，已在 TASKS M-05 备注登记 | `crates/services/src/transfer/conflict.rs` | M-05 片1，2026-10-04；规格 05 册 F-FT-17/23 / R-10（唯一裁决点） |

## 四、C# 传感器 sidecar 的第三方二进制与内核驱动边界（M-01 本机概览，2026-10-03）

sidecar `Aegis/monitor/` 编译产物随包分发，其 NuGet 依赖逐个核可结果（档位按 [ADR-003 §三](../Docs/开发文档/04-决策/ADR-003-第三方代码引入与许可证规范.md) 判定顺序：🔴 命中即停 → 🟡 停并上报 → 🟢 登记）。许可实测来自 NuGet 包页与其声明的项目 URL，**非凭记忆**：

| 包（`project.assets.json` 实测版本） | 许可证 | 上游 | 档位 | 处置 |
|---|---|---|---|---|
| LibreHardwareMonitorLib 0.9.6 | MPL-2.0 | NuGet 包声明（本地双源互证：NexBox `installer/src/locales/LICENSE_EULA_zh.md:91`、SystemToolkit `Docs/50-决策/ADR-002-*.md:172`） | 🟡 | 文件级 copyleft：改动其源文件须开源——本工程**只引用不改其源码**，义务止于保留声明；随 Q-12-B 拍板于 2026-10-03 确认引入并记日期 |
| DiskInfoToolkit 1.1.2 | MPL-2.0 | Blacktempel/DiskInfoToolkit | 🟡 | 同上（LHM 磁盘 SMART 通道依赖，不可摘） |
| RAMSPDToolkit-NDD 1.4.2 | MPL-2.0 | Blacktempel/RAMSPDToolkit | 🟡 | 同上（内存 SPD 通道）；🔴 注意其 `-NDD` 变体经 **PawnIO/WinRing0 运行时接口**访问 SMBus（dll 字节取证见下），非自带驱动二进制 |
| BlackSharp.Core 1.0.7 | MPL-2.0 | blacktempel/BlackSharp | 🟡 | 同上（被 RAMSPDToolkit 引用） |
| HidSharp 2.6.4 | 🟡 **待核**：包页自述 "Apache open-source license"，未给标准 SPDX 表达式（无版本号） | software.seekye.com/hidsharp（作者 James Bellinger） | 🟡 | 按 ADR-003 §三 末行"版本歧义一律按 🟡"处理。LHM 的 HID 传感器通道依赖，运行期不可摘；**发布前须取得上游 LICENSE 文本并按实际版本回填本行**，取不到则 sidecar 须关闭 HID 类传感器后重新出包 |
| System.Text.Json 8.0.0 及 .NET 基础库若干 | MIT | dotnet/runtime | 🟢 | 保留声明即可 |

**内核驱动边界（🔴 本项目的硬闸，不得因"功能需要"放宽）**：

- **PawnIO = GPL-2.0（GitHub 官方 license API 实测 `spdx_id: "GPL-2.0"`，许可证文件名为 `COPYING`，路径 `raw.githubusercontent.com/namazso/PawnIO/master/COPYING`）**。逐字核实该文件＝纯官方 GPL-2.0 文本、**无项目自述的 "or later" 升级授权**，其 "any later version" 字样只出现在文件末尾标准的 "How to Apply" 模板段。故按最严档 **GPL-2.0-only = 🔴 禁止分发** 处理（与 ADR-003 §三 🔴 行举的 DriverStoreExplorer 同一处置先例：核实到 "or later" 授权前只作参考实现、零复制）。
- 🔴 **Aegis 侧结论：零分发 / 零安装 / 仅只读检测 + 纯文字引导**。不复制 `PawnIO_setup.exe`、不进安装包、不发起任何提权进程；未安装时在温度徽章处显示"未检测"并在诊断面板给一行说明 + 可复制的上游仓库 URL 纯文本（用户 2026-10-03 拍板"仅文字说明"，满足 [01 册 N-08](../Docs/功能规格/01-本机概览.md) 无外呼）。此路由与 🔴 禁档不冲突：**禁止的是分发，不是检测**。
- 🟡 **对 NexBox 的有意偏离**（DP-2 口径下必须显式登记）：NexBox 采「随包分发 `src-tauri/resources/binaries/PawnIO_setup.exe` + `Start-Process -Verb RunAs` 静默装驱动」（`src-tauri/src/pawnio_driver.rs:110`，`installer/pack-payload.ps1:72` 列入 payload），Aegis **一律不移植该段**；只移植其 L21 的只读注册表检测半区。
- 🔴 **取证口径（每批收口必跑）**：`grep -rn "PawnIO_setup" Aegis/` = 0 命中；`grep -rn "Verb RunAs" Aegis/src-tauri/crates` = 0 命中。
- 🟢 **"不分发驱动"的实证**：对 `LibreHardwareMonitorLib.dll` / `RAMSPDToolkit-NDD.dll` 做字节扫描，命中 `PawnIO`/`WinRing0` 处全部为 **interop 声明符号**（`IPawnIODriver`、`PawnIOSMBusIdentifier`、`RAMSPDToolkit.I2CSMBus.Interop.PawnIO`），`.sys` 关键字 **0 命中** → LHM 0.9.6 不自带内核驱动，走的是"运行时若已装 PawnIO 则用之"的探测路径。故随包这几个 dll **不构成 PawnIO 的分发**，只需在其 NOTICE 里声明 MPL 部件与"部分传感器需第三方内核驱动、本应用不含该驱动"。

## 五、NVML（`nvml.dll`）运行期加载边界与 K-004 单一传感器出口（M-01 片2-a，2026-10-03）

用户拍板②"显卡显存通道 = 追加 `nvml-wrapper`"落地时的约束。🔴 **先更正一处事实**：这**不是新增技术路线**——母本 `Source/NexBox/src-tauri/Cargo.toml:55 nvml-wrapper = "0.10"`（lock 实测 0.10.0）早已携带并使用，其实现在 `hardware.rs:741 get_nvidia_gpus_with_nvml()`（`:764` 取 `memory_info().total`、`:769` 温度、`:773` 负载、`:776` `sys_driver_version()`）。本节要收窄的是**移植范围**，不是许可档位。

- 🔴 **K-004 单一传感器出口在本域的切分**：NVML 只允许补**静态规格项**——显存**总量**（`memory_info().total`）与**驱动版本**（`sys_driver_version()`）。GPU **温度 / 负载 / 显存已用量** 🚫 一律只准来自 C# sidecar（`services::overview::sensor_bridge`），母本 `hardware.rs:769`/`:773` 两处读取**不移植**；🔴 也不得用 WMI `AdapterRAM` 顶替 NVML 的显存值（WMI 该项在 >4GB 卡上截断，正是母本改用 NVML 的原因，见其 `:1572` 注释）。
- 🔴 **不移植 `std::panic::catch_unwind`**：母本在 `:1073`/`:1228`/`:1571` 三处用 `catch_unwind` 兜 NVML 崩溃。本工程按 [02 分册](../Docs/开发文档/02-专题设计/02-安全与特权通道.md) 的"禁静默失败 + 以 `Result` 表达失败"口径，改走 `Nvml::init()` 返回的 `Result` 做 R-08 降级（显存/驱动版本显"未检测"），不用 panic 捕获作为常规控制流。
- 🟢 **零 NVIDIA 材料分发**：`nvml-wrapper-sys 0.8.0` 发布物内**只有** `src/bindings.rs`（bindgen 0.68.1 生成，`gen_bindings.sh` 带 `--no-doc-comments`）与 `src/lib.rs`，🔴 **无 `nvml.h`、无 NVIDIA 版权/许可文本**；运行期经 `libloading` 动态加载**系统目录里已存在**的 `nvml.dll`。故随包不构成对 NVIDIA 专有头文件/二进制库的分发：禁复制 `nvml.dll`、禁下载、禁进安装包。
- 🟢 **本机实证**：`C:\Windows\System32\nvml.dll` = 1,499,368 B（2026-09-19），`nvcuda.dll` 与 `ProgramData\NVIDIA Corporation` 同在 → 正向路径成立；无 NVIDIA 设备/无 `nvml.dll` 时显存与驱动版本显"未检测"，显卡卡整体按 [01 册 R-07](../Docs/功能规格/01-本机概览.md) 处理（负载与温度皆无则整卡缺席，不留空槽）。
- 🟡 **发布前义务**：`nvml-wrapper` / `nvml-wrapper-sys` 的发布物**不含 LICENSE 文件**（本地解包实测目录清单核实），而其 `Cargo.toml` 声明 MIT——NOTICE 须取上游仓库的 MIT 文本补齐。另有一条待确认项：`bindings.rs` 是 bindgen 从 NVIDIA 头文件派生的常量/签名面，是否构成需声明的许可材料，🔴 **待用户或法务确认**（详见 §一「片2-a 传递依赖逐查」）。
- 🔴 **每批取证口径**：`find Aegis -iname 'nvml*'` = 0 命中（不得出现任何 nvml 相关文件实体）；`grep -rn "temperature(" Aegis/src-tauri/crates/services/src/overview` 命中只准出现在 `sensor_bridge` 一侧。
