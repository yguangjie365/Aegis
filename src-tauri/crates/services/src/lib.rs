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
pub mod network;
pub mod overview;
pub mod sys;
