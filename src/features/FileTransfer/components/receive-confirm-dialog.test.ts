import { describe, expect, it } from "vitest";
import { formatBytes } from "./receive-confirm-dialog";

describe("formatBytes（F-FT-14：弹窗展示文件大小与磁盘剩余）", () => {
  it("小于 1 KiB 时按字节显示", () => {
    expect(formatBytes(0)).toBe("0 B");
    expect(formatBytes(1)).toBe("1 B");
    expect(formatBytes(1023)).toBe("1023 B");
  });

  it("逐级进位（1024 进制）", () => {
    expect(formatBytes(1024)).toBe("1.0 KB");
    expect(formatBytes(1024 * 1024)).toBe("1.0 MB");
    expect(formatBytes(1024 * 1024 * 1024)).toBe("1.0 GB");
    expect(formatBytes(1024 ** 4)).toBe("1.0 TB");
  });

  it("保留一位小数（与界面其余部分的「128.4 MB」一致）", () => {
    expect(formatBytes(128.4 * 1024 * 1024)).toBe("128.4 MB");
    expect(formatBytes(412.7 * 1024 ** 3)).toBe("412.7 GB");
  });

  it("超过 TB 不再进位（单位表到 TB 为止），但数值继续增长而不失真", () => {
    const huge = 2048 * 1024 ** 4; // 2048 TB
    expect(formatBytes(huge)).toBe("2048.0 TB");
  });

  it("🔴 非法值返回「—」而不是 0 B：后者会被读成「空间不足」", () => {
    // 这是 R-12「无法判定」在格式化这一层的对应物——
    // 弹窗对 availableBytes === null 走的是专门的文案，不该落到这里
    expect(formatBytes(Number.NaN)).toBe("—");
    expect(formatBytes(-1)).toBe("—");
    expect(formatBytes(Number.POSITIVE_INFINITY)).toBe("—");
  });
});
