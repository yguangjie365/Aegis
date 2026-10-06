/**
 * 迷你播放器（左下角悬浮胶囊）—— 2026-10-05 重写。
 *
 * 🔴🔴 **为什么重写**（此前完全不生效，根因是三层断裂）：
 *   ① **数据源接错**：原实现用 `useMusicPlayer()`（`contexts/music-context.tsx`），
 *      那套 Context 是 NexBox 旧「本地文件播放器」的遗留物，靠 Tauri `get_music_files`
 *      驱动；而真实播放状态在 **zustand**（`store/music-store.ts`）。
 *      `togglePlay` 只改 zustand、**从不调 Context 的 setIsPlaying(true)**，
 *      所以 `if (!isPlaying) return null` 永远命中 ⇒ 永不显示。
 *   ② **组件零引用**：`MiniMusicPlayer` 从未被任何文件 import / 渲染。
 *   ③ **Provider 未挂载**：`music-context.tsx:54` 在无 `MusicProvider` 时直接 throw。
 * ⇒ 现改为读 zustand（真实数据源），并在 `shell-layout.tsx` 挂载。
 *
 * 字段口径（与 MusicPage 一致，逐字段 selector，不用 useShallow）：
 *   `currentSong.name`（**不是 title**）、`currentSong.cover`（**不是 coverUrl**）、
 *   `coverProxyUrl(url, proxyPort)`（**url 在前、端口在后**）。
 *
 * 「停止」语义：store **没有** `stopPlayback` / `pause` / `stop`。
 * 唯一等价的是 `closePlayback()`（pause + currentTime=0 + isPlaying=false
 * + 清空队列/历史/歌词），与旧 `stopPlayback` 行为一致。
 * ⚠️ 不用 `togglePlay()`：它是 **toggle** 语义，`isPlaying===false` 时会走播放分支。
 */
import { Box, IconButton, Image, Tooltip } from "@chakra-ui/react";
import { StopCircle } from "lucide-react";
import { motion, AnimatePresence } from "framer-motion";
import { useMusicStore, coverProxyUrl } from "@/features/MusicManager/store/music-store";

export function MiniMusicPlayer() {
  /* 🔴 用 CSS 变量而非 useColorModeValue —— 本组件渲染在**壳层**（挂点见 shell-layout），
   *    而 useColorModeValue 在跨壳层的挂载点上会取到**浅色分支**
   *    （实测渲成 #EDF2F7 / #1A202C，在深底上完全看不见）。
   *    判据：CSS 变量在壳层 DOM 上照常解析，hook 的分支判断不跟随。
   *    详见 MusicLoginSection.tsx 的 textColor 处注释。 */
  const isPlaying = useMusicStore((s) => s.isPlaying);
  const currentSong = useMusicStore((s) => s.currentSong);
  const proxyPort = useMusicStore((s) => s.proxyPort);

  // 🔴 currentSong 也判空：封面/歌名都取自它，且 closePlayback 会把它置 null。
  if (!isPlaying || !currentSong) return null;

  const coverUrl = coverProxyUrl(currentSong.cover, proxyPort);
  const label = currentSong.name || "正在播放";

  return (
    <AnimatePresence>
      <motion.div
        initial={{ scale: 0.8, opacity: 0 }}
        animate={{ scale: 1, opacity: 1 }}
        exit={{ scale: 0.8, opacity: 0 }}
        transition={{ duration: 0.2 }}
        style={{
          position: "fixed",
          /* 🔴 zIndex 从 100 提到 1400：原先 100 **低于** MusicPage 的 MV 遮罩（900）
             与展开播放器层级，挂载后会被压住。1400 高于页面内所有遮罩。 */
          bottom: "24px",
          left: "24px",
          zIndex: 1400,
        }}
      >
        <Tooltip label={label} placement="top">
          <Box
            bg="var(--aegis-chrome-elevated)"
            borderRadius="full"
            border="1px solid"
            borderColor="var(--aegis-hairline)"
            boxShadow="var(--aegis-elevation-sticky)"
            p={1}
            /* 🔴 原来整个胶囊挂 animation: spin-slow 8s linear infinite，
               而 **spin-slow 这个 @keyframes 全项目不存在**（只有 aegis-live-pulse /
               aegis-card-enter / aegis-nav-wave-mid 等）⇒ 浏览器静默忽略、唱片不转。
               且**整个胶囊旋转**本身就是错的（停止按钮会跟着转）。
               现改为只让**封面**旋转，且 keyframes 就地定义（沿用 MusicPage 的
               `sx: { "@keyframes x": {...} }` 惯例，见 MusicPage.tsx:382）。 */
            sx={{
              transition: "transform 0.15s ease",
              _hover: { transform: "scale(1.06)" },
              "@keyframes aegisMiniSpin": {
                from: { transform: "rotate(0deg)" },
                to: { transform: "rotate(360deg)" },
              },
            }}
          >
            <Box
              position="relative"
              w="40px"
              h="40px"
              sx={{
                animation: "aegisMiniSpin 8s linear infinite",
                /* 🔴 reduced-motion 兜底（规范 §8.3）：必须**同时**限 duration 与
                   iteration-count，只限其一仍会持续旋转。 */
                "@media (prefers-reduced-motion: reduce)": {
                  animationDuration: "0.01ms",
                  animationIterationCount: "1",
                },
              }}
            >
              {coverUrl ? (
                <Image
                  src={coverUrl}
                  alt={label}
                  w="40px"
                  h="40px"
                  borderRadius="full"
                  objectFit="cover"
                  border="1px solid"
                  borderColor="var(--aegis-hairline)"
                />
              ) : (
                /* 封面 URL 为空（本地导入无封面 / proxyPort 尚未 init）⇒ 主题色占位圆 */
                <Box
                  w="40px"
                  h="40px"
                  borderRadius="full"
                  bg="var(--aegis-surface-soft)"
                  border="1px solid"
                  borderColor="var(--aegis-hairline)"
                />
              )}
              <IconButton
                aria-label="停止播放"
                position="absolute"
                inset={0}
                m="auto"
                w="40px"
                h="40px"
                borderRadius="full"
                size="sm"
                variant="ghost"
                /* 🔴 显式 color：ghost variant 默认前景在深色下是黑色（与深底融在一起）。 */
                color="var(--aegis-ink)"
                _hover={{ bg: "rgba(0,0,0,0.35)" }}
                icon={<StopCircle size={18} color="var(--aegis-accent)" />}
                onClick={(e) => {
                  e.stopPropagation();
                  /* 「停止」= closePlayback（见文件头「停止」语义说明） */
                  useMusicStore.getState().closePlayback();
                }}
              />
            </Box>
          </Box>
        </Tooltip>
      </motion.div>
    </AnimatePresence>
  );
}
