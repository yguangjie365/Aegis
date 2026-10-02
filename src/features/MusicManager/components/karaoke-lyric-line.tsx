// MU-3 卡拉OK歌词行 —— 移植自 NexBox `components/KaraokeLyricLine.tsx`（GPL-3.0 同源衍生）。
// 渲染方案沿用双层叠加：底层完整暗色 + 顶层高亮按 width% 裁剪，RAF 直接操作 DOM（60fps 零重渲染）。
// 适配差异：① 配色走令牌层 CSS 变量；② F-MU-23：RAF 读取的时间统一按 LYRIC_REWIND_S 回拨
// （进度条不经此处，不受影响）；③ i18n 由调用方组件承担，本行纯展示。

import type { CSSProperties } from "react";
import { useEffect, useRef, useState, memo } from "react";
import { Box, Text } from "@chakra-ui/react";
import type { KaraokeLine } from "../lib/karaoke-lyrics";
import { getLineProgress, calculateScrollOffset, LYRIC_REWIND_S } from "../lib/karaoke-lyrics";

interface KaraokeLyricLineProps {
  line: KaraokeLine;
  nextLine?: KaraokeLine;
  isActive: boolean;
  fontSize: number;
  textColor: string;
  highlightColor: string;
  subTextColor: string;
  audioRef?: HTMLAudioElement | null;
  align?: "center" | "left";
}

export const KaraokeLyricLine = memo(function KaraokeLyricLineInner({
  line,
  nextLine,
  isActive,
  fontSize,
  textColor,
  highlightColor,
  subTextColor,
  audioRef,
  align = "center",
}: KaraokeLyricLineProps) {
  const overlayRef = useRef<HTMLSpanElement>(null);
  const containerRef = useRef<HTMLDivElement>(null);
  const scrollRef = useRef<HTMLSpanElement>(null);
  const [scrollNeeded, setScrollNeeded] = useState(false);
  const scrollLimitRef = useRef(0);

  // 超长歌词检测（溢出才水平滚动）
  useEffect(() => {
    if (!isActive || !containerRef.current || !scrollRef.current) return;
    const checkOverflow = () => {
      if (!scrollRef.current || !containerRef.current) return;
      const overflow = scrollRef.current.scrollWidth - containerRef.current.clientWidth;
      const needed = overflow > 4;
      setScrollNeeded(needed);
      scrollLimitRef.current = Math.max(0, overflow / 2);
    };
    checkOverflow();
    const timer = setTimeout(checkOverflow, 100);
    return () => clearTimeout(timer);
  }, [line.text, fontSize, isActive]);

  // 动画循环：更新顶层 width（跟随 audio 真实播放状态）。
  // 不用 isPlaying 门控：暂停缓出期间音频仍在出声，需继续跟随到真正 pause 才停表；
  // 以 audio 的 play/pause 事件驱动启停，单曲循环重播等不经过 isPlaying 的路径也能自动恢复
  useEffect(() => {
    if (!isActive || !overlayRef.current || !audioRef) return;

    const el = overlayRef.current;
    el.style.width = "0%";

    let rafId: number;
    let running = false;

    const tick = () => {
      if (!running) return;
      // F-MU-23：歌词位置回拨约 0.5s 对齐听感（仅歌词，进度条不经此路径）
      const t = Math.max(0, audioRef.currentTime - LYRIC_REWIND_S);
      const progress = getLineProgress(line, nextLine, t);

      el.style.width = `${(progress * 100).toFixed(2)}%`;

      if (scrollNeeded && scrollRef.current) {
        const offset = calculateScrollOffset(progress, scrollLimitRef.current);
        scrollRef.current.style.transform = `translate3d(${offset.toFixed(2)}px, 0, 0)`;
      }

      rafId = requestAnimationFrame(tick);
    };

    const start = () => {
      if (running) return;
      running = true;
      rafId = requestAnimationFrame(tick);
    };
    const stop = () => {
      running = false;
      cancelAnimationFrame(rafId);
    };

    const onPlay = () => start();
    const onPause = () => stop();
    // 挂载时音频已在出声（如缓出暂停期间行切换）直接起跑
    if (!audioRef.paused) start();
    audioRef.addEventListener("play", onPlay);
    audioRef.addEventListener("pause", onPause);
    return () => {
      stop();
      audioRef.removeEventListener("play", onPlay);
      audioRef.removeEventListener("pause", onPause);
    };
  }, [isActive, line, nextLine, scrollNeeded, audioRef]);

  // ── 非当前行：简单显示 ──
  if (!isActive) {
    return (
      <Box py={1}>
        <Text
          fontSize={`${fontSize}px`}
          color={textColor}
          fontWeight="normal"
          textAlign={align}
          noOfLines={2}
          wordBreak="break-word"
        >
          {line.text}
        </Text>
        {line.translation && (
          <Text
            fontSize={`${fontSize - 4}px`}
            color={subTextColor}
            mt={1}
            textAlign={align}
            noOfLines={1}
          >
            {line.translation}
          </Text>
        )}
      </Box>
    );
  }

  // ── 当前行：双层叠加卡拉OK效果 ──
  const sharedTextStyle: CSSProperties = {
    display: "inline-block",
    whiteSpace: "nowrap",
    fontSize: `${fontSize}px`,
    fontWeight: "bold",
    lineHeight: 1.4,
    letterSpacing: "0px",
  };

  // mask 渐变宽度随字号缩放，避免小字号时遮罩过大导致底层透出
  const maskFade = Math.min(18, Math.max(6, fontSize * 0.7));
  const maskGradient = `linear-gradient(90deg, #000 0%, #000 calc(100% - ${maskFade}px), rgba(0,0,0,0.3) 100%)`;

  return (
    <Box ref={containerRef} py={1} overflow="hidden" textAlign={align} position="relative">
      {/* scrollRef 包裹所有文字层，超长歌词时整体平移；纯 span + relative 保证 overlay 绝对定位基准正确 */}
      <span
        ref={scrollRef}
        style={{
          display: "inline-block",
          position: "relative",
          whiteSpace: "nowrap",
          transform: "translate3d(0, 0, 0)",
          willChange: "transform",
        }}
      >
        <span style={{ ...sharedTextStyle, color: textColor, opacity: 0.35 }}>{line.text}</span>
        <span
          ref={overlayRef}
          style={{
            ...sharedTextStyle,
            position: "absolute",
            top: 0,
            left: 0,
            width: "0%",
            overflow: "hidden",
            color: highlightColor,
            maskImage: maskGradient,
            WebkitMaskImage: maskGradient,
          }}
        >
          {line.text}
        </span>
      </span>
      {line.translation && (
        <Text
          fontSize={`${fontSize - 4}px`}
          color={subTextColor}
          opacity={0.6}
          mt={1}
          textAlign={align}
          noOfLines={1}
        >
          {line.translation}
        </Text>
      )}
    </Box>
  );
});
