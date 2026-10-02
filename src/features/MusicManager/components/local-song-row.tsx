// 本地曲目行 —— 视觉与结构移植自 NexBox MusicPage.tsx `LocalSongRow`（GPL-3.0 同源衍生）。
// MU-1 适配差异：数据形状改 Aegis LocalSong（Rust 曲库直出）。
// MU-2 接入：双击起播（F-MU-09：以当前过滤后列表为队列、该曲为起点）、添加到播放队列（R-10 去重）、
// 当前曲高亮；行「移除」= 从曲库删除不属 F-MU-10~18 口径且后端无命令面，保持不接入。

import { memo } from "react";
import { useTranslation } from "react-i18next";
import {
  HStack,
  VStack,
  Box,
  Text,
  Image as ChakraImage,
  IconButton,
  Tooltip,
} from "@chakra-ui/react";
import { convertFileSrc } from "@tauri-apps/api/core";
import { FileMusic, ListPlus } from "lucide-react";
import type { LocalSong } from "@/lib/tauri/music";
import { isTauriRuntime } from "@/lib/tauri/invoke";
import { usePlayback } from "../store/playback-store";

const hoverBg = "var(--aegis-surface-inset)";
const textColor = "var(--aegis-ink)";
const subTextColor = "var(--aegis-mute)";
const accentStrong = "var(--aegis-accent-strong)";

/** mm:ss（NexBox MusicPage.formatTime 同源） */
export function formatTime(seconds: number): string {
  if (isNaN(seconds)) return "0:00";
  const m = Math.floor(seconds / 60);
  const s = Math.floor(seconds % 60);
  return `${m}:${s.toString().padStart(2, "0")}`;
}

interface LocalSongRowProps {
  song: LocalSong;
  isCurrent: boolean;
  /** F-MU-09 双击起播：由页面注入（携带过滤后列表作队列） */
  onPlayFrom: (song: LocalSong) => void;
}

export const LocalSongRow = memo(function LocalSongRow({
  song,
  isCurrent,
  onPlayFrom,
}: LocalSongRowProps) {
  const { t } = useTranslation(["music"]);
  const coverUrl = isTauriRuntime() && song.coverPath ? convertFileSrc(song.coverPath) : "";
  return (
    <HStack
      spacing={3}
      p={2}
      borderRadius="lg"
      cursor="pointer"
      _hover={{ bg: hoverBg }}
      bg={isCurrent ? hoverBg : "transparent"}
      onDoubleClick={() => onPlayFrom(song)}
      transition="background 0.15s"
      title={t("music:local.doubleClickHint")}
    >
      <Box
        w="40px"
        h="40px"
        borderRadius="md"
        flexShrink={0}
        position="relative"
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
            fallback={<FileMusic size={20} color={subTextColor} />}
          />
        ) : (
          <FileMusic size={20} color={subTextColor} />
        )}
      </Box>
      <VStack spacing={0} align="start" flex={1} minW={0}>
        <Text
          color={isCurrent ? accentStrong : textColor}
          fontSize="sm"
          noOfLines={1}
          fontWeight={isCurrent ? "bold" : "normal"}
        >
          {song.title || song.name}
        </Text>
        <HStack spacing={1} minW={0}>
          <Text color={subTextColor} fontSize="xs" noOfLines={1}>
            {song.artist}
          </Text>
          {song.album && (
            <Text color={subTextColor} fontSize="xs" noOfLines={1}>
              {" "}
              - {song.album}
            </Text>
          )}
        </HStack>
      </VStack>
      <Tooltip label={song.path}>
        <Text color={subTextColor} fontSize="xs" flexShrink={0}>
          {song.durationMs > 0 ? formatTime(song.durationMs / 1000) : ""}
        </Text>
      </Tooltip>
      <Tooltip label={t("music:local.addToQueue")}>
        <IconButton
          aria-label="add-to-queue"
          icon={<ListPlus size={14} />}
          size="xs"
          variant="ghost"
          sx={{ color: subTextColor, _hover: { bg: hoverBg, color: accentStrong } }}
          onClick={(e) => {
            e.stopPropagation();
            usePlayback.getState().appendToQueue([song]);
          }}
        />
      </Tooltip>
    </HStack>
  );
});
