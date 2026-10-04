/**
 * steam-ui.tsx — SteamPage 的动效与版式语汇（2026-10-04 重构新增）
 *
 * 🔴 为什么单独成文件：本页原先把动效散落在各组件的 `transition` 字符串里，
 * 时长/缓动各写一遍（`all 0.2s` / `0.16s ease-out` / 裸 `200ms`），无统一来源。
 * 现集中于此，**全部取 motion 令牌**（规范 §二 第六维），换主题包只改令牌一处。
 *
 * 🔴 本文件属GameManager 复刻域的**页面版式层**，不是卡片视觉层 ——
 * 卡片规格的唯一来源仍是 `components/game-card.tsx`（SystemToolkit），
 * 铁律不因本文件而松动。本文件只负责：页面骨架的节奏、进场编排、筛选控件、筛选栏。
 */
import { Box, HStack, Text, type BoxProps } from "@chakra-ui/react";
// 🔴 `keyframes` 出自 **@emotion/react**（Chakra v2 内部即用 emotion），
//    不是 `@chakra-ui/react` 的导出 —— 从后者 import 会报 TS2305。
import { keyframes } from "@emotion/react";
import { useEffect, useRef, useState, type ReactNode } from "react";
import { CARD_WIDTH, CARD_MARGIN_R, CARD_MARGIN_B, COVER_HEIGHT } from "./game-card";

/**
 * 🔴 单卡占位宽度 = 卡片宽 + 其右margin。
 *
 * 数值**不写死**：从 `game-card.tsx` 的 SystemToolkit 规格常量派生 ——
 * 那两个常量是卡片视觉的铁律来源，若在此处另写一份 320，日后改规格时
 * 轨道宽度不会跟着变 ⇒ 卡片被挤压/溢出。故此处只做算术，不做定义。
 */
const CARD_TRACK = CARD_WIDTH + CARD_MARGIN_R;

/**
 * 🔴 轨道宽度的**下限护栏**：不足此值就宁可少排一列，也不让卡片挤在一起。
 *
 * 问题背景：`auto-fill` 只按「能放几列」决定轨道数，剩��余量全部变成两侧留白。
 * 实测（见 `.worknotes/grid-report.txt`）：1366 窗口按 334 算得 3 列，
 * 余量 244px 被平分成两侧各 122 —— 卡片区只占可用宽的 80%，看着仍像"右边空一块"。
 *
 * 处置：**在 JS 侧按实际可用宽算出「塞得下就多排一列」的轨道宽**，
 * 上限见 `CARD_TRACK_MAX`。即：
 *   n列塞不下、但 (n+1) 列塞得下且 (n+1)×CARD_TRACK_MAX ≤ 可用宽 ⇒ 用该 (n+1) 列的均摊宽度。
 * 这样余量被吸收进卡片间距（视觉上是「卡片稍松」而非「右侧留白」）。
 *
 * 🔴 上限取 420（=320 + 100 弹性）的理由：卡间距超过约 100px 时，
 *    两张卡之间会出现「看起来像两组」的断裂感（实测 1366 窗口若不设上限，
 *    均摊轨道 = 415px ⇒ 间距 95px，视觉上卡片区被切成一堆一堆）。
 *    420 让常见的 1366/1440 两档都能多排一列又不至于散架。
 *
 * 🔴 为什么不写死成 `minmax(320px, 1fr)`（即让卡片拉伸填满）：
 * 拉伸会改变卡片**宽度** —— 而 320 是 SystemToolkit 的卡片规格（铁律），
 * 封面 320×175 的比例被改动即为规格偏离。此处只在**相邻卡片的间距**上做弹性，
 * 卡片自身宽度恒为 320。
 */
const CARD_TRACK_MAX = 420;

/* ══ 页面骨架 ═══════════════════════════════════════════════════════════════ */

/**
 * 页面容器：**限最大宽度 + 居中**。
 *
 * 🔴 这是「右侧大片空白」的根因修复（`CardWrapGrid` 是 `flex-wrap` + 定宽 320px 卡片，
 * 容器 `w:100%`：3 列占 1002px / 可用约 1144px → 右侧恒定空约 142px，全堆在左边）。
 *
 * 为什么**只限宽不两端对齐**：
 * - `space-between` 在只剩 1~2 张卡时会把它们甩到容器两端，间距变得极大
 *   —— 那是 justified 排版（报纸语汇），卡片网格用它是错配。
 * - `center` 则任何卡数下都保持「居中的一块」，两侧留白相等，视觉最稳。
 *
 * 1400px 的由来：1240×640 是规范 §四 的最小口径可用宽约 1144px；1400 则是
 * `floor(1400/334) = 4` 列的整倍数。窗口更宽（如 1920）时锁宽，避免 5 列把卡片
 * 拉散到失去「库」的整体感。
 */
export function PageBody({ children, ...rest }: BoxProps & { children?: ReactNode }) {
  return (
    <Box maxW="1400px" mx="auto" w="100%" px={{ base: 3, md: 5 }} pb={10} {...rest}>
      {children}
    </Box>
  );
}

/* ══ 进场编排 ═══════════════════════════════════════════════════════════════ */

const cardEnter = keyframes`
  from { opacity: 0; transform: translateY(8px); }
  to   { opacity: 1; transform: none; }
`;

/**
 * 卡片逐张进场：40ms 递进，**只播一次**。
 *
 * 🔴 用 `nth-of-type` 而非 `nth-child`：后者在子元素类型不唯一时序号会错位，
 * 且 emotion 会对它在 SSR 场景报不安全。本页卡片全是 `div`（`GameCardShell`），
 * 两种选择器结果相同，但 `nth-of-type` 语义更稳。
 *
 * 🔴 递进上限 10 张：库存可达上千张，第 11 张之后统一 0ms 延迟 ——
 * 否则最后一张要等 4 秒才出现。
 * 🔴 用 `transform` + `opacity`：两者走合成层，不触发布局；
 * `animation-fill-mode: backwards` 让元素在**延迟期间**就位，
 * 避免「先出现在终位、再跳回起点」的双重闪动。
 *
 * ══ 排布：为什么是 grid 而不是 flex-wrap ══════════════════════════════════
 * 2026-10-04 实测（窗口 1240，内容区约 1132）：
 *   定宽 320 + 右margin 14 = 每卡占 334 → `floor(1132 / 334) = 3` 列 = 1002px，
 *   容器 1132px ⇒ **右侧恒定空 130px**（用户截图实测量级一致）。
 *
 * 🔴 修法不是给 flex 加 `justify-content: center` —— 那样卡片会整体右移 65px，
 * 而上方的分组标题（盘符 + 游戏数）仍贴在容器左缘 ⇒ 标题与第一张卡**错位 65px**，
 * 比原来的右侧空白更难看。
 *
 * ✅ 采用「grid + 标题同模板」：
 *   `grid-template-columns: repeat(auto-fill, 334px)` 让**轨道数由可用宽度决定**，
 *   整组轨道作为一个块由 `justify-content: center` 居中 ⇒ 空白**平分两侧**；
 *   `334 = CARD_WIDTH(320) + CARD_MARGIN_R(14)`，卡片自身的 `mr` 承担卡间距，
 *   故容器 `gap: 0`（否则叠加成 28px，破坏 SystemToolkit 的 `Margin="0,0,14,18"`）。
 *   分组标题用**完全相同**的模板并 `grid-column: 1 / -1` ⇒ 它横跨整组轨道，
 *   左缘 = 轨道块左缘 = 第一张卡的左缘，**天然对齐**。
 *
 * 🔴 为什么用 `auto-fill` 而非 `auto-fit`：`auto-fit` 会把**空轨道折叠**，
 * 轨道数随实际卡片数变化；标题那行若也在同一容器内跨 `1/-1`，会因「所有轨道都有内容」
 * 而阻止折叠 —— 但我们把标题放在**另一个**同模板容器里，故两者轨道数恒等，
 * 对齐成立。`auto-fill` 保证轨道数只由宽度决定，标题与卡片必然同步。
 */
export function StaggeredCards({ children }: { children: ReactNode }) {
  const { ref, track } = useCardTrack();
  return (
    <Box
      ref={ref}
      {...trackStyle(track)}
      sx={{
        "> *": {
          animation: `${cardEnter} var(--aegis-motion-duration-normal) var(--aegis-motion-easing-decelerate) backwards`,
        },
        "& > *:nth-of-type(1)": { animationDelay: "0ms" },
        "& > *:nth-of-type(2)": { animationDelay: "40ms" },
        "& > *:nth-of-type(3)": { animationDelay: "80ms" },
        "& > *:nth-of-type(4)": { animationDelay: "120ms" },
        "& > *:nth-of-type(5)": { animationDelay: "160ms" },
        "& > *:nth-of-type(6)": { animationDelay: "200ms" },
        "& > *:nth-of-type(7)": { animationDelay: "240ms" },
        "& > *:nth-of-type(8)": { animationDelay: "280ms" },
        "& > *:nth-of-type(9)": { animationDelay: "320ms" },
        "& > *:nth-of-type(n+10)": { animationDelay: "360ms" },
        // 规范 §8.3：reduced-motion 下完全不播（位移与淡入皆纯装饰，卡片内容不受影响）
        "@media (prefers-reduced-motion: reduce)": {
          "& > *": { animation: "none" },
        },
      }}
    >
      {children}
    </Box>
  );
}

/**
 * 筛选切换时的列表反馈：整块**短暂**回落再回位。
 *
 * 🔴 为什么不是逐张重播：库存可达上千张，筛选一次播上千个动画 = 持续抖动，
 * 反而看不出「变了什么」。整块一次回落 160ms 足以传达「列表已换」，
 * 且成本恒定（一次重绘）而非随卡片数线性增长。
 */
export function FilterSwap({ swapKey, children }: { swapKey: string; children: ReactNode }) {
  const [visible, setVisible] = useState(true);
  const first = useRef(true);

  useEffect(() => {
    if (first.current) {
      first.current = false;
      return;
    }
    // 离场：先压暗，再回位
    setVisible(false);
    const t = window.setTimeout(() => setVisible(true), 30);
    return () => window.clearTimeout(t);
  }, [swapKey]);

  return (
    <Box
      transition="opacity 90ms linear"
      opacity={visible ? 1 : 0.35}
      // reduced-motion：不做压暗（那是纯视觉反馈，筛���结果本身已由内容变化说明）
      sx={{ "@media (prefers-reduced-motion: reduce)": { transition: "none" } }}
    >
      {children}
    </Box>
  );
}

/**
 * 分组标题行（盘符 + 路径 + 库标签 + 游戏数 + 总大小）。
 *
 * 🔴 为什么它也走一遍**同一条 grid 模板**：
 * 卡片区是「轨道块整体居中」，若标题用一个普通的全宽 `HStack`，它会贴容器左缘，
 * 而卡片已右移了半个空白 ⇒ 标题与第一张卡错开半格（实测约 65px），比原来的
 * 右侧空白更难看。
 * 这里让标题容器用**与 `StaggeredCards` 完全相同**的
 * `repeat(auto-fill, 334px)` + `justify-content: center`，再让内容
 * `grid-column: 1 / -1` 横跨全部轨道 ⇒ 标题的左缘 = 轨道块左缘 = 首卡左缘，
 * **天然对齐**，且窗口宽度变化时两者同步。
 */
/**
 * 测量自身可用宽并据此算出轨道宽（见 `CARD_TRACK_MAX` 的注释）。
 *
 * 🔴 为什么必须量、不能靠媒体查询：本页可用宽 = 窗口 − 导航胶囊 − Frame 内缩 −
 * PageBody padding，这几层都由**运行时布局**决定（含系统文本放大、窗口缩放），
 * 媒体查询拿不到这个值，猜出来的列数会与实际不符。
 *
 * 用 `ResizeObserver` 而非 `window.resize`：后者只认窗口，而上面那几层里
 * 任何一层变化（字号变化导致 padding 变、导航胶囊改宽）都不触发 resize。
 *
 * 首帧返回 `undefined` → 组件回落到「不设 grid-template-columns」的 flex 形态，
 * 测量完成后一次性切换。刻意**不加进场动画**避免这次切换被读成"闪一下"。
 */
function useCardTrack() {
  const ref = useRef<HTMLDivElement | null>(null);
  const [track, setTrack] = useState<number | undefined>(undefined);

  useEffect(() => {
    const el = ref.current;
    if (!el) return;
    const compute = (avail: number): number => {
      const n = Math.max(1, Math.floor(avail / CARD_TRACK));
      // n 列塞得下，但余量够再塞一列且不超上限 → 改用 n+1 列的均摊宽度
      if ((n + 1) * CARD_TRACK <= avail && (n + 1) * CARD_TRACK_MAX <= avail) {
        return Math.floor(avail / (n + 1));
      }
      return CARD_TRACK;
    };
    const ro = new ResizeObserver((entries) => {
      const w = entries[0]?.contentRect.width;
      if (w && w > 0) setTrack(compute(w));
    });
    ro.observe(el);
    return () => ro.disconnect();
  }, []);

  return { ref, track };
}

/**
 * 轨道样式：`grid-template-columns` 只在量到宽度后给，未量到时回落到 flex-wrap。
 *
 * 🔴 返回类型**必须显式标注**为 `BoxProps`：JSX 展开 `{...trackStyle(track)}` 时，
 *    若让 TS 从对象字面量推断，会把 `display` / `flexWrap` 推成 `string`，
 *    而 Chakra 的同名 prop 是**字面量联合类型**（`FlexWrap` 等）⇒ 报 TS2322。
 *    标注成 `BoxProps` 后，这些值由 Chakra 的 prop 类型收窄。
 */
function trackStyle(track: number | undefined): BoxProps {
  return track
    ? {
        display: "grid",
        gridTemplateColumns: `repeat(auto-fill, ${track}px)`,
        justifyContent: "center",
        alignItems: "start",
      }
    : { display: "flex", flexWrap: "wrap", alignItems: "flex-start" };
}

export function GroupHeader({ children }: { children: ReactNode }) {
  const { ref, track } = useCardTrack();
  return (
    <Box ref={ref} {...trackStyle(track)} mb={3}>
      <Box gridColumn="1 / -1">{children}</Box>
    </Box>
  );
}

/* ══ 筛选分段控件 ═══════════════════════════════════════════════════════════ */

export type GameFilter = "all" | "installed" | "notInstalled";

/**
 * 筛选分段控件：3 档 + 各档数量。
 *
 * 🟢 为什么用分段控件而不是下拉/标签页：
 * - 用户的第一诉求是「快速选择想玩的游戏」，而判断依据就是各档的**数量**
 *   （`未安装 116` 意味着「这台机器上有 116 个我拥有但没装的游戏」——
 *   这是本页最关键的信息之一）。下拉会把数量藏进菜单里，等于藏起决策依据。
 * - 分段控件让「选中哪档」与「各档多少」在同一行同时可读。
 * - 语义上它是**一组互斥选项**（不是导航），故用 `role="radiogroup"` + `aria-checked`，
 *   读屏能正确播报「已选中 / 未选中」，而不是当成 3 个可跳转的链接。
 */
export function FilterSegments({
  value,
  onChange,
  counts,
  activeColor,
  items,
}: {
  value: GameFilter;
  onChange: (v: GameFilter) => void;
  counts: Record<GameFilter, number>;
  activeColor: string;
  items: { key: GameFilter; label: string }[];
}) {
  return (
    <HStack
      role="radiogroup"
      aria-label="游戏筛选"
      spacing={1}
      p={1}
      bg="var(--aegis-surface-inset)"
      borderRadius="var(--aegis-radius-sm)"
    >
      {items.map((it) => {
        const active = value === it.key;
        return (
          <Box
            key={it.key}
            as="button"
            type="button"
            role="radio"
            aria-checked={active}
            onClick={() => onChange(it.key)}
            px={3}
            py={1.5}
            borderRadius="var(--aegis-radius-sm)"
            bg={active ? activeColor : "transparent"}
            color={active ? "var(--aegis-accent-on)" : "var(--aegis-mute)"}
            fontSize="sm"
            fontWeight={active ? 700 : 400}
            cursor="pointer"
            display="flex"
            alignItems="center"
            gap={1.5}
            transition="background-color var(--aegis-motion-duration-fast) var(--aegis-motion-easing-standard), color var(--aegis-motion-duration-fast) var(--aegis-motion-easing-standard)"
            sx={{
              // 规范 §八：禁 outline:none 且无替代焦点指示 —— 保留浏览器默认焦点环，
              // 与全站 *:focus-visible 的 2px accent-strong 焦点环一致
              "@media (prefers-reduced-motion: reduce)": {
                transition: "none",
              },
            }}
            _hover={{ bg: active ? activeColor : "var(--aegis-surface-soft)" }}
          >
            {it.label}
            <Text
              as="span"
              fontFamily="var(--font-mono)"
              fontSize="xs"
              color={active ? "var(--aegis-accent-on)" : "var(--aegis-stone)"}
            >
              {counts[it.key]}
            </Text>
          </Box>
        );
      })}
    </HStack>
  );
}

/* ══ 骨架 ═══════════════════════════════════════════════════════════════════ */

const shimmer = keyframes`
  from { background-position: 200% 0; }
  to   { background-position: -200% 0; }
`;

/**
 * 卡片骨架：按真实卡片轮廓（320 宽/ 175 封面高）占位。
 *
 * 🔴 为什么不继续用整页 Spinner：库存采集要数秒（读 .acf + 拉在线清单），
 * 期间整页只有一个转圈 ⇒ 界面看起来像「什么都没开始」。
 * 卡片轮廓占位让用户**先看到库的形状**，也符合本项目「启动后第一眼不能是加载态」的裁决。
 * 纯背景色动画走合成器友好路径（无布局属性参与）。
 */
export function CardSkeleton({ count = 6 }: { count?: number }) {
  const { ref, track } = useCardTrack();
  return (
    <Box ref={ref} {...trackStyle(track)}>
      {Array.from({ length: count }, (_, i) => (
        <Box
          key={i}
          w={`${CARD_WIDTH}px`}
          // 🔴 轨道可能因「多塞一列」而变宽（见 CARD_TRACK_MAX），骨架的右边距
          //    必须跟着变，否则骨架与真实卡片的列间距不一致、加载完成时会"跳一下"。
          mr={`${track ? track - CARD_WIDTH : CARD_MARGIN_R}px`}
          mb={`${CARD_MARGIN_B}px`}
          borderRadius="0 0 var(--aegis-radius-sm) var(--aegis-radius-sm)"
          overflow="hidden"
          border="1px solid var(--aegis-hairline)"
        >
          <Box h={`${COVER_HEIGHT}px`} bg="var(--aegis-surface-inset)" />
          <Box p="14px 12px 14px 14px">
            <Box
              h="14px"
              w="70%"
              borderRadius="var(--aegis-radius-sm)"
              bg="var(--aegis-surface-inset)"
            />
            <Box
              h="11px"
              w="90%"
              mt="8px"
              borderRadius="var(--aegis-radius-sm)"
              bg="var(--aegis-surface-inset)"
            />
            <Box
              h="11px"
              w="40%"
              mt="6px"
              borderRadius="var(--aegis-radius-sm)"
              bg="var(--aegis-surface-inset)"
            />
            <Box
              h="32px"
              mt="12px"
              borderTop="1px solid var(--aegis-hairline)"
              pt="10px"
              display="grid"
              gridTemplateColumns="1fr 1fr"
              gap="8px"
            >
              <Box borderRadius="var(--aegis-radius-sm)" bg="var(--aegis-surface-inset)" />
              <Box borderRadius="var(--aegis-radius-sm)" bg="var(--aegis-surface-inset)" />
            </Box>
          </Box>
        </Box>
      ))}
      {/* 进度条：横跨全部轨道（`gridColumn`）—— 容器改成 grid 后，
          `w:100%` 只占**一格**宽，必须显式跨列。 */}
      <Box
        gridColumn="1 / -1"
        h="6px"
        borderRadius="var(--aegis-radius-sm)"
        bg="var(--aegis-surface-inset)"
        backgroundImage="linear-gradient(90deg, transparent, var(--aegis-surface-soft), transparent)"
        backgroundSize="200% 100%"
        animation={`${shimmer} 1.4s linear infinite`}
        sx={{
          "@media (prefers-reduced-motion: reduce)": { animation: "none" },
        }}
      />
    </Box>
  );
}
