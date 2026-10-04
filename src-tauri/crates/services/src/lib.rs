//! services — 域服务与 Tauri command 定义处（04 分册 §一）。
//!
//! 一域一模块（`src/<域>/mod.rs`），域名以 [`01-技术栈与架构总览 §六 模块契约表`] 为唯一事实源
//! （overview / software / driver / network / transfer / backup / recovery(预留) / steam / music / settings，
//! 另加横切域 tasks；`sys` 为命令域词汇表成员，见 04 分册 §三）。
//! 命令适配层保持薄壳：纯域逻辑可被主程序 / elevated-helper / worker 三进程复用。
//!
//! 🔴 禁止引用 Tauri UI 宿主类型（Window / `WebviewWindow` / Menu / Tray）；
//! 注册由 `src-tauri` 壳的 `generate_handler!` 完成（W0-02）。
//!
//! ⚠️ 实测教训（W0-01）：`#[tauri::command]` 的 pub 命令**不得直接放在 crate 根 lib.rs**——
//! tauri-macros 对 pub 命令生成 `#[macro_export]`（宏名进 crate 根）+ 同作用域 `use` 自导入 → E0255。
//! 命令必须落在子模块内（本 crate 天然是"每域一模块"结构，勿为此再加平铺写法）。
//!
//! W0-01 只落 `sys` 域一枚接线验证命令，证明 services → 主 crate 注册链路可编译；
//! 契约化（envelope/错误包络生成物接线）随 W0-07 落地。

pub mod music;
// B-G1 Steam 主线（2026-10-03）：NexBox src-tauri/src/steam.rs 原样搬入（2188 行，
// 零 crate:: 跨模块依赖、零 Tauri 宿主类型），命令经 steam/mod.rs 再导出。
pub mod steam;

// 阶段一 100% 复刻（2026-10-03）：随 NexBox audio_engine.rs 搬入的两个本地 DSP 模块
// （FxSound Dattorro LexReverb 混响 / Spectrum Analyzer 频谱分析）
mod dattorro;
// 随 NexBox smtc.rs 搬入：媒体键（global-shortcut）注册
// 阶段一 100% 复刻（2026-10-03）：set_media_keys_enabled 需被根 crate 注册（同 external_player 的处理）
pub mod media_keys;
// 随 NexBox media_keys.rs 搬入：外部播放器探测（F-MU-56）
// 🔴 B3 工程适配（阶段一 100% 复刻）：NexBox 中 external_player 为 src/ 顶层私有 mod
// （其 lib.rs 内可直接注册 external_player::external_control）；Aegis 为 Cargo workspace，
// 根 crate 需跨 crate 引用，故此处改为 pub —— 属「工程分层让位」豁免范围（规格 §〇·0.4），
// 模块内部实现逐字未改。
pub mod external_player;
pub mod network;
pub mod overview;
mod spectrum;
pub mod sys;
