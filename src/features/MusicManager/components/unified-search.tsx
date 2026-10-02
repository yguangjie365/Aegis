// F-MU-32 统一在线搜索组件（占位桩，待后端 API 接入）
// 布局参考 NexBox MusicPage.tsx 搜索区

import { useState, useCallback } from "react";
import { useTranslation } from "react-i18next";
import { HStack, Input, IconButton, Text, VStack, Spinner } from "@chakra-ui/react";
import { Search } from "lucide-react";

const accent = "var(--aegis-accent)";
const mute = "var(--aegis-mute)";
const surface = "var(--aegis-surface)";

export function UnifiedSearch() {
  const { t } = useTranslation(["music"]);
  const [keywords, setKeywords] = useState("");
  const [loading, setLoading] = useState(false);

  const handleSearch = useCallback(() => {
    if (!keywords.trim()) return;
    setLoading(true);
    // 🔴 TODO: 调用 music_search 命令
    setTimeout(() => setLoading(false), 500); // 占位延迟
  }, [keywords]);

  const handleKeyDown = useCallback(
    (e: React.KeyboardEvent<HTMLInputElement>) => {
      if (e.key === "Enter") {
        handleSearch();
      }
    },
    [handleSearch],
  );

  return (
    <VStack spacing={3} align="stretch">
      {/* 搜索输入框 */}
      <HStack spacing={2}>
        <Input
          placeholder={t("music:search_placeholder")}
          value={keywords}
          onChange={(e) => setKeywords(e.target.value)}
          onKeyDown={handleKeyDown}
          size="sm"
          bg={surface}
          borderColor="transparent"
          _focus={{ borderColor: accent }}
          flex={1}
        />
        <IconButton
          aria-label={t("common:search")}
          icon={loading ? <Spinner size="xs" /> : <Search size={16} />}
          onClick={handleSearch}
          disabled={!keywords.trim() || loading}
          size="sm"
          variant="ghost"
          color={accent}
        />
      </HStack>

      {/* 搜索结果提示 */}
      {!loading && keywords && (
        <Text fontSize="xs" color={mute} textAlign="center">
          {t("music:search_stub_hint")}
        </Text>
      )}

      {/* 搜索结果列表（占位） */}
      {!loading && !keywords && (
        <VStack spacing={4} py={8} align="center">
          <Search size={48} color={mute} opacity={0.3} />
          <Text fontSize="sm" color={mute}>
            {t("music:search_empty_hint")}
          </Text>
        </VStack>
      )}
    </VStack>
  );
}
