# GameManager — Steam 主线（B-G1）

> 复刻基线：**NexBox**（`Docs/功能规格/08-游戏管理.md` §〇 v1.1）
> 唯一例外：**游戏卡片视觉**参考 SystemToolkit `GameManagerView.xaml`
> 🔴 **游戏辅助不移植**（自瞄 / 自动点击 / 三角洲 / 游戏进程优化）

## 目录

```text
features/GameManager/
  pages/SteamPage.tsx              NexBox src/pages/SteamPage.tsx（1546 → 1479 行）
  components/game-card.tsx         SystemToolkit 卡片视觉唯一落地处（352 行）
```

后端在 `src-tauri/crates/services/src/steam/`（`steam.rs` 2188 行逐字未改 + `mod.rs`）。
命令注册在 `src-tauri/src/lib.rs`（19 个，**必须写全路径 `services::steam::steam::<名>`**）。

## 关键约定

| 项 | 约定 |
|---|---|
| i18n | **废弃**。`t` 是模块级纯函数（内置 56 键中文词典，取自 NexBox `zh.json` 的 `steam` 命名空间），签名与 i18next 一致，支持 `{{var}}` 与 `steam.` 前缀 |
| 图标 | `lucide-react`（UI 规范 §七）。NexBox 的 `react-icons/lu` `Lu*` 已去前缀 |
| 后端调用 | **必须**经 `@/lib/tauri/invoke` 封装，补 `module="Steam"` + `action` |
| 共享基建 | 走 `@/shared` 门面，**不要**直接 import `features/MusicManager/*`（会形成 feature 反向耦合） |
| 卡片令牌 | 🔴 SystemToolkit 的 `Brush_*` 名**不是** CSS 变量名，必须查 `game-card.tsx` 文件头的映射表 |
| 门禁 | `tsc` 不豁免；eslint 已按目录降级（见 `eslint.config.mjs` 的 game 块） |

## 参考文档

- 规格：`Docs/功能规格/08-游戏管理.md`（v1.1，§〇 复刻基线 / §0.1 两处待裁决冲突）
- 差异台账：`Docs/开发文档/02-专题设计/10-游戏模块100%复刻差异清单.md`（**阶段一改动前先看这份**）
- 调研报告：`Docs/开发文档/02-专题设计/09-游戏管理模块移植调研.md`（v2）
