// MU-3 歌词面板 —— 右侧 Drawer 容器，结构沿用 queue-panel 的 Aegis 化布局（高度收敛在标题栏与
// 状态栏之间、头部按钮不带 Tooltip——用户标注 2026-10-02 两轮采光的口径直接继承）。
// 内容分支（规格 07 §3.8 装载失败/无词 → 明确原因文案，不出现空白无解释）：
// 带时间轴 → KaraokeLyricsView 卡拉OK流；纯文本歌词 → 整段显示（F-MU-19）；
// 装载中且尚无可留旧内容 → 骨架；其余按 noticeKey 给原因文案。
// 全屏三风格歌词（F-MU-24 字号 A−/A+ 等）随 MU-6 全屏播放器接入，本面板为 MU-3 管线验证面。

import { memo, useCallback, type ReactNode } from "react";
import { useTranslation } from "react-i18next";
import {
  Box,
  Drawer,
  DrawerBody,
  DrawerContent,
  DrawerHeader,
  DrawerOverlay,
  HStack,
  IconButton,
  Spinner,
  Text,
  VStack,
} from "@chakra-ui/react";
import { AlignLeft, X } from "lucide-react";
import { usePlayback, getAudioElement } from "../store/playback-store";
import { useLyrics } from "../store/lyrics-store";
import { KaraokeLyricsView } from "./karaoke-lyrics-view";

const ink = "var(--aegis-ink)";
const mute = "var(--aegis-mute)";
const accentStrong = "var(--aegis-accent-strong)";
const hoverBg = "var(--aegis-surface-inset)";

// 面板字号暂用 NexBox 默认 18；规格 12–28 逐档调节属全屏播放器（F-MU-24），随 MU-6 落
const PANEL_FONT_SIZE = 18;

export const LyricsPanel = memo(function LyricsPanel() {
  const { t } = useTranslation(["music"]);
  const isOpen = usePlayback((s) => s.lyricsPanelOpen);
  const currentSongId = usePlayback((s) => s.currentSong?.id);
  const lyricsSongId = useLyrics((s) => s.songId);
  const lines = useLyrics((s) => s.lines);
  const plainText = useLyrics((s) => s.plainText);
  const loading = useLyrics((s) => s.loading);
  const noticeKey = useLyrics((s) => s.noticeKey);

  const close = useCallback(() => usePlayback.getState().setLyricsPanelOpen(false), []);

  // 切歌瞬间旧内容保留渲染（防闪烁）；新歌尚无内容时：装载中/已换曲未出结果 → 骨架，其余 → 原因文案
  const hasContent = (lines !== null || plainText !== null) && lyricsSongId === currentSongId;
  const showSkeleton =
    !hasContent && (loading || (lyricsSongId !== currentSongId && noticeKey === null));

  let body: ReactNode;
  if (hasContent && lines) {
    body = (
      <KaraokeLyricsView
        lines={lines}
        fontSize={PANEL_FONT_SIZE}
        textColor={ink}
        highlightColor={accentStrong}
        subTextColor={mute}
        audioRef={getAudioElement()}
        align="center"
      />
    );
  } else if (hasContent && plainText) {
    body = (
      <Box
        flex={1}
        minH={0}
        overflowY="auto"
        px={2}
        sx={{
          "&::-webkit-scrollbar": { width: "8px" },
          "&::-webkit-scrollbar-thumb": {
            background: "var(--aegis-hairline)",
            borderRadius: "4px",
          },
        }}
      >
        {/* F-MU-19：无时间轴的纯文本歌词按整段纯文本显示（不做行级高亮） */}
        <Text
          color={ink}
          fontSize={`${PANEL_FONT_SIZE}px`}
          textAlign="center"
          whiteSpace="pre-wrap"
          lineHeight={1.8}
        >
          {plainText}
        </Text>
      </Box>
    );
  } else if (showSkeleton) {
    body = (
      <VStack flex={1} justify="center" py={12}>
        <Spinner size="md" sx={{ color: accentStrong }} />
      </VStack>
    );
  } else {
    // F-MU-19/R-21：无词/失败必须给明确原因，不允许空白无解释；null 键兜底"均无"文案
    const key = noticeKey ?? "notFound";
    body = (
      <VStack flex={1} justify="center" spacing={3} py={12}>
        <AlignLeft size={32} color={mute} />
        <Text color={mute} fontSize="sm" textAlign="center" px={4}>
          {t(`music:lyrics.${key}`)}
        </Text>
      </VStack>
    );
  }

  return (
    <Drawer isOpen={isOpen} placement="right" onClose={close} size="xs">
      <DrawerOverlay />
      <DrawerContent
        maxW="360px"
        style={{ top: "32px", bottom: "26px", height: "auto" }}
        bg="var(--aegis-canvas)"
        borderLeft="1px solid var(--aegis-hairline)"
      >
        <DrawerHeader pb={0} px={4}>
          <HStack spacing={2}>
            <AlignLeft size={16} color={accentStrong} />
            <Text fontSize="sm" fontWeight="bold" color={ink}>
              {t("music:lyrics.title")}
            </Text>
            <Box flex={1} />
            {/* 头部按钮不带 Tooltip（沿用队列面板用户标注口径）；不用 DrawerCloseButton（绝对定位重叠） */}
            <IconButton
              aria-label={t("music:player.close")}
              icon={<X size={14} />}
              size="xs"
              variant="ghost"
              sx={{ color: mute, _hover: { bg: hoverBg, color: ink } }}
              onClick={close}
            />
          </HStack>
        </DrawerHeader>
        <DrawerBody p={0} display="flex" flexDirection="column" minH={0}>
          {body}
        </DrawerBody>
      </DrawerContent>
    </Drawer>
  );
});
