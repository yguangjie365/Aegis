/**
 * 游戏卡片视觉外壳 —— 唯一视觉来源：SystemToolkit `GameManagerView.xaml`
 * ============================================================================
 * 依据（2026-10-03 用户裁决）：游戏模块**基线是 NexBox**，但**游戏卡片效果参考
 * SystemToolkit**。本文件即该裁决的唯一落地处 —— 页面的数据流、状态管理、命令调用
 * 全部保持 NexBox 原样（见 `pages/SteamPage.tsx`），**只有卡片这一层**按 SystemToolkit
 * 的 XAML 规格重做。
 *
 * 规格逐条对照（源码行号 → 本文件实现）
 * ---------------------------------------------------------------------------
 * | SystemToolkit                                | 本文件                        |
 * |----------------------------------------------|-------------------------------|
 * | `GameCard` Width=320 (:22)                    | CARD_WIDTH = 320             |
 * | `GameCard` Margin=0,0,14,18 (:23)             | mr="14px" mb="18px"          |
 * | `GameCard` BorderThickness=1 (:25)            | 1px solid                    |
 * | `GameCard` CornerRadius=Radius_XLLgTopFlat    | 0 0 var(--aegis-radius-sm)  |
 * |   (Nvidia 包 = 0,0,2,2) (:28)                |   （上角直角 + 下角 2px）     |
 * | `GameCard` ClipToBounds=True (:30)            | overflow="hidden"            |
 * | `GameCard` hover 边框 accent + 1.5px (:37-39) | _hover 边框色 + 0.5px 外环   |
 * | `IsLibraryOffline` → Opacity 0.55 (:33-35)    | isOffline → opacity 0.55     |
 * | DataTemplate hover 上浮 -5px / 0.2s (:465-478)| _hover translateY(-5px)      |
 * | 封面 Height=175 (:487)                        | COVER_HEIGHT = 175px         |
 * | 封面 Stretch=UniformToFill (:496)             | objectFit="cover"            |
 * | 缺图占位：居中游戏名 (:500-505)               | 居中名称 + CardMeta 排版     |
 * | 底部微遮罩 Height=42 (:508-510)               | 底部 42px 渐变              |
 * | 状态徽章 右下 Padding 7,2 mono 微字 (:512-528)| Badge，padding 7px/2px       |
 * | 名称 Font_SizeBodyLg=14 Bold (:556)           | fontSize 14 / bold           |
 * | 元数据行 mono + Tiny=11 + 3px 圆点分隔 (:565-570)| fontFamily mono / 11px / ●  |
 * | 操作行：顶部分隔线 + 两键均分 (:574-608)       | borderTop + 1:1 两键         |
 * | 主副键 FontSize=12 Bold Padding=14,7 MinH=32 | 12px / bold / 32px          |
 *
 * 🔴 踩坑留档（SystemToolkit 原注释，:62-72，2026-09-13 实机反馈）：
 *   `MinHeight` **不能写 30** —— 副键多 1px 描边（上下各 1）→ 内容比主键高 2px，
 *   写 30 会得到「副键 32 / 主键 30」依旧不齐；32 ≥ 两者内容高（≈29.5 / ≈31.5）
 *   → 双双抬到 32。本文件 `ACTION_BTN_MIN_H = 32` 即该结论。
 *
 * 🔴 视觉差异登记：
 *   1. `Radius_XLLgTopFlat`（上直角 + 下圆角）在 Chakra 无对应预设，用
 *      `borderRadius="0 0 var(--aegis-radius-sm) var(--aegis-radius-sm)"` 近似。
 *      Nvidia 主题下 `--aegis-radius-sm` = 2px，与 SystemToolkit Nvidia 包**同值**。
 *   2. `VirtualizingWrapPanel`（换行瀑布流）React 侧无对应 → 用 `flex-wrap`。
 *      hover 位移走 `transform`，不触发布局重排，故不破坏换行。
 *   3. 🔴 **封面缩放已刻意不做** —— SystemToolkit 2026-09-13 UI 评审裁定：位移(-5px/0.2s)
 *      与封面缩放(1.04/0.35s) 同时发生、时长还不一致 → 观感"跳动"，二选一只留位移 +
 *      边框转青。本文件 `_hover` 只写 translateY + 边框，**不写 scale**。
 *   4. WPF 的 `Storyboard` 动画 → CSS transition；按 UI 规范 §8.3 包
 *      `prefers-reduced-motion: no-preference`，reduced-motion 下关闭上浮与过渡。
 *   5. 🔴 hover 的「加粗 0.5px」改用 `box-shadow: 0 0 0 0.5px` **外环**实现（2026-10-04 用户裁决 G2）。
 *      **实测依据**（无头 Edge 154 / Chromium；CDP 强制真实 `:hover` + 逐像素扫描，探针见
 *      `.worknotes/border-snap-probe.mjs` 与 `card-hover-probe.mjs`）：**Chromium 把 `border-width`
 *      取整到整数 CSS 像素** —— 声明 0.5 / 1 / 1.25 / **1.5** / 1.75px 时 computed 值**全部为 1px**，
 *      border-box 宽恒 302、内容宽恒 300。由此两条推论：
 *        · 基线 `:37-39` 的 `border-width: 1.5px` 是**死声明** —— hover 实际只换颜色，**从未加粗**；
 *        · 它**也不产生任何内容区重排**（内容宽自始至终 300px）。
 *      `box-shadow` 的 spread **不做**该取整：0.5px 环在 DPR2 下实测让边框由 2 → **3 个设备像素**，
 *      DPR1 下与边框合并（无可见差异）。故本文件用它来真正实现规格想要的「hover 加粗」。
 *      ⚠️ 因此在 DPR≥2 的显示缩放下，本卡 hover 会比**现行构建**可见地厚 1 个设备像素 —— 这是
 *      **补齐规格意图**，不是等价重构。
 *   6. hover 时长 0.2s → 0.16s、`ease` → `ease-out`（上浮属"进入"语义）；位移幅度 −5px
 *      与配色**不变**（本文件第 3 条：封面缩放仍不做）。
 */
import { Box, Text, VStack, HStack, Button, Badge, useColorModeValue } from "@chakra-ui/react";
import { Gamepad2 } from "lucide-react";
import { memo, useEffect, useState, type ReactNode } from "react";
import { invoke, isTauriRuntime } from "@/lib/tauri/invoke";

/**
 * 🔴 封面 CDN 候选链（2026-10-04 修「获取不到封面」）
 *
 * 原实现（NexBox 原样搬入）只拼 `cdn.cloudflare.steamstatic.com/steam/apps/{id}/header.jpg` ——
 * 该域名 **2026-09-13 实测已 404**（SystemToolkit `SteamService.Library.cs:236` 留下结论），
 * 故**全部**游戏封面都加载失败、只剩占位图。用户报的「3861280 拿不到封面」即此根因，
 * 不是那个 appid 的特殊问题。
 *
 * 候选顺序对齐 SystemToolkit `BuildCoverCdnUrls`：
 *   1. appinfo `common.header_image` 相对路径（**带 hash 子目录**，只有后端读得到，最准）
 *   2. 新域固定 `header.jpg`（460×215 横版，贴合卡片 320×175）
 *   3. 新域竖版 `library_600x900.jpg`（2:3，仅兜底）
 *   4. 新域旧路径（不含 `store_item_assets` 段）
 *   5. 旧域保底（已 404，留着以防 Steam 回退）
 *
 * 第 1 条由后端 `steam_cover_urls` 提供（appinfo 解析 + mtime 缓存）；1–5 里除第 1 条外
 * 都是**静态可拼**的公开 URL —— 故非 Tauri 运行时（web 预览）直接走第 2 条起，
 * 不伪造后端返回值（invoke.ts 纪律：后端不可用不得假数据静默兜底）。
 */
const COVER_CDN_SHARED_BASE = "https://shared.fastly.steamstatic.com/store_item_assets/steam/apps/";

/** 不依赖后端数据的静态候选（web 预览 / 后端不可用时用） */
export function getStaticCoverUrls(appId: number): string[] {
  if (!appId) return [];
  const id = String(appId);
  return [
    `${COVER_CDN_SHARED_BASE}${id}/header.jpg`,
    `${COVER_CDN_SHARED_BASE}${id}/library_600x900.jpg`,
    `https://shared.fastly.steamstatic.com/steam/apps/${id}/header.jpg`,
    `https://cdn.cloudflare.steamstatic.com/steam/apps/${id}/header.jpg`,
  ];
}

/**
 * 取该游戏的封面候选链：Tauri 运行时优先走后端（含 appinfo 精确路径），
 * 失败/非运行时退化为静态链。逻辑单点化，卡片与详情页共用。
 *
 * ⚠️ 后端不可用只降级候选、不伪造数据；候选全部加载失败仍会落到占位图。
 */
export async function getGameCoverUrls(appId: number): Promise<string[]> {
  if (!appId) return [];
  if (!isTauriRuntime()) return getStaticCoverUrls(appId);
  try {
    // module/action 只用于日志分组（与 SteamPage 同用 "Steam"）；payload 顶层展开给后端
    const urls = await invoke<string[]>("steam_cover_urls", "Steam", "FetchCoverUrls", {
      appId,
    });
    if (Array.isArray(urls) && urls.length > 0) return urls;
    return getStaticCoverUrls(appId);
  } catch {
    // 🔴 不静默：后端真不可用时候选里就没有 appinfo 那条，最终仍会落到占位图
    return getStaticCoverUrls(appId);
  }
}

// ── SystemToolkit 规格常量（唯一事实源 = 上表）──
export const CARD_WIDTH = 320;
export const CARD_MARGIN_R = 14;
export const CARD_MARGIN_B = 18;
export const COVER_HEIGHT = 175;
export const COVER_FADE_H = 42;
export const CARD_INNER_PADDING = "14px 12px 14px 14px";
export const META_FONT_SIZE = "11px";
export const BADGE_FONT_SIZE = "10px";
export const NAME_FONT_SIZE = "14px";
export const ACTION_FONT_SIZE = "12px";
export const ACTION_BTN_MIN_H = 32;
export const HOVER_LIFT_PX = -5;
/** hover 过渡时长。XAML `:465-478` 为 0.2s；此处收紧到 0.16s 让上浮更跟手（幅度与配色不变）。 */
export const HOVER_DURATION = "0.16s";
export const OFFLINE_OPACITY = 0.55;

/**
 * 🔴 令牌映射（SystemToolkit `Brush_*` → Aegis `--aegis-*`）
 * ---------------------------------------------------------------------------
 * SystemToolkit 用 WPF 的 `DynamicResource Brush_X` 动态资源；Aegis 的 CSS 变量是
 * 另一套命名，直接照抄 Brush 名会得到**未定义变量**（computed 值回退成 transparent /
 * inherit，肉眼表现为分隔圆点消失、元数据文字变深色）。故逐条映射并核对色值：
 *
 * | SystemToolkit              | 色值    | Aegis 令牌         | Aegis 色值 |
 * |---------------------------|---------|--------------------|------------|
 * | `Brush_Surface`            | —       | `--aegis-surface-soft` | #f7f7f7 |
 * | `Brush_Border`             | —       | `--aegis-hairline`  | #cccccc    |
 * | `Brush_Accent`             | —       | `--aegis-accent`    | #76b900    |
 * | `Brush_TextPrimary`        | —       | `--aegis-ink`       | #1a1a1a    |
 * | `Brush_TextMuted`(:69)     | #757575 | `--aegis-mute`      | #757575 ✅ 同值 |
 * | `Brush_Track`(:63)         | #707070 | `--aegis-stone`     | #8f8f8f（同为中性弱化色，非同值） |
 * | `Brush_Warning`            | —       | `--aegis-warning`   | #9a4d00    |
 *
 * ⚠️ `Brush_Track` 是唯一**非同值**映射（Nvidia.Dark.xaml:20 = #707070）。选 `stone`
 * 而非 `mute` 是因 SystemToolkit 用它画的是**纯装饰性**分隔点（3px 圆点），比
 * 元数据文字更弱一级；Aegis 无独立装饰色，取同族最弱一档。阶段二换皮时如需精确
 * 复刻，在 `tokens.ts` 增设 `track` 键（需同步规范 §二 与 §五 红线的 48-key 计数）。
 */

/** 上角直角 + 下角圆角（Nvidia 主题 2px，与 SystemToolkit `Radius_XLLgTopFlat` 同值） */
const CARD_RADIUS = "0 0 var(--aegis-radius-sm) var(--aegis-radius-sm)";

/** 卡片元数据文本：mono 微字弱色（SystemToolkit `CardMetaText`） */
export function CardMetaText({ children }: { children: ReactNode }) {
  return (
    <Text
      fontFamily="mono"
      fontSize={META_FONT_SIZE}
      color="var(--aegis-mute)"
      lineHeight="1.45"
      noOfLines={1}
    >
      {children}
    </Text>
  );
}

/** 元数据行分隔圆点（SystemToolkit `MetaDot`：3×3 + Margin 7,0） */
function MetaDot() {
  return (
    <Box
      as="span"
      display="inline-block"
      w="3px"
      h="3px"
      mx="7px"
      borderRadius="full"
      bg="var(--aegis-stone)"
      flexShrink={0}
      aria-hidden
    />
  );
}

/** 元数据行：占用 · 时长 · 最近游玩（SystemToolkit :565-570，单行三段 + 两圆点） */
export function CardMetaRow({ children }: { children: ReactNode }) {
  return (
    <HStack spacing={0} mt="8px" minW={0}>
      {children}
    </HStack>
  );
}

export { MetaDot };

/**
 * 卡片封面（SystemToolkit :486-510）
 * - 高 175px、`objectFit: cover`（WPF `UniformToFill`）
 * - 缺图占位：居中显示游戏名（SystemToolkit 用 `Font_SizeMetric` + Bold + 省略号）
 * - 底部 42px 微遮罩，让封面与卡片过渡更柔
 */
export const GameCover = memo(function GameCover({
  appId,
  name,
  height = COVER_HEIGHT,
}: {
  appId: number;
  name: string;
  height?: number;
}) {
  const [imgError, setImgError] = useState(false);
  const [coverUrls, setCoverUrls] = useState<string[]>(() => getStaticCoverUrls(appId));
  const [urlIndex, setUrlIndex] = useState(0);
  const placeholderBg = useColorModeValue("gray.100", "#0a0a0a");

  // 候选链：Tauri 运行时后台拉一次（含 appinfo 精确路径），命中则换到链首重新加载
  useEffect(() => {
    let cancelled = false;
    setUrlIndex(0);
    setImgError(false);
    getGameCoverUrls(appId).then((urls) => {
      if (cancelled || urlIndex !== 0) return;
      if (urls[0] !== coverUrls[0]) setCoverUrls(urls);
    });
    return () => {
      cancelled = true;
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [appId]);

  const advanceCover = () => {
    const next = urlIndex + 1;
    if (next >= coverUrls.length) {
      setImgError(true); // 候选全失败 → 占位图
      return;
    }
    setUrlIndex(next); // 加载下一个候选（appinfo 精确 → 新域 header → 竖版 → 旧域）
  };

  return (
    <Box position="relative" h={`${height}px`} bg={placeholderBg} overflow="hidden">
      {!imgError ? (
        <Box
          as="img"
          key={coverUrls[urlIndex]}
          src={coverUrls[urlIndex]}
          alt={name}
          w="100%"
          h="100%"
          objectFit="cover"
          onError={advanceCover}
          loading="lazy"
          decoding="async"
          referrerPolicy="no-referrer"
        />
      ) : (
        // 缺图占位：SystemToolkit 显示游戏名；此处按规格用 CardMeta 排版并叠 Gamepad 图标
        <FlexCenter>
          <VStack spacing={1} px="10px" maxW="100%">
            <Gamepad2 size={22} color="var(--aegis-mute)" />
            <Text
              fontSize={META_FONT_SIZE}
              fontFamily="mono"
              fontWeight="bold"
              color="var(--aegis-mute)"
              textAlign="center"
              noOfLines={2}
            >
              {name}
            </Text>
          </VStack>
        </FlexCenter>
      )}
      {/* 底部微遮罩（SystemToolkit Rectangle Height=42 + Brush_CoverFade） */}
      <Box
        position="absolute"
        left={0}
        right={0}
        bottom={0}
        h={`${COVER_FADE_H}px`}
        bgGradient="linear(to-t, rgba(0,0,0,0.55), transparent)"
        pointerEvents="none"
      />
    </Box>
  );
});

function FlexCenter({ children }: { children: ReactNode }) {
  return (
    <Box position="absolute" inset={0} display="flex" alignItems="center" justifyContent="center">
      {children}
    </Box>
  );
}

/**
 * 状态徽章（SystemToolkit :512-528）
 * 右下角、Warning 软底 + 描边、Radius_XS、Padding 7,2、mono 微字。
 * 🔴 判据必须是**精确状态**（下载/更新中），不绑「未安装取反」—— 否则会与库存卡片的
 * 「未安装」徽章同时可见，两层半透明遮罩叠加压不住（SystemToolkit 2026-09-13 收紧）。
 */
export function CoverStateBadge({ children }: { children: ReactNode }) {
  return (
    <Box position="absolute" right="12px" bottom="8px" zIndex={1}>
      <Badge
        bg="rgba(255,193,7,0.16)"
        border="1px solid"
        borderColor="var(--aegis-accent)"
        color="var(--aegis-accent)"
        borderRadius="var(--aegis-radius-sm)"
        px="7px"
        py="2px"
        fontFamily="mono"
        fontSize={BADGE_FONT_SIZE}
        fontWeight="normal"
        textTransform="none"
      >
        {children}
      </Badge>
    </Box>
  );
}

/**
 * 卡片瀑布流容器（SystemToolkit `VirtualizingWrapPanel` :455-456 的等价实现）
 *
 * 🔴 为什么不能用 Chakra `SimpleGrid`（NexBox 原件用的就是它）
 * ---------------------------------------------------------------------------
 * `SimpleGrid` 是 **grid 布局**：列数由 `columns` 断点决定，子项被强制拉伸填满格宽。
 * 后果（本轮实机截图实证）：
 *   - 卡片设的 `w: 320px` 被 grid **覆盖** → 1230px 窗口下 `lg:4` 挤成 4 列、每列仅约 250px；
 *   - 卡片高度随之被压扁，封面 175px 变矮，视觉与 SystemToolkit 规格完全不符；
 *   - `spacing={4}` 覆盖了 SystemToolkit 要求的 `0,0,14,18`（grid 的 `spacing` 是均匀四向）。
 *
 * SystemToolkit 用的是 `VirtualizingWrapPanel`：**换行瀑布流，卡片保持自身宽度**。
 * React 侧无等价控件（且 virtualizing 在千级列表下才值得），故用 `flex-wrap`：
 *   - 子项 `flex: 0 0 320px` = 不拉伸不压缩，保持定宽；
 *   - 卡片自身的 `mr/mb`（14/18）承担间距，故容器**不再设 gap**（否则叠加成 30/34）；
 *   - hover 位移走 `transform`，不触发布局重排 → 不破坏换行。
 */
export function CardWrapGrid({ children }: { children: ReactNode }) {
  return (
    <Box display="flex" flexWrap="wrap" alignItems="flex-start" w="100%">
      {children}
    </Box>
  );
}

/**
 * 卡片外壳（SystemToolkit `GameCard` 样式 :21-46 + DataTemplate :460-612）
 *
 * @param isOffline 库离线 → 整卡 Opacity 0.55（SystemToolkit :33-35）
 * @param children  卡片内容（封面 + 名称 + 元数据行 + 操作行）
 */
export const GameCardShell = memo(function GameCardShell({
  children,
  isOffline = false,
  onClick,
  title,
}: {
  children: ReactNode;
  isOffline?: boolean;
  onClick?: () => void;
  title?: string;
}) {
  return (
    <Box
      // `flex: 0 0 320px` 而非只给 `w`：在 `flex-wrap` 容器里必须显式禁缩禁伸，
      // 否则窄屏时 flex 会压缩卡片（截图实证：1230px 下被压到约 250px、封面随之变矮）
      flex="0 0 auto"
      w={`${CARD_WIDTH}px`}
      mr={`${CARD_MARGIN_R}px`}
      mb={`${CARD_MARGIN_B}px`}
      bg="var(--aegis-surface-soft)"
      border="1px solid"
      borderColor="var(--aegis-hairline)"
      borderRadius={CARD_RADIUS}
      padding={0}
      overflow="hidden"
      opacity={isOffline ? OFFLINE_OPACITY : 1}
      cursor={onClick ? "pointer" : "default"}
      transition={`transform ${HOVER_DURATION} ease-out, border-color ${HOVER_DURATION} ease-out, box-shadow ${HOVER_DURATION} ease-out`}
      _hover={{
        borderColor: "var(--aegis-accent)",
        // 🔴 见文件头「视觉差异登记」第 5 条：Chromium 把 border-width 取整到整数 CSS 像素，
        //    基线写的 1.5px 实际 computed 为 1px（死声明，hover 从未加粗）；box-shadow 的
        //    0.5px spread 不做该取整，才是真正能画出「加粗」的那条路。
        boxShadow: "0 0 0 0.5px var(--aegis-accent)",
        // 🔴 reduced-motion 下关闭上浮与过渡（UI 规范 §8.3）
        transform: `translateY(${HOVER_LIFT_PX}px)`,
        "@media (prefers-reduced-motion: reduce)": {
          transform: "none",
          transition: "none",
        },
      }}
      onClick={onClick}
      title={title}
    >
      {children}
    </Box>
  );
});

/**
 * 卡片名称行（SystemToolkit :556：Font_SizeBodyLg=14 + Bold + CharacterEllipsis）
 */
export function CardName({ children }: { children: ReactNode }) {
  return (
    <Text
      fontSize={NAME_FONT_SIZE}
      fontWeight="bold"
      color="var(--aegis-ink)"
      noOfLines={1}
      title={typeof children === "string" ? children : undefined}
    >
      {children}
    </Text>
  );
}

/**
 * 操作行（SystemToolkit :574-608）
 * 顶部分隔线（BorderThickness 0,1,0,0 + Margin 0,12,0,0 + Padding 0,10,0,0）+ 两键均分。
 * 两列在**两种状态下都有键**（未安装：安装+商店；已安装：商店+启动），故无需铺满 hack。
 */
export function CardActionRow({ children }: { children: ReactNode }) {
  return (
    <Box mt="12px" pt="10px" borderTop="1px solid" borderColor="var(--aegis-hairline)">
      <Box display="grid" gridTemplateColumns="1fr 1fr" gap="8px">
        {children}
      </Box>
    </Box>
  );
}

/**
 * 卡片操作键（SystemToolkit `ActionPrimaryButton` / `ActionSecondaryButton` :62-72）
 * FontSize 12 / Bold / Padding 14,7 / MinHeight 32（32 的取值理由见文件头踩坑留档）。
 */
export const CardActionButton = memo(function CardActionButton({
  children,
  primary = false,
  isLoading = false,
  onClick,
  ariaLabel,
}: {
  children: ReactNode;
  primary?: boolean;
  isLoading?: boolean;
  onClick?: () => void;
  ariaLabel?: string;
}) {
  const activeColor = "var(--aegis-accent)";
  return (
    <Button
      size="sm"
      minH={`${ACTION_BTN_MIN_H}px`}
      px="14px"
      py="7px"
      fontSize={ACTION_FONT_SIZE}
      fontWeight="bold"
      borderRadius="var(--aegis-radius-sm)"
      aria-label={ariaLabel}
      isLoading={isLoading}
      spinnerPlacement="start"
      variant={primary ? "solid" : "outline"}
      {...(primary
        ? { bg: activeColor, color: "var(--aegis-ink)", _hover: { opacity: 0.85 } }
        : { borderColor: "var(--aegis-hairline)", _hover: { borderColor: activeColor } })}
      onClick={onClick}
    >
      {children}
    </Button>
  );
});
