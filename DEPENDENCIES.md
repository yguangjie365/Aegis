# DEPENDENCIES — 依赖与许可证登记表（唯一清单）

> 口径事实源：[ADR-003 §二.2](../Docs/开发文档/04-决策/ADR-003-第三方代码引入与许可证规范.md)（档位表见其 §三，本表不复述）。
> 引入五步（02 分册 §6.2）缺一即违规；`NOTICE` 由本表生成。
> 版本一律记 **lockfile 实测值**（钉版纪律见 TASKS §五 "W0-01/W0-02 收口"条目；基线 = NexBox 实测）。
> 本表 2026-10-02 建表时一次性补登 W0 期已引入的直接依赖（登记日期如实标注）。

## 一、Rust 直接依赖（`src-tauri/Cargo.lock` 实测）

| 名称 | 版本（lock 实测） | 许可证 | 引入方式 | 用途 | 档位 | 登记日期 | 变更行 |
|---|---|---|---|---|---|---|---|
| tauri | 2.11.1 | MIT OR Apache-2.0 | crate 引用 | 应用宿主（features: protocol-asset, tray-icon） | 🟢 | 2026-10-02（补登 W0-01） | 建表补登 |
| tauri-build | 2.6.1 | MIT OR Apache-2.0 | crate（build-dep） | 构建期代码生成 | 🟢 | 2026-10-02（补登 W0-01） | 建表补登 |
| tauri-plugin-single-instance | 2.4.2 | MIT OR Apache-2.0 | crate 引用 | 单实例守卫 | 🟢 | 2026-10-02（补登 W0-01） | 建表补登 |
| serde | 1.0.x | MIT OR Apache-2.0 | crate 引用 | 序列化基座 | 🟢 | 2026-10-02（补登 W0-01） | 建表补登 |
| serde_json | 1.0.x | MIT OR Apache-2.0 | crate 引用 | JSON（缓存/IPC 载荷） | 🟢 | 2026-10-02（补登 W0-01） | 建表补登 |
| tauri-plugin-dialog | 2.7.1 | MIT OR Apache-2.0 | crate 引用 | 原生目录选择框（MU-1 导入文件夹） | 🟢 | 2026-10-02 | MU-1 新增；连带 tauri-plugin-fs 2.5.1（同许可，钉回 NexBox 基线） |
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

> 🟡 **MU-4 钉版偏差如实申报**：上表 6 个 crate（tokio / regex / futures-util / chrono / rand / sha1）实测版本**高于 NexBox lockfile 基线**，均为 `Cargo.toml` 声明区间内的依赖解析漂移，非本工程主动跳版本。🔴 tauri 全家桶（含 `tauri-plugin-*`）**零偏差**，符合"全家桶锁 NexBox"红线。收口动作 = 把 MU-4 这批声明补成 NexBox 实测精确版本或整体抬基线，**须用户拍板**（见 TASKS §三 Q-23）。许可证档位本身全部落在 ADR-003 允许档（MIT / Apache-2.0 / MIT OR Apache-2.0），无许可风险。

内部 crate（`contracts` / `infra` / `services` / `elevated-helper` / `worker`，path 引用）随仓库整体 GPL-3.0，不占本表。

## 二、npm 直接依赖（`package-lock.json` 实测）

| 名称 | 版本（lock 实测） | 许可证 | 引入方式 | 用途 | 档位 | 登记日期 | 变更行 |
|---|---|---|---|---|---|---|---|
| @tauri-apps/api | 2.11.1（exact） | MIT OR Apache-2.0 | npm 引用 | 前端宿主通道（invoke/event/window） | 🟢 | 2026-10-02（补登 W0） | 建表补登 |
| @tauri-apps/cli | 2.11.4（exact, dev） | MIT OR Apache-2.0 | npm（dev） | 构建 CLI | 🟢 | 2026-10-02（补登 W0） | 建表补登 |
| @tauri-apps/plugin-dialog | 2.7.3（exact） | MIT OR Apache-2.0 | npm 引用 | 目录选择器 JS 侧（MU-1） | 🟢 | 2026-10-02 | MU-1 新增 |
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
