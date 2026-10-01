// MU-3 歌词状态（F-MU-19）——装载管线切片，与 playback-store 旁挂联动（起播/切歌触发）。
// 移植口径：状态形状取 NexBox music-store 的 currentLyrics/loadingLyrics（GPL-3.0 同源衍生），
// 装载路径按规格 07 §3.8 重写：本地来源经 music_lyric_read（同名外挂 → 内嵌 → 无，Rust 侧裁决），
// 在线取词随 MU-4/5 接入。适配差异：① 解析产物直接为 KaraokeLine[]（不经 Lyrics 中间态回灌）；
// ② 整篇无时间戳 → 纯文本整段显示（F-MU-19，NexBox 无此级）；③ 装载失败/无词给明确原因文案，
// 不出现空白无解释；④ 切歌瞬间不立即清空旧词，新词到达才替换（防闪烁，NexBox 同策略）。

import { create } from "zustand";
import { isTauriRuntime } from "@/lib/tauri/invoke";
import { musicLyricRead, type LocalSong } from "@/lib/tauri/music";
import { log, newCorrelationId } from "@/lib/logging";
import { buildLines, type KaraokeLine } from "../lib/karaoke-lyrics";

export interface LyricsState {
  /** 带时间轴歌词行；null = 非卡拉OK态（纯文本/无词/未装载） */
  lines: KaraokeLine[] | null;
  /** F-MU-19：无时间戳时的整段纯文本 */
  plainText: string | null;
  /** 装载中（首屏/切歌期间旧内容保留展示，不闪空） */
  loading: boolean;
  /** 装载终态提示：null = 正常有词；否则为 i18n 键（music:lyrics.*） */
  noticeKey: string | null;
  /** 已装载歌词对应的曲目 id（展示层判断"当前曲尚无词"） */
  songId: string | null;

  loadForSong: (song: LocalSong) => Promise<void>;
  /** 扫描期已知两级来源均无词：免 IPC 直接给"均无"提示（F-MU-19） */
  setNoLyric: (songId: string) => void;
  clearLyrics: () => void;
}

// 起播代际号：切歌后迟到的旧装载结果一律丢弃（与 playback R-18 同构）
let loadSeq = 0;

export const useLyrics = create<LyricsState>((set) => ({
  lines: null,
  plainText: null,
  loading: false,
  noticeKey: null,
  songId: null,

  loadForSong: async (song) => {
    const cid = newCorrelationId();
    const mySeq = ++loadSeq;
    if (!isTauriRuntime()) {
      set({ lines: null, plainText: null, loading: false, noticeKey: "backendUnavailable", songId: song.id });
      return;
    }
    // 不立即清空 lines：新词到达前保留旧内容渲染，避免切歌瞬间整块闪白
    set({ loading: true });
    let noticeKey: string | null = null;
    let raw = "";
    try {
      const result = await musicLyricRead(song.path);
      raw = result.text;
    } catch (e) {
      log({
        level: "error",
        module: "Music",
        action: "loadLyrics",
        result: "Failed",
        exception: String(e),
        correlationId: cid,
      });
      noticeKey = "loadFailed";
    }
    if (mySeq !== loadSeq) return; // 已有更新的切歌装载，丢弃本次迟到结果
    if (noticeKey) {
      set({ lines: null, plainText: null, loading: false, noticeKey, songId: song.id });
      return;
    }
    if (!raw.trim()) {
      // F-MU-19 验收口径：明确说明两级来源均无（同名外挂文本与内嵌歌词）
      set({ lines: null, plainText: null, loading: false, noticeKey: "notFound", songId: song.id });
      return;
    }
    const lines = buildLines({ lyric: raw });
    if (lines) {
      set({ lines, plainText: null, loading: false, noticeKey: null, songId: song.id });
    } else {
      set({ lines: null, plainText: raw, loading: false, noticeKey: null, songId: song.id });
    }
  },

  setNoLyric: (songId) => {
    loadSeq++; // 作废飞行中的装载
    set({ lines: null, plainText: null, loading: false, noticeKey: "notFound", songId });
  },

  clearLyrics: () => {
    loadSeq++; // 作废飞行中的装载
    set({ lines: null, plainText: null, loading: false, noticeKey: null, songId: null });
  },
}));
