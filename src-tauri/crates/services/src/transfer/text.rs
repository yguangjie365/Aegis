//! 文本通道的单条上限、字/字节统计与历史预览（F-FT-15 / F-FT-32 / F-FT-35）。
//!
//! 复用结论（DP-1）：③思路复用 ←
//! `Source/SystemToolkit/src/SystemToolkit.Core/FileTransfer/Models/TransferText.cs`
//! （GPL-3.0 同源，ADR-003 五步登记见 `Aegis/DEPENDENCIES.md §三`）。
//!
//! ## 🟠 一处有意偏离：怎么数"字"
//!
//! 母本是 C#，`string.Length` 数的是 **UTF-16 码元** —— 一个 emoji（代理对）算 **2**。
//! 本实现数 **Unicode 标量值**（Rust `chars()`），同一个 emoji 算 **1**。
//!
//! 选后者的理由不是"更对"，而是**两端口径必须逐值相同**：前端展示同一串时用 `[...s].length`
//! （JS 码点迭代），它与 Rust `chars()` 的结果**完全一致**；若照搬 UTF-16 码元，则
//! `"🚀".length === 2` 但 `[...'🚀'].length === 1`，同一串在前后端会显示两个"字数"。
//!
//! 🔴 **上限判据不受此偏离影响**：256 KB 按 **UTF-8 字节数**判（F-FT-32 原文："按 UTF-8 字节数判定
//! （不按字符数）"），字节数与怎么数"字"无关。故本偏离只影响**展示**，不影响**拒绝**。
//!
//! 🟡 残留：预览截断按**标量值**切，不按**字形簇**（grapheme cluster）。带 ZWJ 的合成 emoji
//! （如 👨‍👩‍👧）仍可能被切成半个字形簇。母本同样只防代理对、不防字形簇；要防需引入
//! `unicode-segmentation`（新依赖，须走 ADR-003 五步），**本批不做**，已登记为已知限制。

/// 单条文本上限：**256 KiB，按 UTF-8 字节数判定**（F-FT-32，母本 `TransferText.MaxBytes` 同值）。
pub const MAX_BYTES: usize = 256 * 1024;

/// 历史条目里文本预览的字符数上限（F-FT-35，母本 `TransferText.PreviewLength` 同值）。
pub const PREVIEW_CHARS: usize = 120;

/// 校验结论的三态（F-FT-32：空内容与超限都是**拒绝**，且**绝不静默截断**）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TextValidationKind {
    /// 可发送
    Valid,
    /// 空内容 → 拒绝，文案见 [`error_text`]
    Empty,
    /// 超出 [`MAX_BYTES`] → 拒绝，文案见 [`error_text`]
    TooLong,
}

/// 一次校验的完整结论：三态 + 双口径计数（界面需要同时显示字与字节）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TextValidation {
    pub kind: TextValidationKind,
    /// 字数（Unicode 标量值；见模块头的有意偏离）
    pub char_count: usize,
    /// UTF-8 字节数 —— **上限判据**
    pub byte_count: usize,
}

impl TextValidation {
    /// 是否可发送
    #[must_use]
    pub fn is_valid(self) -> bool {
        self.kind == TextValidationKind::Valid
    }
}

/// 字数：Unicode 标量值个数。与前端 `[...text].length` **逐值一致**。
#[must_use]
pub fn char_count(text: &str) -> usize {
    text.chars().count()
}

/// 字节数：UTF-8 编码长度 —— 上限判据。
#[must_use]
pub fn byte_count(text: &str) -> usize {
    text.len()
}

/// 校验一段待发送文本（F-FT-32 的判定顺序：先空后超限）。
///
/// 🔴 **只判不截**：本函数不提供"截断到上限"的入口，因为规格明写"绝不静默截断"。
#[must_use]
pub fn validate(text: &str) -> TextValidation {
    let characters = char_count(text);
    let bytes = byte_count(text);
    let kind = if text.is_empty() {
        TextValidationKind::Empty
    } else if bytes > MAX_BYTES {
        TextValidationKind::TooLong
    } else {
        TextValidationKind::Valid
    };
    TextValidation {
        kind,
        char_count: characters,
        byte_count: bytes,
    }
}

/// 把字节数折算成"KB"用于超限文案。
///
/// 🔴 **向上取整**：截断会让 256.5 KB 显示成"256 KB"，与紧随其后的"单条上限 256 KB"
/// 自相矛盾（看起来没超却报超）。
#[must_use]
pub fn kilobytes_ceil(bytes: usize) -> usize {
    // `usize::div_ceil` 自 Rust 1.73 稳定，本 crate MSRV 1.77.2 可用（曾误以为不可用而手写展开式，clippy 纠正）。
    bytes.div_ceil(1024)
}

/// 拒绝文案（F-FT-32 原文措辞）；`Valid` 返回 `None`。
///
/// - 超限：`文本过大（{N} KB），单条上限 256 KB。`
/// - 空：`文本为空，没有可发送的内容。`
#[must_use]
pub fn error_text(validation: TextValidation) -> Option<String> {
    match validation.kind {
        TextValidationKind::Valid => None,
        TextValidationKind::TooLong => Some(format!(
            "文本过大（{} KB），单条上限 256 KB。",
            kilobytes_ceil(validation.byte_count)
        )),
        TextValidationKind::Empty => Some("文本为空，没有可发送的内容。".to_owned()),
    }
}

/// 发送卡的体量显示行（F-FT-32 原文示例：`137 字（UTF-8 412 B）`）。
#[must_use]
pub fn size_line(validation: TextValidation) -> String {
    format!(
        "{} 字（UTF-8 {} B）",
        validation.char_count, validation.byte_count
    )
}

/// 接收确认弹窗的字数行（F-FT-15 原文示例：`共 N 字（UTF-8 M 字节）`）。
#[must_use]
pub fn received_size_line(validation: TextValidation) -> String {
    format!(
        "共 {} 字（UTF-8 {} 字节）",
        validation.char_count, validation.byte_count
    )
}

/// 历史条目"大小"列的文本形态（F-FT-35：文本显示"共 N 字"，文件才显示体积）。
#[must_use]
pub fn history_size_text(validation: TextValidation) -> String {
    format!("共 {} 字", validation.char_count)
}

/// 历史里的预览（F-FT-35：120 字、**不拆散成对字符**、完整正文不落盘）。
///
/// 🔴 调用方**只能**用返回值落历史，不得把完整正文写进历史条目——规格把它列为隐私取舍。
#[must_use]
pub fn preview(text: &str) -> String {
    text.chars().take(PREVIEW_CHARS).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn limit_is_measured_in_utf8_bytes_not_characters() {
        // 规格 F-FT-32 原文：上限"按 UTF-8 字节数判定（不按字符数）"。
        // 中文每字 3 字节：256 KiB / 3 ≈ 87381 字，远少于 262144 字。
        let at_limit = "中".repeat(MAX_BYTES / 3); // 87381 字 = 262143 字节
        assert!(validate(&at_limit).is_valid());
        assert_eq!(validate(&at_limit).byte_count, 262_143);

        // 再加一个字就超（3 字节 → 262146 > 262144）
        let over = "中".repeat(MAX_BYTES / 3 + 1);
        assert_eq!(validate(&over).kind, TextValidationKind::TooLong);
        assert!(
            over.chars().count() < MAX_BYTES,
            "字数远未到上限，证明判的是字节不是字数"
        );
    }

    #[test]
    fn exactly_at_limit_is_valid_and_one_byte_over_is_not() {
        let exact = "a".repeat(MAX_BYTES);
        assert!(validate(&exact).is_valid(), "恰好等于上限应放行");
        let over = "a".repeat(MAX_BYTES + 1);
        assert_eq!(validate(&over).kind, TextValidationKind::TooLong);
    }

    #[test]
    fn empty_text_is_rejected_with_its_own_wording() {
        let v = validate("");
        assert_eq!(v.kind, TextValidationKind::Empty);
        assert_eq!(v.char_count, 0);
        assert_eq!(
            error_text(v).as_deref(),
            Some("文本为空，没有可发送的内容。")
        );
    }

    /// 🔴 向上取整：截断会让 256.5 KB 显示"256 KB"，与后半句"单条上限 256 KB"自相矛盾。
    #[test]
    fn oversized_wording_rounds_kilobytes_up() {
        let v = validate(&"a".repeat(MAX_BYTES + 1));
        assert_eq!(kilobytes_ceil(v.byte_count), 257);
        assert_eq!(
            error_text(v).as_deref(),
            Some("文本过大（257 KB），单条上限 256 KB。")
        );
        // 恰好 256 KiB 时不报错；257 KiB 报 257
        assert_eq!(kilobytes_ceil(MAX_BYTES), 256);
        assert_eq!(kilobytes_ceil(1024), 1);
        assert_eq!(kilobytes_ceil(1025), 2);
    }

    /// 本模块头登记的那处**有意偏离**：emoji 算 1 字（不是 C# 的 2），以便与前端 `[...s].length` 一致。
    #[test]
    fn char_count_matches_js_code_point_iteration_not_utf16_units() {
        let text = "🚀";
        assert_eq!(char_count(text), 1, "标量值计数：emoji 算 1");
        assert_eq!(byte_count(text), 4, "UTF-8 下 emoji 占 4 字节");
        // 对照：C# `"🚀".Length` 会是 2（UTF-16 码元）。前端 `[...'🚀'].length` 是 1 → 与本实现一致。
        assert_eq!(size_line(validate(text)), "1 字（UTF-8 4 B）");
    }

    /// F-FT-32 原文示例格式：`137 字（UTF-8 412 B）`
    #[test]
    fn size_line_follows_the_spec_example_format() {
        let text = "中".repeat(137); // 411 字节
        let line = size_line(validate(&text));
        assert_eq!(line, "137 字（UTF-8 411 B）");
        assert!(line.ends_with("B）"));
    }

    /// F-FT-15 原文示例格式：`共 N 字（UTF-8 M 字节）`
    #[test]
    fn received_size_line_follows_the_spec_example_format() {
        assert_eq!(
            received_size_line(validate("你好")),
            "共 2 字（UTF-8 6 字节）"
        );
        assert_eq!(history_size_text(validate("你好")), "共 2 字");
    }

    /// F-FT-35：120 字预览、不拆散成对字符；且**完整正文不进历史**。
    #[test]
    fn preview_truncates_at_120_characters_without_splitting_code_points() {
        let long = "字".repeat(200);
        let p = preview(&long);
        assert_eq!(char_count(&p), PREVIEW_CHARS);
        assert_eq!(p, "字".repeat(120));

        // 边界：第 120 个位置正好是 emoji 时不得被切坏
        let mut mixed = "a".repeat(119);
        mixed.push('🚀');
        mixed.push_str("tail");
        let p2 = preview(&mixed);
        assert_eq!(char_count(&p2), PREVIEW_CHARS);
        assert!(p2.ends_with('🚀'), "第 120 个标量值须完整保留");
        // 不含后续内容
        assert!(!p2.contains("tail"));
    }

    #[test]
    fn short_text_preview_is_identity() {
        assert_eq!(preview("短文本"), "短文本");
        assert_eq!(preview(""), "");
    }
}
