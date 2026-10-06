// overview 域上行类型（M-01 片1）——Rust 生成物的临时手写形状（沿 music.ts 先例）。
// 🔴 字段口径唯一事实源在 crates/services/src/overview/dto.rs（serde 全部 camelCase）；
// 改后端字段必须同批改本文件，Q-17 ts-rs 生成器接入后本文件由生成物替换。
//
// 🔴 设计纪律（与 dto.rs 头注同源）：「要不要显示」的判定已在 Rust 侧做完（R-07 回退链、
// R-08 状态诚实、R-09 徽章分级），前端只按 `null` / 枚举渲染，不得二次猜测口径。

export type CardKind = "cpu" | "gpu" | "memory" | "storage";

/** R-07 回退链命中的哪一级（决定占用说明的措辞） */
export type ValueSource = "usage" | "sensorLoad" | "temperature";

/** R-09 徽章三态：有读数 / 有温度语义但本次无证据 / 本硬件无温度语义（隐藏徽章） */
export type BadgeState = "measured" | "undetected" | "notApplicable";

export type BadgeLevel = "normal" | "warning" | "critical";

export interface TempBadge {
  state: BadgeState;
  temperatureC: number | null;
  level: BadgeLevel | null;
}

export interface StatCard {
  kind: CardKind;
  /** 同类多设备序号（R-01；单值卡恒 0） */
  index: number;
  /** 精简型号（R-21） */
  label: string;
  detail: string;
  /** 0–100 整数；null = 本项无数据（R-08，显示破折号） */
  bigValue: number | null;
  valueSource: ValueSource | null;
  barPercent: number | null;
  /**
   * 分段条三级分级（🔴 由 Rust 判完：`SegLevel::from_percent`，阈值 <60 / <85 /其余）。
   * 前端**不得**自行按百分比判级——两处判级必然漂成两种颜色。
   */
  segLevel: "normal" | "warning" | "critical";
  badge: TempBadge;
  /** L-08 卡内一行降级原因（卡不消失、卡高不变） */
  degradeReason: string | null;
}

export interface HeaderInfo {
  computerName: string;
  osName: string;
  /**
   * 🔴 `null` = **尚未知**，不是 0。
   *
   * F-09 快照只存四卡与慢变量，页头无历史值；冷启动秒显时用 `null` 占位。
   * 此前填 `0`，而 `formatUptime(0)` 会渲染成「不足 1 分钟」——真实开机时长可能数天，
   * 凭空造出一个「刚开机」的假事实（R-08 状态诚实）。
   * 后端真机上行恒为数字，`null` 只来自前端快照占位。
   */
  uptimeSecs: number | null;
}

/**
 * F-02 详情卡类目（与后端 `dto.rs` 的 `DetailKind` 逐项对应）。
 *
 * 🔴 收窄为联合类型而非 `string`：两侧（后端 `kv(key: &str, …)`、前端 i18n 映射表）
 * 都没有约束时，写错键只会变成「界面透出裸键」，`tsc`/lint/build 全绿也发现不了
 * —— 本项目 22 处裸键事故即此根因（2026-10-04）。
 * 新增类目时**必须**同时改：`dto.rs` 的 `DetailKind`、此处、`DETAIL_KIND_I18N_PREFIX`、
 * `src/locales`下的 `overview.json`（各语言一份）。
 */
export type DetailKind =
  | "cpu"
  | "gpu"
  | "storage"
  | "memory"
  | "board"
  | "bios"
  | "network"
  | "audio"
  | "display"
  | "os"
  | "userRegion";

/** F-12 传感器通道状态（N-06 降级留痕的用户可见面） */
export type MonitorState = "ready" | "starting" | "coolingDown" | "disabled" | "exeMissing";

export interface MonitorStatus {
  state: MonitorState;
  reason: string | null;
  sidecarPid: number | null;
  driverInstalled: boolean;
  driverVersion: string | null;
}

export interface LiveSample {
  correlationId: string;
  header: HeaderInfo;
  cards: StatCard[];
  /**
   * 🔴 2026-10-07 改为**可缺省**：冷启动快照预置时**不知道**通道状态
   * （`Snapshot` 结构里没有该字段），此前靠编一个 `exeMissing` 顶替，
   * 导致回源返回前显示假的「组件缺失」。缺省 = 未知，界面不声称任何状态。
   * 后端 `overview_live_sample` 始终给足该字段，故只在预置窗口内缺省。
   */
  monitor?: MonitorStatus;
  /** R-13 重入闸门：本拍是否有采集在途 */
  inFlight: boolean;
  /** F-06 磁盘健康面板（慢变量，只在首次/全量采集时更新） */
  diskHealth: DiskHealthRowDto[];
}

/** F-06 磁盘健康一行 */
export interface DiskHealthRowDto {
  deviceName: string;
  temperatureC: number | null;
  smartAttributeCount: number;
  healthStatus: string;
  healthPercent: number | null;
  reason: string | null;
}

/** 进程榜一行（F-04；null = 宁缺毋滥，R-17） */
export interface ProcessRow {
  rank: number;
  name: string;
  pid: number;
  /** 占整机百分比（已按逻辑核数归一，与任务管理器一致） */
  cpuPercent: number | null;
  memoryMb: number | null;
}

export interface TopProcesses {
  correlationId: string;
  /** false = 首拍无差分基线（R-08：显示破折号 +「首次采样中…」，🔴 不得显示 0%） */
  hasBaseline: boolean;
  cpu: ProcessRow[];
  memory: ProcessRow[];
  reason: string | null;
}

/** 端点归属一行（F-05：按进程归属的端点数前列） */
export interface EndpointRow {
  rank: number;
  processName: string;
  endpointCount: number;
  /** 监听端口摘要（去重升序逗号分隔；无监听为空串） */
  listenPorts: string;
}

export interface NetEndpoints {
  correlationId: string;
  total: number;
  rows: EndpointRow[];
  /** 空面板原因（R-08：空态必写为什么是空的） */
  reason: string | null;
}

/** F-02 硬件详情卡的一行键值 */
export interface KvRowDto {
  key: string;
  value: string | null;
}

/** F-02 一张详情卡 */
export interface DetailCardDto {
  kind: DetailKind;
  index: number;
  rows: KvRowDto[];
  reason: string | null;
}

/** F-02 静态硬件详情上行 */
export interface HardwareDetailDto {
  cards: DetailCardDto[];
  collectedAtMs: number;
}

/** F-09 快照载体 */
export interface SnapshotDto {
  collectedAtMs: number;
  writtenAtMs: number;
  fingerprint: string;
  payload: HardwareDetailDto;
  /**
   * 🔴 F-01 四卡的**上次**读数（启动秒显用，2026-10-04）。
   * 语义是「上次读数」而非当前值，故读到后必须立即回源；空数组 = 无历史，四卡自然缺席。
   */
  cards: StatCard[];
}

/** F-07 已安装软件清单一行 */
export interface SoftwareRow {
  name: string;
  version: string | null;
  publisher: string | null;
  installDate: string | null;
  /** 估计占用字节数（前端格式化为 KB/MB/GB） */
  sizeBytes: number | null;
}

/** F-07 软件清单上行 */
export interface SoftwareList {
  apps: SoftwareRow[];
  total: number;
  /** 空态原因（R-08：无数据必写为什么是空的） */
  reason: string | null;
}

/** F-11 逐条传感器原始读数（聚合快照折叠掉了，报告要全量表） */
export interface SensorRow {
  hardware: string;
  /**
   * sidecar 原始硬件类型串（Cpu / SuperIO / Memory / Ssd / GpuNvidia…）。
   *
   * 🔴 2026-10-07 补：原先 DTO 里没有这一列，导致「聚合判据命中了哪些行」无法自查
   *   —— 而后端 `matching.rs` 的全部族判定都建立在它上面。
   */
  hardwareType: string;
  name: string;
  /**
   * 🔴 本仓归一后的**稳定指标标识**（后端 `sensor_bridge/metrics.rs` 产出）。
   *
   * 与 `name` 并存的意义：上游（LHM）改命名时 `name` 变而 `metricId` 不变 ——
   * 一眼就能区分「只是上游改了名」与「本仓判据失效了」。未识别条目归入 `misc.*`，
   * 因此上游新增传感器也会在此显形，而不是被悄悄吞掉。
   */
  metricId: string;
  /** sidecar 原始类型串（Temperature / Load / Fan…）；译名归前端 i18n */
  sensorType: string;
  value: number;
  /** sidecar 随读数上报的测量单位（数据而非文案，故后端透传） */
  unit: string;
}

/** F-11 传感器全量表上行（空数组 = 本路未读到，报告整节省略该章节） */
export interface SensorTable {
  rows: SensorRow[];
}
