//! 硬件信息报告写出（M-01 片4 / F-11，2026-10-03）。
//!
//! **复用结论（DP-1）**：
//! - 报告章节结构与表格转义口径 ← `SystemToolkit` `OverviewReportBuilder.cs`（③思路复用，非代码移植）；
//! - 原子写 ← `infra::fs::atomic_write`（与 `snapshot.rs` 同一把刀，断电不产生半截文件）。
//!
//! 🔴 落点裁定：母本的构建器是纯函数、入参里的 Label/Key **已是解析好的中文文案**；
//! 本仓的 [`dto::KvRowDto`] 刻意只带 i18n 键（"文案归前端"，见 `dto.rs` 头注），
//! 故 **Markdown 组装放前端** `src/features/Overview/lib/export-report.ts`，
//! 本文件只负责把前端拼好的文本按用户选定的路径落盘。
//!
//! 🔴 R-15 报告范围：硬件概览 + 硬件详情 + 系统概览 + 传感器快照（有读数才输出），
//! **不含已安装软件清单**——该裁剪在前端组装时执行，本层不设防。

use std::path::Path;

/// 把已组装好的报告文本写到用户选定的路径。
///
/// 路径由前端 `plugin-dialog` 的另存为框产出（🔴 本侧不拼路径、不猜目录）。
///
/// # Errors
///
/// 建目录或原子写失败时返回原因（由界面弹窗呈现，F-11「失败弹窗给出原因」）。
pub fn write_report(path: &Path, content: &str) -> Result<(), String> {
    infra::fs::atomic_write(path, content.as_bytes())
        .map_err(|error| format!("写出报告失败: {error}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn write_report_creates_file_with_exact_content() {
        let dir = std::env::temp_dir().join("aegis_test_report_exact");
        let path = dir.join("report.md");
        std::fs::remove_dir_all(&dir).ok();

        write_report(&path, "# 硬件信息报告\n\n- 计算机：TEST-PC\n").expect("写出应成功");
        let text = std::fs::read_to_string(&path).expect("文件应在场");
        assert!(text.contains("硬件信息报告"));
        assert!(text.contains("TEST-PC"));

        std::fs::remove_dir_all(&dir).ok();
    }

    /// 覆盖写：第二次写不得留半截旧内容（原子写的换名语义）。
    #[test]
    fn write_report_overwrites_previous_content() {
        let dir = std::env::temp_dir().join("aegis_test_report_overwrite");
        let path = dir.join("report.md");
        std::fs::remove_dir_all(&dir).ok();

        write_report(&path, "旧内容旧内容旧内容旧内容旧内容").expect("首次写应成功");
        write_report(&path, "新").expect("二次写应成功");
        let text = std::fs::read_to_string(&path).expect("文件应在场");
        assert_eq!(text, "新");

        std::fs::remove_dir_all(&dir).ok();
    }
}
