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
import { useDynamicIsland } from "@/shared";
import { useState, useEffect, useMemo, useCallback, useRef, memo, useDeferredValue } from "react";
// Aegis 硬纪律：业务代码的后端调用必须经 @/lib/tauri/invoke 封装（统一 correlationId
// + 结构化日志 + 非 Tauri 运行时显式抛 BackendUnavailableError），禁直触 @tauri-apps/api。
// NexBox 原件为 `invoke(cmd, { args })` 两参形态，此处按封装签名补 module/action，
// 命令名与参数名逐字未改。module 取 LOG_MODULES 的 "Steam"。
import { invoke } from "@/lib/tauri/invoke";
import { useAdaptiveTextColor } from "@/shared";
// 图标库适配（UI 规范 §七锁定 lucide-react）：Aegis 不引入 react-icons 的 Lu* 前缀命名，
// 改为 lucide-react 原生导出名。19 个图标逐一对应，仅命名去前缀，图形语义未变。
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
  Circle,
  CircleDot,
  ChevronDown,
  ChevronUp,
  CircleUser,
  Library,
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
  CardWrapGrid,
  CoverStateBadge,
  GameCardShell,
  GameCover,
  MetaDot,
} from "@/features/GameManager/components/game-card";
import { useBackground } from "@/shared";
import { useThemeColor } from "@/shared";
import steamIcon from "@/assets/tools/Steam.webp";

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
  return (
    <Badge
      colorScheme={running ? "green" : "gray"}
      variant="subtle"
      borderRadius="full"
      px={3}
      py={1}
      fontSize="xs"
    >
      <HStack spacing={1.5}>
        <Box w={2} h={2} borderRadius="full" bg={running ? "green.400" : "gray.400"} />
        <Text>{running ? t("steam.running") : t("steam.notRunning")}</Text>
      </HStack>
    </Badge>
  );
}

function UserCard({
  user,
  onSwitch,
  onDelete,
  isSwitching,
  isLoadingAvatar,
}: {
  user: SteamUser;
  onSwitch: (accountName: string) => void;
  onDelete: (user: SteamUser) => void;
  isSwitching: boolean;
  isLoadingAvatar: boolean;
}) {
  const { liquidGlassEnabled } = useBackground();
  const { colorMode } = useColorMode();
  const { getActiveColor } = useThemeColor();
  const headerColor = colorMode === "light" ? "#000000" : "#ffffff";
  const subTextColor = useColorModeValue("gray.500", "#ffffff");
  const cardBg = useColorModeValue("white", "#1a1a1a");
  const borderColor = useColorModeValue("gray.200", "#333333");
  const activeColor = getActiveColor();

  const displayName = user.persona_name || user.account_name || "Unknown";
  const avatarSrc = user.avatar_medium_url || user.avatar_url;
  const isAvatarLoading = isLoadingAvatar && !avatarSrc;

  const inner = (
    <HStack spacing={3} align="center">
      {avatarSrc ? (
        <Image
          src={avatarSrc}
          alt={displayName}
          boxSize="36px"
          borderRadius="full"
          objectFit="cover"
          onError={(e) => {
            (e.target as HTMLImageElement).style.display = "none";
          }}
        />
      ) : isAvatarLoading ? (
        <Box
          boxSize="36px"
          borderRadius="full"
          bg={`${activeColor}15`}
          display="flex"
          alignItems="center"
          justifyContent="center"
        >
          <Spinner size="sm" color={activeColor} thickness="2px" speed="0.8s" />
        </Box>
      ) : (
        <Box color={user.most_recent ? activeColor : subTextColor}>
          <CircleUser size={36} />
        </Box>
      )}
      <Box flex={1} minW={0}>
        <Text fontSize="sm" fontWeight="bold" color={headerColor} noOfLines={1}>
          {displayName}
        </Text>
        <Text fontSize="xs" color={subTextColor} noOfLines={1}>
          {user.account_name || "-"}
        </Text>
        <Text fontSize="2xs" color={subTextColor} mt={0.5}>
          ID: {steamId64To32(user.steam_id64)}
        </Text>
      </Box>
      <HStack spacing={1}>
        {!user.most_recent && (
          <Tooltip label={t("steam.switchAccount")} placement="top">
            <IconButton
              aria-label={t("steam.switchAccount")}
              icon={<CircleDot size={16} />}
              size="sm"
              variant="ghost"
              color={activeColor}
              isLoading={isSwitching}
              onClick={() => onSwitch(user.account_name)}
            />
          </Tooltip>
        )}
        <Tooltip label={t("steam.deleteAccount")} placement="top">
          <IconButton
            aria-label={t("steam.deleteAccount")}
            icon={<Trash2 size={16} />}
            size="sm"
            variant="ghost"
            color="red.400"
            _hover={{ bg: "red.50", color: "red.500" }}
            onClick={() => onDelete(user)}
          />
        </Tooltip>
        {user.most_recent && <Box w="16px" h="16px" borderRadius="full" bg={activeColor} />}
      </HStack>
    </HStack>
  );

  const badge = user.most_recent ? (
    <Badge
      position="absolute"
      top={"-1px"}
      right={"-1px"}
      bg={activeColor}
      color="white"
      variant="solid"
      borderTopRightRadius="xl"
      borderBottomLeftRadius="md"
      borderTopLeftRadius={0}
      borderBottomRightRadius={0}
      fontSize="2xs"
      px={2}
      py={0.5}
      zIndex={1}
    >
      {t("steam.currentUser")}
    </Badge>
  ) : null;

  if (liquidGlassEnabled) {
    return (
      <LiquidGlassCard p={4} position="relative" overflow="visible">
        {badge}
        {inner}
      </LiquidGlassCard>
    );
  }

  return (
    <Box
      bg={cardBg}
      border="1px solid"
      borderColor={borderColor}
      borderRadius="xl"
      p={4}
      transition="all 0.2s"
      position="relative"
      overflow="visible"
    >
      {badge}
      {inner}
    </Box>
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
// 卡片视觉层 = SystemToolkit（用户裁决 2026-10-03：卡片参考 SystemToolkit，
// 其余复用 NexBox）。本函数保留 NexBox 的**数据流与命令回调**（onLaunch /
// onOpenFolder / onStorePage 签名逐字未改），只把 JSX 换成
// `components/game-card.tsx` 的 SystemToolkit 规格实现。
// ============================================================================
const GameCard = memo(function GameCard({
  game,
  isLaunching,
  onLaunch,
  onOpenFolder,
  onStorePage,
}: {
  game: SteamGame;
  isLaunching: boolean;
  onLaunch: (appId: number) => void;
  onOpenFolder: (libPath: string, installDir: string) => void;
  onStorePage: (appId: number) => void;
}) {
  const isDownloading = game.bytes_downloaded > 0 && game.bytes_downloaded < game.bytes_to_download;
  const downloadProgress =
    game.bytes_to_download > 0
      ? Math.round((game.bytes_downloaded / game.bytes_to_download) * 100)
      : 0;

  return (
    <GameCardShell title={game.name}>
      <Box position="relative">
        <GameCover appId={game.app_id} name={game.name} />
        {/* 状态徽章判据 = 精确的下载中状态（SystemToolkit :512-528 的 ShowProgressBadge 口径），
            不用「未安装取反」—— 那会与库存卡片的「未安装」徽章同时可见 */}
        {isDownloading && <CoverStateBadge>{downloadProgress}%</CoverStateBadge>}
      </Box>

      <Box p={CARD_INNER_PADDING}>
        <CardName>{game.name}</CardName>

        {/* 元数据行：占用 · 时长 · 最近游玩合并为一行（SystemToolkit :565-570，
            2026-09-13 UI 评审：原「最近游玩」另起一行且同为 mono 微字弱色，视觉权重无差、白占约 17px）*/}
        <CardMetaRow>
          <CardMetaText>{formatSize(game.size_on_disk)}</CardMetaText>
          <MetaDot />
          <CardMetaText>
            {game.playtime_minutes > 0 ? formatPlaytime(game.playtime_minutes) : "-"}
          </CardMetaText>
          <MetaDot />
          <CardMetaText>
            {game.last_played > 0 ? formatLastPlayed(game.last_played) : "-"}
          </CardMetaText>
        </CardMetaRow>

        <CardActionRow>
          {/* 副键恒为「商店页面」；打开目录 / 卸载在 NexBox 的 ⋯ 菜单内保留 */}
          <CardActionButton onClick={() => onStorePage(game.app_id)}>
            {t("steam.storePage")}
          </CardActionButton>
          <Box position="relative">
            <CardActionButton primary isLoading={isLaunching} onClick={() => onLaunch(game.app_id)}>
              {t("steam.launch")}
            </CardActionButton>
            {/* NexBox 的 ⋯ 操作菜单（打开目录 / 商店页）保留，浮于副键之上。
                🔴 卸载项已按用户裁决删除 —— 卸载由 Steam 客户端管理 */}
            <Box position="absolute" right="2px" top="50%" transform="translateY(-50%)" zIndex={2}>
              <GameActionMenu game={game} onOpenFolder={onOpenFolder} onStorePage={onStorePage} />
            </Box>
          </Box>
        </CardActionRow>
      </Box>
    </GameCardShell>
  );
});

// 库存卡片：视觉同样按 SystemToolkit 规格（与已安装卡片共用外壳与规格组件），
// 数据流保持 NexBox 原样。
// 🔴 `onInstall` prop 已按用户裁决（2026-10-03）删除：安装改由「打开商店页」引导，
//    本模块不再提供安装动作。
const InventoryGameCard = memo(function InventoryGameCard({
  game,
  isLaunching,
  onLaunch,
  onStorePage,
}: {
  game: SteamInventoryGame;
  isLaunching: boolean;
  onLaunch: (appId: number) => void;
  onStorePage: (appId: number) => void;
}) {
  return (
    <GameCardShell title={game.name}>
      <Box position="relative">
        <GameCover appId={game.app_id} name={game.name} />
        {game.installed && <CoverStateBadge>{t("steam.installedBadge")}</CoverStateBadge>}
      </Box>

      <Box p={CARD_INNER_PADDING}>
        <CardName>{game.name}</CardName>

        <CardMetaRow>
          <CardMetaText>{game.size_on_disk > 0 ? formatSize(game.size_on_disk) : "-"}</CardMetaText>
          <MetaDot />
          <CardMetaText>
            {game.playtime_minutes > 0 ? formatPlaytime(game.playtime_minutes) : "-"}
          </CardMetaText>
          <MetaDot />
          <CardMetaText>
            {game.last_played > 0 ? formatLastPlayed(game.last_played) : "-"}
          </CardMetaText>
        </CardMetaRow>

        <CardActionRow>
          <CardActionButton onClick={() => onStorePage(game.app_id)}>
            {t("steam.storePage")}
          </CardActionButton>
          {/* 🔴 未安装游戏不再提供「安装」动作（用户裁决 2026-10-03，后端命令同批删除）。
              未安装时**不渲染**主按钮（它没有任何可执行动作），仅保留「商店页」引导用户
              到 Steam 客户端安装。此处刻意不改 `CardActionButton` 的 props —— 那是
              SystemToolkit `ActionPrimaryButton` 的复刻组件，契约受铁律保护。 */}
          {game.installed && (
            <CardActionButton primary isLoading={isLaunching} onClick={() => onLaunch(game.app_id)}>
              {t("steam.launch")}
            </CardActionButton>
          )}
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
  const subTextColor = useColorModeValue("gray.500", "#ffffff");
  const cardBg = useColorModeValue("white", "#1a1a1a");
  const borderColor = useColorModeValue("gray.200", "#333333");
  const progressBg = useColorModeValue("gray.100", "#333");
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
  const [showUsers, setShowUsers] = useState(true);
  const [inventory, setInventory] = useState<SteamInventoryData | null>(null);
  const [loadingInventory, setLoadingInventory] = useState(false);
  // Aegis 新增：在线补全失败原因（null = 未尝试或成功）
  const [onlineError, setOnlineError] = useState<string | null>(null);
  const [showInventory, setShowInventory] = useState(true);
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
  const textColor = useColorModeValue("gray.800", "#ffffff");
  const subTextColor = useColorModeValue("gray.500", "#ffffff");
  const cardBg = useColorModeValue("white", "#1a1a1a");
  const borderColor = useColorModeValue("gray.200", "#333333");
  const inputBg = useColorModeValue("white", "#1a1a1a");
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

  // Aegis 新增：在线库存的数据形状（后端 `SteamOnlineInventory` 的前端投影）
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
  const emptyInventoryData = (): SteamInventoryData => ({
    source: "none",
    steam_running: false,
    current_user: null,
    stats: { total: 0, installed: 0, not_installed: 0, total_playtime_minutes: 0 },
    games: [],
    error: null,
  });

  /**
   * 把在线清单合并进本地库存（`installed` 以本地 .acf 为准）。
   *
   * 规则（与后端 `inventory_online.rs` 的口径一致）：
   * - 本地已有的条目：只刷新 `playtime_minutes` / `last_played`（在线值更权威），
   *   **不覆盖** `installed` / `size_on_disk` / `install_dir`（只有本地知道这些）；
   * - 在线独有的条目：`installed` 由 `installed_ids` 判定（后端读 .acf 给的），
   *   缺省字段补 0 / 空串。
   */
  const mergeOnlineInventory = (
    local: SteamInventoryData | null,
    online: SteamOnlineInventory,
    installedIds: Set<number>,
  ): SteamInventoryData => {
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
  };

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

  const filteredGames = useMemo(() => {
    if (!data?.games) return [];
    if (!searchText.trim()) return data.games;
    const lower = searchText.toLowerCase();
    return data.games.filter(
      (g) => g.name.toLowerCase().includes(lower) || g.app_id.toString().includes(lower),
    );
  }, [data?.games, searchText]);

  // 按库（硬盘）分组后的游戏列表
  const groupedGames = useMemo(() => {
    if (!data || filteredGames.length === 0) return [];
    const groups: {
      key: string;
      driveLabel: string;
      path: string;
      label: string;
      games: SteamGame[];
      totalSize: number;
    }[] = [];
    const matched = new Set<number>();
    for (const lib of data.libraries) {
      const libNorm = normalizePath(lib.path);
      const games = filteredGames.filter((g) => {
        const gameNorm = normalizePath(g.library_path);
        if (gameNorm === libNorm || gameNorm.startsWith(libNorm + "\\")) {
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
        totalSize: games.reduce((s, g) => s + g.size_on_disk, 0),
      });
    }
    // 未匹配到任何库的游戏（理论上少见，兜底处理）
    const rest = filteredGames.filter((g) => !matched.has(g.app_id));
    if (rest.length > 0) {
      groups.push({
        key: "other",
        driveLabel: t("steam.other"),
        path: "",
        label: "",
        games: rest,
        totalSize: rest.reduce((s, g) => s + g.size_on_disk, 0),
      });
    }
    return groups;
  }, [data, filteredGames]);

  // 库存游戏过滤（共用顶部搜索框，用延迟值避免大列表频繁重算）
  const filteredInventory = useMemo(() => {
    if (!inventory?.games) return [];
    if (!deferredSearch.trim()) return inventory.games;
    const lower = deferredSearch.toLowerCase();
    return inventory.games.filter(
      (g) => g.name.toLowerCase().includes(lower) || g.app_id.toString().includes(lower),
    );
  }, [inventory?.games, deferredSearch]);

  // 库存按可见数量渲染（默认 120，点「加载更多」递增）
  const inventoryGamesShown = useMemo(() => {
    return filteredInventory.slice(0, visibleCount);
  }, [filteredInventory, visibleCount]);

  // 搜索词变化时重置分页
  useEffect(() => {
    setVisibleCount(120);
  }, [deferredSearch]);

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
    <Box pb={8}>
      {/* 标题栏 */}
      <HStack justify="space-between" mb={6} flexShrink={0}>
        <HStack spacing={3}>
          <Image src={steamIcon} alt="Steam" w="64px" h="64px" objectFit="contain" />
          <Text
            fontSize="3xl"
            fontWeight="bold"
            color={adaptiveTitle.text}
            textShadow={adaptiveTitle.shadow}
          >
            Steam
          </Text>
          {data && <StatusBadge running={data.install_info.is_running} />}
          {data && !data.install_info.is_running && (
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
        </HStack>
        <HStack spacing={3}>
          <InputGroup w="220px">
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

      {/* 统计信息 */}
      {data && (
        <HStack spacing={4} mb={6} flexWrap="wrap">
          <StatCard
            icon={<Gamepad2 size={16} />}
            label={t("steam.totalGames")}
            value={data.games.length}
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
        </HStack>
      )}

      {/* 账户管理 - 始终显示 */}
      {data && (
        <Box mb={6}>
          <HStack
            justify="space-between"
            cursor="pointer"
            onClick={() => setShowUsers(!showUsers)}
            mb={showUsers ? 3 : 0}
          >
            <HStack spacing={2}>
              <Box color={activeColor}>
                <Users size={18} />
              </Box>
              <Text
                fontSize="lg"
                fontWeight="bold"
                color={adaptiveTitle.text}
                textShadow={adaptiveTitle.shadow}
              >
                {t("steam.accounts")}
              </Text>
              <Badge colorScheme="teal" variant="subtle" borderRadius="full">
                {data.users.length}
              </Badge>
            </HStack>
            <IconButton
              aria-label="toggle"
              icon={showUsers ? <ChevronUp size={16} /> : <ChevronDown size={16} />}
              size="sm"
              variant="ghost"
            />
          </HStack>
          <Collapse in={showUsers}>
            {data.users.length > 0 ? (
              <SimpleGrid columns={{ base: 1, sm: 2, md: 3, lg: 4 }} spacing={3}>
                {data.users.map((user) => (
                  <UserCard
                    key={user.steam_id64}
                    user={user}
                    onSwitch={handleSwitchAccount}
                    onDelete={onDeleteClick}
                    isSwitching={switchingAccount === user.account_name}
                    isLoadingAvatar={loadingAvatars.has(user.steam_id64)}
                  />
                ))}
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
                  <Users size={32} color={subTextColor} />
                  <Text color={subTextColor} fontSize="sm">
                    {t("steam.noAccounts")}
                  </Text>
                </VStack>
              </Box>
            )}
          </Collapse>
        </Box>
      )}

      {/* 游戏库 - 始终显示 */}
      {data && (
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
      )}

      {/* 库存游戏 - 可折叠区块（Steam 运行中在线获取账号库存，否则读本地缓存） */}
      {inventory && (
        <Box mb={6}>
          <HStack
            justify="space-between"
            cursor="pointer"
            onClick={() => setShowInventory(!showInventory)}
            mb={showInventory ? 3 : 0}
          >
            <HStack spacing={2}>
              <Box color={activeColor}>
                <Library size={18} />
              </Box>
              <Text
                fontSize="lg"
                fontWeight="bold"
                color={adaptiveTitle.text}
                textShadow={adaptiveTitle.shadow}
              >
                {t("steam.inventoryGames")}
              </Text>
              <Badge colorScheme="teal" variant="subtle" borderRadius="full">
                {inventory.stats.total}
              </Badge>
              {/* 🔴 数据来源必须如实标明（规格 08 §一「如实标注当前数据来源」）：
                  online = 已合并在线清单；cache = 本地缓存；onlineError 非空 = 在线失败已退回 */}
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
            <IconButton
              aria-label="toggle"
              icon={showInventory ? <ChevronUp size={16} /> : <ChevronDown size={16} />}
              size="sm"
              variant="ghost"
            />
          </HStack>
          <Collapse in={showInventory}>
            {inventory.current_user && (
              <HStack spacing={4} mb={4} flexWrap="wrap">
                <StatCard
                  icon={<CircleUser size={16} />}
                  label={t("steam.currentAccount")}
                  value={inventory.current_user.persona_name || inventory.current_user.account_name}
                />
                <StatCard
                  icon={<Library size={16} />}
                  label={t("steam.inventoryTotal")}
                  value={inventory.stats.total}
                />
                <StatCard
                  icon={<CircleDot size={16} />}
                  label={t("steam.inventoryInstalledCount")}
                  value={inventory.stats.installed}
                />
                <StatCard
                  icon={<Circle size={16} />}
                  label={t("steam.inventoryNotInstalledCount")}
                  value={inventory.stats.not_installed}
                />
                <StatCard
                  icon={<Clock3 size={16} />}
                  label={t("steam.totalPlaytime")}
                  value={formatPlaytime(inventory.stats.total_playtime_minutes) || "0"}
                />
              </HStack>
            )}
            {inventoryGamesShown.length === 0 ? (
              <VStack py={16} spacing={3}>
                <Library size={40} color={subTextColor} />
                <Text color={subTextColor} fontSize="md">
                  {searchText ? t("steam.noSearchResults") : t("steam.inventoryEmpty")}
                </Text>
              </VStack>
            ) : (
              <>
                {/* 🔴 卡片区用 CardWrapGrid（flex-wrap 定宽瀑布流）而非 NexBox 原件的
                    SimpleGrid —— grid 会拉伸子项覆盖 320px 定宽，导致卡片被压扁、
                    封面高度失真、间距变成均匀 16px。SystemToolkit 用的是
                    VirtualizingWrapPanel（换行瀑布流），故按其口径对齐。
                    用户卡 / 库卡沿用 SimpleGrid（那两个是 NexBox 自有组件，不属卡片视觉范围）。*/}
                <CardWrapGrid>
                  {inventoryGamesShown.map((game) => (
                    <InventoryGameCard
                      key={game.app_id}
                      game={game}
                      isLaunching={launchingAppId === game.app_id}
                      onLaunch={handleLaunch}
                      onStorePage={handleStorePage}
                    />
                  ))}
                </CardWrapGrid>
                {filteredInventory.length > visibleCount && (
                  <VStack mt={4} align="center">
                    <Button
                      size="sm"
                      variant="outline"
                      borderRadius="lg"
                      onClick={() => setVisibleCount((c) => c + 120)}
                    >
                      {t("steam.loadMore")} ({filteredInventory.length - visibleCount})
                    </Button>
                  </VStack>
                )}
              </>
            )}
          </Collapse>
        </Box>
      )}

      <Divider mb={6} borderColor={borderColor} />

      {/* 已安装游戏列表 */}
      <HStack spacing={2} mb={4}>
        <Box color={activeColor}>
          <Gamepad2 size={18} />
        </Box>
        <Text
          fontSize="lg"
          fontWeight="bold"
          color={adaptiveTitle.text}
          textShadow={adaptiveTitle.shadow}
        >
          {t("steam.installedGames")}
        </Text>
        <Badge colorScheme="teal" variant="subtle" borderRadius="full">
          {filteredGames.length}
          {searchText && ` / ${data?.games.length ?? 0}`}
        </Badge>
      </HStack>

      {filteredGames.length === 0 ? (
        <VStack py={20} spacing={3}>
          <Gamepad2 size={40} color={subTextColor} />
          <Text color={subTextColor} fontSize="md">
            {searchText ? t("steam.noSearchResults") : t("steam.noGames")}
          </Text>
        </VStack>
      ) : (
        <VStack spacing={6} align="stretch">
          {groupedGames.map((group) => (
            <Box key={group.key}>
              {/* 分组标题：盘符 + 路径 + 游戏数 + 总大小 */}
              <HStack spacing={2} mb={3} flexWrap="wrap">
                <Box color={activeColor}>
                  <HardDrive size={16} />
                </Box>
                <Text
                  fontSize="md"
                  fontWeight="bold"
                  color={adaptiveTitle.text}
                  textShadow={adaptiveTitle.shadow}
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
              <CardWrapGrid>
                {group.games.map((game) => (
                  <GameCard
                    key={game.app_id}
                    game={game}
                    isLaunching={launchingAppId === game.app_id}
                    onLaunch={handleLaunch}
                    onOpenFolder={handleOpenFolder}
                    onStorePage={handleStorePage}
                  />
                ))}
              </CardWrapGrid>
            </Box>
          ))}
        </VStack>
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
    </Box>
  );
}
