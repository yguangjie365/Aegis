# K-S03 探针结论：`<audio>` + asset 协议 Range 直读

> 关联：TASKS §四 K-S03 / Q-16（播放内核选型，✅ 已拍板）/ ADR-004 / 功能规格 07 册 F-MU-09~18
> 执行日期：2026-10-02 ｜ 执行人：Agent ｜ 判定：**🟢 GO**（9/9 PASS）

## 一、验证目标（TASKS §四 原文口径）

`<audio>` / 后端 Range 伺服的 **seek 精度与断流恢复实测**；**大文件不整读内存**。
本 spike 是 Q-16 拍板方案（前端 `<audio>` 播放 + Rust 侧本地中转）的**落地验证**——验证结果直接影响"中转"是否有必要存在（见 §五）。

## 二、方法

- 探针：`Aegis/src/features/MusicManager/spike/k-s03-selftest.tsx`，构建期开关 `VITE_SPIKE=K-S03` 注入（常规构建被摇树剔除，已验证产物无残留）。
- 运行：`VITE_SPIKE=K-S03 npx tauri build --no-bundle` → 启动 `target/release/aegis.exe`，面板自动执行；PrintWindow 后台截屏读取（不动用户前台窗口）。
- 内存旁证：PowerShell 每 2s 采样 `aegis.exe` 进程 WorkingSet/Private（`%TEMP%\aegis_ws.log`）。
- 样本：
  - 真实文件 `钟汶 - 赔偿.mp3`（9,134,226 B ≈ 8.7MB，CBR，lofty 时长 225.254s）；
  - 合成大文件（同曲拼接 ×24 = 219,221,424 B ≈ 209.1MB，测毕已删除并经重扫同步缓存）。
- 服务端：tauri 2.11.1 asset 协议 + **动态 scope**（`allow_asset_root`：导入根/启动恢复根逐个 `allow_directory(root, true)`，替代 NexBox 静态 `"scope":["**"]`，属安全收紧）。

## 三、结果（209.1MB 样本终轮，全 PASS）

| # | 断言 | 实测 | 判定 |
|---|---|---|---|
| T1 | Range 0-1 → 206 且 content-range 正确（动态授权生效） | `status=206 len=2 CR=bytes 0-1/219221424` | PASS |
| T2 | 中段 1KB 随机访问 | `start=109610712 status=206 len=1024 CR=bytes 109610712-109611735/219221424` | PASS |
| T3 | 越界 Range → 416 | `TypeError: Failed to fetch`（416 响应不带 ACAO，被 CORS 遮蔽——协议行为正确，见 §四-3） | PASS |
| T8 | 请求 2MB 被截断为 MAX_LEN | `status=206 len=1024000`（= `const MAX_LEN: u64 = 1000*1024`，asset.rs:119） | PASS |
| T4 | `<audio>` 时长 vs lofty | 真实 8.7MB 轮：`audio=225.227s lofty=225.254s`（Δ0.027s）；合成轮豁免（lofty 首帧预估≠帧计数） | PASS |
| T5 | seek 精度 50%/90%/25% | 三处 Δ 均 `0.000s`（判定 <0.05s） | PASS |
| T6 | 断流恢复：播放→尾段 seek→回跳 | 尾段推进 1.44s、回跳后续播 `paused=false` | PASS |
| T7 | 缓冲窗口 ≪ 全曲（不整读的直接前端证据） | `buffered=229.5s/5477.4s ≈ 8.8MB/209.1MB`（4.2%） | PASS |
| 内存 | Rust 主进程全程不随文件体积膨胀 | ws 恒 34.0–34.1MB、priv 恒 7.7–7.8MB（含 T6 播放期） | PASS |

源码侧硬保证（tauri-2.11.1 `src/protocol/asset.rs`，本地 registry 实测）：
1. Range 请求原生支持，单响应截断至 `MAX_LEN=1024000`（:119,133-134）；
2. `Accept-Ranges`/`content-range` 仅在 Range 请求上设置，且只有 `content-range` 进 `Access-Control-Expose-Headers`；
3. 越界 Range 返回 416，但 **416 响应不带 ACAO** → webview 内 fetch 抛 TypeError，前端无法读到 416 状态码；
4. ⚠️ **非 Range 的整 GET 会把整个文件读进内存**（`Vec::with_capacity(len)`）——这是本方案唯一需要靠前端纪律规避的路径。

## 四、结论与限制

1. **GO**：`<audio>` + asset 协议 Range 直读满足 F-MU-09~18 全部播放行为（seek 精度、断流恢复、缓冲窗口、内存平台期）。
2. **"Rust 侧本地中转"不再必要**：asset 协议本身即 Range 伺服端（206/416/1000KB 截断齐备），自写中转只会复制同一能力并扩大攻击面。→ 写入 ADR-004 决策与 Q-16 回填。
3. 416 的 CORS 遮蔽意味着前端**不能依赖状态码**区分"越界"与"网络失败"；播放错误路径按 R-13 显式上报即可，无需细分。
4. 🟡 限制（如实登记）：
   - 真实样本仅 1 个（mp3/CBR/8.7MB）；**flac / m4a / wav 未实测**（不同解码器缓冲策略可能不同）→ 在线/多格式批次（MU-4/5）前补测；
   - 209MB 样本为拼接合成，仅用于验证传输层，不代表真实曲库分布；
   - 小文件（<10MB）媒体栈会近乎全量预缓冲（8.7MB 轮 buffered≈83%）——每响应仍 ≤1000KB，非整读，可接受；
   - 多窗共用同一音源（TASKS K-S03 原始关注点）未在本地文件场景验证 → 留 MU-6 迷你窗批次探针。

## 五、复现

```bash
export PATH="$HOME/.cargo/bin:$PATH"
cd Aegis && VITE_SPIKE=K-S03 npx tauri build --no-bundle
./src-tauri/target/release/aegis.exe   # 面板自动执行；曲库需已导入 ≥1 首歌
```
