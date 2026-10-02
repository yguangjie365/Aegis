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
