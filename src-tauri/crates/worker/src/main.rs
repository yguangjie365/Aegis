//! aegis-worker — 定时任务后台二进制骨架（01 总览 §4.3）。
//!
//! 计划承载：定时备份执行、错过补做、巡检类周期任务；同任务互斥（后到者提示"正在执行"）；
//! 🔴 不依赖 tauri 运行时与前端资源；无界面执行结果写本机日志文件、不回灌正在打开的 UI（规格 06 已知限制 16）。
//! 调度形态与 VSS 联动待 ADR（TASKS Q 组）。
//!
//! 本骨架仅声明入口：任务注册/执行链未落地前显式退出（禁止静默失败）。

use std::process::ExitCode;

fn main() -> ExitCode {
    eprintln!("aegis-worker: 骨架阶段（调度形态待 ADR），无任务可执行，退出码 78 (EX_CONFIG)");
    ExitCode::from(78)
}
