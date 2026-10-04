import { describe, expect, it } from "vitest";
import {
  MAX_BYTES,
  PREVIEW_CHARS,
  byteCount,
  charCount,
  errorText,
  historySizeText,
  kilobytesCeil,
  preview,
  sizeLine,
  validate,
} from "./text-metrics";

/**
 * 🔴 本文件的**唯一职责**是把前端口径钉在 Rust 侧 `services::transfer::text` 上。
 * 两侧任一改动而另一侧没跟上，这里的用例必须红。
 */
describe("text-metrics：与 Rust 侧口径逐值一致", () => {
  it("『字』是 Unicode 标量值，不是 UTF-16 码元（emoji 算 1）", () => {
    expect("🚀".length).toBe(2); // 若用 text.length 就会得到 2 —— 这正是要避开的坑
    expect(charCount("🚀")).toBe(1);
    expect(charCount("你好a")).toBe(3);
    expect(charCount("")).toBe(0);
    // 与 Rust chars().count() 同值：emoji 与中文混排
    expect(charCount("中🚀文")).toBe(3);
  });

  it("『字节』是 UTF-8 长度", () => {
    expect(byteCount("a")).toBe(1);
    expect(byteCount("中")).toBe(3);
    expect(byteCount("🚀")).toBe(4);
  });

  it("上限按字节判，不按字数", () => {
    const atLimit = "中".repeat(MAX_BYTES / 3); // 262143 字节
    expect(validate(atLimit).kind).toBe("valid");
    expect(validate(atLimit).bytes).toBe(262_143);

    const over = "中".repeat(MAX_BYTES / 3 + 1);
    expect(validate(over).kind).toBe("tooLong");
    expect(charCount(over)).toBeLessThan(MAX_BYTES); // 字数远未到上限
  });

  it("恰好等于上限放行，多 1 字节拒绝", () => {
    expect(validate("a".repeat(MAX_BYTES)).kind).toBe("valid");
    expect(validate("a".repeat(MAX_BYTES + 1)).kind).toBe("tooLong");
  });

  it("空文本有独立文案（不是超限）", () => {
    const v = validate("");
    expect(v.kind).toBe("empty");
    expect(errorText(v)).toBe("文本为空，没有可发送的内容。");
  });

  it("超限文案的 KB 向上取整（否则与『上限 256 KB』自相矛盾）", () => {
    expect(kilobytesCeil(MAX_BYTES)).toBe(256);
    expect(kilobytesCeil(MAX_BYTES + 1)).toBe(257);
    expect(errorText(validate("a".repeat(MAX_BYTES + 1)))).toBe(
      "文本过大（257 KB），单条上限 256 KB。",
    );
  });

  it("显示行格式与规格示例一致", () => {
    expect(sizeLine(validate("中".repeat(137)))).toBe("137 字（UTF-8 411 B）");
    expect(historySizeText(validate("你好"))).toBe("共 2 字");
    expect(sizeLine(validate("🚀"))).toBe("1 字（UTF-8 4 B）");
  });

  it("预览截到 120 个标量值且不切坏码点；完整正文不回流", () => {
    expect(charCount(preview("字".repeat(200)))).toBe(PREVIEW_CHARS);
    const mixed = "a".repeat(119) + "🚀" + "tail";
    const p = preview(mixed);
    expect(charCount(p)).toBe(PREVIEW_CHARS);
    expect(p.endsWith("🚀")).toBe(true);
    expect(p.includes("tail")).toBe(false);
    expect(preview("短文本")).toBe("短文本");
  });

  it("valid 时没有错误文案", () => {
    expect(errorText(validate("ok"))).toBeNull();
  });
});
