// 在线音乐主页面（MU-4.13）—— 从 NexBox MusicPage.tsx 提取核心在线功能
// 🔴 完全复用 NexBox UI 结构，适配 Aegis 架构

import { useEffect, useState, useCallback } from "react";
import { useTranslation } from "react-i18next";
import {
  Box,
  VStack,
  HStack,
  Text,
  Input,
  IconButton,
  Button,
  Spinner,
  Tabs,
  TabList,
  TabPanels,
  Tab,
  TabPanel,
} from "@chakra-ui/react";
import { Search, ListMusic, TrendingUp, Heart, LogIn, X } from "lucide-react";
import { useOnlineStore } from "../../store/online-store";
import { usePlayback } from "../../store/playback-store";
import { LoginModal } from "../components/login-modal";
import { invoke } from "@/lib/tauri/invoke";
import type { Song, Playlist, MusicProvider, SongUrlResult } from "@/types/music";
import type { LocalSong } from "@/lib/tauri/music";

const accent = "var(--aegis-accent)";
const ink = "var(--aegis-ink)";
const mute = "var(--aegis-mute)";
const surface = "var(--aegis-surface)";

const PROVIDERS: { id: MusicProvider; name: string; color: string }[] = [
  { id: "netease", name: "网易云音乐", color: "#C20C0C" },
  { id: "qq", name: "QQ 音乐", color: "#FEC135" },
];

/** 歌曲行组件 */
function SongRow({ song, onPlay }: { song: Song; onPlay: (s: Song) => void }) {
  return (
    <HStack
      p={2}
      borderRadius="md"
      cursor="pointer"
      _hover={{ bg: "var(--aegis-surface-inset)" }}
      onClick={() => onPlay(song)}
      transition="background 0.15s"
    >
      {/* 封面 */}
      <Box
        w="40px"
        h="40px"
        borderRadius="md"
        flexShrink={0}
        bg="var(--aegis-surface-inset)"
        display="flex"
        alignItems="center"
        justifyContent="center"
        overflow="hidden"
      >
        {song.cover ? (
          <img
            src={song.cover}
            alt=""
            style={{ width: "100%", height: "100%", objectFit: "cover" }}
          />
        ) : (
          <Heart size={18} color={mute} />
        )}
      </Box>

      {/* 信息 */}
      <VStack spacing={0} align="start" flex={1} minW={0}>
        <Text color={ink} fontSize="sm" fontWeight="medium" noOfLines={1}>
          {song.name}
        </Text>
        <Text color={mute} fontSize="xs" noOfLines={1}>
          {song.artist}
          {song.album ? ` - ${song.album}` : ""}
        </Text>
      </VStack>

      {/* 时长 */}
      <Text color={mute} fontSize="xs" flexShrink={0}>
        {formatDuration(song.duration)}
      </Text>

      {/* VIP 标识 */}
      {song.fee > 0 && (
        <Box
          px={1}
          py={0.5}
          borderRadius="sm"
          bg={song.fee === 1 ? "red.500" : "orange.500"}
          color="white"
          fontSize="xs"
          fontWeight="bold"
        >
          {song.fee === 1 ? "VIP" : "付费"}
        </Box>
      )}
    </HStack>
  );
}

/** 歌单卡片组件 */
function PlaylistCard({ playlist, onClick }: { playlist: Playlist; onClick: () => void }) {
  return (
    <Box
      p={2}
      borderRadius="lg"
      cursor="pointer"
      _hover={{ bg: "var(--aegis-surface-inset)" }}
      onClick={onClick}
      transition="background 0.15s"
    >
      <Box
        w="100%"
        aspectRatio="1"
        borderRadius="md"
        mb={2}
        bg="var(--aegis-surface-inset)"
        overflow="hidden"
        position="relative"
      >
        {playlist.cover ? (
          <img
            src={playlist.cover}
            alt=""
            style={{ width: "100%", height: "100%", objectFit: "cover" }}
          />
        ) : (
          <ListMusic
            size={32}
            color={mute}
            style={{
              position: "absolute",
              top: "50%",
              left: "50%",
              transform: "translate(-50%, -50%)",
            }}
          />
        )}
      </Box>
      <Text color={ink} fontSize="sm" fontWeight="medium" noOfLines={2}>
        {playlist.name}
      </Text>
      {playlist.trackCount && (
        <Text color={mute} fontSize="xs">
          {playlist.trackCount} 首
        </Text>
      )}
    </Box>
  );
}

/** 格式化时长 */
function formatDuration(ms: number): string {
  const totalSec = Math.floor(ms / 1000);
  const min = Math.floor(totalSec / 60);
  const sec = totalSec % 60;
  return `${min}:${sec.toString().padStart(2, "0")}`;
}

export function OnlineMusicPage() {
  const { t } = useTranslation(["music"]);
  const store = useOnlineStore();
  const [searchInput, setSearchInput] = useState("");
  const [activeTab, setActiveTab] = useState(0);
  const [loginOpen, setLoginOpen] = useState(false);
  const [playError, setPlayError] = useState("");
  const [loadingSongId, setLoadingSongId] = useState("");

  // 加载初始数据；注册登录窗口事件监听（MU-4.15：扫码登录成功后自动刷新）
  useEffect(() => {
    const s = useOnlineStore.getState();
    void s.initLoginListeners();
    void s.loadLoginStatuses();
    void s.loadUserPlaylists();
    void s.loadDailyRecommend();
    void s.loadRankList();
  }, []);

  // 播放歌曲：先取在线播放地址，再经本地代理（/audio Range 透传）起播
  // 🔴 不做 song.playable 预拦截：QQ 列表接口的 playable 恒 false（与 NexBox 同源），
  // 可播性只能由取地址结果判定（NexBox 同样在拿到 URL 后才判）
  const playSong = useCallback(async (song: Song) => {
    setLoadingSongId(song.id);
    setPlayError("");
    try {
      const port = await invoke<number>("music_get_proxy_port", "Music", "proxyPort");
      const res = await invoke<SongUrlResult>("music_song_url", "Music", "songUrl", {
        songId: song.id,
        platform: song.provider,
        mediaMid: song.mediaMid,
      });
      if (!res.url) {
        setPlayError(`《${song.name}》无法播放：${res.reason ?? "无可用音源"}`);
        return;
      }
      const proxied = `http://127.0.0.1:${port}/audio?url=${encodeURIComponent(res.url)}`;
      // Song → LocalSong 兼容形状：playback-store 对 http(s) path 直填 src，不走 asset 协议
      const compat = {
        ...song,
        path: proxied,
        title: song.name,
        durationMs: song.duration,
        coverPath: "",
        hasEmbeddedCover: false,
        hasEmbeddedLyric: false,
        hasLyric: false,
      } as unknown as LocalSong;
      usePlayback.getState().playSong(compat, [compat]);
    } catch (e) {
      console.error("Online play failed:", e);
      setPlayError(`《${song.name}》播放失败：${e instanceof Error ? e.message : String(e)}`);
    } finally {
      setLoadingSongId("");
    }
  }, []);

  // 搜索
  const handleSearch = useCallback(() => {
    if (!searchInput.trim()) return;
    void store.search(searchInput);
  }, [searchInput, store]);

  // 平台切换
  const handlePlatformChange = useCallback(
    (platform: MusicProvider) => {
      store.setCurrentPlatform(platform);
      void store.loadUserPlaylists();
      void store.loadDailyRecommend();
      void store.loadRankList();
    },
    [store],
  );

  return (
    <VStack spacing={4} align="stretch" h="full" minH={0} p={4}>
      {/* 顶部：平台选择 + 登录入口 */}
      <HStack spacing={2} wrap="wrap" flexShrink={0}>
        {PROVIDERS.map((p) => (
          <Button
            key={p.id}
            size="sm"
            variant={store.currentPlatform === p.id ? "solid" : "ghost"}
            bg={store.currentPlatform === p.id ? p.color : undefined}
            color={store.currentPlatform === p.id ? "white" : undefined}
            _hover={{ bg: store.currentPlatform === p.id ? p.color : "var(--aegis-surface-inset)" }}
            onClick={() => handlePlatformChange(p.id)}
          >
            <Text fontSize="xs">{p.name}</Text>
          </Button>
        ))}
        <Box flex={1} />
        <Button
          size="sm"
          variant="ghost"
          color={accent}
          leftIcon={<LogIn size={14} />}
          onClick={() => setLoginOpen(true)}
        >
          <Text fontSize="xs">
            {store.loginStatuses[store.currentPlatform]?.loggedIn
              ? store.loginStatuses[store.currentPlatform]?.nickname || "已登录"
              : "登录"}
          </Text>
        </Button>
      </HStack>

      {/* 搜索框 */}
      <HStack spacing={2} flexShrink={0}>
        <Input
          placeholder={t("music:search_placeholder")}
          value={searchInput}
          onChange={(e) => setSearchInput(e.target.value)}
          onKeyDown={(e) => e.key === "Enter" && handleSearch()}
          size="sm"
          bg={surface}
          borderColor="transparent"
          _focus={{ borderColor: accent }}
          flex={1}
        />
        <IconButton
          aria-label="搜索"
          icon={store.searching ? <Spinner size="xs" /> : <Search size={16} />}
          onClick={handleSearch}
          disabled={!searchInput.trim() || store.searching}
          size="sm"
          variant="ghost"
          color={accent}
        />
      </HStack>

      {/* 播放状态横幅（禁静默失败：出错必见文案） */}
      {(playError || loadingSongId) && (
        <HStack
          spacing={2}
          p={2}
          borderRadius="md"
          flexShrink={0}
          bg={playError ? "red.900" : "var(--aegis-surface-inset)"}
        >
          {playError ? (
            <Text fontSize="xs" color="red.200" flex={1}>
              {playError}
            </Text>
          ) : (
            <>
              <Spinner size="xs" color={accent} />
              <Text fontSize="xs" color={mute}>
                正在获取播放地址…
              </Text>
            </>
          )}
          {playError && (
            <IconButton
              aria-label="关闭提示"
              size="xs"
              variant="ghost"
              icon={<X size={12} />}
              onClick={() => setPlayError("")}
            />
          )}
        </HStack>
      )}

      {/* 标签页：推荐 / 歌单 / 搜索 / 榜单 */}
      {/* 🔴 Chakra 水平 Tabs 根是 display:block，不补链时长歌单会直接撑破外层 Card */}
      <Tabs
        index={activeTab}
        onChange={setActiveTab}
        variant="enclosed"
        flex={1}
        minH={0}
        display="flex"
        flexDirection="column"
      >
        <TabList>
          <Tab>
            <TrendingUp size={16} style={{ marginRight: "4px" }} />
            推荐
          </Tab>
          <Tab>
            <ListMusic size={16} style={{ marginRight: "4px" }} />
            歌单
          </Tab>
          <Tab>
            <Search size={16} style={{ marginRight: "4px" }} />
            搜索
          </Tab>
          <Tab>
            <Heart size={16} style={{ marginRight: "4px" }} />
            榜单
          </Tab>
        </TabList>

        <TabPanels flex={1} minH={0} display="flex" flexDirection="column">
          {/* 推荐页 */}
          <TabPanel flex={1} minH={0} overflowY="auto">
            {store.loadingRecommendations ? (
              <Box textAlign="center" py={8}>
                <Spinner size="lg" color={accent} />
              </Box>
            ) : (
              <VStack spacing={6} align="stretch">
                {/* 每日推荐歌曲 */}
                {store.dailyRecommendations.length > 0 && (
                  <VStack spacing={2} align="stretch">
                    <Text fontSize="md" fontWeight="bold" color={ink}>
                      每日推荐
                    </Text>
                    <VStack spacing={1} align="stretch">
                      {store.dailyRecommendations.slice(0, 10).map((song) => (
                        <SongRow key={song.id} song={song} onPlay={(s) => void playSong(s)} />
                      ))}
                    </VStack>
                  </VStack>
                )}

                {/* 推荐歌单 */}
                {store.recommendPlaylists.length > 0 && (
                  <VStack spacing={2} align="stretch">
                    <Text fontSize="md" fontWeight="bold" color={ink}>
                      推荐歌单
                    </Text>
                    <Box
                      display="grid"
                      gridTemplateColumns="repeat(auto-fill, minmax(150px, 1fr))"
                      gap={4}
                    >
                      {store.recommendPlaylists.slice(0, 6).map((pl) => (
                        <PlaylistCard
                          key={pl.id}
                          playlist={pl}
                          onClick={() => void store.loadPlaylistTracks(pl.id)}
                        />
                      ))}
                    </Box>
                  </VStack>
                )}

                {store.dailyRecommendations.length === 0 &&
                  store.recommendPlaylists.length === 0 && (
                    <Text color={mute} textAlign="center" py={8}>
                      暂无推荐，请先登录
                    </Text>
                  )}
              </VStack>
            )}
          </TabPanel>

          {/* 歌单页 */}
          <TabPanel flex={1} minH={0} overflowY="auto">
            {store.loadingPlaylists ? (
              <Box textAlign="center" py={8}>
                <Spinner size="lg" color={accent} />
              </Box>
            ) : (
              <VStack spacing={4} align="stretch">
                {store.userPlaylists.length === 0 ? (
                  <Text color={mute} textAlign="center" py={8}>
                    暂无歌单，请先登录
                  </Text>
                ) : (
                  <>
                    {/* 当前歌单的曲目 */}
                    {store.currentPlaylist && store.playlistTracks.length > 0 && (
                      <VStack spacing={2} align="stretch">
                        <Text fontSize="md" fontWeight="bold" color={ink}>
                          {store.currentPlaylist.name}
                        </Text>
                        <VStack spacing={1} align="stretch">
                          {store.playlistTracks.map((song) => (
                            <SongRow key={song.id} song={song} onPlay={(s) => void playSong(s)} />
                          ))}
                        </VStack>
                      </VStack>
                    )}

                    {/* 歌单列表 */}
                    <Box
                      display="grid"
                      gridTemplateColumns="repeat(auto-fill, minmax(150px, 1fr))"
                      gap={4}
                    >
                      {store.userPlaylists.map((pl) => (
                        <PlaylistCard
                          key={pl.id}
                          playlist={pl}
                          onClick={() => void store.loadPlaylistTracks(pl.id)}
                        />
                      ))}
                    </Box>
                  </>
                )}
              </VStack>
            )}
          </TabPanel>

          {/* 搜索结果页 */}
          <TabPanel flex={1} minH={0} overflowY="auto">
            {store.searching ? (
              <Box textAlign="center" py={8}>
                <Spinner size="lg" color={accent} />
              </Box>
            ) : store.searchResults.songs.length > 0 ? (
              <VStack spacing={2} align="stretch">
                <Text fontSize="sm" color={mute}>
                  找到 {store.searchResults.songs.length} 首歌曲
                </Text>
                {store.searchResults.songs.map((song) => (
                  <SongRow key={song.id} song={song} onPlay={(s) => void playSong(s)} />
                ))}
              </VStack>
            ) : (
              <Text color={mute} textAlign="center" py={8}>
                输入关键词开始搜索
              </Text>
            )}
          </TabPanel>

          {/* 榜单页 */}
          <TabPanel flex={1} minH={0} overflowY="auto">
            {store.loadingRankLists ? (
              <Box textAlign="center" py={8}>
                <Spinner size="lg" color={accent} />
              </Box>
            ) : store.rankLists.length > 0 ? (
              <VStack spacing={4} align="stretch">
                {/* 榜单曲目（点榜后展示） */}
                {store.playlistTracks.length > 0 && (
                  <VStack spacing={1} align="stretch">
                    {store.playlistTracks.map((song) => (
                      <SongRow key={song.id} song={song} onPlay={(s) => void playSong(s)} />
                    ))}
                  </VStack>
                )}
                <Box
                  display="grid"
                  gridTemplateColumns="repeat(auto-fill, minmax(150px, 1fr))"
                  gap={4}
                >
                  {store.rankLists.map((pl) => (
                    <PlaylistCard
                      key={pl.id}
                      playlist={pl}
                      onClick={() => void store.loadRankSongs(pl.id)}
                    />
                  ))}
                </Box>
              </VStack>
            ) : (
              <Text color={mute} textAlign="center" py={8}>
                暂无榜单数据
              </Text>
            )}
          </TabPanel>
        </TabPanels>
      </Tabs>

      <LoginModal isOpen={loginOpen} onClose={() => setLoginOpen(false)} />
    </VStack>
  );
}
