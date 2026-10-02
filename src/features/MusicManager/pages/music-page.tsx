// 音乐管理主页（MU-1 本地曲库 + MU-4 在线音乐）—— 整合本地与在线功能
// 🔴 MU-4.13：添加在线音乐标签页，复用 NexBox UI 结构

import { useCallback, useEffect, useMemo, useState } from "react";
import { useTranslation } from "react-i18next";
import {
  Box,
  HStack,
  IconButton,
  Input,
  Spinner,
  Text,
  Tooltip,
  VStack,
  Tabs,
  TabList,
  TabPanels,
  Tab,
  TabPanel,
} from "@chakra-ui/react";
import styled from "@emotion/styled";
import {
  ArrowLeft,
  ChevronRight,
  FolderOpen,
  ListMusic,
  Play,
  RefreshCw,
  FileMusic,
  Globe,
} from "lucide-react";
import { PageHeader } from "@/ui/components/page-header";
import { AppButton } from "@/ui/components/app-button";
import { ErrorStateBlock, LoadingSkeleton } from "@/ui/components/state-block";
import { useMusicStore, detachScanEvents } from "../store/music-store";
import { usePlayback } from "../store/playback-store";
import { VirtualList } from "../components/virtual-list";
import { LocalSongRow } from "../components/local-song-row";
import { PlayerBar } from "../components/player-bar";
import { QueuePanel } from "../components/queue-panel";
import { LyricsPanel } from "../components/lyrics-panel";
import { OnlineMusicPage } from "../online/pages/online-music-page";
import type { LocalSong } from "@/lib/tauri/music";

const Card = styled.section`
  display: flex;
  flex-direction: column;
  border: 1px solid var(--aegis-hairline);
  border-radius: var(--aegis-rounded-sm);
  background: var(--aegis-canvas);
  padding: var(--aegis-space-lg);
  height: calc(100vh - 208px);
  min-height: 420px;
`;

const statusColor: Record<string, string> = {
  info: "var(--aegis-mute)",
  success: "var(--aegis-success)",
  warning: "var(--aegis-warning)",
  error: "var(--aegis-error)",
};

export function MusicManagerPage() {
  const { t } = useTranslation(["music", "common"]);
  const store = useMusicStore();
  const { view, loading, error, songs, roots, scanning, progress, status, importing, keyword } =
    store;
  const [activeTab, setActiveTab] = useState(0); // 0=本地, 1=在线

  useEffect(() => {
    void useMusicStore.getState().open();
    return detachScanEvents;
  }, []);

  const filtered = useMemo(() => {
    const kw = keyword.trim().toLowerCase();
    if (!kw) return songs;
    return songs.filter(
      (s) =>
        s.title.toLowerCase().includes(kw) ||
        s.artist.toLowerCase().includes(kw) ||
        s.album.toLowerCase().includes(kw),
    );
  }, [songs, keyword]);

  const currentSongId = usePlayback((s) => s.currentSong?.id);

  // F-MU-09：双击行 → 以当前（过滤后）列表为播放队列、该曲为起点
  const playFrom = useCallback(
    (song: LocalSong) => {
      void usePlayback.getState().playSong(song, filtered);
    },
    [filtered],
  );

  // F-MU-13：一次起播 = 整组曲目作为队列（R-9 整体替换）
  const playAll = useCallback(() => {
    const first = filtered[0];
    if (!first || filtered.length === 0) return;
    void usePlayback.getState().playSong(first, filtered);
  }, [filtered]);

  const renderRow = useMemo(
    () => (song: LocalSong) => (
      <LocalSongRow song={song} isCurrent={song.id === currentSongId} onPlayFrom={playFrom} />
    ),
    [currentSongId, playFrom],
  );

  return (
    <>
      <PageHeader title={t("music:title")} sub={t("music:sub")} />
      <Card>
        {/* MU-4.13：标签页切换本地/在线 */}
        {/* 🔴 Chakra 2.10 水平 Tabs 根是 display:block、TabPanels 无基础样式，
            不显式建高度链时长列表会直接撑破 Card 固定高度 */}
        <Tabs
          index={activeTab}
          onChange={setActiveTab}
          variant="enclosed"
          mb={3}
          flex={1}
          minH={0}
          display="flex"
          flexDirection="column"
        >
          <TabList>
            <Tab>
              <FileMusic size={16} style={{ marginRight: "6px" }} />
              {t("music:local.tab")}
            </Tab>
            <Tab>
              <Globe size={16} style={{ marginRight: "6px" }} />
              {t("music:online.tab")}
            </Tab>
          </TabList>

          <TabPanels flex={1} minH={0} display="flex" flexDirection="column">
            {/* 本地音乐面板 */}
            <TabPanel
              p={0}
              flex={1}
              minH={0}
              display="flex"
              flexDirection="column"
              overflow="hidden"
            >
              {error ? (
                <ErrorStateBlock
                  title={t("music:error.loadTitle")}
                  body={error}
                  onRetryLabel={t("music:retry")}
                  onRetry={() => void useMusicStore.getState().refresh()}
                />
              ) : loading && songs.length === 0 && !scanning ? (
                <LoadingSkeleton rows={6} />
              ) : view === "home" ? (
                <VStack spacing={0} align="stretch" flex={1} minH={0} overflowY="auto">
                  {/* 顶部：我的音乐（计数 + 导入文件夹，对齐 F-MU-01 交互位） */}
                  <HStack spacing={2} mb={2} flexShrink={0}>
                    <ListMusic size={16} color="var(--aegis-accent-strong)" />
                    <Text
                      fontSize="sm"
                      fontWeight="bold"
                      color="var(--aegis-ink)"
                      flex={1}
                      noOfLines={1}
                    >
                      {t("music:home.myMusic")}
                    </Text>
                    <Text color="var(--aegis-mute)" fontSize="xs" flexShrink={0}>
                      {t("music:home.songCount", { n: songs.length })}
                    </Text>
                    <AppButton
                      variant="primary"
                      size="sm"
                      disabled={importing || scanning}
                      onClick={() => void useMusicStore.getState().importFolder()}
                    >
                      {importing ? <Spinner size="xs" /> : <FolderOpen size={14} />}
                      {importing ? t("music:home.importing") : t("music:home.importFolder")}
                    </AppButton>
                  </HStack>

                  {/* 本地导入的歌单入口（NexBox entry-card 同源结构） */}
                  <Box
                    p={2}
                    borderRadius="lg"
                    cursor="pointer"
                    mb={3}
                    flexShrink={0}
                    _hover={{ bg: "var(--aegis-surface-inset)" }}
                    onClick={() => useMusicStore.getState().setView("local")}
                    transition="background 0.15s"
                  >
                    <HStack spacing={3}>
                      <Box
                        w="44px"
                        h="44px"
                        borderRadius="md"
                        flexShrink={0}
                        display="flex"
                        alignItems="center"
                        justifyContent="center"
                        bg="var(--aegis-surface-inset)"
                      >
                        <FileMusic size={22} color="var(--aegis-accent-strong)" />
                      </Box>
                      <VStack spacing={0} align="start" flex={1} minW={0}>
                        <Text
                          color="var(--aegis-ink)"
                          fontSize="sm"
                          fontWeight="medium"
                          noOfLines={1}
                        >
                          {t("music:home.localEntry")}
                        </Text>
                        <Text color="var(--aegis-mute)" fontSize="xs">
                          {songs.length > 0
                            ? t("music:home.localEntrySub", { n: songs.length })
                            : t("music:home.localEntrySubEmpty")}
                        </Text>
                      </VStack>
                      <ChevronRight size={16} color="var(--aegis-mute)" />
                    </HStack>
                  </Box>

                  {/* 曲库根目录清单（F-MU-01 多根合并的可见化；R-2 去重在后端） */}
                  <Text
                    fontSize="xs"
                    fontWeight="bold"
                    color="var(--aegis-mute-on-soft)"
                    mb={1}
                    flexShrink={0}
                  >
                    {t("music:home.rootsTitle")}
                  </Text>
                  <VStack spacing={1} align="start" flexShrink={0}>
                    {roots.length === 0 ? (
                      <Text fontSize="xs" color="var(--aegis-disabled)">
                        {t("music:home.rootsEmpty")}
                      </Text>
                    ) : (
                      roots.map((r) => (
                        <Text
                          key={r}
                          fontSize="xs"
                          color="var(--aegis-mute)"
                          fontFamily="var(--font-mono)"
                          noOfLines={1}
                          title={r}
                        >
                          {r}
                        </Text>
                      ))
                    )}
                  </VStack>
                  {scanning && (
                    <Text fontSize="xs" color="var(--aegis-warning)" mt={3} flexShrink={0}>
                      {progress
                        ? t("music:local.scanning", {
                            percent: progress.percent,
                            processed: progress.processed,
                            total: progress.total,
                          })
                        : t("music:local.scanPreparing")}
                    </Text>
                  )}
                </VStack>
              ) : (
                <VStack spacing={0} align="stretch" flex={1} minH={0}>
                  {/* 本地歌曲视图头（NexBox leftPanelView==='local' 同源结构） */}
                  <HStack spacing={2} mb={2} flexShrink={0}>
                    <Tooltip label={t("music:local.back")}>
                      <IconButton
                        aria-label={t("music:local.back")}
                        icon={<ArrowLeft size={16} />}
                        size="sm"
                        variant="ghost"
                        onClick={() => useMusicStore.getState().setView("home")}
                        sx={{
                          color: "var(--aegis-accent-strong)",
                          _hover: { bg: "var(--aegis-surface-inset)" },
                        }}
                      />
                    </Tooltip>
                    <Text fontSize="sm" fontWeight="bold" color="var(--aegis-ink)" noOfLines={1}>
                      {t("music:local.title")}
                    </Text>
                    <Text color="var(--aegis-mute)" fontSize="xs" flexShrink={0}>
                      {t("music:local.count", { n: songs.length })}
                    </Text>
                    <Box flex={1} />
                    {scanning ? (
                      <AppButton
                        variant="ghost"
                        size="sm"
                        onClick={() => void useMusicStore.getState().cancelScan()}
                      >
                        {t("music:local.cancel")}
                      </AppButton>
                    ) : (
                      <AppButton
                        variant="secondary"
                        size="sm"
                        disabled={roots.length === 0}
                        onClick={() => void useMusicStore.getState().rescan()}
                      >
                        <RefreshCw size={13} />
                        {t("music:local.rescan")}
                      </AppButton>
                    )}
                    {/* MU-2 接入：播放全部（F-MU-13 整组入队，从第一首起播） */}
                    <AppButton
                      variant="primary"
                      size="sm"
                      disabled={filtered.length === 0}
                      onClick={playAll}
                    >
                      <Play size={13} />
                      {t("music:local.playAll")}
                    </AppButton>
                  </HStack>

                  {(status || scanning) && (
                    <HStack spacing={2} mb={2} flexShrink={0}>
                      {scanning && (
                        <Text fontSize="xs" color="var(--aegis-warning)">
                          {progress
                            ? t("music:local.scanning", {
                                percent: progress.percent,
                                processed: progress.processed,
                                total: progress.total,
                              })
                            : t("music:local.scanPreparing")}
                        </Text>
                      )}
                      {!scanning && status && (
                        <Text
                          fontSize="xs"
                          color={statusColor[status.tone] ?? "var(--aegis-mute)"}
                          aria-live="polite"
                        >
                          {status.text}
                        </Text>
                      )}
                    </HStack>
                  )}

                  <Input
                    size="sm"
                    mb={2}
                    flexShrink={0}
                    placeholder={t("music:local.searchPlaceholder")}
                    value={keyword}
                    onChange={(e) => useMusicStore.getState().setKeyword(e.target.value)}
                    sx={{
                      borderColor: "var(--aegis-hairline)",
                      _hover: { borderColor: "var(--aegis-border-interactive)" },
                      _focus: { borderColor: "var(--aegis-accent-strong)", boxShadow: "none" },
                    }}
                  />

                  <VirtualList
                    items={filtered}
                    itemHeight={60}
                    renderItem={renderRow}
                    getKey={(song, i) => `${song.id}-${i}`}
                    loading={false}
                    emptyText={t("music:local.empty")}
                    resetKey={`local|${keyword}`}
                    scrollbarSx={{
                      "&::-webkit-scrollbar": { width: "8px" },
                      "&::-webkit-scrollbar-thumb": {
                        background: "var(--aegis-hairline)",
                        borderRadius: "4px",
                      },
                    }}
                  />
                </VStack>
              )}
            </TabPanel>

            {/* 在线音乐面板（MU-4.13） */}
            <TabPanel
              p={0}
              flex={1}
              minH={0}
              display="flex"
              flexDirection="column"
              overflow="hidden"
            >
              <OnlineMusicPage />
            </TabPanel>
          </TabPanels>
        </Tabs>

        {/* F-MU-10：底部常驻播放条（模块页内常驻，曲库视图/主页均可见） */}
        <PlayerBar />
      </Card>
      <QueuePanel />
      <LyricsPanel />
    </>
  );
}
