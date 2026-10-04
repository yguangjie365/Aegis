import { describe, expect, it } from "vitest";
import type { DetailCardDto, SensorRow, StatCard } from "@/types/overview";
import { buildReport, defaultFileName, splitStaticCards } from "./export-report";

// F-11 报告组装器的单测：锁的是**结构与诚实性**（章节顺序、| 转义、无数据不臆造单位、
// 传感器空则整节省略），中文措辞本身归词典，不在断言范围内。

const DICT: Record<string, string> = {
  "report.title": "硬件信息报告",
  "report.generatedAt": "生成时间：{{value}}",
  "report.computer": "计算机：{{value}}",
  "report.computerNone": "未知",
  "report.noValue": "—",
  "report.reasonPrefix": "> 说明：{{reason}}",
  "report.section.stats": "硬件概览",
  "report.section.detail": "硬件详情",
  "report.section.system": "系统概览",
  "report.section.sensors": "传感器快照",
  "report.colKey": "项目",
  "report.colValue": "值",
  "report.colHardware": "硬件",
  "report.colSensor": "传感器",
  "report.colType": "类型",
  "report.fileName": "概览报告",
  "card.title.cpu": "处理器",
  "card.title.gpu": "显卡",
  "card.caption.cpu": "使用率",
  "card.caption.temperature": "温度顶位（无占用读数）",
  "detail.cpu.title": "处理器",
  "detail.cpu.model": "型号",
  "detail.gpu.title": "显卡",
  "detail.gpu.model": "型号",
  "detail.os.title": "操作系统",
  "detail.os.osName": "操作系统名称",
  "detail.userRegion.title": "用户 / 区域",
  "badge.undetected": "未检测",
  "sensor.type.temperature": "温度",
  "sensor.type.load": "负载",
};

function t(key: string, options?: Record<string, unknown>): string {
  const raw = DICT[key] ?? key;
  if (!options) return raw;
  return raw.replace(/\{\{(\w+)\}\}/g, (_, name: string) => {
    const value = options[name];
    return typeof value === "string" || typeof value === "number" ? String(value) : "";
  });
}

const at = new Date(2026, 9, 3, 15, 4, 5);

function statCard(patch: Partial<StatCard> = {}): StatCard {
  return {
    kind: "cpu",
    index: 0,
    label: "AMD Ryzen 7",
    detail: "8 核 16 线程",
    bigValue: 37,
    valueSource: "usage",
    barPercent: 37,
    segLevel: "normal",
    badge: { state: "measured", temperatureC: 45, level: "normal" },
    degradeReason: null,
    ...patch,
  };
}

function detailCard(patch: Partial<DetailCardDto> = {}): DetailCardDto {
  return {
    kind: "cpu",
    index: 0,
    rows: [{ key: "detail.cpu.model", value: "AMD Ryzen 7 5800H" }],
    reason: null,
    ...patch,
  };
}

function sensorRow(patch: Partial<SensorRow> = {}): SensorRow {
  return {
    hardware: "CPU",
    name: "Core (Tctl/Tdie)",
    sensorType: "Temperature",
    value: 45.678,
    unit: "℃",
    ...patch,
  };
}

function baseInput() {
  // 🔴 `systemCards` 必须逐元素标成 `DetailCardDto`：直接写对象字面量时
  // `kind: "os"` 会被推断成 `string`，而 `DetailCardDto.kind` 已收窄为 `DetailKind` 联合类型
  // （2026-10-04 审查一般 6）。这也正是「不写类型约束 → 错键静默通过」的反面证明：
  // 收窄后夹具的形状漂移会在编译期暴露，而不是运行时透出裸键。
  const systemCards: DetailCardDto[] = [
    {
      kind: "os",
      index: 0,
      rows: [{ key: "detail.os.osName", value: "Windows 11" }],
      reason: null,
    },
  ];
  return {
    computerName: "DESKTOP-TEST",
    generatedAt: at,
    statCards: [statCard()],
    detailCards: [detailCard()],
    systemCards,
    sensors: [sensorRow()],
  };
}

describe("buildReport", () => {
  // 🔴 防回归（2026-10-04 用户截图暴露）：`row.key` 是后端给的**完整** i18n 键
  // （`hardware/cpu.rs` 的 `kv("detail.cpu.model", …)`）。若前端再拼一次 `prefix`，
  // 会得到 `detail.cpu.detail.cpu.model`，词典查不到 → 裸键被写进用户的报告文件。
  // 这条断言把该契约钉死：报告正文里不得出现任何含`detail.` 的裸键片段。
  it("正文不泄漏任何未解析的 i18n 键", () => {
    const md = buildReport(baseInput(), t);
    const leaked = md
      .split("\n")
      .filter((line) =>
        /\b(detail|systemInfo|badge|card|processes|endpoints)\.[a-zA-Z]/.test(line),
      );
    expect(leaked).toEqual([]);
  });

  it("按硬件概览→硬件详情→系统概览→传感器快照的顺序出四节", () => {
    const md = buildReport(baseInput(), t);
    expect(md.indexOf("# 硬件信息报告")).toBe(0);
    expect(md).toContain("生成时间：2026-10-03 15:04:05");
    expect(md).toContain("计算机：DESKTOP-TEST");
    const order = ["硬件概览", "硬件详情", "系统概览", "传感器快照"].map((s) =>
      md.indexOf(`## ${s}`),
    );
    expect(order.every((pos) => pos >= 0)).toBe(true);
    expect(order).toEqual([...order].sort((a, b) => a - b));
  });

  it("R-15：报告不含已安装软件清单", () => {
    expect(buildReport(baseInput(), t)).not.toContain("软件");
  });

  it("统计卡行含大数字、副文案与占用说明", () => {
    const line = buildReport(baseInput(), t)
      .split("\n")
      .find((l) => l.startsWith("- 处理器"));
    expect(line).toBe("- 处理器 · AMD Ryzen 7：37%（8 核 16 线程） · 使用率 37%");
  });

  it("R-07 温度顶位：单位与说明一起换成温度，不得谎称使用率", () => {
    const md = buildReport(
      {
        ...baseInput(),
        statCards: [statCard({ bigValue: 88, valueSource: "temperature", barPercent: null })],
      },
      t,
    );
    const line = md.split("\n").find((l) => l.startsWith("- 处理器"));
    expect(line).toContain("88℃");
    expect(line).not.toContain("88%");
    expect(line).toContain("温度顶位");
  });

  it("R-08 无数据只出破折号，不带任何单位", () => {
    const line = buildReport(
      {
        ...baseInput(),
        statCards: [statCard({ bigValue: null, valueSource: null, barPercent: null })],
      },
      t,
    )
      .split("\n")
      .find((l) => l.startsWith("- 处理器"));
    expect(line).toBe("- 处理器 · AMD Ryzen 7：—（8 核 16 线程）");
  });

  it("降级原因如实随行，不静默吞掉", () => {
    const line = buildReport(
      { ...baseInput(), statCards: [statCard({ degradeReason: "传感器未接入" })] },
      t,
    )
      .split("\n")
      .find((l) => l.startsWith("- 处理器"));
    expect(line).toContain("⚠️ 传感器未接入");
  });

  it("键值表逐行取词典标签，缺失值填「未检测」", () => {
    const md = buildReport(
      {
        ...baseInput(),
        detailCards: [detailCard({ rows: [{ key: "detail.cpu.model", value: null }] })],
      },
      t,
    );
    expect(md).toContain("| --- | --- |");
    expect(md).toContain("| 型号 | 未检测 |");
    expect(md).toContain("### 处理器");
  });

  it("详情卡带原因时写进报告（R-08 空态必写为什么是空的）", () => {
    const md = buildReport(
      { ...baseInput(), detailCards: [detailCard({ reason: "WMI 无响应" })] },
      t,
    );
    expect(md).toContain("> 说明：WMI 无响应");
  });

  it("R-01 同名多卡加序号区分", () => {
    const md = buildReport(
      {
        ...baseInput(),
        detailCards: [detailCard({ kind: "gpu", index: 0 }), detailCard({ kind: "gpu", index: 1 })],
      },
      t,
    );
    expect(md).toContain("### 显卡 1");
    expect(md).toContain("### 显卡 2");
  });

  it("传感器空数组时整节省略（母本同口径）", () => {
    const md = buildReport({ ...baseInput(), sensors: [] }, t);
    expect(md).not.toContain("传感器快照");
  });

  it("传感器行 = 硬件 / 译名 / 值+单位；未知类型原样透出", () => {
    const rows: SensorRow[] = [
      sensorRow(),
      sensorRow({
        hardware: "NVGPU_1",
        name: "Fan #1",
        sensorType: "FanRpm",
        value: 1200,
        unit: "RPM",
      }),
    ];
    const md = buildReport({ ...baseInput(), sensors: rows }, t);
    expect(md).toContain("| CPU | Core (Tctl/Tdie) | 温度 | 45.68 ℃ |");
    expect(md).toContain("| NVGPU_1 | Fan #1 | FanRpm | 1200 RPM |");
  });

  it("值里的竖线被转义，不破坏表格结构", () => {
    const md = buildReport(
      {
        ...baseInput(),
        detailCards: [detailCard({ rows: [{ key: "detail.cpu.model", value: "A|B" }] })],
      },
      t,
    );
    expect(md).toContain("| 型号 | A\\|B |");
  });

  it("计算机名缺失时写兜底文案而不是留空行", () => {
    const md = buildReport({ ...baseInput(), computerName: "" }, t);
    expect(md).toContain("计算机：未知");
  });
});

describe("splitStaticCards", () => {
  it("os / userRegion 归系统概览，其余归硬件详情", () => {
    const { detailCards, systemCards } = splitStaticCards([
      detailCard({ kind: "cpu" }),
      detailCard({ kind: "os" }),
      detailCard({ kind: "userRegion" }),
      detailCard({ kind: "gpu" }),
    ]);
    expect(detailCards.map((c) => c.kind)).toEqual(["cpu", "gpu"]);
    expect(systemCards.map((c) => c.kind)).toEqual(["os", "userRegion"]);
  });
});

describe("defaultFileName", () => {
  it("形如 概览报告-YYYYMMDD-HHmm.md", () => {
    expect(defaultFileName("概览报告", at)).toBe("概览报告-20261003-1504.md");
  });
});
