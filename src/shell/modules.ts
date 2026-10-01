// 模块注册表（W1-06）：导航项与路由由同一张表派生——
// 🔴 禁用 = 不注册（不出现在表内即无路由、无导航、无后台组件），非仅隐藏导航项。
// features/ 是 8 模块封闭清单（04 分册 §一）；重装助手/设置未纳入功能规格（TASKS W2~W5 表下"范围外"注），
// 故只挂外壳级占位路由，不建 feature 目录、不注册任何服务。

// 图标统一语系（规范 §七 lucide-react，1.5px 描边，轨内恒 20px 档）：
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
  { id: "overview", route: "/overview", nameKey: "nav.item.overview", icon: LayoutDashboard, group: "modules", milestone: "M-01", featureDir: "Overview" },
  { id: "app-manager", route: "/app-manager", nameKey: "nav.item.appManager", icon: AppWindow, group: "modules", milestone: "M-02", featureDir: "AppManager" },
  { id: "driver-manager", route: "/driver-manager", nameKey: "nav.item.driverManager", icon: Cpu, group: "modules", milestone: "M-03", featureDir: "DriverManager" },
  { id: "net-manager", route: "/net-manager", nameKey: "nav.item.netManager", icon: Globe, group: "modules", milestone: "M-04", featureDir: "NetManager" },
  { id: "file-transfer", route: "/file-transfer", nameKey: "nav.item.fileTransfer", icon: ArrowLeftRight, group: "modules", milestone: "M-05", featureDir: "FileTransfer" },
  { id: "file-backup", route: "/file-backup", nameKey: "nav.item.fileBackup", icon: ShieldCheck, group: "modules", milestone: "M-06", featureDir: "FileBackup" },
  { id: "music-manager", route: "/music-manager", nameKey: "nav.item.musicManager", icon: Headphones, group: "modules", milestone: "M-07", featureDir: "MusicManager" },
  { id: "game-manager", route: "/game-manager", nameKey: "nav.item.gameManager", icon: Gamepad, group: "modules", milestone: "M-08", featureDir: "GameManager" },
  { id: "restore", route: "/restore", nameKey: "nav.item.restore", icon: History, group: "orchestration", milestone: null, featureDir: null },
  { id: "settings", route: "/settings", nameKey: "nav.item.settings", icon: Settings, group: "utility", milestone: null, featureDir: null },
];
