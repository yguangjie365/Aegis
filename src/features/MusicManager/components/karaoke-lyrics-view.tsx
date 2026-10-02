// MU-3 卡拉OK歌词列表视图 —— 移植自 NexBox `components/KaraokeLyricsView.tsx`（GPL-3.0 同源衍生）。
// 三层渲染解耦沿用：① 父组件不因 timeupdate 重渲染；② 本组件 250ms 低频定时器算 activeIndex，
// 仅行变化才 setState；③ KaraokeLyricLine 内部 RAF 直改 DOM width，60fps 零 React 参与。
// 适配差异：① F-MU-23/20：activeIndex 用回拨后的歌词位置 + findActiveIndex（含 0.05s 前瞻容差），
// 不再用 NexBox 裸 `<=`；② 切歌防跳窗（0.5s 同曲判定 / 2.0s 旧曲位置跳过）原样保留；
// ③ 配色令牌化、空态文案走 i18n（原因文案由父级 lyrics-panel 决定，本组件只渲染行流）。

import { useEffect, useRef, useState, memo } from "react";
import { Box, VStack } from "@chakra-ui/react";
import type { KaraokeLine } from "../lib/karaoke-lyrics";
import { findActiveIndex, LYRIC_REWIND_S } from "../lib/karaoke-lyrics";
import { KaraokeLyricLine } from "./karaoke-lyric-line";

interface KaraokeLyricsViewProps {
  lines: KaraokeLine[];
  fontSize: number;
  textColor: string;
  highlightColor: string;
  subTextColor: string;
  audioRef?: HTMLAudioElement | null;
  maxHeight?: string;
  align?: "center" | "left";
}

export const KaraokeLyricsView = memo(function KaraokeLyricsViewInner({
  lines,
  fontSize,
  textColor,
  highlightColor,
  subTextColor,
  audioRef,
  maxHeight = "100%",
  align = "center",
}: KaraokeLyricsViewProps) {
  const scrollRef = useRef<HTMLDivElement>(null);
  const [activeIndex, setActiveIndex] = useState(-1);

  // 用 ref 保存最新 lines，供定时器闭包读取
  const linesRef = useRef(lines);
  linesRef.current = lines;

  // F-MU-23 切歌防跳窗：lines 变化后设标记，等 currentTime 明显回落（新曲从 0 附近起播）才恢复判定，
  // 防止上一曲末尾时刻用新歌词算出高 activeIndex、歌词闪一下再跳回
  const waitingForNewSongRef = useRef(false);
  const lastCurrentTimeRef = useRef(0);

  useEffect(() => {
    setActiveIndex(-1);
    if (audioRef) {
      lastCurrentTimeRef.current = audioRef.currentTime;
    }
    waitingForNewSongRef.current = true;
    if (scrollRef.current) {
      scrollRef.current.scrollTo({ top: 0, behavior: "auto" });
    }
  }, [lines]); // eslint-disable-line react-hooks/exhaustive-deps

  // 低频定时器（~250ms）：仅在行变化时 setState
  useEffect(() => {
    if (!audioRef) return;

    const checkActive = () => {
      const t = audioRef.currentTime;

      if (waitingForNewSongRef.current) {
        // 同一首歌重新挂载（面板收起再展开）：时间几乎没变，直接清除标记
        if (Math.abs(t - lastCurrentTimeRef.current) < 0.5) {
          waitingForNewSongRef.current = false;
        } else if (t > 2.0 && t >= lastCurrentTimeRef.current - 1) {
          // 仍在旧歌曲位置（时间没明显变小），本轮跳过
          return;
        }
        waitingForNewSongRef.current = false; // 新曲已开始，清除标记
      }

      // F-MU-23：歌词位置统一回拨；F-MU-20：判定含 0.05s 前瞻容差
      const newIdx = findActiveIndex(linesRef.current, Math.max(0, t - LYRIC_REWIND_S));
      setActiveIndex((prev) => (prev !== newIdx ? newIdx : prev));
    };

    checkActive();
    const interval = setInterval(checkActive, 250);
    return () => clearInterval(interval);
  }, [audioRef]);

  // 自动滚动：当前行定位到容器中部偏下，让用户看到更多即将到来的歌词
  useEffect(() => {
    if (activeIndex < 0 || !scrollRef.current) return;
    const container = scrollRef.current;
    const activeEl = container.querySelector<HTMLElement>(`[data-lyric-idx="${activeIndex}"]`);
    if (activeEl) {
      container.scrollTo({
        top: activeEl.offsetTop - container.clientHeight * 0.5,
        behavior: "smooth",
      });
    }
  }, [activeIndex]);

  return (
    <Box
      ref={scrollRef}
      flex={1}
      minH={0}
      maxH={maxHeight}
      overflowY="auto"
      overflowX="hidden"
      sx={{
        overflowX: "hidden",
        maskImage:
          "linear-gradient(to bottom, transparent 0%, black 12%, black 88%, transparent 100%)",
        WebkitMaskImage:
          "linear-gradient(to bottom, transparent 0%, black 12%, black 88%, transparent 100%)",
        "&::-webkit-scrollbar": { width: "8px" },
        "&::-webkit-scrollbar-thumb": { background: "var(--aegis-hairline)", borderRadius: "4px" },
      }}
      pr={2}
    >
      <Box minH="100%" display="flex" flexDirection="column" justifyContent="center">
        <VStack spacing={3} align="stretch">
          {lines.map((line, idx) => {
            const isActive = idx === activeIndex;
            return (
              <Box
                key={idx}
                data-lyric-idx={idx}
                py={1}
                flexShrink={0}
                sx={{
                  transition: "opacity 0.3s ease",
                  opacity: isActive ? 1 : 0.55,
                  transform: isActive ? "scale(1.02)" : "scale(1)",
                }}
              >
                <KaraokeLyricLine
                  line={line}
                  nextLine={lines[idx + 1]}
                  isActive={isActive}
                  fontSize={fontSize}
                  textColor={textColor}
                  highlightColor={highlightColor}
                  subTextColor={subTextColor}
                  audioRef={audioRef}
                  align={align}
                />
              </Box>
            );
          })}
        </VStack>
      </Box>
    </Box>
  );
});
