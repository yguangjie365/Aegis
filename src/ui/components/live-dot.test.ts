import { describe, expect, it } from "vitest";
import { isChannelDegraded } from "./live-dot";
import type { MonitorStatus } from "@/types/overview";

// live-dot 的降级判据（纯函数层）。
//
// 🔴 **为什么只测判据、不测渲染**：门禁 vitest 是 **node 环境**（`vitest.config.ts`
//   注：加 jsdom + testing-library 属新第三方依赖，须走 ADR-003 五步登记后才准进
//   `package.json`）。本轮不擅自加依赖，故「详情默认收起 / 点击展开」这项
//   **渲染行为无自动化覆盖**，如实登记在此，由真机验证兜底。
//   判据层能独立钉住，故抽出 `isChannelDegraded` 供此处测试。

const make = (state: MonitorStatus["state"]): MonitorStatus => ({
  state,
  reason: null,
  sidecarPid: null,
  driverInstalled: false,
  driverVersion: null,
});

describe("isChannelDegraded", () => {
  it("ready 是唯一无需解释的态", () => {
    expect(isChannelDegraded(make("ready"))).toBe(false);
  });

  it("其余四态都要向用户解释原因（N-06 降级留痕）", () => {
    for (const state of ["starting", "coolingDown", "disabled", "exeMissing"] as const) {
      expect(isChannelDegraded(make(state))).toBe(true);
    }
  });

  it("缺省 monitor 时退化为纯刷新指示（不误报降级）", () => {
    // 🔴 `monitor` 是可选 prop：未传时不得凭空把点染成警示色，
    // 否则复用此组件的其它页面会莫名显示「降级」。
    expect(isChannelDegraded(undefined)).toBe(false);
  });
});
