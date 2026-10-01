// MU-2 待播面板（F-MU-13/14）——结构移植自 NexBox MusicPage.tsx `QueuePanel`（GPL-3.0 同源衍生）。
// 适配差异：① 数据形状 LocalSong + playback store；② 剔除 dnd-kit 拖拽排序（规格 R-9~R-11 未要求，
// 不为其引入新第三方依赖；如后续规格加需求再立）；③ 点选只切换当前曲、不重建队列（R-9）；
// ④ 令牌化配色，剔除 LiquidGlass 分支。

import { memo, useCallback, useMemo } from "react";
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
  Text,
  Tooltip,
  VStack,
  Image as ChakraImage,
} from "@chakra-ui/react";
import { convertFileSrc } from "@tauri-apps/api/core";
import { isTauriRuntime } from "@/lib/tauri/invoke";
import { ListMusic, Pause, Play, Trash2, X, FileMusic } from "lucide-react";
import { usePlayback } from "../store/playback-store";
import type { LocalSong } from "@/lib/tauri/music";
import { formatTime } from "./local-song-row";
import { VirtualList } from "./virtual-list";

const ink = "var(--aegis-ink)";
const mute = "var(--aegis-mute)";
const accentStrong = "var(--aegis-accent-strong)";
const hoverBg = "var(--aegis-surface-inset)";
const QUEUE_ROW_H = 56;

const QueueRow = memo(function QueueRow({
  song,
  index,
  isCurrent,
  isPlaying,
  onPlay,
  onRemove,
}: {
  song: LocalSong;
  index: number;
  isCurrent: boolean;
  isPlaying: boolean;
  onPlay: (song: LocalSong) => void;
  onRemove: (index: number) => void;
}) {
  const { t } = useTranslation(["music"]);
  const coverUrl = isTauriRuntime() && song.coverPath ? convertFileSrc(song.coverPath) : "";
  return (
    <HStack
      spacing={3}
      px={4}
      h={`${QUEUE_ROW_H}px`}
      cursor="pointer"
      _hover={{ bg: hoverBg }}
      bg={isCurrent ? hoverBg : "transparent"}
      onClick={() => onPlay(song)}
      transition="background 0.15s"
    >
      <Box flexShrink={0} w="20px" textAlign="center">
        {isCurrent ? (
          isPlaying ? (
            <Pause size={14} color={accentStrong} />
          ) : (
            <Play size={14} color={accentStrong} />
          )
        ) : (
          <Text fontSize="xs" color={mute}>
            {index + 1}
          </Text>
        )}
      </Box>
      <Box
        w="36px"
        h="36px"
        borderRadius="md"
        flexShrink={0}
        overflow="hidden"
        display="flex"
        alignItems="center"
        justifyContent="center"
        bg={hoverBg}
      >
        {coverUrl ? (
          <ChakraImage
            src={coverUrl}
            alt=""
            w="100%"
            h="100%"
            objectFit="cover"
            fallback={<FileMusic size={16} color={mute} />}
          />
        ) : (
          <FileMusic size={16} color={mute} />
        )}
      </Box>
      <VStack spacing={0} align="start" flex={1} minW={0}>
        <Text color={ink} fontSize="sm" noOfLines={1} fontWeight={isCurrent ? "bold" : "normal"}>
          {song.title || song.name}
        </Text>
        <Text color={mute} fontSize="xs" noOfLines={1}>
          {song.artist || song.album}
        </Text>
      </VStack>
      <Text color={mute} fontSize="xs" flexShrink={0}>
        {song.durationMs > 0 ? formatTime(song.durationMs / 1000) : ""}
      </Text>
      <Tooltip label={t("music:queue.remove")}>
        <IconButton
          aria-label="remove-from-queue"
          icon={<Trash2 size={13} />}
          size="xs"
          variant="ghost"
          sx={{ color: mute, _hover: { bg: hoverBg, color: "var(--aegis-error)" } }}
          onClick={(e) => {
            e.stopPropagation();
            onRemove(index);
          }}
        />
      </Tooltip>
    </HStack>
  );
});

export const QueuePanel = memo(function QueuePanel() {
  const { t } = useTranslation(["music"]);
  const playQueue = usePlayback((s) => s.playQueue);
  const currentIndex = usePlayback((s) => s.currentIndex);
  const isOpen = usePlayback((s) => s.queuePanelOpen);
  const isPlaying = usePlayback((s) => s.isPlaying);

  const close = useCallback(() => usePlayback.getState().setQueuePanelOpen(false), []);
  // R-9：面板内点选只切换当前曲——传入现有队列引用做定位，不重建集合
  const handlePlay = useCallback((song: LocalSong) => {
    const st = usePlayback.getState();
    void st.playSong(song, st.playQueue);
  }, []);
  const handleRemove = useCallback((index: number) => {
    usePlayback.getState().removeFromQueue(index);
  }, []);
  const handleClear = useCallback(() => {
    usePlayback.getState().clearQueue();
  }, []);

  const renderRow = useCallback(
    (song: LocalSong, i: number) => (
      <QueueRow
        key={`q-${song.id}-${i}`}
        song={song}
        index={i}
        isCurrent={i === currentIndex}
        isPlaying={isPlaying}
        onPlay={handlePlay}
        onRemove={handleRemove}
      />
    ),
    [currentIndex, isPlaying, handlePlay, handleRemove],
  );

  const scrollbarSx = useMemo(
    () => ({
      "&::-webkit-scrollbar": { width: "8px" },
      "&::-webkit-scrollbar-thumb": { background: "var(--aegis-hairline)", borderRadius: "4px" },
    }),
    [],
  );

  return (
    <Drawer isOpen={isOpen} placement="right" onClose={close} size="xs">
      <DrawerOverlay />
      {/* 高度收敛在标题栏(32px)与状态栏(26px)之间，不遮挡外壳 chrome（用户标注 2026-10-02）。
          用内联 style 而非 Chakra prop，确保压过 Drawer placement 默认的 top:0/height:100%。 */}
      <DrawerContent
        maxW="360px"
        style={{ top: "32px", bottom: "26px", height: "auto" }}
        bg="var(--aegis-canvas)"
        borderLeft="1px solid var(--aegis-hairline)"
      >
        <DrawerHeader pb={0} px={4}>
          <HStack spacing={2}>
            <ListMusic size={16} color={accentStrong} />
            <Text fontSize="sm" fontWeight="bold" color={ink}>
              {t("music:queue.title")}
            </Text>
            <Text fontSize="xs" color={mute}>
              {t("music:queue.count", { n: playQueue.length })}
            </Text>
            <Box flex={1} />
            {/* 头部按钮不带 Tooltip（用户标注 2026-10-02：提示语遮挡行内按钮） */}
            <IconButton
              aria-label={t("music:queue.clear")}
              icon={<Trash2 size={14} />}
              size="xs"
              variant="ghost"
              isDisabled={playQueue.length === 0}
              sx={{ color: mute, _hover: { bg: hoverBg, color: "var(--aegis-error)" } }}
              onClick={handleClear}
            />
            {/* 不用 DrawerCloseButton（绝对定位会与清空按钮重叠，用户标注 2026-10-02），改行内 IconButton */}
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
          <VirtualList
            items={playQueue}
            itemHeight={QUEUE_ROW_H}
            renderItem={renderRow}
            getKey={(_, i) => `q-${i}`}
            loading={false}
            emptyText={t("music:queue.empty")}
            resetKey={`queue|${playQueue.length}`}
            scrollToIndex={currentIndex >= 0 ? currentIndex : undefined}
            scrollbarSx={scrollbarSx}
          />
        </DrawerBody>
      </DrawerContent>
    </Drawer>
  );
});
