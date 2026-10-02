// MU-2 底部常驻播放条（F-MU-10~18）——结构移植自 NexBox MusicPage.tsx `PlayerBar`/`PlayerProgress`
// （GPL-3.0 同源衍生）。
// 适配差异：① 数据形状 LocalSong + Aegis playback store；② 剔除音质菜单（MU-4/5 接入，暂禁用态）、
// 桌面歌词/设置/展开全屏（排除特性与 MU-6）、LiquidGlass 分支（令牌层）；
// ③ 音量按 F-MU-17 定档 0–100 语义与三态图标；④ 进度用 Chakra Slider 承接（键盘可达，
// 无障碍规范），渐让策略保持 NexBox 同源（拖动期间引擎回传不改写，R-14）。

import { memo, useCallback, useEffect, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import {
  Box,
  HStack,
  IconButton,
  Slider,
  SliderFilledTrack,
  SliderThumb,
  SliderTrack,
  Text,
  Tooltip,
  VStack,
  Image as ChakraImage,
} from "@chakra-ui/react";
import { convertFileSrc } from "@tauri-apps/api/core";
import { isTauriRuntime } from "@/lib/tauri/invoke";
import {
  AlignLeft,
  ListMusic,
  Pause,
  Play,
  Repeat,
  Repeat1,
  Shuffle,
  SkipBack,
  SkipForward,
  Volume1,
  Volume2,
  VolumeX,
  X,
  FileMusic,
} from "lucide-react";
import { usePlayback, getAudioElement } from "../store/playback-store";
import { formatTime } from "./local-song-row";

const ink = "var(--aegis-ink)";
const mute = "var(--aegis-mute)";
const accent = "var(--aegis-accent)";
const accentStrong = "var(--aegis-accent-strong)";
const accentOn = "var(--aegis-accent-on)";
const hoverBg = "var(--aegis-surface-inset)";
const trackBg = "var(--aegis-hairline)";

// PlayerProgress：timeupdate 走局部 state（NexBox 同源策略），播放期间不更新 store，
// 搜索框等其他组件完全不受播放影响
const PlayerProgress = memo(function PlayerProgress() {
  const duration = usePlayback((s) => s.duration);
  const currentSongId = usePlayback((s) => s.currentSong?.id);
  const seekTo = usePlayback((s) => s.seekTo);
  const [display, setDisplay] = useState(0);
  const draggingRef = useRef(false);

  useEffect(() => {
    setDisplay(0);
  }, [currentSongId]);

  useEffect(() => {
    const audio = getAudioElement();
    if (!audio || !currentSongId) return;
    if (!draggingRef.current) setDisplay(audio.currentTime);
    const onTime = () => {
      // R-14：拖动期间引擎位置回传不得改写进度条
      if (draggingRef.current) return;
      setDisplay(audio.currentTime);
    };
    audio.addEventListener("timeupdate", onTime);
    return () => audio.removeEventListener("timeupdate", onTime);
  }, [currentSongId]);

  const handleChange = useCallback((v: number) => {
    draggingRef.current = true;
    // R-14：松手前立即本地回显新位置
    setDisplay(v);
  }, []);

  const handleCommit = useCallback(
    (v: number) => {
      seekTo(v);
      window.setTimeout(() => {
        draggingRef.current = false;
      }, 300);
    },
    [seekTo],
  );

  return (
    <HStack spacing={2}>
      <Text color={mute} fontSize="xs" w="40px" textAlign="center">
        {formatTime(display)}
      </Text>
      <Slider
        aria-label="seek"
        value={display}
        min={0}
        max={duration > 0 ? duration : 1}
        step={0.1}
        size="sm"
        isDisabled={duration <= 0}
        onChange={handleChange}
        onChangeEnd={handleCommit}
      >
        <SliderTrack bg={trackBg} h="4px">
          <SliderFilledTrack bg={accent} />
        </SliderTrack>
        <SliderThumb boxSize={3} />
      </Slider>
      <Text color={mute} fontSize="xs" w="40px" textAlign="center">
        {formatTime(duration)}
      </Text>
    </HStack>
  );
});

export const PlayerBar = memo(function PlayerBar() {
  const { t } = useTranslation(["music"]);
  const currentSong = usePlayback((s) => s.currentSong);
  const isPlaying = usePlayback((s) => s.isPlaying);
  const volume = usePlayback((s) => s.volume);
  const isMuted = usePlayback((s) => s.isMuted);
  const playMode = usePlayback((s) => s.playMode);
  const queuePanelOpen = usePlayback((s) => s.queuePanelOpen);
  const lyricsPanelOpen = usePlayback((s) => s.lyricsPanelOpen);
  const errorMessage = usePlayback((s) => s.errorMessage);
  const store = usePlayback.getState();

  const ModeIcon = playMode === "one" ? Repeat1 : playMode === "shuffle" ? Shuffle : Repeat;
  // F-MU-17 三态：静音（警示色）/ 低音量 / 有声
  const mutedVisual = isMuted || volume === 0;
  const VolIcon = mutedVisual ? VolumeX : volume < 0.34 ? Volume1 : Volume2;

  if (!currentSong) {
    return (
      <Box flexShrink={0} mt="auto" borderTop="1px solid var(--aegis-hairline)" pt={2}>
        <HStack spacing={4} align="center" justify="center" h="56px">
          <FileMusic size={16} color={mute} />
          <Text color={mute} fontSize="sm">
            {t("music:player.idle")}
          </Text>
        </HStack>
      </Box>
    );
  }

  const coverUrl =
    isTauriRuntime() && currentSong.coverPath ? convertFileSrc(currentSong.coverPath) : "";
  // F-MU-10 副标题：艺术家，缺失时回退专辑/来源
  const subtitle = currentSong.artist || currentSong.album || t("music:player.localSource");

  return (
    <Box flexShrink={0} mt="auto" borderTop="1px solid var(--aegis-hairline)" pt={2}>
      <VStack spacing={1} align="stretch">
        <HStack spacing={3} align="center">
          {/* 左：封面 + 曲名/艺术家（封面点击进全屏播放器随 MU-6 接入） */}
          <Box
            w="48px"
            h="48px"
            borderRadius="md"
            overflow="hidden"
            flexShrink={0}
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
                fallback={<FileMusic size={20} color={mute} />}
              />
            ) : (
              <FileMusic size={20} color={mute} />
            )}
          </Box>
          <VStack spacing={0} align="start" flexShrink={0} minW={0} maxW="220px">
            <Text
              color={ink}
              fontWeight="medium"
              fontSize="sm"
              noOfLines={1}
              title={currentSong.title || currentSong.name}
            >
              {currentSong.title || currentSong.name}
            </Text>
            <Text color={mute} fontSize="xs" noOfLines={1}>
              {subtitle}
            </Text>
          </VStack>

          {/* 中：传输控制 + 循环模式 */}
          <HStack spacing={1} position="absolute" left="50%" transform="translateX(-50%)">
            <Tooltip label={t("music:player.previous")}>
              <IconButton
                aria-label="prev"
                icon={<SkipBack size={18} />}
                size="sm"
                variant="ghost"
                sx={{ color: ink, _hover: { bg: hoverBg } }}
                onClick={() => usePlayback.getState().prevTrack()}
              />
            </Tooltip>
            <Tooltip label={isPlaying ? t("music:player.pause") : t("music:player.play")}>
              <IconButton
                aria-label="play-pause"
                icon={isPlaying ? <Pause size={18} /> : <Play size={18} />}
                size="sm"
                borderRadius="full"
                bg={accent}
                color={accentOn}
                _hover={{ bg: accentStrong }}
                onClick={() => void usePlayback.getState().togglePlay()}
              />
            </Tooltip>
            <Tooltip label={t("music:player.next")}>
              <IconButton
                aria-label="next"
                icon={<SkipForward size={18} />}
                size="sm"
                variant="ghost"
                sx={{ color: ink, _hover: { bg: hoverBg } }}
                onClick={() => usePlayback.getState().nextTrack()}
              />
            </Tooltip>
            <Tooltip
              label={t(
                playMode === "one"
                  ? "music:player.modeOne"
                  : playMode === "shuffle"
                    ? "music:player.modeShuffle"
                    : "music:player.modeList",
              )}
            >
              <IconButton
                aria-label="play-mode"
                icon={<ModeIcon size={16} />}
                size="sm"
                variant="ghost"
                sx={{ color: playMode !== "list" ? accentStrong : mute, _hover: { bg: hoverBg } }}
                onClick={() => usePlayback.getState().togglePlayMode()}
              />
            </Tooltip>
          </HStack>

          <Box flex={1} />

          {/* 右：音质（禁用态，MU-4/5）+ 音量 + 队列 + 关闭 */}
          <Tooltip label={t("music:player.qualityDeferred")}>
            <span>
              <IconButton
                aria-label="quality"
                icon={<span style={{ fontSize: 10, fontWeight: 700 }}>HQ</span>}
                size="sm"
                variant="ghost"
                isDisabled
                sx={{ color: "var(--aegis-disabled)" }}
              />
            </span>
          </Tooltip>
          <Tooltip label={isMuted ? t("music:player.unmute") : t("music:player.mute")}>
            <IconButton
              aria-label="mute"
              icon={<VolIcon size={16} />}
              size="sm"
              variant="ghost"
              sx={{ color: mutedVisual ? "var(--aegis-error)" : ink, _hover: { bg: hoverBg } }}
              onClick={() => store.toggleMute()}
            />
          </Tooltip>
          {/* F-MU-17：0–100 百分比语义 */}
          <Box w="90px">
            <Slider
              aria-label="volume"
              value={Math.round(volume * 100)}
              min={0}
              max={100}
              step={1}
              size="sm"
              onChange={(v) => usePlayback.getState().setVolume(v / 100)}
            >
              <SliderTrack bg={trackBg}>
                <SliderFilledTrack bg={accent} />
              </SliderTrack>
              <SliderThumb boxSize={3} />
            </Slider>
          </Box>
          <Tooltip label={t("music:player.lyrics")} isDisabled={lyricsPanelOpen}>
            <IconButton
              aria-label="lyrics"
              icon={<AlignLeft size={18} />}
              size="sm"
              variant="ghost"
              sx={{ color: lyricsPanelOpen ? accentStrong : ink, _hover: { bg: hoverBg } }}
              onClick={() => store.setLyricsPanelOpen(true)}
            />
          </Tooltip>
          <Tooltip label={t("music:player.queue")} isDisabled={queuePanelOpen}>
            <IconButton
              aria-label="queue"
              icon={<ListMusic size={18} />}
              size="sm"
              variant="ghost"
              sx={{ color: ink, _hover: { bg: hoverBg } }}
              onClick={() => store.setQueuePanelOpen(true)}
            />
          </Tooltip>
          <Tooltip label={t("music:player.close")}>
            <IconButton
              aria-label="close-playback"
              icon={<X size={18} />}
              size="sm"
              variant="ghost"
              sx={{ color: mute, _hover: { bg: hoverBg, color: ink } }}
              onClick={() => store.closePlayback()}
            />
          </Tooltip>
        </HStack>

        <PlayerProgress />

        {/* R-13：播放失败显式上报（aria-live 播报），不表现为无声 */}
        {errorMessage && (
          <Text
            fontSize="xs"
            color="var(--aegis-error)"
            aria-live="polite"
            noOfLines={1}
            title={errorMessage}
          >
            {errorMessage}
          </Text>
        )}
      </VStack>
    </Box>
  );
});
