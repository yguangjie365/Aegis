/**
 * 文本单条上限的**前端口径**——🔴 必须与 Rust 侧 `services::transfer::text` 逐值一致。
 *
 * 冻结口径（2026-10-04，登记于 `Aegis/DEPENDENCIES.md §三` 的 text.rs 行）：
 * - "字" = **Unicode 标量值**个数，即 JS 的 `[...text].length`（码点迭代），**不是** `text.length`
 *   （后者是 UTF-16 码元数，会把 emoji 算成 2）。
 * - "字节" = **UTF-8** 编码长度，用 `TextEncoder`。
 * - 上限判据是**字节**（规格 F-FT-32 原文："按 UTF-8 字节数判定（不按字符数）"）。
 *
 * 🔴 为什么必须有这道 `[...text]` 纪律：`'🚀'.length === 2` 而 `[...'🚀'].length === 1`。
 * 若这里图省事写 `text.length`，同一个字符串会在 Rust 侧显示"1 字"、前端显示"2 字"。
 */

/** 单条文本上限：256 KiB（F-FT-32；与 Rust `text::MAX_BYTES` 同源）。 */
export const MAX_BYTES = 256 * 1024;

/** 历史预览字数上限（F-FT-35；与 Rust `text::PREVIEW_CHARS` 同源）。 */
export const PREVIEW_CHARS = 120;

/** 字数 = Unicode 标量值个数（`[...text].length`，与 Rust `chars().count()` 一致）。 */
export function charCount(text: string): number {
  return [...text].length;
}

/** 字节数 = UTF-8 编码长度（上限判据）。 */
export function byteCount(text: string): number {
  return new TextEncoder().encode(text).length;
}

export type TextValidationKind = "valid" | "empty" | "tooLong";

export interface TextValidation {
  kind: TextValidationKind;
  chars: number;
  bytes: number;
}

/** 校验（顺序：先空后超限，与 Rust 侧一致；🔴 只判不截）。 */
export function validate(text: string): TextValidation {
  const chars = charCount(text);
  const bytes = byteCount(text);
  const kind: TextValidationKind =
    text.length === 0 ? "empty" : bytes > MAX_BYTES ? "tooLong" : "valid";
  return { kind, chars, bytes };
}

/**
 * 字节数折算"KB"，**向上取整**（与 Rust `text::kilobytes_ceil` 一致）。
 *
 * 🔴 截断会让 256.5 KB 显示成"256 KB"，与后半句"单条上限 256 KB"自相矛盾。
 */
export function kilobytesCeil(bytes: number): number {
  return Math.ceil(bytes / 1024);
}

/** 拒绝文案（措辞照抄规格 F-FT-32）；可发送时返回 `null`。 */
export function errorText(validation: TextValidation): string | null {
  if (validation.kind === "valid") {
    return null;
  }
  if (validation.kind === "tooLong") {
    return `文本过大（${kilobytesCeil(validation.bytes)} KB），单条上限 256 KB。`;
  }
  return "文本为空，没有可发送的内容。";
}

/** 发送卡的体量显示行（F-FT-32 原文示例：`137 字（UTF-8 412 B）`）。 */
export function sizeLine(validation: TextValidation): string {
  return `${validation.chars} 字（UTF-8 ${validation.bytes} B）`;
}

/** 历史"大小"列（F-FT-35：文本显示"共 N 字"）。 */
export function historySizeText(validation: TextValidation): string {
  return `共 ${validation.chars} 字`;
}

/** 历史预览：截到 120 个**标量值**（不切坏码点；🔴 完整正文不得落历史，见 F-FT-35）。 */
export function preview(text: string): string {
  return [...text].slice(0, PREVIEW_CHARS).join("");
}
