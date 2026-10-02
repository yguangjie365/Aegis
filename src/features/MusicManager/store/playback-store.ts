// MU-2 播放与队列 store —— 内核移植自 NexBox `stores/music-store.ts` 播放切片（GPL-3.0 同源衍生）。
// 适配差异（规格 07 v1.2 口径）：
// ① 数据形状改 Aegis LocalSong（曲库单源在 Rust 侧，播放全部在前端完成，无后端命令面）；
// ② 剔除 heartbeat 心动模式/汽水续播/桌面歌词 emit/SMTC 推送/在线取址重试（分属 MU-4~7 与排除特性）；
// ③ 循环模式定档三种（F-MU-12：列表→随机→单曲）；淡出 0.2s/淡入 0.25s（F-MU-18/R-15，NexBox 350ms 定档改规格值）；
// ④ 移除当前曲不打断播放（R-11，修正 NexBox「移除当前曲即续播邻曲」的偏离）；清空队列同时清空历史（R-11）；
// ⑤ 持久化走 localStorage 兜底（与 shell/store.ts 同口径，settings 服务 W0-08/W1-11 就绪后迁通道）。

import { create } from "zustand";
import { convertFileSrc } from "@tauri-apps/api/core";
import { isTauriRuntime } from "@/lib/tauri/invoke";
import { log, newCorrelationId } from "@/lib/logging";
import type { LocalSong } from "@/lib/tauri/music";
import { useLyrics } from "./lyrics-store";

export type PlayMode = "list" | "shuffle" | "one";

// 队列/历史上限（NexBox 同源常量）
const MAX_PLAY_QUEUE = 2000;
const MAX_PLAY_HISTORY = 100;
// F-MU-18 / R-15：淡出约 0.2 秒、淡入约 0.25 秒
const FADE_OUT_MS = 200;
const FADE_IN_MS = 250;
const FADE_TICK_MS = 16;

const LS_VOLUME = "aegis.music.volume";
const LS_MODE = "aegis.music.playMode";

function loadVolume(): number {
  const raw = Number.parseFloat(localStorage.getItem(LS_VOLUME) ?? "");
  return Number.isFinite(raw) && raw >= 0 && raw <= 1 ? raw : 0.7;
}
function loadMode(): PlayMode {
  const raw = localStorage.getItem(LS_MODE);
  return raw === "shuffle" || raw === "one" ? raw : "list";
}

// ── 音量缓入缓出（NexBox music-store.ts:807-845 移植；渐变时长改 F-MU-18 定档值）──
// 进度按已流逝时间计算：定时器被后台节流时只会跳变到终点，不会卡在半途
let volumeFadeTimer: ReturnType<typeof setInterval> | null = null;
let volumeFadeSeq = 0;
// 当前渐变是否为「渐出后暂停」：渐出中途调音量时需立即补上暂停，避免"已暂停"状态下仍出声
let volumeFadeIsPause = false;

function cancelVolumeFade() {
  volumeFadeSeq++;
  if (volumeFadeTimer) {
    clearInterval(volumeFadeTimer);
    volumeFadeTimer = null;
  }
}

function startVolumeFade(
  audio: HTMLAudioElement,
  to: number,
  durationMs: number,
  onDone?: () => void,
) {
  cancelVolumeFade();
  const seq = ++volumeFadeSeq;
  const from = audio.volume;
  const start = performance.now();
  volumeFadeTimer = setInterval(() => {
    if (seq !== volumeFadeSeq) {
      if (volumeFadeTimer) clearInterval(volumeFadeTimer);
      volumeFadeTimer = null;
      return;
    }
    const t = Math.min(1, (performance.now() - start) / durationMs);
    const eased = t * t * (3 - 2 * t);
    audio.volume = from + (to - from) * eased;
    if (t >= 1) {
      if (volumeFadeTimer) clearInterval(volumeFadeTimer);
      volumeFadeTimer = null;
      onDone?.();
    }
  }, FADE_TICK_MS);
}

// R-18 代际号：起播动作各自递增，迟到结果（含异常分支）代际不符一律丢弃
let playSongSeq = 0;
// R-12「开播上报」：仅真正开播成功的曲目才算历史
let lastPlayedSong: LocalSong | null = null;

interface PlaybackState {
  currentSong: LocalSong | null;
  playQueue: LocalSong[];
  currentIndex: number;
  isPlaying: boolean;
  duration: number;
  playMode: PlayMode;
  volume: number;
  isMuted: boolean;
  playHistory: LocalSong[];
  queuePanelOpen: boolean;
  lyricsPanelOpen: boolean;
  /** R-13：播放失败必须显式上报，不得表现为无声 */
  errorMessage: string | null;

  playSong: (
    song: LocalSong,
    queue?: LocalSong[],
    opts?: { fromHistory?: boolean; startTime?: number },
  ) => void;
  togglePlay: () => Promise<void>;
  closePlayback: () => void;
  nextTrack: () => void;
  prevTrack: () => void;
  seekTo: (time: number) => void;
  setVolume: (v: number) => void;
  toggleMute: () => void;
  togglePlayMode: () => void;
  appendToQueue: (songs: LocalSong[]) => number;
  removeFromQueue: (index: number) => void;
  clearQueue: () => void;
  setQueuePanelOpen: (open: boolean) => void;
  setLyricsPanelOpen: (open: boolean) => void;
}

// ── 音频元素单例（离开路由不中断播放：挂在模块级，与 NexBox「Audio 留在 store」同构）──
let audioEl: HTMLAudioElement | null = null;

function ensureAudio(): HTMLAudioElement | null {
  if (audioEl || typeof document === "undefined") return audioEl;
  const audio = document.createElement("audio");
  // K-S03：preload=metadata 只取元数据，避免整首预缓冲
  audio.preload = "metadata";
  audio.style.display = "none";
  audio.volume = usePlayback.getState().volume;
  document.body.appendChild(audio);
  audioEl = audio;

  audio.addEventListener("loadedmetadata", () => {
    if (Number.isFinite(audio.duration)) {
      usePlayback.setState({ duration: audio.duration });
    }
  });
  // R-13：自动连播只认自然结束
  audio.addEventListener("ended", () => {
    usePlayback.getState().nextTrack();
  });
  // 同一首歌的自愈只允许一次：坏文件会 error → 重设 src → error 死循环
  let recovering = false;
  let recoveredSongId = "";
  audio.addEventListener("error", () => {
    const st = usePlayback.getState();
    if (recovering || !st.currentSong || !st.isPlaying) return;
    const cid = newCorrelationId();
    if (recoveredSongId === st.currentSong.id) {
      log({
        level: "error",
        module: "Music",
        action: "audio:error",
        result: "Failed",
        exception: `曲 ${st.currentSong.name} 解码/读取失败`,
        correlationId: cid,
      });
      usePlayback.setState({
        isPlaying: false,
        errorMessage: "无法播放该本地音频文件，请检查格式是否受支持",
      });
      return;
    }
    recoveredSongId = st.currentSong.id;
    recovering = true;
    const savedTime = audio.currentTime;
    log({
      level: "warn",
      module: "Music",
      action: "audio:error",
      result: "Success",
      exception: "播放出错，自愈重试一次",
      correlationId: cid,
    });
    void usePlayback.getState().playSong(st.currentSong, undefined, {
      startTime: savedTime,
    });
    recovering = false;
  });
  // 真正播起来了就放开自愈额度
  audio.addEventListener("playing", () => {
    recoveredSongId = "";
  });
  return audioEl;
}

/** 供进度条组件挂 timeupdate（NexBox「local state 监听、不触发 store 更新」同源策略） */
export function getAudioElement(): HTMLAudioElement | null {
  return audioEl;
}

function persist(key: string, value: string) {
  try {
    localStorage.setItem(key, value);
  } catch (e) {
    log({
      level: "warn",
      module: "Music",
      action: "persistSetting",
      result: "Failed",
      exception: String(e),
      correlationId: newCorrelationId(),
    });
  }
}

export const usePlayback = create<PlaybackState>((set, get) => ({
  currentSong: null,
  playQueue: [],
  currentIndex: -1,
  isPlaying: false,
  duration: 0,
  playMode: loadMode(),
  volume: loadVolume(),
  isMuted: false,
  playHistory: [],
  queuePanelOpen: false,
  lyricsPanelOpen: false,
  errorMessage: null,

  playSong: (song, queue, opts) => {
    const cid = newCorrelationId();
    if (!isTauriRuntime()) {
      // 诚实降级：本地音频直读依赖 asset 协议，浏览器预览模式无播放能力（禁假数据）
      log({
        level: "warn",
        module: "Music",
        action: "playSong",
        result: "Rejected",
        exception: "浏览器模式无桌面运行时，播放能力不可用",
        correlationId: cid,
      });
      set({ errorMessage: "后端未接入：播放需要桌面运行时" });
      return;
    }
    const audio = ensureAudio();
    if (!audio) return;

    // 立即作废飞行中的旧起播（R-18），并取消旧渐变定时器防切歌后被拉低音量/误暂停
    const mySeq = ++playSongSeq;
    cancelVolumeFade();
    volumeFadeIsPause = false;

    // R-9：传入 queue = 整体替换并定位当前曲；不传 = 在既有队列内切换当前曲（点选不重建）
    if (queue) {
      const idx = queue.findIndex((s) => s.id === song.id);
      set({ playQueue: queue, currentIndex: idx >= 0 ? idx : 0 });
    } else {
      const idx = get().playQueue.findIndex((s) => s.id === song.id);
      if (idx >= 0) set({ currentIndex: idx });
    }
    set({
      currentSong: song,
      duration: song.durationMs / 1000,
      isPlaying: false,
      errorMessage: null,
    });
    // F-MU-19：起播/切歌自动装载歌词（后台异步，不卡界面；空文件省一次 IPC 直接给"均无"提示）
    if (song.hasLyric) void useLyrics.getState().loadForSong(song);
    else useLyrics.getState().setNoLyric(song.id);

    const effVolume = get().isMuted ? 0 : get().volume;
    const begin = () => {
      if (mySeq !== playSongSeq) return;
      audio.src = convertFileSrc(song.path);
      // 自愈重试带 savedTime：回到出错位置续播，其余起播一律从头
      audio.currentTime = opts?.startTime ?? 0;
      // F-MU-18：新曲从静音淡入到目标音量，避免首帧硬响
      audio.volume = 0;
      void audio
        .play()
        .then(() => {
          if (mySeq !== playSongSeq) return;
          startVolumeFade(audio, effVolume, FADE_IN_MS);
          set({ isPlaying: true });
          recordHistory(song, !!opts?.fromHistory);
        })
        .catch((e: unknown) => {
          if (mySeq !== playSongSeq) return;
          audio.volume = effVolume;
          log({
            level: "error",
            module: "Music",
            action: "playSong",
            result: "Failed",
            exception: String(e),
            correlationId: cid,
          });
          set({ isPlaying: false, errorMessage: "无法播放该本地音频文件，请检查格式是否受支持" });
        });
    };

    // F-MU-18：旧曲仍在响 → 先淡出再停，然后起新曲；已停则直接起
    if (get().isPlaying && !audio.paused) {
      startVolumeFade(audio, 0, FADE_OUT_MS, begin);
    } else {
      audio.pause();
      begin();
    }
  },

  togglePlay: async () => {
    const audio = ensureAudio();
    const { isPlaying, currentSong, volume, isMuted } = get();
    if (!audio || !currentSong) return;
    const effVolume = isMuted ? 0 : volume;
    if (isPlaying) {
      // R-15：暂停也是平滑渐出；渐出期间再点播放会取消渐变从当前音量继续渐入
      volumeFadeIsPause = true;
      startVolumeFade(audio, 0, FADE_OUT_MS, () => {
        audio.pause();
        volumeFadeIsPause = false;
        // 恢复音量，避免未走渐入的路径（如单曲循环重播）静音
        audio.volume = usePlayback.getState().isMuted ? 0 : usePlayback.getState().volume;
      });
      set({ isPlaying: false });
    } else {
      if (audio.paused) audio.volume = 0;
      volumeFadeIsPause = false;
      try {
        await audio.play();
        if (audio.paused) return;
        startVolumeFade(audio, effVolume, FADE_IN_MS);
        set({ isPlaying: true });
      } catch (e) {
        cancelVolumeFade();
        audio.volume = effVolume;
        log({
          level: "error",
          module: "Music",
          action: "togglePlay",
          result: "Failed",
          exception: String(e),
          correlationId: newCorrelationId(),
        });
        set({ isPlaying: false, errorMessage: "播放失败：无法启动音频输出" });
      }
    }
  },

  closePlayback: () => {
    cancelVolumeFade();
    volumeFadeIsPause = false;
    lastPlayedSong = null;
    useLyrics.getState().clearLyrics();
    if (audioEl) {
      audioEl.pause();
      audioEl.src = "";
      audioEl.currentTime = 0;
      audioEl.volume = get().volume;
    }
    set({
      currentSong: null,
      isPlaying: false,
      duration: 0,
      playQueue: [],
      currentIndex: -1,
      playHistory: [],
      errorMessage: null,
    });
  },

  nextTrack: () => {
    const { playQueue, currentIndex, playMode } = get();
    if (playQueue.length === 0) return;

    // F-MU-12：单曲循环不推进队列，原地重播
    if (playMode === "one") {
      if (audioEl) {
        audioEl.currentTime = 0;
        void audioEl.play().catch(() => {});
      }
      return;
    }

    let next: number;
    if (playMode === "shuffle") {
      // F-MU-12：随机尽量不连续重复当前曲（队列多于一首时排除当前）
      if (playQueue.length > 1 && currentIndex >= 0) {
        next = Math.floor(Math.random() * (playQueue.length - 1));
        if (next >= currentIndex) next += 1;
      } else {
        next = Math.floor(Math.random() * playQueue.length);
      }
    } else {
      next = currentIndex + 1;
      if (next >= playQueue.length) next = 0;
    }
    const song = playQueue[next];
    if (song) void get().playSong(song);
  },

  // R-12：优先回退「最近真正开播过」的曲目；历史空才按队列索引前移循环
  prevTrack: () => {
    const history = get().playHistory;
    if (history.length > 0) {
      const prevSong = history[history.length - 1];
      if (prevSong) {
        set({ playHistory: history.slice(0, -1) });
        void get().playSong(prevSong, undefined, { fromHistory: true });
        return;
      }
    }
    const { playQueue, currentIndex } = get();
    if (playQueue.length === 0) return;
    let prev = currentIndex - 1;
    if (prev < 0) prev = playQueue.length - 1;
    const song = playQueue[prev];
    if (song) void get().playSong(song);
  },

  // R-14：松手定位——钳制在曲目时长边界内；引擎侧由渐让策略保证不回拽进度条
  seekTo: (time) => {
    const audio = ensureAudio();
    const { duration, isPlaying } = get();
    if (!audio) return;
    if (!(duration > 0)) {
      set({ errorMessage: "当前曲目尚未开始播放或时长未知，无法定位进度" });
      return;
    }
    const target = Math.min(Math.max(time, 0), duration);
    audio.currentTime = target;
    // 在线音源 seek 后可能自动暂停的教训同样适用于本地重缓冲：播放态下确保续播
    if (isPlaying) audio.play().catch(() => {});
  },

  setVolume: (v) => {
    const clamped = Math.min(Math.max(v, 0), 1);
    // R-15：调大音量视为取消静音；静音只改输出不改设定值
    const nextMuted = clamped > 0 ? false : get().isMuted;
    const audio = audioEl;
    if (audio) {
      if (volumeFadeIsPause && !audio.paused) {
        // 渐出暂停中途调音量：视为放弃渐出，立即暂停并应用新音量
        cancelVolumeFade();
        audio.pause();
        volumeFadeIsPause = false;
      } else {
        cancelVolumeFade();
      }
      audio.volume = nextMuted ? 0 : clamped;
    }
    set({ volume: clamped, isMuted: nextMuted });
    persist(LS_VOLUME, String(clamped));
  },

  // R-15：静音只改输出不改设定值
  toggleMute: () => {
    const { isMuted, volume } = get();
    const next = !isMuted;
    if (audioEl) audioEl.volume = next ? 0 : volume;
    set({ isMuted: next });
  },

  // F-MU-12：列表循环 → 随机播放 → 单曲循环 依次切换
  togglePlayMode: () => {
    const modes: PlayMode[] = ["list", "shuffle", "one"];
    const current = modes.indexOf(get().playMode);
    const next = modes[(current + 1) % modes.length] ?? "list";
    set({ playMode: next });
    persist(LS_MODE, next);
  },

  // R-10：同一曲目重复加入只算一次；上限防内存无限增长
  appendToQueue: (songs) => {
    if (songs.length === 0) return 0;
    const { playQueue } = get();
    const remaining = MAX_PLAY_QUEUE - playQueue.length;
    if (remaining <= 0) return 0;
    const queueIds = new Set(playQueue.map((q) => q.id));
    const unique = songs.filter((s) => !queueIds.has(s.id));
    if (unique.length === 0) return 0;
    const toAdd = unique.slice(0, remaining);
    set({ playQueue: [...playQueue, ...toAdd] });
    return toAdd.length;
  },

  // R-11：移除当前曲不打断播放——正在播的曲目引用继续有效，仅队列显示变化
  removeFromQueue: (index) => {
    const { playQueue, currentIndex } = get();
    if (index < 0 || index >= playQueue.length) return;
    const newQueue = playQueue.filter((_, i) => i !== index);
    if (index < currentIndex) {
      set({ playQueue: newQueue, currentIndex: currentIndex - 1 });
      return;
    }
    if (index > currentIndex) {
      set({ playQueue: newQueue });
      return;
    }
    // 移除的是当前曲：保持播放，队列显示变化、当前指针悬空（-1 = 不在队内）
    set({
      playQueue: newQueue,
      currentIndex: newQueue.length > 0 && currentIndex < newQueue.length ? currentIndex : -1,
    });
  },

  // R-11：清空队列同时清空播放历史；当前曲不受打断（继续播完）
  clearQueue: () => {
    set({ playQueue: [], currentIndex: -1, playHistory: [] });
  },

  setQueuePanelOpen: (open) => set({ queuePanelOpen: open }),

  setLyricsPanelOpen: (open) => set({ lyricsPanelOpen: open }),
}));

// R-12：开播上报历史（仅新起播算历史，历史回退取出的不再重复入栈）
function recordHistory(song: LocalSong, fromHistory: boolean) {
  if (!fromHistory) {
    const prev = lastPlayedSong;
    if (prev && prev.id !== song.id) {
      usePlayback.setState((st) => ({
        playHistory: [...st.playHistory, prev].slice(-MAX_PLAY_HISTORY),
      }));
    }
  }
  lastPlayedSong = song;
}
