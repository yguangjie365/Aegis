import {
  Box,
  Text,
  SimpleGrid,
  Spinner,
  VStack,
  HStack,
  useColorModeValue,
  useColorMode,
  Image,
  Button,
  IconButton,
  Input,
  InputGroup,
  InputLeftElement,
  Badge,
  Tooltip,
  AlertDialog,
  AlertDialogBody,
  AlertDialogFooter,
  AlertDialogHeader,
  AlertDialogContent,
  AlertDialogOverlay,
  useDisclosure,
  Menu,
  MenuButton,
  MenuList,
  MenuItem,
  Divider,
  Collapse,
  Portal,
} from "@chakra-ui/react";
import { useDynamicIsland, useRegisterBrand } from "@/shared";
import { useState, useEffect, useMemo, useCallback, useRef, memo, useDeferredValue } from "react";
// Aegis 硬纪律：业务代码的后端调用必须经 @/lib/tauri/invoke 封装（统一 correlationId
// + 结构化日志 + 非 Tauri 运行时显式抛 BackendUnavailableError），禁直触 @tauri-apps/api。
// NexBox 原件为 `invoke(cmd, { args })` 两参形态，此处按封装签名补 module/action，
// 命令名与参数名逐字未改。module 取 LOG_MODULES 的 "Steam"。
import { invoke } from "@/lib/tauri/invoke";
import { useAdaptiveTextColor } from "@/shared";
import {
  PageBody,
  StaggeredCards,
  GroupHeader,
  FilterSwap,
  FilterSegments,
  CardSkeleton,
  type GameFilter,
} from "../components/steam-ui";
// 图标库适配（UI 规范 §七锁定 lucide-react）：Aegis 不引入 react-icons 的 Lu* 前缀命名，
// 改为 lucide-react 原生导出名。仅命名去前缀，图形语义未变。
// 🟢 2026-10-04 重构后移除 3 个：`Circle` / `CircleDot`（原「已安装/未安装」两枚统计卡的图标，
//    那5 张统计卡已下移并改用统一筛选器的数字徽章表达）· `Library`（原「库存游戏」区块标题
//    图标，该区块已与已安装合并）—— 属布局重排的连带结果，非功能删除。
import {
  Search,
  RefreshCw,
  Play,
  Folder,
  ExternalLink,
  EllipsisVertical,
  Trash2,
  Gamepad2,
  HardDrive,
  Users,
  ChevronDown,
  ChevronUp,
  CircleUser,
  Clock3,
} from "lucide-react";
import { LiquidGlassCard } from "@/shared";
import { steamDirFor } from "@/features/Settings/lib/steam-dir";
// 游戏卡片视觉层（唯一来源 = SystemToolkit GameManagerView.xaml，规格逐条对照见该文件头）
import {
  CARD_INNER_PADDING,
  CardActionButton,
  CardActionRow,
  CardMetaRow,
  CardMetaText,
  CardName,
  CoverStateBadge,
  GameCardShell,
  GameCover,
  MetaDot,
} from "@/features/GameManager/components/game-card";
import { useBackground } from "@/shared";
import { useThemeColor } from "@/shared";
// ============================================================================
// i18n 适配（Aegis 口径：单语言 zh-CN，废弃 i18next）
// ----------------------------------------------------------------------------
// NexBox 原件用 `useTranslation()` + `t("steam.*")` 取文案（64 处）。Aegis 音乐模块
// （M-07）已裁决「放弃 i18n、硬编码中文」，本模块沿用同一口径，故：
//   - 删除 react-i18next 依赖与全部 useTranslation() 调用；
//   - 下方 `t` 为**纯函数**词典查表，签名与原 `t` 完全一致（支持 {{var}} 插值），
//     因此 64 处 `t("steam.xxx")` 调用点与 StatusBadge 的 `t` prop 传递**逐字未改**。
// 词典逐条取自 NexBox `src/locales/zh.json` 的 `steam` 命名空间（56 键，未增删）。
// ============================================================================

const STEAM_STRINGS = {
  notInstalled: "未检测到 Steam 安装，请先安装 Steam 客户端",
  running: "运行中",
  notRunning: "未运行",
  launchSteam: "启动 Steam",
  launchingSteam: "启动中...",
  currentUser: "当前用户",
  remembered: "已记住密码",
  switchAccount: "切换到此账户",
  switchSuccess: "账户切换中，Steam 正在重启...",
  accounts: "账户管理",
  libraries: "游戏库",
  installedGames: "已安装游戏",
  totalGames: "游戏总数",
  totalSize: "总占用",
  accountCount: "账户数",
  installPath: "安装路径",
  searchGame: "搜索游戏...",
  other: "其他",
  noGames: "暂无已安装游戏",
  noSearchResults: "未找到匹配的游戏",
  games: "个游戏",
  launch: "启动",
  launching: "启动中...",
  diskUsage: "磁盘占用",
  free: "可用",
  openFolder: "打开目录",
  storePage: "商店页面",
  cancel: "取消",
  fetchError: "获取 Steam 数据失败",
  refresh: "刷新",
  debug: "调试信息",
  noAccounts: '未找到已保存的 Steam 账户，请先在 Steam 客户端登录并勾选"记住密码"',
  noLibraries: "未找到游戏库文件夹，可能是 Steam 数据未加载",
  deleteAccount: "删除账户",
  deleteAccountTitle: "确认删除账户",
  deleteAccountConfirm:
    '确定要删除账户 "{{name}}" 吗？将从本机移除该账户的登录信息，不会影响 Steam 云端数据，下次登录需重新输入密码。',
  deleteAccountSuccess: "账户已删除",
  deleteAccountError: "删除账户失败",
  deleteAccountCloseSteam: "Steam 正在运行，将自动关闭 Steam 后执行删除",
  lastPlayed: "最近游玩",
  inventoryGames: "库存游戏",
  inventorySourceCache: "本地缓存",
  inventorySourceOnline: "在线库存",
  inventoryOnlineFailed: "在线不可用",
  inventoryOnlineHint: "配置 API Key 可显示账号拥有的全部游戏（设置页）",
  currentAccount: "当前账号",
  inventoryTotal: "库存总数",
  inventoryInstalledCount: "已安装",
  inventoryNotInstalledCount: "未安装",
  totalPlaytime: "总游玩时长",
  inventoryEmpty: "暂无库存游戏数据",
  installedBadge: "已安装",
  notInstalledBadge: "未安装",
  loadMore: "加载更多",
  // 🔴 `install` / `installSent` 已随 `install_steam_game` 命令删除（用户裁决 2026-10-03）：
  //    本模块不再提供安装动作，未安装游戏主按钮改用 `notInstalledBadge` 置灰。
  loading: "正在加载 Steam 数据...",
  // 🔴 2026-10-04 布局重构**新增**（原 56 键一个未删、语义未改）。
  // 新增项全部服务于「统一卡片列表 + 筛选」：原「库存游戏」与「已安装游戏」两个区块
  // 合并为一个列表后，需要筛选档位与账户入口的紧凑呈现。
  filterAll: "全部",
  filterInstalled: "已安装",
  filterNotInstalled: "未安装",
  accountMenu: "账户",
  switchTo: "切换到此账户",
  statsTitle: "库信息",
  noGamesFiltered: "该筛选下暂无游戏",
} as const;

type SteamTKey = keyof typeof STEAM_STRINGS;

/**
 * 内置中文文案查表；`{{name}}` 占位符按 i18next 同语义插值。
 *
 * 键名兼容两种形态：`t("launch")` 与 NexBox 原件的 `t("steam.launch")` 等价。
 * 前缀剥离在此处完成，故 64 处调用点保持 NexBox 原样逐字未改。
 */
function t(key: SteamTKey | `steam.${SteamTKey}`, vars?: Record<string, unknown>): string {
  const k = (key.startsWith("steam.") ? key.slice(6) : key) as SteamTKey;
  let s: string = STEAM_STRINGS[k] ?? String(key);
  if (vars) {
    for (const [name, v] of Object.entries(vars)) {
      s = s.replaceAll(`{{${name}}}`, String(v ?? ""));
    }
  }
  return s;
}

// ======================== 类型定义 ========================

interface SteamInstallInfo {
  installed: boolean;
  install_path: string | null;
  is_running: boolean;
}

interface SteamUser {
  steam_id64: string;
  account_name: string;
  persona_name: string;
  most_recent: boolean;
  remember_password: boolean;
  timestamp: number;
  avatar_url: string | null;
  avatar_medium_url: string | null;
  avatar_full_url: string | null;
}

interface SteamAvatarInfo {
  steam_id64: string;
  avatar_url: string | null;
  avatar_medium_url: string | null;
  avatar_full_url: string | null;
}

interface SteamLibrary {
  index: number;
  path: string;
  label: string;
  total_size: number;
  free_size: number;
  apps: string[];
}

interface SteamGame {
  app_id: number;
  name: string;
  install_dir: string;
  library_path: string;
  size_on_disk: number;
  state_flags: number;
  last_updated: number;
  last_owner: string;
  build_id: number;
  bytes_to_download: number;
  bytes_downloaded: number;
  playtime_minutes: number;
  last_played: number;
}

interface SteamAllData {
  install_info: SteamInstallInfo;
  users: SteamUser[];
  libraries: SteamLibrary[];
  games: SteamGame[];
}

interface SteamInventoryGame {
  app_id: number;
  name: string;
  installed: boolean;
  playtime_minutes: number;
  last_played: number;
  size_on_disk: number;
  install_dir: string;
  library_path: string;
}

interface SteamInventoryStats {
  total: number;
  installed: number;
  not_installed: number;
  total_playtime_minutes: number;
}

interface SteamInventoryUser {
  steam_id64: string;
  account_name: string;
  persona_name: string;
}

interface SteamInventoryData {
  source: "online" | "cache" | "none";
  steam_running: boolean;
  current_user: SteamInventoryUser | null;
  stats: SteamInventoryStats;
  games: SteamInventoryGame[];
  error: string | null;
}

// ======================== 在线库存（2026-10-05 自组件内上移）========================
// 纯类型 + 纯函数：不依赖任何组件状态/props，放模块级 —— 根除 fetchInventory 的
// react-hooks/exhaustive-deps 警告，且避免每次渲染重建函数体。

/** 在线库存的数据形状（后端 `SteamOnlineInventory` 的前端投影） */
interface SteamOnlineInventory {
  source: string;
  total: number;
  installed: number;
  not_installed: number;
  total_playtime_minutes: number;
  games: { app_id: number; name: string; playtime_minutes: number; last_played: number }[];
  error: string | null;
}

/** 库存空态（与后端 `none` 分支同形，避免各处手写字面量） */
function emptyInventoryData(): SteamInventoryData {
  return {
    source: "none",
    steam_running: false,
    current_user: null,
    stats: { total: 0, installed: 0, not_installed: 0, total_playtime_minutes: 0 },
    games: [],
    error: null,
  };
}

/**
 * 把在线清单合并进本地库存（`installed` 以本地 .acf 为准）。
 *
 * 规则（与后端 `inventory_online.rs` 的口径一致）：
 * - 本地已有的条目：只刷新 `playtime_minutes` / `last_played`（在线值更权威），
 *   **不覆盖** `installed` / `size_on_disk` / `install_dir`（只有本地知道这些）；
 * - 在线独有的条目：`installed` 由 `installed_ids` 判定（后端读 .acf 给的），
 *   缺省字段补 0 / 空串。
 */
function mergeOnlineInventory(
  local: SteamInventoryData | null,
  online: SteamOnlineInventory,
  installedIds: Set<number>,
): SteamInventoryData {
  const byId = new Map<number, SteamInventoryGame>(
    (local?.games ?? []).map((g) => [g.app_id, { ...g }]),
  );
  for (const og of online.games) {
    const exist = byId.get(og.app_id);
    if (exist) {
      exist.playtime_minutes = Math.max(exist.playtime_minutes, og.playtime_minutes);
      exist.last_played = Math.max(exist.last_played, og.last_played);
      if (!exist.name) exist.name = og.name;
    } else {
      byId.set(og.app_id, {
        app_id: og.app_id,
        name: og.name,
        installed: installedIds.has(og.app_id),
        playtime_minutes: og.playtime_minutes,
        last_played: og.last_played,
        size_on_disk: 0,
        install_dir: "",
        library_path: "",
      });
    }
  }
  const games = Array.from(byId.values());
  const installed = games.filter((g) => g.installed).length;
  return {
    ...(local ?? emptyInventoryData()),
    source: "online",
    games,
    stats: {
      total: games.length,
      installed,
      not_installed: games.length - installed,
      total_playtime_minutes: games.reduce((s, g) => s + g.playtime_minutes, 0),
    },
  };
}

// ======================== 工具函数 ========================

function formatSize(bytes: number): string {
  if (bytes === 0) return "0 B";
  const units = ["B", "KB", "MB", "GB", "TB"];
  let size = bytes;
  let unitIndex = 0;
  while (size >= 1024 && unitIndex < units.length - 1) {
    size /= 1024;
    unitIndex++;
  }
  return `${size.toFixed(1)} ${units[unitIndex]}`;
}

/** 分钟 → "X小时Y分钟"，不足 1 小时只显示分钟 */
function formatPlaytime(minutes: number): string {
  if (minutes <= 0) return "";
  const hours = Math.floor(minutes / 60);
  const mins = minutes % 60;
  if (hours > 0 && mins > 0) return `${hours}小时${mins}分钟`;
  if (hours > 0) return `${hours}小时`;
  return `${mins}分钟`;
}

/** 最近游玩相对时间（X天前 / X小时前 / 日期） */
function formatLastPlayed(timestamp: number): string {
  if (timestamp <= 0) return "";
  const diff = Date.now() / 1000 - timestamp;
  if (diff < 3600) return "刚刚";
  if (diff < 86400) return `${Math.floor(diff / 3600)}小时前`;
  if (diff < 86400 * 7) return `${Math.floor(diff / 86400)}天前`;
  return new Date(timestamp * 1000).toLocaleDateString("zh-CN", {
    month: "2-digit",
    day: "2-digit",
  });
}

function steamId64To32(id64: string): string {
  try {
    const id = BigInt(id64);
    const base = BigInt(76561197960265728);
    if (id > base) return (id - base).toString();
    return id.toString();
  } catch {
    return id64;
  }
}

function normalizePath(path: string): string {
  return path.replace(/\//g, "\\").toLowerCase().replace(/\\+$/, "");
}

/** 从库路径中提取盘符，如 "D:\SteamLibrary" → "D:" */
function getDriveLabel(path: string): string {
  const m = path.match(/^[A-Za-z]:/);
  return m ? m[0].toUpperCase() : path;
}

// ======================== 组件 ========================

function StatusBadge({ running }: { running: boolean }) {
  // 🔴 2026-10-05 token 化（真机截图 + CDP 假后端实测取证）：原 colorScheme
  // green/gray + subtle 走 Chakra 双模式语义，colorMode 脱节（localStorage 遗留
  // light）时渲染成 gray.100 亮白底 pill —— 深色界面上全屏最亮的元素，负面状态
  // 反成视觉焦点。改 aegis token：CSS 变量不经 colorMode 分支、任何状态下都按
  // 激活包解析（含 portal 子树）；凹陷面 + hairline 边 + ink 文字，圆点 success/stone。
  return (
    <Badge
      bg="var(--aegis-surface-inset)"
      border="1px solid var(--aegis-hairline)"
      color="var(--aegis-ink)"
      borderRadius="full"
      px={3}
      py={1}
      fontSize="xs"
    >
      <HStack spacing={1.5}>
        <Box w={2} h={2} borderRadius="full" bg={running ? "aegis.success" : "aegis.stone"} />
        <Text>{running ? t("steam.running") : t("steam.notRunning")}</Text>
      </HStack>
    </Badge>
  );
}
// 分离的游戏封面组件
// 🔴 `onUninstall` 与「卸载」菜单项已按用户裁决（2026-10-03）删除：本项目只做展示 +
// 启动，安装与卸载均交由 Steam 客户端管理（规格 08 §一）。菜单保留「打开目录 / 商店页面」。
const GameActionMenu = memo(function GameActionMenu({
  game,
  onOpenFolder,
  onStorePage,
}: {
  game: SteamGame;
  onOpenFolder: (libPath: string, installDir: string) => void;
  onStorePage: (appId: number) => void;
}) {
  return (
    <Menu>
      <MenuButton
        as={IconButton}
        aria-label="more"
        icon={<EllipsisVertical size={14} />}
        size="xs"
        variant="outline"
        borderRadius="lg"
      />
      <Portal>
        <MenuList fontSize="sm" zIndex={9999}>
          <MenuItem
            icon={<Folder size={14} />}
            onClick={() => onOpenFolder(game.library_path, game.install_dir)}
          >
            {t("steam.openFolder")}
          </MenuItem>
          <MenuItem icon={<ExternalLink size={14} />} onClick={() => onStorePage(game.app_id)}>
            {t("steam.storePage")}
          </MenuItem>
        </MenuList>
      </Portal>
    </Menu>
  );
});
// ============================================================================
// 统一游戏卡片（2026-10-04 布局重构）
//
// 🔴 合并背景：原先有**两个**卡片组件 —— `GameCard`（已安装，data.games）与
// `InventoryGameCard`（库存，inventory.games）。二者用同一套 SystemToolkit 规格组件
// （`GameCardShell` / `GameCover` / `CardName` / `CardMetaRow` / `CardActionRow`）、
// 同一批元数据（名称 · 占用 · 时长 · 最近游玩），内容高度重叠 —— 而「已安装」在数据上
// 本就是「库存」的子集（库存每项自带 `installed` 布尔）。两个区块 = 用户要跨两处
// 才能建立「我有这些游戏」的心智模型。
//
// 本组件是二者的**并集**，按 `installed` 决定差异部分，逐条对应关系：
//   · 徽章：已安装 → `installedBadge`；下载中 → 百分比（**互斥**，见下）
//   · 主按钮：已安装 → 「启动」；未安装 → **不渲染**（无动作不渲染按钮，R-13）
//   · ⋯ 菜单：**仅已安装**（打开目录需要 `install_dir` / `library_path`，
//     库存的未安装条目这两个字段是空串，菜单会变成一个点了报错的死按钮）
//
// 🔴 徽章互斥性的坑（原 InventoryGameCard 的注释已记）：判据必须是**精确的下载中状态**，
// 不用「未安装取反」—— 否则「下载中」与「已安装」两层半透明遮罩叠加压不住。
// ============================================================================

/** 库存条目 + 已安装专有字段（`install_dir` / `library_path` / 下载进度从 data.games 补齐） */
interface UnifiedGame extends SteamInventoryGame {
  /** 从 `data.games` 按 app_id 匹配到的已安装条目（未安装则为 undefined） */
  installedEntry?: SteamGame;
  /** 已安装条目的盘符标签（如 `D:`），用于卡片上的库归属提示 */
  driveLabel?: string;
}

const UnifiedGameCard = memo(function UnifiedGameCard({
  game,
  isLaunching,
  onLaunch,
  onOpenFolder,
  onStorePage,
}: {
  game: UnifiedGame;
  isLaunching: boolean;
  onLaunch: (appId: number) => void;
  onOpenFolder: (libPath: string, installDir: string) => void;
  onStorePage: (appId: number) => void;
}) {
  const entry = game.installedEntry;
  // 下载中判据 = 精确状态（有下载量且未完成）；只有已安装条目才有这三个字段
  const isDownloading =
    entry != null && entry.bytes_downloaded > 0 && entry.bytes_downloaded < entry.bytes_to_download;
  const downloadProgress =
    isDownloading && entry.bytes_to_download > 0
      ? Math.round((entry.bytes_downloaded / entry.bytes_to_download) * 100)
      : 0;

  // 占用以已安装条目为准（本地 .acf 才有真实值）；未安装条目 size_on_disk 为 0 → 显示「-」
  const size = entry != null ? entry.size_on_disk : game.size_on_disk;
  const playtime = game.playtime_minutes;
  const lastPlayed = game.last_played;

  return (
    <GameCardShell title={game.name}>
      <Box position="relative">
        <GameCover appId={game.app_id} name={game.name} />
        {/* 🔴 徽章互斥：下载中优先（它更具体、时效更短），否则已安装才亮。
            两个都 true 的情况 = 正在更新一个已安装的游戏，此时「更新中 42%」比
            「已安装」更有信息量。 */}
        {isDownloading ? (
          <CoverStateBadge>{downloadProgress}%</CoverStateBadge>
        ) : game.installed ? (
          <CoverStateBadge>{t("steam.installedBadge")}</CoverStateBadge>
        ) : null}
      </Box>

      <Box p={CARD_INNER_PADDING}>
        <CardName>{game.name}</CardName>

        {/* 元数据行：占用 · 时长 · 最近游玩（SystemToolkit :565-570 规格，三段 + 两圆点）*/}
        <CardMetaRow>
          <CardMetaText>{size > 0 ? formatSize(size) : "-"}</CardMetaText>
          <MetaDot />
          <CardMetaText>{playtime > 0 ? formatPlaytime(playtime) : "-"}</CardMetaText>
          <MetaDot />
          <CardMetaText>{lastPlayed > 0 ? formatLastPlayed(lastPlayed) : "-"}</CardMetaText>
        </CardMetaRow>

        <CardActionRow>
          {/* 🔴 2026-10-05 主次修正（截图 ×4 放大取证）：原布局次动作「商店页面」
              作为 grid 子项被拉伸撑满左格（143px），主动作「启动」在 relative Box 内
              auto 宽（~54px）靠左、⋯ 菜单绝对定位孤悬右缘 —— 视觉主次颠倒。
              统一为「格 Box 撑满 + 内部按钮 width:100%」：已安装 = [启动 flex:1 | ⋯]，
              未安装 = 商店页横跨两列（单键缩在半边失衡）。
              CardActionButton 是 SystemToolkit ActionPrimaryButton 的复刻组件，
              契约不透传布局 props（见 game-card.tsx）⇒ 布局收敛在使用处 Box，
              复刻组件定义零改动。R-13 依旧：未安装无动作不渲染主按钮。 */}
          <Box
            gridColumn={game.installed ? undefined : "1 / -1"}
            sx={{ "& > button": { width: "100%" } }}
          >
            <CardActionButton onClick={() => onStorePage(game.app_id)}>
              {t("steam.storePage")}
            </CardActionButton>
          </Box>
          <Box display="flex" gap="8px" alignItems="center" minW={0}>
            {game.installed && (
              <Box flex={1} minW={0} sx={{ "& > button": { width: "100%" } }}>
                <CardActionButton
                  primary
                  isLoading={isLaunching}
                  onClick={() => onLaunch(game.app_id)}
                >
                  {t("steam.launch")}
                </CardActionButton>
              </Box>
            )}
            {/* ⋯ 菜单**仅已安装**：打开目录需要 install_dir + library_path，
                未安装条目这两项是空串，渲染出来就是个点了报错的死按钮。
                2026-10-05 由绝对定位改为 flex 正常流（主动作右侧、gap 8）。 */}
            {game.installed && entry && (
              <GameActionMenu game={entry} onOpenFolder={onOpenFolder} onStorePage={onStorePage} />
            )}
          </Box>
        </CardActionRow>
      </Box>
    </GameCardShell>
  );
});

function LibraryCard({
  lib,
  gameCount,
  gameSize,
}: {
  lib: SteamLibrary;
  gameCount: number;
  gameSize: number;
}) {
  const { liquidGlassEnabled } = useBackground();
  const { getActiveColor } = useThemeColor();
  const { colorMode } = useColorMode();
  const headerColor = colorMode === "light" ? "#000000" : "#ffffff";
  const subTextColor = useColorModeValue("gray.500", "aegis.mute");
  // 🔴 2026-10-05：原为硬编码 `#1a1a1a`，深色下与 canvas 的层差仅 1.068 ⇒ 面板「融」进背景。
  // 改走 surface-soft 语义键：层差 1.127，且换包自动跟随（硬编码不跟随）。
  // 同批另有 inputBg（line ~646）与 pageBg 一并改，四处口径统一。
  const cardBg = useColorModeValue("white", "aegis.surface-soft");
  const borderColor = useColorModeValue("gray.200", "aegis.hairline");
  const progressBg = useColorModeValue("gray.100", "aegis.surface-inset");
  const activeColor = getActiveColor();

  // 使用实际磁盘容量计算占用百分比
  const totalDisk = lib.total_size > 0 ? lib.total_size : gameSize;
  const usedByGames = gameSize;
  const usedPercent = totalDisk > 0 ? (usedByGames / totalDisk) * 100 : 0;
  const usedPercentStr = totalDisk > 0 ? usedPercent.toFixed(1) : "0.0";

  // 显示：游戏大小 / 磁盘总容量
  const sizeText =
    lib.total_size > 0
      ? `${formatSize(gameSize)} / ${formatSize(totalDisk)}`
      : formatSize(gameSize);

  const inner = (
    <HStack spacing={3} align="start">
      <Box color={activeColor} mt={1}>
        <HardDrive size={20} />
      </Box>
      <Box flex={1} minW={0}>
        <Tooltip label={lib.path}>
          <Text fontSize="sm" fontWeight="medium" color={headerColor} noOfLines={1}>
            {lib.path}
          </Text>
        </Tooltip>
        {lib.label && (
          <Text fontSize="xs" color={subTextColor}>
            {lib.label}
          </Text>
        )}
        <HStack spacing={3} mt={1} fontSize="2xs" color={subTextColor}>
          <Text>
            {gameCount} {t("steam.games")}
          </Text>
          <Text>{sizeText}</Text>
        </HStack>
        {/* 磁盘占用进度条 - 始终显示，最小宽度 2px */}
        <Box mt={2} w="100%" h={1.5} bg={progressBg} borderRadius="full" overflow="hidden">
          <Box
            h="100%"
            bg={activeColor}
            borderRadius="full"
            minW="2px"
            w={`${Math.min(usedPercent, 100).toFixed(1)}%`}
          />
        </Box>
        <Text fontSize="2xs" color={subTextColor} mt={1}>
          {t("steam.diskUsage")}: {usedPercentStr}%{" "}
          {lib.free_size > 0 ? `· ${t("steam.free")}: ${formatSize(lib.free_size)}` : ""}
        </Text>
      </Box>
    </HStack>
  );

  if (liquidGlassEnabled) {
    return <LiquidGlassCard p={4}>{inner}</LiquidGlassCard>;
  }

  return (
    <Box bg={cardBg} border="1px solid" borderColor={borderColor} borderRadius="xl" p={4}>
      {inner}
    </Box>
  );
}

// ======================== 主页面 ========================

/** 品牌区副标题（模块级常量，2026-10-05 用户裁决改为 GameManager）。
 *  🔴 必须是模块级常量：useRegisterBrand 的依赖数组含 sub，每次渲染新建字符串
 *     会让品牌区每帧重渲染（与 overview 的 actions / music 的 sub 同一纪律）。 */
const GAME_PAGE_SUB = "GameManager";

/**
 * 首帧渲染的卡片数（2026-10-05 修「切到本页卡顿」）。
 *
 * 🔴 为什么是 24：内容区宽 1156、卡片 320+14 ⇒ 每行 3 张 ⇒ 24 张 ≈ 8 行，
 * 1280×800 窗口下**首屏能完整看到**（工具栏 + 品牌区约占 120px，8 行 ×≈330px 略溢出，
 * 但 6 行以内必定在视野内）。定这个数的判据是「首屏可见量」，不是拍脑袋的整数。
 * ⚠️ 若将来窗口更小或卡片更宽，此值应随之下调；反之可上调，但超过 36 收益递减
 * （第二帧的补齐成本开始显现）。
 */
const BATCH_FIRST = 24;

export default function SteamPage() {
  const toast = useDynamicIsland("gamepad");
  const { liquidGlassEnabled } = useBackground();
  const { getActiveColor } = useThemeColor();
  const { colorMode } = useColorMode();

  const [loading, setLoading] = useState(true);
  const [data, setData] = useState<SteamAllData | null>(null);
  const [searchText, setSearchText] = useState("");
  const [switchingAccount, setSwitchingAccount] = useState<string | null>(null);
  const [launchingAppId, setLaunchingAppId] = useState<number | null>(null);
  const [launchingSteam, setLaunchingSteam] = useState(false);
  const [showLibraries, setShowLibraries] = useState(true);
  // 🟢 2026-10-04 重构：`showUsers` / `showInventory` 已**删除**（非静默移除，见下）。
  // 原先「账户管理」与「库存游戏」是两个可折叠的独立区块，各带一个展开/收起开关。
  // 重构后：账户收进顶栏 Menu（低频操作，不再需要折叠）；库存与已安装**合并为一个
  // 列表**（区块消失，折叠开关随之失去对象）。两处的**内容**均完整迁移到新位置，
  // 只是不再有「折叠」这个交互 —— 这是布局重排的必然结果，非功能丢失。
  const [inventory, setInventory] = useState<SteamInventoryData | null>(null);
  const [loadingInventory, setLoadingInventory] = useState(false);
  // 🔴 2026-10-04 布局重构新增：统一卡片列表的筛选档（合并了原先的「库存游戏」与
  // 「已安装游戏」两个区块）。默认 `all` —— 首屏即用户的完整游戏库。
  const [gameFilter, setGameFilter] = useState<GameFilter>("all");
  // Aegis 新增：在线补全失败原因（null = 未尝试或成功）
  const [onlineError, setOnlineError] = useState<string | null>(null);
  // 库存分页加载数量：默认只渲染前 120 张卡片，按「加载更多」递增，避免大库存一次渲染过重导致卡顿
  const [visibleCount, setVisibleCount] = useState(120);
  // 延迟值：搜索输入即时响应，列表过滤/渲染延后执行，输入上千条库存时避免每次按键都重算
  const deferredSearch = useDeferredValue(searchText);

  const [deleteTarget, setDeleteTarget] = useState<SteamUser | null>(null);
  const { isOpen: isDeleteOpen, onOpen: onDeleteOpen, onClose: onDeleteClose } = useDisclosure();
  const cancelRef = useRef<HTMLButtonElement>(null);

  const activeColor = getActiveColor();
  const headerColor = colorMode === "light" ? "#000000" : "#ffffff";
  const adaptiveTitle = useAdaptiveTextColor();
  const textColor = useColorModeValue("gray.800", "aegis.ink");
  const subTextColor = useColorModeValue("gray.500", "aegis.mute");
  const cardBg = useColorModeValue("white", "aegis.surface-soft");
  const borderColor = useColorModeValue("gray.200", "aegis.hairline");
  const inputBg = useColorModeValue("white", "aegis.surface-soft");
  /**
   * 🟢 2026-10-04 重构：sticky 顶栏的底色。
   * 🔴 必须**不透明**：顶栏与筛选器是 sticky 的，滚动时卡片会从其下方穿过；
   * 半透明底色会让卡片文字与顶栏文字叠影（这是典型的 sticky 布局事故）。
   * 取`canvas` 而非 `surface-soft` —— 与页面底色一致，视觉上「浮起」只靠一条发丝线。
   * 🔴 2026-10-05：原为硬编码 `#0a0a0a`，与当时的 canvas `#0d0d0d` 层差仅 1.066 ⇒ 顶栏偏「融」。
   * 改走 `canvas` 语义键后自动跟随主题包，且**与页面底完全一致**（1.000）——
   * 这正是本注释要的效果：「浮起」只靠 `borderColor` 的发丝线，不靠亮度差。
   */
  const pageBg = useColorModeValue("white", "aegis.canvas");
  /**
   * 头像异步补全的 loading 集合（`fetchData` 内的 `get_steam_user_avatars` 二次调用写入）。
   * 账户进Menu 后不再逐卡渲染头像，但该集合仍用于「头像还没到」时显示占位图标。
   */
  const [loadingAvatars, setLoadingAvatars] = useState<Set<string>>(new Set());

  const fetchData = useCallback(async () => {
    setLoading(true);
    try {
      const result = await invoke<SteamAllData>("get_steam_all_data", "Steam", "FetchAllData");
      setData(result);
      // 异步获取用户头像
      if (result.users.length > 0) {
        const userIds = new Set(result.users.map((u) => u.steam_id64));
        setLoadingAvatars(userIds);
        invoke<SteamAvatarInfo[]>("get_steam_user_avatars", "Steam", "FetchAvatars")
          .then((avatars) => {
            if (avatars.length > 0) {
              const avatarMap = new Map<string, SteamAvatarInfo>();
              for (const a of avatars) {
                avatarMap.set(a.steam_id64, a);
              }
              setData((prev) => {
                if (!prev) return prev;
                return {
                  ...prev,
                  users: prev.users.map((u) => {
                    const info = avatarMap.get(u.steam_id64);
                    if (info) {
                      return {
                        ...u,
                        avatar_url: info.avatar_url,
                        avatar_medium_url: info.avatar_medium_url,
                        avatar_full_url: info.avatar_full_url,
                      };
                    }
                    return u;
                  }),
                };
              });
            }
            setLoadingAvatars(new Set());
          })
          .catch((err) => {
            console.warn("Failed to fetch Steam avatars:", err);
            setLoadingAvatars(new Set());
          });
      }
    } catch (err) {
      console.error("Failed to fetch Steam data:", err);
      toast({
        title: t("steam.fetchError"),
        status: "error",
        duration: 3000,
        isClosable: true,
      });
    } finally {
      setLoading(false);
    }
  }, [toast]);

  // 库存加载 = NexBox 的本地缓存（底）+ Aegis 新增的在线清单（覆盖）
  //
  // 🔴 合并顺序不可颠倒：本地先落 `installed` / `size_on_disk` / `install_dir`，
  //    在线再补「账号拥有但本机没装」的那些，并刷新游玩时长/最近游玩。
  //    在线失败**不报错、不清空** —— 静默退回本地并在来源徽章上标明（规格 08 F-GM-03）。
  const fetchInventory = useCallback(async (forceOnline: boolean) => {
    setLoadingInventory(true);
    try {
      const local = await invoke<SteamInventoryData>(
        "get_steam_inventory",
        "Steam",
        "FetchInventory",
        { forceOnline },
      );
      setInventory(local);

      // 在线补全（Aegis 新增，2026-10-03 用户裁决）
      try {
        const dir = await steamDirFor();
        if (!dir) return; // 无 Steam 客户端，本地已是 none 态
        const online = await invoke<SteamOnlineInventory>(
          "get_steam_inventory_online",
          "Steam",
          "FetchInventoryOnline",
          { steamDir: dir },
        );
        if (online.source === "online") {
          const installedIds = new Set(
            (await invoke<number[]>("steam_installed_appids", "Steam", "FetchInstalledAppIds", {
              steamDir: dir,
            })) ?? [],
          );
          setInventory((prev) => mergeOnlineInventory(prev, online, installedIds));
          setOnlineError(null);
        } else {
          setOnlineError(online.error ?? "在线库存不可用");
        }
      } catch (err) {
        // 🔴 不静默吞：记入状态并交给来源徽章显示
        setOnlineError(err instanceof Error ? err.message : String(err));
      }
    } catch (err) {
      console.error("Failed to fetch Steam inventory:", err);
    } finally {
      setLoadingInventory(false);
    }
  }, []);

  useEffect(() => {
    fetchData();
    fetchInventory(false);
  }, [fetchData, fetchInventory]);

  // ══ 统一游戏列表（2026-10-04 布局重构）══════════════════════════════════
  //
  // 原先是两条独立管道（`filteredGames` 走 data.games、`filteredInventory` 走
  // inventory.games），渲染成两个区块。现合并为**一条**：
  //
  //   1. 以 `inventory.games` 为**主源**（它更全：含「已安装」也含「未拥有」）；
  //   2. 按 app_id 到 `data.games` 查已安装条目，补齐库存没有的三个字段
  //      （`install_dir` / `library_path` / 下载进度）—— 只有本地 .acf 知道这些；
  //   3. 搜索 + 筛选 + 分页在此一次完成。
  //
  // 🔴 为什么要以库存为主源而不是已安装列表：用户诉求是「第一眼看到自己的游戏库」，
  // 而「游戏库」在 Steam 语义里是**账号拥有的全部游戏**（含未安装），不只是本机装了的。
  // 只显示已安装 = 少了 90% 的可见内容（实测典型比例 12 已装 / 116 未装）。
  //
  // 🔴 等价性核对（对照重构前的基线）：
  //   · 旧 `filteredGames`（已安装+搜索） = 新 `installed` 档 ∘ 搜索 →✅ 覆盖
  //   · 旧 `filteredInventory`（库存+搜索） = 新 `all` 档 ∘ 搜索 → ✅ 覆盖
  //   · `app_id` 数字匹配（旧两处都有）→ 保留
  //   · `deferredSearch`（延迟值）→ 保留，仍是搜索专用

  /** app_id → 已安装条目（用于补齐 + 判定）。查一次建Map，避免 O(n·m) 遍历。 */
  const installedById = useMemo(() => {
    const m = new Map<number, SteamGame>();
    for (const g of data?.games ?? []) m.set(g.app_id, g);
    return m;
  }, [data?.games]);

  /** 库路径前缀 → 盘符标签（用于给已安装卡片标归属库） */
  const driveByPath = useMemo(() => {
    const arr = (data?.libraries ?? []).map((lib) => ({
      norm: normalizePath(lib.path),
      drive: getDriveLabel(lib.path),
    }));
    return (p: string): string | undefined => {
      if (!p) return undefined;
      const n = normalizePath(p);
      const hit = arr.find((l) => n === l.norm || n.startsWith(l.norm + "\\"));
      return hit?.drive;
    };
  }, [data?.libraries]);

  /** 合并后的全集（未搜索、未筛选、未分页） */
  const unifiedAll = useMemo<UnifiedGame[]>(() => {
    const list = inventory?.games ?? [];
    return list.map((g) => {
      const entry = installedById.get(g.app_id);
      // 🔴 `installed` 以**库存的布尔**为准（它已由 `steam_installed_appids` 判定过）；
      // `entry` 存在与否是独立事实（.acf 里有但不在库存列表 —— 极少见，仍按已安装处理）。
      const installed = g.installed || entry != null;
      return {
        ...g,
        installed,
        installedEntry: entry,
        driveLabel: entry ? driveByPath(entry.library_path) : undefined,
      };
    });
  }, [inventory?.games, installedById, driveByPath]);

  /** 搜索 + 筛选（一次算完，避免两处各过滤一遍） */
  const unifiedFiltered = useMemo(() => {
    let list = unifiedAll;
    if (deferredSearch.trim()) {
      const lower = deferredSearch.toLowerCase();
      list = list.filter(
        (g) => g.name.toLowerCase().includes(lower) || g.app_id.toString().includes(lower),
      );
    }
    if (gameFilter === "installed") return list.filter((g) => g.installed);
    if (gameFilter === "notInstalled") return list.filter((g) => !g.installed);
    return list;
  }, [unifiedAll, deferredSearch, gameFilter]);

  /** 三档计数（筛选器上直接可见，是用户判断「该装什么」的关键信息） */
  const filterCounts = useMemo(() => {
    let installed = 0;
    for (const g of unifiedAll) if (g.installed) installed++;
    return { all: unifiedAll.length, installed, notInstalled: unifiedAll.length - installed };
  }, [unifiedAll]);

  /**
   * 分页 + **分批渲染**（2026-10-05 修「切到本页卡顿一下」）。
   *
   * 🔴 根因（用户实测：所有页面切到游戏管理都卡 ⇒ 与来源页无关，是**进入**方向的成本）：
   * `visibleCount` 原为 120，切页挂载时 `unifiedShown` 一次性产出 120 张卡片 DOM，
   * 于同一帧内还要做四件事 —— 120 个 `cardEnter` 动画同时起跑、120 张封面 `<img>`
   * 同帧发起请求、`unifiedAll` 全库对象展开、`ResizeObserver` 量宽触发重排。
   * 这四笔全在主线程一帧内排队 ⇒ 长任务 ⇒ 观感是「切过去顿一下」。
   *
   * 🔴 改法：**首帧只渲染 `BATCH_FIRST` 张，下一帧补齐到 `visibleCount`**。
   *   不是砍数据量（用户仍要看得到全部卡片，仍有「加载更多」），而是把一笔长任务
   *   拆成两笔 —— 首帧成本降到约 1/5，浏览器得以绘制并响应输入，第二帧再补完。
   *   卡片总数与顺序完全不变，用户只是晚一帧看到下方内容。
   *
   * 🔴 为什么不用 `requestIdleCallback`：它在「刚 mount、页面还在 loading 态」时
   *   常常排不进空闲时机（此时主线程正忙），实测不可靠；`rAF` 与渲染时序天然对齐，
   *   保证「本帧绘制完成后」再补第二笔。
   *
   * 🔴 为什么不用 `useDeferredValue` 包 `visibleCount`：它只让**本次渲染**降级，
   *   React 仍会在同一轮把 120 张都算出来，长任务依旧，只是延后一帧 —— 治标不治本。
   */
  const [renderCount, setRenderCount] = useState(BATCH_FIRST);
  useEffect(() => {
    // 首帧已够（数据少）或已在补齐中（count 未到位）⇒ 无需补
    if (renderCount >= visibleCount) return;
    const raf = window.requestAnimationFrame(() => {
      setRenderCount(visibleCount);
    });
    return () => window.cancelAnimationFrame(raf);
  }, [renderCount, visibleCount]);

  // 筛选/搜索变化时回到首批（与 visibleCount 重置同批，见下方两处 setVisibleCount）
  useEffect(() => {
    setRenderCount(BATCH_FIRST);
  }, [deferredSearch, gameFilter]);

  /** 分页：默认 120，「加载更多」递增（保持原值，避免大库存一次渲染过重） */
  const unifiedShown = useMemo(
    () => unifiedFiltered.slice(0, renderCount),
    [unifiedFiltered, renderCount],
  );

  /**
   * 「已安装」档下按盘符分组（原`groupedGames` 的能力，保留）。
   *
   * 🔴 为什么只在已安装档分组：未安装游戏在本机**不存在**归属盘符（`library_path` 为空串），
   * 给它们编一个分组是凭空造信息。故 all/未安装 两档用平铺，已安装档保留盘符分组。
   */
  const unifiedGrouped = useMemo(() => {
    if (gameFilter !== "installed" || !data) return null;
    const groups: {
      key: string;
      driveLabel: string;
      path: string;
      label: string;
      games: UnifiedGame[];
      totalSize: number;
    }[] = [];
    const matched = new Set<number>();
    for (const lib of data.libraries) {
      const libNorm = normalizePath(lib.path);
      const games = unifiedFiltered.filter((g) => {
        if (!g.installedEntry) return false;
        const gn = normalizePath(g.installedEntry.library_path);
        if (gn === libNorm || gn.startsWith(libNorm + "\\")) {
          matched.add(g.app_id);
          return true;
        }
        return false;
      });
      if (games.length === 0) continue;
      groups.push({
        key: `${lib.index}-${lib.path}`,
        driveLabel: getDriveLabel(lib.path),
        path: lib.path,
        label: lib.label,
        games,
        totalSize: games.reduce((s, g) => s + (g.installedEntry?.size_on_disk ?? 0), 0),
      });
    }
    const rest = unifiedFiltered.filter((g) => !matched.has(g.app_id));
    if (rest.length > 0) {
      groups.push({
        key: "other",
        driveLabel: t("steam.other"),
        path: "",
        label: "",
        games: rest,
        totalSize: rest.reduce((s, g) => s + (g.installedEntry?.size_on_disk ?? 0), 0),
      });
    }
    return groups;
  }, [data, unifiedFiltered, gameFilter]);

  // 搜索词变化时重置分页（沿用原逻辑）
  useEffect(() => {
    setVisibleCount(120);
  }, [deferredSearch]);

  // 筛选变化时也重置分页 —— 否则「加载更多」加到 600 的位置切到只剩 3 张的档位会空着
  useEffect(() => {
    setVisibleCount(120);
  }, [gameFilter]);

  // 直接按路径匹配计算每个库的游戏统计
  const getLibraryStats = useCallback(
    (libPath: string) => {
      if (!data?.games) return { count: 0, size: 0 };
      const libNorm = normalizePath(libPath);
      let count = 0,
        size = 0;
      for (const g of data.games) {
        const gameNorm = normalizePath(g.library_path);
        if (gameNorm === libNorm || gameNorm.startsWith(libNorm + "\\")) {
          count++;
          size += g.size_on_disk;
        }
      }
      return { count, size };
    },
    [data],
  );

  const totalSize = useMemo(() => {
    return data?.games.reduce((sum, g) => sum + g.size_on_disk, 0) ?? 0;
  }, [data?.games]);

  const handleLaunch = useCallback(
    async (appId: number) => {
      setLaunchingAppId(appId);
      try {
        await invoke("launch_steam_game", "Steam", "LaunchGame", { appId });
        toast({ title: t("steam.launching"), status: "info", duration: 2000, isClosable: true });
      } catch (err) {
        toast({ title: String(err), status: "error", duration: 2000, isClosable: true });
      } finally {
        setTimeout(() => setLaunchingAppId(null), 2000);
      }
    },
    [toast],
  );

  const handleLaunchSteam = useCallback(async () => {
    if (data?.install_info.is_running) return;
    setLaunchingSteam(true);
    try {
      await invoke("launch_steam_client", "Steam", "LaunchClient");
      toast({ title: t("steam.launchingSteam"), status: "info", duration: 2000, isClosable: true });
      // 延迟刷新状态以检测 Steam 进程
      setTimeout(() => {
        fetchData();
        setLaunchingSteam(false);
      }, 3000);
    } catch (err) {
      toast({ title: String(err), status: "error", duration: 3000, isClosable: true });
      setLaunchingSteam(false);
    }
  }, [data, toast, fetchData]);

  const handleOpenFolder = useCallback(
    async (libraryPath: string, installDir: string) => {
      try {
        await invoke("open_game_folder", "Steam", "OpenGameFolder", { libraryPath, installDir });
      } catch (err) {
        toast({ title: String(err), status: "error", duration: 2000, isClosable: true });
      }
    },
    [toast],
  );

  const handleStorePage = useCallback(
    async (appId: number) => {
      try {
        await invoke("open_steam_store_page", "Steam", "OpenStorePage", { appId });
      } catch (err) {
        toast({ title: String(err), status: "error", duration: 2000, isClosable: true });
      }
    },
    [toast],
  );

  // 🔴 `handleInstall` 已按用户裁决（2026-10-03）删除：本模块只做展示 + 启动，
  //    安装/卸载一律由用户在 Steam 客户端内完成（后端 `install_steam_game` 同批删除）。
  //    未安装的库存游戏其主按钮改为「打开商店页」引导用户去客户端安装。

  const handleSwitchAccount = useCallback(
    async (accountName: string) => {
      setSwitchingAccount(accountName);
      try {
        await invoke("switch_steam_account", "Steam", "SwitchAccount", { accountName });
        toast({
          title: t("steam.switchSuccess"),
          status: "success",
          duration: 3000,
          isClosable: true,
        });
        setTimeout(() => fetchData(), 3000);
      } catch (err) {
        toast({ title: String(err), status: "error", duration: 3000, isClosable: true });
      } finally {
        setSwitchingAccount(null);
      }
    },
    [toast, fetchData],
  );

  const handleDeleteAccount = useCallback(async () => {
    if (!deleteTarget) return;
    try {
      if (data?.install_info.is_running) {
        toast({
          title: t("steam.deleteAccountCloseSteam"),
          status: "info",
          duration: 3000,
          isClosable: true,
        });
      }
      await invoke("delete_steam_account", "Steam", "DeleteAccount", {
        steamId64: deleteTarget.steam_id64,
      });
      toast({
        title: t("steam.deleteAccountSuccess"),
        status: "success",
        duration: 3000,
        isClosable: true,
      });
      setDeleteTarget(null);
      onDeleteClose();
      // 后端删除完成后 vdf 已更新，立即刷新
      fetchData();
    } catch (err) {
      toast({ title: String(err), status: "error", duration: 3000, isClosable: true });
      setDeleteTarget(null);
      onDeleteClose();
    }
  }, [deleteTarget, data, toast, onDeleteClose, fetchData]);

  const onDeleteClick = useCallback(
    (user: SteamUser) => {
      setDeleteTarget(user);
      onDeleteOpen();
    },
    [onDeleteOpen],
  );

  /* ── 品牌区动作（2026-10-05 用户三项裁决：状态徽章 / 启动按钮 / 账户头像昵称直显）──
     🔴 注册必须放在所有 hook 之后、早退分支（loading / 未安装）之前 ——
        useRegisterBrand 内部是 useLayoutEffect，放到早退 return 之后 = 条件调用 hook（违规）。
     🔴 actions 必须 useMemo：useRegisterBrand 的依赖数组含它，每次渲染新建引用
        ⇒ 品牌区每帧重渲染（与 overview 同一纪律）。
     🔴 顺序（用户裁决 2026-10-05）：启动 Steam（未运行时）→ 状态徽章 → 账户头像昵称。
        按钮放徽章**前**（左）：进入 loading 态按钮变宽时向左扩展，
        徽章/账户距右缘距离不变，不会被挤压。
     点击头像弹原有账户菜单（切换 / 删除）。页内工具栏只留筛选 / 搜索 / 刷新同行。 */
  // 🔴 2026-10-05 修复「品牌区头像/昵称不显示」：原实现只认 most_recent ——
  //    后端语义（steam.rs:444）是 vdf 无 MostRecent=1 即 false，而注册表兜底只在
  //    Steam **运行中**生效 ⇒ Steam 未运行 + 无标记时全部为 false ⇒ find 落空
  //    ⇒ 账户区整块不渲染（用户截图实锤：徽章/启动按钮在、头像昵称消失）。
  //    兜底取列表首位（后端已按 MostRecent/timestamp 排序，首位即最可能的当前账户），
  //    仅用于**展示**；菜单里「当前用户」徽章与切换禁用仍以 user.most_recent 为准。
  const currentUser = data
    ? (data.users.find((u) => u.most_recent) ?? data.users[0] ?? null)
    : null;
  const brandActions = useMemo(() => {
    const pageActive = !!data && data.install_info.installed;
    const avatarSrc = currentUser ? currentUser.avatar_medium_url || currentUser.avatar_url : null;
    return (
      <>
        {pageActive && !data.install_info.is_running && (
          <Button
            size="sm"
            leftIcon={<Play size={14} />}
            variant="solid"
            borderRadius="lg"
            bg={activeColor}
            color="white"
            _hover={{ opacity: 0.85, bg: activeColor }}
            isLoading={launchingSteam}
            loadingText={t("steam.launchingSteam")}
            spinnerPlacement="start"
            onClick={handleLaunchSteam}
          >
            {t("steam.launchSteam")}
          </Button>
        )}
        {pageActive && <StatusBadge running={data.install_info.is_running} />}
        {pageActive && currentUser && (
          <Menu>
            <MenuButton
              as={Button}
              size="sm"
              variant="ghost"
              borderRadius="lg"
              color={textColor}
              leftIcon={
                avatarSrc ? (
                  <Image
                    src={avatarSrc}
                    alt=""
                    boxSize="20px"
                    borderRadius="full"
                    onError={(e) => {
                      (e.target as HTMLImageElement).style.display = "none";
                    }}
                  />
                ) : (
                  <CircleUser size={16} color={subTextColor} />
                )
              }
              rightIcon={<ChevronDown size={14} />}
            >
              {currentUser.persona_name || currentUser.account_name}
            </MenuButton>
            <Portal>
              <MenuList fontSize="sm" zIndex={9999} maxH="60vh" overflowY="auto">
                {data && data.users.length > 0 ? (
                  data.users.map((user) => {
                    const displayName = user.persona_name || user.account_name || "Unknown";
                    const userAvatar = user.avatar_medium_url || user.avatar_url;
                    const switching = switchingAccount === user.account_name;
                    return (
                      <MenuItem
                        key={user.steam_id64}
                        // 🔴 当前账户不可点切换（原UserCard 用「不渲染切换按钮」表达，
                        // 菜单里改为保留行但禁用 + 标注「当前」—— 位置更直观）
                        isDisabled={user.most_recent || switching}
                        icon={
                          switching ? (
                            <Spinner size="xs" color={activeColor} />
                          ) : userAvatar ? (
                            <Image
                              src={userAvatar}
                              alt=""
                              boxSize="20px"
                              borderRadius="full"
                              onError={(e) => {
                                (e.target as HTMLImageElement).style.display = "none";
                              }}
                            />
                          ) : loadingAvatars.has(user.steam_id64) ? (
                            <Spinner size="xs" color={activeColor} />
                          ) : (
                            <CircleUser
                              size={16}
                              color={user.most_recent ? activeColor : subTextColor}
                            />
                          )
                        }
                        onClick={() => void handleSwitchAccount(user.account_name)}
                      >
                        <HStack justify="space-between" w="100%" spacing={3}>
                          <Box minW={0}>
                            <Text noOfLines={1}>{displayName}</Text>
                            {/* 🔴 账户 ID 与登录名**保留**（原 UserCard 里有这两行）：
                                切换/删除账户时用户需要凭ID 核对是哪个账号，
                                收进菜单不等于可以省掉这个信息。 */}
                            {user.account_name && (
                              <Text fontSize="2xs" color={subTextColor} noOfLines={1}>
                                {user.account_name}
                              </Text>
                            )}
                            <Text fontFamily="var(--font-mono)" fontSize="2xs" color={subTextColor}>
                              ID: {steamId64To32(user.steam_id64)}
                            </Text>
                          </Box>
                          {user.most_recent && (
                            <Badge colorScheme="green" variant="subtle" fontSize="2xs">
                              {t("steam.currentUser")}
                            </Badge>
                          )}
                        </HStack>
                      </MenuItem>
                    );
                  })
                ) : (
                  <MenuItem isDisabled>{t("steam.noAccounts")}</MenuItem>
                )}
                {data && data.users.length > 0 && <Divider my={1} />}
                {data &&
                  data.users.map((user) => (
                    <MenuItem
                      key={`del-${user.steam_id64}`}
                      color="aegis.error"
                      icon={<Trash2 size={14} />}
                      onClick={() => onDeleteClick(user)}
                    >
                      {t("steam.deleteAccount")} · {user.persona_name || user.account_name}
                    </MenuItem>
                  ))}
              </MenuList>
            </Portal>
          </Menu>
        )}
      </>
    );
  }, [
    data,
    currentUser,
    launchingSteam,
    switchingAccount,
    loadingAvatars,
    activeColor,
    textColor,
    subTextColor,
    handleLaunchSteam,
    handleSwitchAccount,
    onDeleteClick,
  ]);

  useRegisterBrand("/game-manager", { sub: GAME_PAGE_SUB, actions: brandActions });

  // 加载中
  if (loading) {
    return (
      <Box h="calc(100vh - 100px)" display="flex" alignItems="center" justifyContent="center">
        <VStack spacing={4}>
          <Spinner size="xl" thickness="3px" speed="0.65s" color={activeColor} />
          <Text color={subTextColor} fontSize="md">
            {t("steam.loading")}
          </Text>
        </VStack>
      </Box>
    );
  }

  // Steam 未安装
  if (data && !data.install_info.installed) {
    return (
      <Box h="calc(100vh - 100px)" display="flex" alignItems="center" justifyContent="center">
        <VStack spacing={4}>
          <Gamepad2 size={48} color={subTextColor} />
          <Text color={subTextColor} fontSize="lg">
            {t("steam.notInstalled")}
          </Text>
        </VStack>
      </Box>
    );
  }

  // 统计卡片组件
  const StatCard = ({
    icon,
    label,
    value,
  }: {
    icon: React.ReactNode;
    label: string;
    value: string | number;
  }) => {
    if (liquidGlassEnabled) {
      return (
        <LiquidGlassCard px={4} py={2}>
          <HStack spacing={2}>
            <Box color={activeColor}>{icon}</Box>
            <Text fontSize="sm" color={subTextColor}>
              {label}
            </Text>
            <Text fontSize="sm" fontWeight="bold" color={headerColor}>
              {value}
            </Text>
          </HStack>
        </LiquidGlassCard>
      );
    }
    return (
      <Box bg={cardBg} border="1px solid" borderColor={borderColor} borderRadius="xl" px={4} py={2}>
        <HStack spacing={2}>
          <Box color={activeColor}>{icon}</Box>
          <Text fontSize="sm" color={subTextColor}>
            {label}
          </Text>
          <Text fontSize="sm" fontWeight="bold" color={textColor}>
            {value}
          </Text>
        </HStack>
      </Box>
    );
  };

  return (
    <PageBody>
      {/* ══ 2026-10-04 布局重构：sticky 顶栏 ═══════════════════════════════
          原标题栏是「64px Steam 大图标 + 标题 + 搜索 + 刷新」一行，占 64px 高。
          现改为 40px 图标 + **sticky** 顶栏：搜索/ 刷新 / 账户入口收进同一行，
          且滚动时保持可见 —— 用户任何时候都能改筛选或换账户，不必滚回顶部。*/}

      {/* ══ 2026-10-04 布局重构：游戏库前置══════════════════════════════════
          原顺序：统计卡 → 账户管理 → 游戏库 → 库存游戏 →分隔线 → 已安装游戏
          ⇒ 首屏（1240×640）只能看到统计卡与账户第一行，游戏卡片在首屏之外约 400~500px。

          新顺序：顶栏 → 筛选器 → **卡片区（首屏主体）** → 库信息（统计 + 库卡下移）
          账户管理收进顶栏最右的「账户」菜单（Popover），不再占据独立区块。
          另：整页用 `PageBody` 限宽 1400 居中，消除卡片区右侧的大片空白。 */}

      {/* ── 工具栏（sticky）：筛选 / 搜索 / 刷新 同一行 ───────────────────────
          🔴 2026-10-05 用户裁决①：状态标签 / 启动按钮 / 账户信息上收品牌区靠右；
          页内「Steam」图标与大标题删除（与品牌区主标题重复，音乐管理页同款处理）。
          🔴 2026-10-05 用户裁决②：搜索框与「全部/已安装/未安装」筛选器放**同一行**，
          原两行结构（上：搜索+刷新右对齐 / 下：筛选+数据来源）合并为一行 ——
          左组 = 筛选器 + 数据来源徽章，右组 = 搜索 + 刷新；窄宽下 flexWrap 换行。 */}
      <Box
        position="sticky"
        top={0}
        zIndex={10}
        bg={pageBg}
        borderBottom="1px solid"
        borderColor={borderColor}
        pb={3}
        pt={2}
        mb={4}
      >
        {/* 左组：筛选档 + 数据来源徽章
            合并原「库存游戏」与「已安装游戏」两个区块为这一个列表。
            🟢 用分段控件而非下拉：各档**数量**必须同屏可见 —— `未安装 116`
            是用户判断「该装什么」的关键信息，下拉会把它藏起来。 */}
        <HStack justify="space-between" flexWrap="wrap" gap={3}>
          <HStack spacing={3} flexWrap="wrap">
            <FilterSegments
              value={gameFilter}
              onChange={setGameFilter}
              counts={filterCounts}
              activeColor={activeColor}
              items={[
                { key: "all", label: t("steam.filterAll") },
                { key: "installed", label: t("steam.filterInstalled") },
                { key: "notInstalled", label: t("steam.filterNotInstalled") },
              ]}
            />
            {/* 🔴 数据来源必须如实标明（规格 08 §一「如实标注当前数据来源」）：
                online = 已合并在线清单；cache = 本地缓存；徽章 Tooltip 带失败原因。 */}
            {inventory && (
              <HStack spacing={2}>
                {inventory.source === "online" ? (
                  <Badge colorScheme="green" variant="subtle" borderRadius="full">
                    {t("steam.inventorySourceOnline")}
                  </Badge>
                ) : (
                  <Badge colorScheme="orange" variant="subtle" borderRadius="full">
                    {t("steam.inventorySourceCache")}
                  </Badge>
                )}
                {onlineError && (
                  <Tooltip
                    label={
                      onlineError.includes("API Key") ? t("steam.inventoryOnlineHint") : onlineError
                    }
                  >
                    <Badge colorScheme="red" variant="subtle" borderRadius="full" cursor="help">
                      {t("steam.inventoryOnlineFailed")}
                    </Badge>
                  </Tooltip>
                )}
                {loadingInventory && (
                  <Spinner size="sm" color={activeColor} thickness="2px" speed="0.8s" />
                )}
              </HStack>
            )}
          </HStack>
          {/* 右组：搜索 + 刷新（2026-10-05 用户裁决：与筛选器同一行） */}
          <HStack spacing={2}>
            <InputGroup w={{ base: "160px", md: "240px" }}>
              <InputLeftElement
                pointerEvents="none"
                display="flex"
                alignItems="center"
                justifyContent="center"
                h="full"
              >
                <Search size={16} color={subTextColor} />
              </InputLeftElement>
              <Input
                placeholder={t("steam.searchGame")}
                value={searchText}
                onChange={(e) => setSearchText(e.target.value)}
                size="sm"
                borderRadius="lg"
                bg={inputBg}
                borderColor={borderColor}
              />
            </InputGroup>
            <IconButton
              aria-label={t("steam.refresh")}
              icon={<RefreshCw size={16} />}
              size="sm"
              variant="outline"
              borderRadius="lg"
              onClick={() => {
                fetchData();
                fetchInventory(false);
              }}
              isLoading={loading}
            />
          </HStack>
        </HStack>
      </Box>

      {/* ── 卡片区：统一游戏列表（本页主体，首屏即可见）──────────────────── */}
      {loadingInventory && !inventory ? (
        <CardSkeleton count={6} />
      ) : unifiedFiltered.length === 0 ? (
        <VStack py={20} spacing={3}>
          <Gamepad2 size={40} color={subTextColor} />
          <Text color={subTextColor} fontSize="md">
            {searchText
              ? t("steam.noSearchResults")
              : gameFilter === "installed"
                ? t("steam.noGames")
                : t("steam.inventoryEmpty")}
          </Text>
        </VStack>
      ) : (
        <>
          {/* 🔴 `FilterSwap` 在筛选变化时给整块一次极短的压暗回落（160ms 内完成），
              告诉用户「列表换了」。**不逐张重播进场** —— 库存可达上千张，
              逐张播 = 持续抖动且成本随卡片数线性增长；整块一次回落成本恒定。*/}
          <FilterSwap swapKey={gameFilter}>
            {/* 已安装档保留原「按盘符分组」（能力不丢）；all / 未安装两档平铺 ——
                未安装游戏本机无归属盘符（library_path 为空串），编分组是凭空造信息。*/}
            {unifiedGrouped ? (
              <VStack spacing={6} align="stretch">
                {unifiedGrouped.map((group) => (
                  <Box key={group.key}>
                    {/* 🔴 用 GroupHeader（与卡片区同一条 grid 模板）而非普通 HStack：
                        卡片轨道块整体居中，标题若贴左缘会与首卡错开约 65px。*/}
                    <GroupHeader>
                      <HStack spacing={2} flexWrap="wrap">
                        <Box color={activeColor}>
                          <HardDrive size={16} />
                        </Box>
                        <Text
                          fontSize="md"
                          fontWeight="bold"
                          color={adaptiveTitle.text}
                          noOfLines={1}
                        >
                          {group.driveLabel}
                        </Text>
                        {group.path && (
                          <Text fontSize="xs" color={subTextColor} noOfLines={1}>
                            {group.path}
                          </Text>
                        )}
                        {group.label && (
                          <Badge colorScheme="teal" variant="subtle" borderRadius="full">
                            {group.label}
                          </Badge>
                        )}
                        <Badge colorScheme="gray" variant="subtle" borderRadius="full">
                          {group.games.length} {t("steam.games")}
                        </Badge>
                        <Text fontSize="xs" color={subTextColor}>
                          {formatSize(group.totalSize)}
                        </Text>
                      </HStack>
                    </GroupHeader>
                    <StaggeredCards>
                      {group.games.map((game) => (
                        <UnifiedGameCard
                          key={game.app_id}
                          game={game}
                          isLaunching={launchingAppId === game.app_id}
                          onLaunch={handleLaunch}
                          onOpenFolder={handleOpenFolder}
                          onStorePage={handleStorePage}
                        />
                      ))}
                    </StaggeredCards>
                  </Box>
                ))}
              </VStack>
            ) : (
              /* 🔴 animate={renderCount <= BATCH_FIRST}：只在**首批**播进场动画。
                 补齐批（renderCount 已是 120）若照播，下方 96 张会在首帧之后
                 再 staggered 一遍（nth-of-type 的 delay 从 0 重算）⇒ 读成「又弹一下」。
                 分组档（上方 unifiedGrouped）走各自的 group.games，不经 renderCount，
                 故保持默认 animate=true 不变。 */
              <StaggeredCards animate={renderCount <= BATCH_FIRST}>
                {unifiedShown.map((game) => (
                  <UnifiedGameCard
                    key={game.app_id}
                    game={game}
                    isLaunching={launchingAppId === game.app_id}
                    onLaunch={handleLaunch}
                    onOpenFolder={handleOpenFolder}
                    onStorePage={handleStorePage}
                  />
                ))}
              </StaggeredCards>
            )}
          </FilterSwap>

          {/* 「加载更多」：按用户档位 visibleCount 判剩余量，但**等补齐完成再显示**
              （renderCount >= visibleCount）。否则首帧只渲染了 24 张、档位却是 120 时，
              按钮会在下方凭空出现一下再被补齐的卡片顶掉 —— 读成「闪」。 */}
          {unifiedFiltered.length > visibleCount && renderCount >= visibleCount && (
            <VStack mt={4} align="center">
              <Button
                size="sm"
                variant="outline"
                borderRadius="lg"
                onClick={() => {
                  setVisibleCount((c) => c + 120);
                  setRenderCount(BATCH_FIRST);
                }}
              >
                {t("steam.loadMore")} ({unifiedFiltered.length - visibleCount})
              </Button>
            </VStack>
          )}
        </>
      )}

      <Divider my={8} borderColor={borderColor} />

      {/* ══ 库信息（2026-10-04 用户裁决：「游戏库信息下移至卡片列表区域下方」）══
          原先统计卡在**首屏顶部**、游戏库区块在卡片之上，两者都抢占了游戏库的
          首屏位置。现整体下移到卡片列表之后 —— 用户先看到游戏，玩够了再看统计。
          功能零删除：5 张统计卡与库卡（含占用条）全部保留。 */}
      {data && (
        <>
          <HStack spacing={2} mb={4}>
            <Box color={activeColor}>
              <HardDrive size={18} />
            </Box>
            <Text
              fontSize="lg"
              fontWeight="bold"
              color={adaptiveTitle.text}
              textShadow={adaptiveTitle.shadow}
            >
              {t("steam.statsTitle")}
            </Text>
          </HStack>

          <HStack spacing={4} mb={6} flexWrap="wrap">
            <StatCard
              icon={<Gamepad2 size={16} />}
              label={t("steam.totalGames")}
              value={filterCounts.all || data.games.length}
            />
            <StatCard
              icon={<HardDrive size={16} />}
              label={t("steam.totalSize")}
              value={formatSize(totalSize)}
            />
            <StatCard
              icon={<Users size={16} />}
              label={t("steam.accountCount")}
              value={data.users.length}
            />
            <StatCard
              icon={<HardDrive size={16} />}
              label={t("steam.libraries")}
              value={data.libraries.length}
            />
            {data.install_info.install_path && (
              <StatCard
                icon={<HardDrive size={16} />}
                label={t("steam.installPath")}
                value={data.install_info.install_path}
              />
            )}
            {/* 原「库存」区块的 5 张统计（当前账号/库存总数/已装/未装/总时长）
                一并下移至此 —— 合并区块后它们不再是独立区块的标题附属品。 */}
            {inventory?.current_user && (
              <StatCard
                icon={<CircleUser size={16} />}
                label={t("steam.currentAccount")}
                value={inventory.current_user.persona_name || inventory.current_user.account_name}
              />
            )}
            <StatCard
              icon={<Clock3 size={16} />}
              label={t("steam.totalPlaytime")}
              value={formatPlaytime(inventory?.stats.total_playtime_minutes ?? 0) || "0"}
            />
          </HStack>

          {/* 游戏库（每个盘一张，含占用进度条）—— 原「游戏库」区块的折叠能力保留 */}
          <Box mb={6}>
            <HStack
              justify="space-between"
              cursor="pointer"
              onClick={() => setShowLibraries(!showLibraries)}
              mb={showLibraries ? 3 : 0}
            >
              <HStack spacing={2}>
                <Box color={activeColor}>
                  <HardDrive size={18} />
                </Box>
                <Text
                  fontSize="lg"
                  fontWeight="bold"
                  color={adaptiveTitle.text}
                  textShadow={adaptiveTitle.shadow}
                >
                  {t("steam.libraries")}
                </Text>
                <Badge colorScheme="teal" variant="subtle" borderRadius="full">
                  {data.libraries.length}
                </Badge>
              </HStack>
              <IconButton
                aria-label="toggle"
                icon={showLibraries ? <ChevronUp size={16} /> : <ChevronDown size={16} />}
                size="sm"
                variant="ghost"
              />
            </HStack>
            <Collapse in={showLibraries}>
              {data.libraries.length > 0 ? (
                <SimpleGrid columns={{ base: 1, sm: 2, lg: 3 }} spacing={3}>
                  {data.libraries.map((lib) => {
                    const stats = getLibraryStats(lib.path);
                    return (
                      <LibraryCard
                        key={`${lib.index}-${lib.path}`}
                        lib={lib}
                        gameCount={stats.count}
                        gameSize={stats.size}
                      />
                    );
                  })}
                </SimpleGrid>
              ) : (
                <Box
                  bg={cardBg}
                  border="1px solid"
                  borderColor={borderColor}
                  borderRadius="xl"
                  p={6}
                  textAlign="center"
                >
                  <VStack spacing={2}>
                    <HardDrive size={32} color={subTextColor} />
                    <Text color={subTextColor} fontSize="sm">
                      {t("steam.noLibraries")}
                    </Text>
                    {data.install_info.install_path && (
                      <Text color={subTextColor} fontSize="xs">
                        {t("steam.installPath")}: {data.install_info.install_path}
                      </Text>
                    )}
                  </VStack>
                </Box>
              )}
            </Collapse>
          </Box>
        </>
      )}

      {/* 删除账户确认弹窗 */}
      <AlertDialog isOpen={isDeleteOpen} leastDestructiveRef={cancelRef} onClose={onDeleteClose}>
        <AlertDialogOverlay>
          <AlertDialogContent bg={cardBg} borderColor={borderColor}>
            <AlertDialogHeader fontSize="lg" fontWeight="bold" color={textColor}>
              {t("steam.deleteAccountTitle")}
            </AlertDialogHeader>
            <AlertDialogBody color={subTextColor}>
              {t("steam.deleteAccountConfirm", {
                name:
                  deleteTarget?.account_name ||
                  deleteTarget?.persona_name ||
                  deleteTarget?.steam_id64,
              })}
            </AlertDialogBody>
            <AlertDialogFooter>
              <Button ref={cancelRef} onClick={onDeleteClose}>
                {t("steam.cancel")}
              </Button>
              <Button colorScheme="red" onClick={handleDeleteAccount} ml={3}>
                {t("steam.deleteAccount")}
              </Button>
            </AlertDialogFooter>
          </AlertDialogContent>
        </AlertDialogOverlay>
      </AlertDialog>
    </PageBody>
  );
}
