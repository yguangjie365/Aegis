// F-MU-36 每日推荐与推荐歌单组件（占位桩，仅网易云有来源）
// 布局参考 NexBox MusicPage.tsx 发现区

import { useTranslation } from "react-i18next";
import { Text, VStack, Spinner } from "@chakra-ui/react";
import { Music, ListMusic } from "lucide-react";

const accent = "var(--aegis-accent)";
const mute = "var(--aegis-mute)";

interface DailyRecommendProps {
  platform: "netease" | "qq";
  loading: boolean;
}

export function DailyRecommend({ platform, loading }: DailyRecommendProps) {
  const { t } = useTranslation(["music"]);

  // QQ 音乐不支持每日推荐
  if (platform === "qq") {
    return (
      <VStack spacing={4} py={8} align="center">
        <Music size={48} color={mute} opacity={0.3} />
        <Text fontSize="sm" color={mute}>
          {t("music:daily_recommend_unsupported")}
        </Text>
      </VStack>
    );
  }

  if (loading) {
    return (
      <VStack spacing={3} py={8} align="center">
        <Spinner size="md" color={accent} />
        <Text fontSize="xs" color={mute}>
          {t("music:daily_recommend_loading")}
        </Text>
      </VStack>
    );
  }

  // 占位空态
  return (
    <VStack spacing={4} py={8} align="center">
      <Music size={48} color={mute} opacity={0.3} />
      <Text fontSize="sm" color={mute}>
        {t("music:daily_recommend_stub")}
      </Text>
    </VStack>
  );
}

interface RankListProps {
  platform: "netease" | "qq";
  loading: boolean;
}

export function RankList({ platform, loading }: RankListProps) {
  const { t } = useTranslation(["music"]);

  // 网易云不支持官方榜单（仅 QQ）
  if (platform === "netease") {
    return (
      <VStack spacing={4} py={8} align="center">
        <ListMusic size={48} color={mute} opacity={0.3} />
        <Text fontSize="sm" color={mute}>
          {t("music:rank_list_unsupported")}
        </Text>
      </VStack>
    );
  }

  if (loading) {
    return (
      <VStack spacing={3} py={8} align="center">
        <Spinner size="md" color={accent} />
        <Text fontSize="xs" color={mute}>
          {t("music:rank_list_loading")}
        </Text>
      </VStack>
    );
  }

  // 占位空态
  return (
    <VStack spacing={4} py={8} align="center">
      <ListMusic size={48} color={mute} opacity={0.3} />
      <Text fontSize="sm" color={mute}>
        {t("music:rank_list_stub")}
      </Text>
    </VStack>
  );
}
