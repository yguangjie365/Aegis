import { describe, expect, it } from "vitest";
import { EXPIRING_SOON_MS, formatCountdown, isExpiringSoon } from "./countdown";

describe("formatCountdown（F-FT-38 的 MM:SS）", () => {
  it("按分钟与秒补零", () => {
    expect(formatCountdown(10 * 60 * 1000)).toBe("10:00");
    expect(formatCountdown(9 * 60 * 1000 + 5_000)).toBe("09:05");
    expect(formatCountdown(59_000)).toBe("00:59");
  });

  it("🔴 向上取整：还剩 0.9 秒时不能显示 00:00（那会让人以为已经失效）", () => {
    expect(formatCountdown(900)).toBe("00:01");
    expect(formatCountdown(1)).toBe("00:01");
    expect(formatCountdown(1_000)).toBe("00:01");
    expect(formatCountdown(1_001)).toBe("00:02");
  });

  it("已过期与非法值一律 00:00", () => {
    expect(formatCountdown(0)).toBe("00:00");
    expect(formatCountdown(-1)).toBe("00:00");
    expect(formatCountdown(Number.NaN)).toBe("00:00");
    expect(formatCountdown(Number.POSITIVE_INFINITY)).toBe("00:00");
  });

  it("跨分钟边界连续（59 → 60 秒）", () => {
    expect(formatCountdown(59_500)).toBe("01:00");
    expect(formatCountdown(60_000)).toBe("01:00");
    expect(formatCountdown(60_500)).toBe("01:01");
  });

  it("超过 60 分钟也能正确进位（后端给的是 10 分钟，但别假设上限）", () => {
    expect(formatCountdown(61 * 60 * 1000)).toBe("61:00");
  });
});

describe("isExpiringSoon（F-FT-38 的「即将轮换」）", () => {
  it("阈值内为真，阈值外为假", () => {
    expect(isExpiringSoon(EXPIRING_SOON_MS)).toBe(true);
    expect(isExpiringSoon(EXPIRING_SOON_MS + 1)).toBe(false);
    expect(isExpiringSoon(30_000)).toBe(true);
  });

  it("🔴 已过期不算「即将」——那是已经发生的事，不该说成快要发生", () => {
    expect(isExpiringSoon(0)).toBe(false);
    expect(isExpiringSoon(-5_000)).toBe(false);
  });

  it("非法值不算", () => {
    expect(isExpiringSoon(Number.NaN)).toBe(false);
  });
});
