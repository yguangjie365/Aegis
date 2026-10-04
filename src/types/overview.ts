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
  badge: TempBadge;
  /** L-08 卡内一行降级原因（卡不消失、卡高不变） */
  degradeReason: string | null;
}

export interface HeaderInfo {
  computerName: string;
  osName: string;
  uptimeSecs: number;
}

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
  monitor: MonitorStatus;
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
  kind: string;
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
  name: string;
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
