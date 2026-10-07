// 模块注册表（W1-06）：导航项与路由由同一张表派生——
// 🔴 禁用 = 不注册（不出现在表内即无路由、无导航、无后台组件），非仅隐藏导航项。
// features/ 是 9 模块封闭清单（04 分册 §一，2026-10-07 用户裁决新增「系统优化」）；
// 重装助手/设置未纳入功能规格（TASKS W2~W5 表下"范围外"注），
// 故只挂外壳级占位路由，不建 feature 目录、不注册任何服务。

// 图标统一语系（规范 §七 lucide-react，1.5px 描边，轨内恒 22px 档——2026-10-06 用户裁决，原 20）：
// 全部选用"有界形状"图标（外框接近方形/圆形轮廓），避免悬散细线（如裸 Wifi/Music 波束）造成的密度不齐。
import {
  LayoutDashboard,
  AppWindow,
  Cpu,
  Globe,
  ArrowLeftRight,
  ShieldCheck,
  Headphones,
  Gamepad,
  History,
  Settings,
  Cog,
  type LucideIcon,
} from "lucide-react";

export type NavGroup = "modules" | "orchestration" | "utility";

export interface ShellModule {
  id: string;
  route: string;
  nameKey: string;
  icon: LucideIcon;
  group: NavGroup;
  /** M-xx 里程碑编号；out-of-scope 项为 null */
  milestone: string | null;
  /** null = 外壳占位页（未纳入功能规格）；否则渲染 src/features/<dir> 页面 */
  featureDir: string | null;
}

export const SHELL_MODULES: readonly ShellModule[] = [
  {
    id: "overview",
    route: "/overview",
    nameKey: "nav.item.overview",
    icon: LayoutDashboard,
    group: "modules",
    milestone: "M-01",
    featureDir: "Overview",
  },
  {
    id: "app-manager",
    route: "/app-manager",
    nameKey: "nav.item.appManager",
    icon: AppWindow,
    group: "modules",
    milestone: "M-02",
    featureDir: "AppManager",
  },
  {
    id: "driver-manager",
    route: "/driver-manager",
    nameKey: "nav.item.driverManager",
    icon: Cpu,
    group: "modules",
    milestone: "M-03",
    featureDir: "DriverManager",
  },
  {
    id: "net-manager",
    route: "/net-manager",
    nameKey: "nav.item.netManager",
    icon: Globe,
    group: "modules",
    milestone: "M-04",
    featureDir: "NetManager",
  },
  {
    id: "file-transfer",
    route: "/file-transfer",
    nameKey: "nav.item.fileTransfer",
    icon: ArrowLeftRight,
    group: "modules",
    milestone: "M-05",
    featureDir: "FileTransfer",
  },
  {
    id: "file-backup",
    route: "/file-backup",
    nameKey: "nav.item.fileBackup",
    icon: ShieldCheck,
    group: "modules",
    milestone: "M-06",
    featureDir: "FileBackup",
  },
  {
    id: "music-manager",
    route: "/music-manager",
    nameKey: "nav.item.musicManager",
    icon: Headphones,
    group: "modules",
    milestone: "M-07",
    featureDir: "MusicManager",
  },
  {
    id: "game-manager",
    route: "/game-manager",
    nameKey: "nav.item.gameManager",
    icon: Gamepad,
    group: "modules",
    milestone: "M-08",
    featureDir: "GameManager",
  },
  {
    // 🔴 2026-10-07 用户裁决新增：NexBox「内置工具 + 优化」两模块全量搬迁（M-09）。
    //   图标 Cog = 有界形状（外框接近圆形轮廓），符合本文件「禁悬散细线」口径。
    id: "system-optimization",
    route: "/system-optimization",
    nameKey: "nav.item.systemOptimization",
    icon: Cog,
    group: "modules",
    milestone: "M-09",
    featureDir: "SystemOptimization",
  },
  {
    id: "restore",
    route: "/restore",
    nameKey: "nav.item.restore",
    icon: History,
    group: "orchestration",
    milestone: null,
    featureDir: null,
  },
  {
    id: "settings",
    route: "/settings",
    nameKey: "nav.item.settings",
    icon: Settings,
    group: "utility",
    milestone: null,
    featureDir: null,
  },
];

/* ── 子路由归属判定（2026-10-06，代码审查 #4）────────────────────────────────
 * `matchModule` 是「某路由属于哪个模块」的**唯一**谓词，两处消费：
 *   ① NavRail 高亮（activeId）、② BrandBar 主标题派生（title 的回退值）。
 * 方向 = **前缀继承**（子路由命中所属模块）：与 NavRail 既有高亮语义一致
 *   —— 同一个问题只能有一个答案；子路由不会出现「空标题」空洞（精确匹配
 *   会，除非页面自行注册 title）；需要区分具体子页时走已有覆盖通道
 *   （useRegisterBrand({ title })，见 brand-bar.tsx BrandContent.title ②）。
 * 按 route 长度降序预排 = 最长前缀优先；当前表内无互为前缀的路由，
 *   排序是为将来出现嵌套（如 /a 与 /a/b）时不产生「先到先得」歧义。 */
const BY_LENGTH_DESC = [...SHELL_MODULES].sort((a, b) => b.route.length - a.route.length);

export function matchModule(pathname: string): ShellModule | undefined {
  return BY_LENGTH_DESC.find((m) => pathname === m.route || pathname.startsWith(`${m.route}/`));
}
