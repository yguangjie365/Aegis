// F-MU-34 我的歌单列表组件（占位桩，待后端 API 接入）
// 布局参考 NexBox MusicPage.tsx 左侧面板

import { useTranslation } from "react-i18next";
import { Box, HStack, Text, VStack, IconButton, Spinner } from "@chakra-ui/react";
import { ListMusic, RefreshCw } from "lucide-react";

const accent = "var(--aegis-accent)";
const ink = "var(--aegis-ink)";
const mute = "var(--aegis-mute)";
const surface = "var(--aegis-surface)";
const hoverBg = "var(--aegis-surface-inset)";

interface PlaylistItem {
  id: string;
  name: string;
  coverUrl?: string;
  trackCount?: number;
}

interface PlaylistListProps {
  playlists: PlaylistItem[];
  loading: boolean;
  onRefresh: () => void;
  onSelect: (id: string) => void;
}

export function PlaylistList({ playlists, loading, onRefresh, onSelect }: PlaylistListProps) {
  const { t } = useTranslation(["music"]);

  if (loading) {
    return (
      <VStack spacing={3} py={8} align="center">
        <Spinner size="md" color={accent} />
        <Text fontSize="xs" color={mute}>
          {t("music:playlist_loading")}
        </Text>
      </VStack>
    );
  }

  if (playlists.length === 0) {
    return (
      <VStack spacing={4} py={8} align="center">
        <ListMusic size={48} color={mute} opacity={0.3} />
        <Text fontSize="sm" color={mute}>
          {t("music:playlist_empty_hint")}
        </Text>
        <IconButton
          aria-label={t("common:refresh")}
          icon={<RefreshCw size={16} />}
          onClick={onRefresh}
          size="sm"
          variant="ghost"
          color={accent}
        />
      </VStack>
    );
  }

  return (
    <VStack spacing={2} align="stretch">
      {/* 刷新按钮 */}
      <HStack justify="space-between" px={2}>
        <Text fontSize="sm" fontWeight="semibold" color={ink}>
          {t("music:my_playlists")}
        </Text>
        <IconButton
          aria-label={t("common:refresh")}
          icon={<RefreshCw size={14} />}
          onClick={onRefresh}
          size="xs"
          variant="ghost"
          color={mute}
          _hover={{ color: accent }}
        />
      </HStack>

      {/* 歌单列表 */}
      {playlists.map((pl) => (
        <HStack
          key={pl.id}
          spacing={3}
          px={2}
          py={2}
          borderRadius="sm"
          cursor="pointer"
          _hover={{ bg: hoverBg }}
          onClick={() => onSelect(pl.id)}
        >
          {/* 封面占位 */}
          <Box
            w="40px"
            h="40px"
            borderRadius="sm"
            bg={surface}
            display="flex"
            alignItems="center"
            justifyContent="center"
            flexShrink={0}
          >
            <ListMusic size={20} color={mute} />
          </Box>

          {/* 歌单信息 */}
          <VStack spacing={0} align="start" flex={1} overflow="hidden">
            <Text fontSize="sm" color={ink} noOfLines={1}>
              {pl.name}
            </Text>
            <Text fontSize="xs" color={mute}>
              {pl.trackCount ?? 0} {t("music:songs_unit")}
            </Text>
          </VStack>
        </HStack>
      ))}
    </VStack>
  );
}
