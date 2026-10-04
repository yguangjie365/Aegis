// F-12 页头摘要纯函数用例（vitest 引入的首批用例之一，M-01 片1-e）。
// 选这两条开腿的理由：它们是概览页唯一"口径写死在规格文字里"的前端纯函数——
// 时长分段与"缺失项省略分隔符"都能被单测钉住，不需要起后端也不需要 DOM。

import { describe, expect, it } from "vitest";
import { formatUptime, joinHeader } from "./format";

const UNITS = { day: "天", hour: "小时", minute: "分钟", belowMinute: "不足 1 分钟" };

describe("formatUptime（F-12 运行时长）", () => {
  it("超过一天只给「天 + 小时」两段，分钟不进摘要", () => {
    expect(formatUptime(3 * 86400 + 4 * 3600 + 59 * 60, UNITS)).toBe("3天 4小时");
  });

  it("整小时省略尾段，不留「0分钟」这种空段", () => {
    expect(formatUptime(5 * 3600, UNITS)).toBe("5小时");
    expect(formatUptime(2 * 86400, UNITS)).toBe("2天");
  });

  it("不足一小时给分钟；不足一分钟给文案而非 0 分钟", () => {
    expect(formatUptime(200, UNITS)).toBe("3分钟");
    expect(formatUptime(0, UNITS)).toBe("不足 1 分钟");
    expect(formatUptime(59, UNITS)).toBe("不足 1 分钟");
  });

  it("🔴 负数与 NaN 不得算出「-3天」这类冒充值（R-08 状态诚实）", () => {
    expect(formatUptime(-10, UNITS)).toBe("不足 1 分钟");
    expect(formatUptime(Number.NaN, UNITS)).toBe("不足 1 分钟");
    expect(formatUptime(Number.POSITIVE_INFINITY, UNITS)).toBe("不足 1 分钟");
  });

  it("🔴 null（尚未知）返回 null 而非「不足 1 分钟」——两者语义不同", () => {
    // 快照占位曾传 0，折成「不足 1 分钟」显示成「刚开机」，而真实开机时长可能数天
    expect(formatUptime(null, UNITS)).toBeNull();
    // 真机 0 秒仍应给「不足 1 分钟」（那是可信的观测结论）
    expect(formatUptime(0, UNITS)).toBe("不足 1 分钟");
  });
});

describe("joinHeader（F-12「任一项缺失该段自然省略」）", () => {
  it("缺项不产生空分隔符", () => {
    expect(joinHeader(["DESKTOP-A", "Windows 11", "运行 3天 4小时"])).toBe(
      "DESKTOP-A · Windows 11 · 运行 3天 4小时",
    );
    expect(joinHeader(["DESKTOP-A", null, "运行 1分钟"])).toBe("DESKTOP-A · 运行 1分钟");
    expect(joinHeader([undefined, "Windows 11", ""])).toBe("Windows 11");
    expect(joinHeader([])).toBe("");
  });

  it("全空白输入读作空串，不输出孤立分隔符", () => {
    expect(joinHeader(["  ", null])).toBe("");
  });
});
