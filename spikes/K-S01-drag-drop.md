# K-S01 探针结论：Tauri WebView 拖放 + 原生文件路径获取

> 关联：TASKS §四 K-S01（🟡 2026-10-04 由 dsh（互传批）认领）/ M-05 片1 / 功能规格 05 册 F-FT-11、F-FT-12 / [RISK-5](../../../Docs/开发文档/00-项目总控.md)
> 执行日期：2026-10-04 ｜ 执行人：dsh（互传批）｜ 判定：**🟢 GO**（T10/T11 实测通过；2 项实现纪律见 §四）

## 一、验证目标（TASKS §四 原文口径）

**WebView 内拖放 + 原生文件路径获取（含跨盘、长路径、非 ASCII）实测通过；延迟与多次拖放稳定。**

失败时的替代路径（原文）：走 Tauri 原生插件/系统对话框（选择器代替拖放），功能规格相应降级。

## 二、环境与方法

**环境**（实测值，非记忆）

| 项 | 值 | 出处 |
|---|---|---|
| tauri | **2.12.1** | `src-tauri/Cargo.lock` |
| wry | **0.57.0** | 同上 |
| 主窗配置 | `label=main`、`dragDropEnabled` 见 §三 起始值 | `src-tauri/tauri.conf.json:15,28` |
| 调试通道 | `WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS=--remote-debugging-port=9222`（**环境变量注入，不改仓库**） | 本轮 |
| `withGlobalTauri` | **未开启** → 页面无 `window.__TAURI__`，须 `import()` 动态引入 | 探针实测 |
| 系统长路径策略 | `LongPathsEnabled = 0` | `HKLM\SYSTEM\CurrentControlSet\Control\FileSystem` |

**样本**（`D:\MyProject\MyAPP\.ks01-test\`，测毕保留供复现）

| 样本 | 路径长度 | 用途 |
|---|---|---|
| `中文文件名 带空格 与emoji 🚀.txt` | 52 | 非 ASCII + 空格 + 代理对（emoji） |
| `C:\...\Temp\ks01-c-drive\跨盘样本-C盘.txt` | 60 | 跨盘（D: → C:） |
| `…\很长的目录名称段-01..12\长路径样本.txt` | **363** | 逼近/超过 MAX_PATH |

**探针**（已收进仓库 `Aegis/spikes/K-S01-probes/`，与 K-S03 惯例一致；样本因含长路径不宜入库，用生成脚本复现）

| 探针 | 手段 | 文件 |
|---|---|---|
| A | CDP `Input.dispatchDragEvent` 注入带 `files`（真实绝对路径）的拖放，捕获 DOM `drop` 的全部可见信息 | `spikes/K-S01-probes/probe-a-dom-path.mjs` |
| B | `rustc` 单文件程序：Rust std 对长路径/非 ASCII/跨盘路径的 `metadata` + `read` | `spikes/K-S01-probes/probe-b-longpath.rs` |
| B2 | 同上，测 `fs::canonicalize` 是否泄漏 `\\?\` 前缀 | `spikes/K-S01-probes/probe-b2-canon.rs` |
| C-武装 | CDP 注入 `tauri://drag-enter/over/drop/leave` 与 `getCurrentWebview().onDragDropEvent` 监听器，等**真实 OS 拖放** | `spikes/K-S01-probes/probe-c-arm.mjs` |
| C-读取 | dump 捕获结果并直接给 T10/T11 判定 | `spikes/K-S01-probes/probe-c-dump.mjs` |
| 样本生成 | 重建 3 个样本（长路径样本手工造很麻烦） | `spikes/K-S01-probes/make-samples.ps1` |

## 三、结果

起始配置 `dragDropEnabled: false`（本轮临时改为 `true` 做对照，见 T8/T9）。

| # | 断言 | 实测 | 判定 |
|---|---|---|---|
| T1 | `dragDropEnabled:false` 时，DOM `drop` 能拿到**原生绝对路径** | `hasPathProp=false`、`pathValue=null`、`ownProps=[]`、`webkitRelativePath=""`（2 个文件皆然） | 🔴 **FAIL** |
| T2 | DOM `drop` 至少能拿到文件元数据 | `fileCount=2`；`size=24/20`、`type=text/plain`、`lastModified` 有值 | ✅ PASS（但无路径） |
| T3 | `DataTransferItem.getAsFileSystemHandle` 可作路径替代 | 可用，但只给出 `handle.name`，**无绝对路径** | 🟡 不足以替代 |
| T4 | `dragDropEnabled:true` 时 DOM 仍能收到 drop（CDP 注入法） | 与 `false` 时**完全一致**（`fileCount=2`，仍无 path） | ⚠️ **该法不可用于本判定**（见 §四-4） |
| T5 | Rust std 读 >MAX_PATH 长路径（363 字符，`LongPathsEnabled=0`） | **直接可读**（`metadata=23B`、`read_bytes=Some(23)`），无需手写 `\\?\` 前缀 | ✅ **PASS（意外之喜）** |
| T6 | Rust std 读非 ASCII + emoji 路径 | 可读（`metadata=24B`） | ✅ PASS |
| T7 | Rust std 读跨盘路径（D:→C:） | 可读（`metadata=20B`） | ✅ PASS |
| T8 | `fs::canonicalize` 是否泄漏 `\\?\` 前缀（会显示到界面） | **会**：`D:\…`（52）→ `\\?\D:\…`（56）；`PathBuf::from`/`join` 则不加 | 🟡 **陷阱确认** |
| T9 | `dragDropEnabled:true` 是否导致页面回归 | 无：`/overview` 正常、`nav a` 10 项、React root 在、控制台无新增错误 | ✅ PASS |
| T10 | **真实 OS 拖放** → `tauri://drag-drop` 交付**原生绝对路径** | ✅ 10 次真实拖放，每次 `payload.paths = ["D:\\MyProject\\MyAPP\\.ks01-test\\中文文件名 带空格 与emoji 🚀.txt"]`（len=52，中文/空格/emoji 原样）；另带 `position:{x,y}` 物理像素 | ✅ **PASS** |
| T11 | `dragDropEnabled:true` 是否真的拦住 HTML5 拖放 | DOM 侧 **0 条事件**（enter/over/drop 全为 0）；Tauri 侧 enter 20 / over 7652 / drop 20 | ✅ **确认拦截** |
| T12 | `drag-over` 事件的频率（"延迟"判定标准的实质） | **7652 次 / 32.2 秒 ≈ 238 次/秒**（约每帧 4 次，每次跨 IPC） | 🟠 见 §四-5 |
| T13 | 一次拖放会收到几次 `drag-drop` | **2 次**，同一毫秒、载荷完全相同（10 次物理拖放 → 20 次事件） | 🔴 见 §四-6 |
| T14 | 多次拖放稳定性（TASKS 判定标准后半句） | 连续 **10 次**拖放全部正确交付、无丢失/错序；路径与 position 每次正确 | ✅ PASS（部分覆盖） |

## 四、结论与限制

### 已成立（可直接作为设计依据）

1. 🔴 **`dragDropEnabled: false` 下拿不到原生路径。** DOM 的 `File` 对象只有 name/size/type/lastModified，**没有任何路径信息**，且 `getAsFileSystemHandle()` 也只给名字。
   ⇒ **F-FT-11「拖拽发送到指定设备」在当前配置下不可能实现**（后端要按路径流式读盘 + 断点续传 + 尺寸预检）。
2. 🟢 **Rust 侧的路径能力齐备**：长路径（363 字符）、非 ASCII + emoji、跨盘全部通过。障碍**只在"如何拿到路径"这一步**，不在处理链路。
   - Rust 1.99 的 `std::fs` 内部已做 verbatim 归一化，**不需要**我们手写 `\\?\` 前缀。这是本轮最有价值的意外发现——省掉一层易错的手工归一化。
3. 🟡 **`canonicalize` 会把 `\\?\` 前缀泄漏到调用方**（T8）。凡把 canonicalize 的结果送上界面（如"接收目录：…"）或写进日志/历史，都会显示成 `\\?\D:\…`。
   ⇒ 落到实现纪律：**显示用路径不得来自 `canonicalize`**；安全比对若必须 canonicalize，出界面前须剥前缀。
4. 🟠 **方法学结论（本轮踩到）**：**CDP 合成拖放无法用于验证本 spike 的关键项。** 注入的拖放直接走渲染进程，**绕过 wry 在 OS 层的拦截**，因此：
   - 既不能用来验证"Tauri 原生事件能否交付路径"（T10）；
   - 也不能用来验证"开启后 HTML5 是否被拦"（T11）。
   两项**都必须真实 OS 拖放**。→ 写入本 spike 供后续复用，避免他人重复踩。
5. 🟠 **`tauri://drag-over` 的频率是 ~238 次/秒**（T12：7652 次 / 32.2 秒），即**约每帧 4 次**，且每次都跨 IPC 到前端。
   ⇒ 实现纪律：**drop 目标高亮不要直接挂在 `over` 上逐次 setState**（会以 4×帧率触发 React 渲染）。要么在前端用 DOM 的 `dragover`（但 T11 表明 DOM 收不到）、要么在 `over` 处理器里做**位置比对去重**（位置未变则直接 return），要么干脆不做拖拽高亮、只在 `enter` 时高亮一次。
6. 🔴 **一次物理拖放会收到 2 次 `drag-drop`**（T13）。**根因已在 Tauri 2.12.1 源码坐实，非猜测**：
   - `tauri-2.12.1/src/manager/webview.rs:758` → `webview.emit_to_webview(DRAG_DROP_EVENT, &payload)?`
   - `tauri-2.12.1/src/manager/window.rs:253-259` → 因 `window.is_webview_window()` 为真，走 `window.manager().emit_to(EventTarget::labeled(window.label()), DRAG_DROP_EVENT, …)`
   - 而 `@tauri-apps/api/event` 的 `listen()` 默认 `target: { kind: 'Any' }`，`Webview.listen()` 用 `{ kind: 'Webview', label }` —— **两者都匹配上述两次投递**（WebviewWindow 的 window 与 webview 同 label）。
   ⇒ **实现纪律（必须做，否则一次拖放入队两个传输任务）**：drop 处理器必须**幂等/去重**。
   建议判据：以 `(paths.join('\u0000'), position.x, position.y)` 为键，**同一毫秒内重复到达即丢弃**；或处理完一次 drop 后开启 ~100ms 抑制窗。
   ⚠️ 我实测的是"两个监听器各收 20 条"（10 次拖放）——**每个监听器都收到两遍**，所以**换用官方 `onDragDropEvent` 并不能规避**，必须自己写去重。

### 🔴 需要裁决的一处文档口径（我未擅自改文档）

[`Docs/开发文档/01-产品与架构/01-技术栈与架构总览.md:205`](../../../Docs/开发文档/01-产品与架构/01-技术栈与架构总览.md) 原文：

> 🔴 Tauri `dragDropEnabled` 会拦截 HTML5 原生拖放：FileTransfer / MusicManager 等需要 HTML5 拖放的页面，须在**窗口级关闭该选项并自行接管 drop**——列入窗口配置守卫核对项。

与实测对照后，该表述**方向可疑**：

| 页面需求 | 正确配置 | 理由 |
|---|---|---|
| 要**文件路径**（互传发送、备份选目录） | `dragDropEnabled: **true**` | 只有 Tauri 原生事件给绝对路径；关闭=拿不到路径（T1） |
| 只要**页内拖排**（曲库排序、卡片重排），不需要路径 | `dragDropEnabled: **false**`（或改用指针事件） | 关闭后 HTML5 DnD 可用 |

即"关闭该选项"**并不是**"需要路径"的页面的解法，恰恰相反。该句把两类需求混成一句，且把 FileTransfer 归到了会**失去路径**的那一侧。
**建议**：由用户/主控裁决后回改该句，并同步 `00-项目总控.md` RISK-5 的表述。**本轮只登记，不擅自改。**

### 当前配置的处置

`src-tauri/tauri.conf.json:28` 已由 `false` **临时**改为 `true`（本轮 spike 用）。
🟢 连带确认：**当前应用内零 HTML5 拖放用法**（`grep` `src/` 全仓无 `onDrop`/`dataTransfer`/`dragover`），故翻面对现有功能**无回归**（T9）。
⇒ **T10/T11 已通过，建议保持 `true`**，并把"窗口配置守卫核对项"按上表重写：
   - `dragDropEnabled: true` = 要**路径**（互传/备份）+ **丧失** HTML5 拖排
   - 未来的游戏库/曲库**拖排**若要做，须改用**指针事件**（pointerdown/move/up）实现，不能指望 HTML5 DnD。
   - 🔴 若真出现"同一页既要路径又要拖排"，本 spike **未评估**该冲突（§六-3）。

## 五、复现

```bash
# 1) 生成样本（脚本在仓库内；样本本身含长路径，不宜入库）
pwsh -File Aegis/spikes/K-S01-probes/make-samples.ps1

# 2) 带调试端口起 dev（环境变量注入，不改仓库）
cd Aegis
export npm_config_cache='D:/MyProject/MyAPP/.tmp-npm-cache'
export WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS='--remote-debugging-port=9222'
npx tauri dev

# 3) 测 A（DOM 侧，无需人工）
node spikes/K-S01-probes/probe-a-dom-path.mjs

#    测 B / B2（Rust 侧，无需人工）
export PATH="$HOME/.cargo/bin:$PATH"
rustc -O -o /tmp/probe-b.exe spikes/K-S01-probes/probe-b-longpath.rs && /tmp/probe-b.exe <非ASCII路径> <跨盘路径> <长路径>
rustc -O -o /tmp/probe-b2.exe spikes/K-S01-probes/probe-b2-canon.rs && /tmp/probe-b2.exe <非ASCII路径> <长路径>

# 4) 测 C（需人工）：武装监听器后，用资源管理器把样本拖到 Aegis 窗口上（可连续拖多次）
node spikes/K-S01-probes/probe-c-arm.mjs
node spikes/K-S01-probes/probe-c-dump.mjs
```

## 六、残留与后续

1. ✅ T10 / T11 / T14 已于 2026-10-04 由真实拖放验证通过（10 次连续拖放）。
2. 🟡 **未测**：拖放**延迟**（从松手到前端收到事件的耗时）——本轮只测了事件**频率**（T12），未测单次端到端延迟。片1 实现时若发现"松手后要等一会儿才入队"再补测。
3. 🟡 "同一页既要路径又要页内拖排"的冲突**未评估**（若出现，需按窗口拆分或拖排改用指针事件）。
4. 🟠 若最终采用 DOM 路线（拿不到路径时的备选），需另行评估"把文件内容经 IPC 流式传给 Rust"的性能与断点续传可行性——**本 spike 未做该评估，且当前证据表明不必走这条路**（T10 已给路径）。
5. 🟠 **只测了单文件拖放**。"一次拖多个文件"（F-FT-11 要求"一次拖多个文件即多个任务"）**未单独验证**——`paths` 是数组、机制上支持，但未实测。
6. 🟡 未测**拖放目录**（`payload.paths` 里是目录时的行为）。参照工程对目录有 `allow_directory` 分支（`webview.rs:751` vs `window.rs:243` 的 `recursive` 参数还不一致），片1 若支持目录需单独验。
