# 系统优化（SystemOptimization）

> NexBox「内置工具」+「优化」两个模块的**全量搬迁**工程（Aegis = NexBox 换皮工程）。
> 复刻基线：`Source/NexBox/src/pages/BuiltinToolsPage.tsx`、`OptimizePage.tsx`
> 及其 37 个工具子页 + 共享组件 + Rust 命令面。

## 交付批次

| 批次 | 内容 | 状态 |
|---|---|---|
| **B0** | 模块骨架 + 两个入口页 + 共享组件 + 路由/导航/i18n/门禁接线 + 子页**诚实占位** | ✅ 本次交付 |
| B1 | 只读类子页（磁盘健康/屏幕测试/显卡改名/DLSS 预设/分辨率换算…） | 待启动 |
| B2 | 系统信息类（内存清理/显存/存储清理/启动项…） | 待启动 |
| B3 | 网络与调度类（网络优化/CPU 调度/电源管理） | 待启动 |
| B4 | 需管理员权限类（显式禁用，提权通道 W0-08 落地后接） | 阻塞于 W0-08 |
| B5 | Rust 命令面 + 新依赖许可证五步（`nvapi-wrapper` 等） | 阻塞于 ADR-003 |
| — | 🔴 **需澄清管辖边界**：`crosshair`（自瞄）、`autoclicker`（自动点击）、`game-process-optimize`（游戏进程优化）落在功能规格 08 §明确不做（用户 2026-10-03 裁决「游戏辅助一律不移植」）清单内。该裁决是否及于本模块（其上下文为游戏管理模块的 NexBox 基线）**尚未澄清** ⇒ 已标 `blockedBySpec` 保留卡片，不搬入也不删除，待你裁定 |

### B5 验收条件（勿漏）

1. **`LOG_MODULES` 两侧同步**：`src/lib/logging/index.ts` 与
   `crates/contracts/src/lib.rs` 的 `LOG_MODULES` 各加一项（建议 `SysOpt`），并同步
   `contracts/src/lib.rs` 中 `assert_eq!(LOG_MODULES.len(), 12)` → `13`。
   🔴 该测试逐字比对前后端两份表：**只改一侧必定打红**（当前两侧同为 12，故为绿）。
2. **invoke 参数形状**：Rust 侧统一带 `envelope: contracts::CmdEnvelope`（DoD 要求
   correlationId），**不走**复刻域的「裸签名 + 前端裸调」那条路（与 Aegis envelope
   纪律冲突）；前端一律经 `@/lib/tauri` 封装。
3. **特权面**：任何写系统设置 / 注册表 / 服务的命令，必须先落 ADR-002 的按需提权通道
   （verb 白名单 + 命名管道 + 一次性令牌），**不得**沿用 NexBox 的 inline `runas`
   （`ShellExecuteExW` + `is_admin()` 直调）——Aegis 明确禁该形态。

## 目录

```
src/features/SystemOptimization/
├── pages/
│   ├── builtin-tools-page.tsx   # 内置工具入口（复刻 BuiltinToolsPage.tsx）
│   ├── optimize-page.tsx        # 系统优化入口（复刻 OptimizePage.tsx）
│   └── tool-placeholder-page.tsx# 子页诚实占位（B1~B5 逐个替换）
├── components/                  # 模块私有组件（ViewGrid/ViewList/LayoutToggle/PressTilt）
├── lib/
│   ├── tools.ts                 # 37 项工具登记表（路由派生 + 状态 + 提权标记）
│   ├── store.ts                 # 模块私有排序持久化（独立 LazyStore 文件）
│   └── build-flags.ts           # 商店版标志占位（Aegis 恒 false）
├── types/view-types.ts          # ViewItem 契约（复刻）
└── assets/                      # nvidia.png / cs-logo.png（品牌图标）
```

## 复刻纪律

- **共享基建走 `@/shared` 门面**：`LiquidGlassCard` / `useBackground` / `useThemeColor` /
  `useAdaptiveTextColor` 一律从 `@/shared` 取，**禁**直接 import `features/MusicManager/*`
  （否则形成 feature → feature 反向耦合，违反 `src/shared/index.ts` 的裁决）。
- **模块私有组件不得被外部 import**（`components/` 下四个组件仅本模块使用）。
- 页面/组件为 NexBox 原件逐字搬入，仅 import 路径与下表所列差异改动。
- **lint 豁免**：复刻域口径与 MusicManager 一致（`eslint.config.mjs` 豁免块 + `eslint.strict.mjs`
  ignores + `.husky/pre-commit` 的 `--ignore-pattern` 三处同步登记）。
- **无独立 tsconfig 豁免**（与音乐域不同）：本模块消费 `@/shared` 门面（其实现落在
  MusicManager 域），若为本模块单开 `tsconfig.*-replica.json`，会触发 TS6307
  （「文件不在本 project 的 file list 内」）把整个音乐域拖进 project 引用链。
  故留在**主 tsconfig** 内并逐条消除原文遗留的 strict 违规（当前 4 处，已清零）。
  B1~B5 若某批子页为自包含（不消费 `@/shared`）且 strict 违规较多，再按批评估豁免。

## 与 NexBox 的差异（逐条登记）

| # | 差异 | 原因 |
|---|---|---|
| 1 | 卡片路由前缀 → `/system-optimization/tools/<id>` | 避让 Aegis 既有路由（`/app-manager` = 软件管理模块等），用户 2026-10-07 裁决「改名并完整搬入」 |
| 2 | `defaultTools` 从页面内联数组抽出到 `lib/tools.ts` | 占位页需按 id 反查工具；后续批次单点维护状态 |
| 3 | 排序持久化改用模块私有 `system-optimization.json` + `aegis_sysopt_*` 键 | NexBox 与音乐域共用 `settings.json`，Aegis 不跨域写同一文件。**附带收益**：不与背景/主题上下文共写同一文件 ⇒ 从根上规避「跨域覆盖 + 两种写节奏（无防抖 `saveOrder` vs 250ms 合并 flush）」的耦合风险，而非仅规避调用期覆盖 |
| 4 | 移除 `IS_STORE_BUILD` 条件分支 | Aegis 无 MSIX 商店版，该标志恒 false；保留 `lib/build-flags.ts` 占位以便将来恢复 |
| 5 | 横切件走 `@/shared` 门面 | 见「复刻纪律」 |
| 6 | 子页当前为**占位页** | B1~B5 逐批迁入；占位页如实标注状态，不做死链 |

## 门禁登记（复刻域三处同步）

| 位置 | 登记内容 |
|---|---|
| `eslint.config.mjs` | 新增 `SystemOptimization` 豁免块（与音乐/游戏域同口径） |
| `eslint.strict.mjs` | 复刻域 ignores 增加本模块目录 |
| `.husky/pre-commit` | `--ignore-pattern` 增加本模块；复刻域 error 检查路径增加本模块 |
| `.worknotes/scan-all-backtick.cjs` | `EXCLUDE_DIRS` 增加本模块 |

🔴 已知机制债：`scan-all-backtick.cjs` **不在 git 版本控制内**（`.worknotes/` 被忽略），
故该登记只在本地生效；而音乐/游戏/系统优化三个复刻域的反引号风险目前**全靠这一条本地
exclude 兜底**。若守卫机制要长期有效，需把该脚本（或门禁接线）纳入版本控制 —— 属
既有机制债，非本模块引入，建议随门禁专项一并处理。

## 已知阻塞

1. **提权通道**：`elevated-helper` 为 18 行空壳（无 IPC / verb 白名单 / 令牌），
   需管理员权限的工具在通道落地（W0-08）前**操作区禁用**，不得假装可用。
2. **Rust 命令面**：Aegis 无 `process` / `systeminfo` / `computer` 等价 crate，
   NexBox 对应约 8000 行后端需按 Aegis 分层新建；`nvapi-wrapper` 等新依赖须过
   ADR-003 许可证五步。