import type { DetailCardDto, DetailKind, SensorRow, StatCard } from "@/types/overview";
import { DETAIL_KIND_I18N_PREFIX, SYSTEM_INFO_KINDS } from "@/ui/components/key-value-card";
import { statCaptionKey, statValueUnit } from "@/ui/components/stat-card";

// F-11 导出硬件信息报告：Markdown 文本组装（纯函数，零 DOM / 零 invoke，可直接 vitest 单测）。
//
// **复用结论（DP-1）**：章节顺序（硬件概览→硬件详情→系统概览→传感器快照）、统计卡单行体
// `- 标题：大数字（副文案） · 占用说明 N%`、键值表 `| 项目 | 值 |`、`|` 转义、
// "无读数则整节省略"、"不含已安装软件清单"（R-15）← `SystemToolkit` `OverviewReportBuilder.cs`（③思路复用）。
//
// 🔴 为什么落点在前端而不在 Rust：本仓 `KvRowDto.key` 刻意只带 i18n 键（文案单源，见 dto.rs 头注），
// 中文串只存在于前端词典；Rust 侧拼 Markdown 会把 `detail.cpu.model` 这类裸键写进用户文件。
// 故 Rust 只做落盘（`overview_report_write`），组装在此。
//
// 🔴 文案一律经传入的 `t` 取词典，本文件不得内联任何中文（界面与报告同源，不允许两份措辞）。

type Translate = (key: string, options?: Record<string, unknown>) => string;

/** Markdown 表格分隔行：纯语法，不是文案，故不进词典 */
const TABLE_RULE = "| --- | --- |";

export interface ReportInput {
  /** 页头计算机名（缺失传空串，报告头那行由 `t("report.computerNone")` 兜底） */
  computerName: string;
  generatedAt: Date;
  /** F-01 四张统计卡（硬件概览节） */
  statCards: StatCard[];
  /** F-02 硬件详情卡（kind 不在 `SYSTEM_INFO_KINDS` 内的那批） */
  detailCards: DetailCardDto[];
  /** F-03 系统信息卡（os / userRegion） */
  systemCards: DetailCardDto[];
  /** F-11 全量传感器逐条读数；空数组 = 本路未读到，整节省略 */
  sensors: SensorRow[];
}

/** Markdown 表格管道符转义，防止值里出现 `|` 破坏表格结构（母本同口径）。 */
function escapeCell(text: string): string {
  return text.replace(/\|/g, "\\|");
}

function pad(value: number): string {
  return String(value).padStart(2, "0");
}

/** `yyyy-MM-dd HH:mm:ss`（本地时刻，与母本 `yyyy-MM-dd HH:mm:ss zzz` 同量信息，去掉本仓不需要的小时偏移噪声） */
function formatGeneratedAt(date: Date): string {
  return (
    `${date.getFullYear()}-${pad(date.getMonth() + 1)}-${pad(date.getDate())} ` +
    `${pad(date.getHours())}:${pad(date.getMinutes())}:${pad(date.getSeconds())}`
  );
}

/** 传感器类型译名：词典缺失时**原样透出** sidecar 串，不臆造译法（R-08）。 */
function sensorTypeLabel(type: string, t: Translate): string {
  const key = `sensor.type.${type.charAt(0).toLowerCase()}${type.slice(1)}`;
  const label = t(key);
  return label === key ? type : label;
}

/** 报告里一条读数 = 值 + sidecar 单位（`0.##` 精度，母本同口径）。 */
function formatSensorValue(row: SensorRow): string {
  const rounded = Math.round(row.value * 100) / 100;
  return row.unit === "" ? String(rounded) : `${rounded} ${row.unit}`;
}

/**
 * 同名多卡（R-01 多显卡 / 多盘 / 多网卡全列）时给标题加序号，
 * 单卡保持光标题——与母本"一类一条"的排版等价，又能区分本仓的一设备一卡。
 */
function cardTitle(card: DetailCardDto, counted: Map<DetailKind, number>, t: Translate): string {
  // 🔴 同 `KeyValueCard`：kind 已是联合类型、映射表已穷尽，**不加 `??` 兜底**
  // （加了就把「新增类目忘配文案」重新变成静默裸键写进用户文件）。
  const prefix = DETAIL_KIND_I18N_PREFIX[card.kind];
  const base = t(`${prefix}.title`);
  const total = counted.get(card.kind) ?? 1;
  return total > 1 ? `${base} ${card.index + 1}` : base;
}

/** 统计卡行：`- 处理器 · 型号：45%（副文案） · 使用率 45%` */
function statLine(card: StatCard, t: Translate): string {
  const label = `${t(`card.title.${card.kind}`)} · ${card.label}`;
  const value =
    card.bigValue === null ? t("report.noValue") : `${card.bigValue}${statValueUnit(card)}`;
  const parts = [`- ${escapeCell(label)}：${escapeCell(value)}`];
  if (card.detail !== "") {
    parts.push(`（${escapeCell(card.detail)}）`);
  }
  if (card.bigValue !== null) {
    // 说明跟随**大数字的来源**（R-07 回退链命中哪一级），而不只是跟随进度条：
    // 温度顶位时没有进度条，但报告必须写明这个数是温度而不是占用率。
    const percent = card.barPercent === null ? "" : ` ${card.barPercent}%`;
    parts.push(` · ${t(statCaptionKey(card))}${percent}`);
  }
  if (card.degradeReason !== null) {
    parts.push(` · ⚠️ ${escapeCell(card.degradeReason)}`);
  }
  return parts.join("");
}

/** 键值卡块：`### 标题` + `| 项目 | 值 |` 表（+ 降级原因一行，R-08 空态必写为什么是空的） */
function detailBlock(
  card: DetailCardDto,
  counted: Map<DetailKind, number>,
  t: Translate,
  out: string[],
): void {
  out.push(`### ${escapeCell(cardTitle(card, counted, t))}`, "");
  out.push(`| ${t("report.colKey")} | ${t("report.colValue")} |`, TABLE_RULE);
  for (const row of card.rows) {
    // 🔴 同 `KeyValueCard`：`row.key` 已是后端给的**完整** i18n 键（如 `detail.cpu.model`），
    // 再拼 `prefix` 会得到 `detail.cpu.detail.cpu.model` —— 词典查不到，裸键会被写进用户文件。
    const key = t(row.key);
    const value = row.value ?? t("badge.undetected");
    out.push(`| ${escapeCell(key)} | ${escapeCell(value)} |`);
  }
  out.push("");
  if (card.reason !== null) {
    out.push(t("report.reasonPrefix", { reason: escapeCell(card.reason) }), "");
  }
}

/** 同类卡计数（决定 `cardTitle` 是否加序号） */
function countKinds(cards: DetailCardDto[]): Map<DetailKind, number> {
  const counted = new Map<DetailKind, number>();
  for (const card of cards) {
    counted.set(card.kind, (counted.get(card.kind) ?? 0) + 1);
  }
  return counted;
}

/** 默认文件名 `概览报告-20261003-1504.md`（F-11：含"概览报告-日期时间"；前缀取词典，不在此内联中文） */
export function defaultFileName(prefix: string, date: Date): string {
  const stamp =
    `${date.getFullYear()}${pad(date.getMonth() + 1)}${pad(date.getDate())}` +
    `-${pad(date.getHours())}${pad(date.getMinutes())}`;
  return `${prefix}-${stamp}.md`;
}

export function buildReport(input: ReportInput, t: Translate): string {
  const lines: string[] = [
    `# ${t("report.title")}`,
    "",
    `- ${t("report.generatedAt", { value: formatGeneratedAt(input.generatedAt) })}`,
    `- ${t("report.computer", { value: input.computerName === "" ? t("report.computerNone") : input.computerName })}`,
    "",
  ];

  lines.push(`## ${t("report.section.stats")}`, "");
  for (const card of input.statCards) {
    lines.push(statLine(card, t));
  }
  lines.push("");

  lines.push(`## ${t("report.section.detail")}`, "");
  const detailCounted = countKinds(input.detailCards);
  for (const card of input.detailCards) {
    detailBlock(card, detailCounted, t, lines);
  }

  lines.push(`## ${t("report.section.system")}`, "");
  const systemCounted = countKinds(input.systemCards);
  for (const card of input.systemCards) {
    detailBlock(card, systemCounted, t, lines);
  }

  // 传感器快照：探测失败 / 驱动不可用时整节省略，不输出空表（母本同口径）
  if (input.sensors.length > 0) {
    lines.push(`## ${t("report.section.sensors")}`, "");
    lines.push(
      `| ${t("report.colHardware")} | ${t("report.colSensor")} | ${t("report.colType")} | ${t("report.colValue")} |`,
      TABLE_RULE,
    );
    for (const row of input.sensors) {
      lines.push(
        `| ${escapeCell(row.hardware)} | ${escapeCell(row.name)} | ` +
          `${escapeCell(sensorTypeLabel(row.sensorType, t))} | ${escapeCell(formatSensorValue(row))} |`,
      );
    }
    lines.push("");
  }

  return lines.join("\n");
}

/**
 * 按 kind 把静态上行（硬件详情 + 系统信息同装一个 DTO）分流成报告的两节输入。
 * 分流规则的唯一 owner 是 `SYSTEM_INFO_KINDS`（界面上 F-02/F-03 两区将来也用它）。
 */
export function splitStaticCards(cards: DetailCardDto[]): {
  detailCards: DetailCardDto[];
  systemCards: DetailCardDto[];
} {
  return {
    detailCards: cards.filter((card) => !SYSTEM_INFO_KINDS.has(card.kind)),
    systemCards: cards.filter((card) => SYSTEM_INFO_KINDS.has(card.kind)),
  };
}
