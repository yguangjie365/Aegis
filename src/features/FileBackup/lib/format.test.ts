import { describe, expect, it } from "vitest";
import {
  formatBytes,
  formatClock,
  formatCount,
  formatSnapshotTime,
  integrityKey,
  integrityTone,
  statusKey,
  statusTone,
} from "./format";

describe("formatBytes", () => {
  it("零与非法值都归零，不吐 NaN", () => {
    // 🔴 备份刚创建、还没有 info.json 时 totalBytes 可能是 0 或 undefined
    expect(formatBytes(0)).toBe("0 B");
    expect(formatBytes(-1)).toBe("0 B");
    expect(formatBytes(Number.NaN)).toBe("0 B");
  });

  it("用 1024 进制（与资源管理器一致）", () => {
    expect(formatBytes(1024)).toBe("1.0 KB");
    expect(formatBytes(1024 * 1024)).toBe("1.0 MB");
    expect(formatBytes(1024 * 1024 * 1024)).toBe("1.0 GB");
  });

  it("小于 10 保留一位、大于等于 10 取整", () => {
    expect(formatBytes(2.5 * 1024 ** 3)).toBe("2.5 GB");
    expect(formatBytes(23.4 * 1024 ** 3)).toBe("23 GB");
  });

  it("不上溢到不存在的单位", () => {
    // 🔴 超过 PB 时钳在最后一档，而不是索引越界返回 undefined
    expect(formatBytes(1024 ** 7)).toMatch(/PB$/);
  });
});

describe("formatSnapshotTime", () => {
  it("非法时间显示占位而非 Invalid Date", () => {
    // 🔴 目录名无法解析时 createdUnixMs 可能是 0 —— 显示 "Invalid Date" 会被当成 bug 报上来
    expect(formatSnapshotTime(0)).toBe("————");
    expect(formatSnapshotTime(-1)).toBe("————");
    expect(formatSnapshotTime(Number.NaN)).toBe("————");
  });

  it("按本地时区格式化（与快照目录名口径一致）", () => {
    // 用本地时间构造，避免测试随运行时区漂移
    const d = new Date(2026, 9, 5, 9, 12, 0);
    expect(formatSnapshotTime(d.getTime())).toBe("2026-10-05 09:12");
  });

  it("个位数补零", () => {
    const d = new Date(2026, 0, 2, 3, 4, 0);
    expect(formatSnapshotTime(d.getTime())).toBe("2026-01-02 03:04");
  });
});

describe("formatCount", () => {
  it("千分位", () => {
    expect(formatCount(1204)).toBe("1,204");
    expect(formatCount(0)).toBe("0");
  });

  it("非法值给占位", () => {
    expect(formatCount(Number.NaN)).toBe("—");
  });
});

describe("校验三态（R-11：禁止把「没查」写成「通过」）", () => {
  it("三态各有独立措辞与色彩，不折叠成两态", () => {
    // 🔴 若把 notFullyVerified 归入 passed，抽样校验就会被显示成「通过」
    //    —— 那正是 R-11 列为红线的「状态诚实」被破坏。
    expect(integrityKey("passed")).toBe("status.integrity.passed");
    expect(integrityKey("notFullyVerified")).toBe("status.integrity.notFullyVerified");
    expect(integrityKey("failed")).toBe("status.integrity.failed");

    expect(integrityTone("passed")).toBe("ok");
    expect(integrityTone("notFullyVerified")).toBe("warn");
    expect(integrityTone("failed")).toBe("bad");
  });

  it("三态的 key 互不相同", () => {
    const keys = new Set([
      integrityKey("passed"),
      integrityKey("notFullyVerified"),
      integrityKey("failed"),
    ]);
    expect(keys.size).toBe(3);
  });
});

describe("formatClock（日志行前缀）", () => {
  it("非法时间显示占位而非 Invalid Date", () => {
    // 日志条目的 at 在极端情况下可能是 0（时钟未就绪）—— 显示 Invalid Date 会被报成 bug
    expect(formatClock(0)).toBe("--:--:--");
    expect(formatClock(-1)).toBe("--:--:--");
    expect(formatClock(Number.NaN)).toBe("--:--:--");
  });

  it("时分秒都补零（等宽字体下才能按列对齐）", () => {
    const d = new Date(2026, 9, 5, 3, 4, 5);
    expect(formatClock(d.getTime())).toBe("03:04:05");
  });

  it("按本地时区（与 formatSnapshotTime 同口径）", () => {
    const d = new Date(2026, 9, 5, 23, 59, 58);
    expect(formatClock(d.getTime())).toBe("23:59:58");
  });
});

describe("快照终态", () => {
  it("成功/失败映射到色与文案", () => {
    expect(statusKey("success")).toBe("status.success");
    expect(statusTone("success")).toBe("ok");
    expect(statusKey("failed")).toBe("status.failed");
    expect(statusTone("failed")).toBe("bad");
  });
});
