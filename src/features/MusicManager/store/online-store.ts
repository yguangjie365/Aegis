// 在线音乐状态管理（MU-4.14）—— 从 NexBox music-store.ts 提取在线部分
// 🔴 所有 invoke 调用经 Aegis 封装层（src/lib/tauri/invoke.ts）；命令名/参数名与后端 online.rs 逐一对齐

import { create } from "zustand";
import { listen } from "@tauri-apps/api/event";
import type { Song, Playlist, LoginInfo, MusicProvider, SearchResults } from "@/types/music";
import { invoke, isTauriRuntime } from "@/lib/tauri/invoke";

interface OnlineMusicState {
  // 登录状态
  loginStatuses: Record<MusicProvider, LoginInfo | null>;
  currentPlatform: MusicProvider;
  /** 登录窗口失败事件文案（music-login-failed），成功即清空 */
  loginError: string | null;

  // 搜索结果
  searchResults: SearchResults;
  searchKeyword: string;
  searchHistory: string[];
  searching: boolean;

  // 歌单
  userPlaylists: Playlist[];
  currentPlaylist: Playlist | null;
  playlistTracks: Song[];
  loadingPlaylists: boolean;
  loadingPlaylistTracks: boolean;

  // 推荐与榜单
  dailyRecommendations: Song[];
  recommendPlaylists: Playlist[];
  rankLists: Playlist[];
  loadingRecommendations: boolean;
  loadingRankLists: boolean;

  // Actions
  loadLoginStatuses: () => Promise<void>;
  /** 打开平台官方登录窗口（页面内扫码登录，MU-4.15 移植 NexBox） */
  openLoginWindow: (platform: MusicProvider) => Promise<void>;
  /** 注册登录窗口事件监听（music-login-success / music-login-failed，幂等） */
  initLoginListeners: () => Promise<void>;
  loginWithCookie: (platform: MusicProvider, cookie: string) => Promise<void>;
  logout: (platform: MusicProvider) => Promise<void>;
  search: (keywords: string, platform?: MusicProvider) => Promise<void>;
  loadUserPlaylists: () => Promise<void>;
  loadPlaylistTracks: (id: string) => Promise<void>;
  loadDailyRecommend: () => Promise<void>;
  loadRankList: () => Promise<void>;
  loadRankSongs: (rankId: string) => Promise<void>;
  setSearchKeyword: (keyword: string) => void;
  setCurrentPlatform: (platform: MusicProvider) => void;
}

const ONLINE = "Music" as const;

const emptySearch: SearchResults = { correlationId: "", songs: [] };

/** 登录窗口事件监听只注册一次（store 级幂等） */
let loginListenersReady = false;

export const useOnlineStore = create<OnlineMusicState>((set, get) => ({
  // 初始状态
  loginStatuses: { netease: null, qq: null },
  currentPlatform: "netease",
  loginError: null,
  searchResults: emptySearch,
  searchKeyword: "",
  searchHistory: [],
  searching: false,
  userPlaylists: [],
  currentPlaylist: null,
  playlistTracks: [],
  loadingPlaylists: false,
  loadingPlaylistTracks: false,
  dailyRecommendations: [],
  recommendPlaylists: [],
  rankLists: [],
  loadingRecommendations: false,
  loadingRankLists: false,

  loadLoginStatuses: async () => {
    const platforms: MusicProvider[] = ["netease", "qq"];
    const results = await Promise.all(
      platforms.map((p) =>
        invoke<LoginInfo>("music_login_status", ONLINE, "loginStatus", { platform: p }).catch(
          () => null,
        ),
      ),
    );
    set({
      loginStatuses: {
        netease: results[0] ?? null,
        qq: results[1] ?? null,
      },
    });
  },

  openLoginWindow: async (platform) => {
    set({ loginError: null });
    await invoke("music_open_login_window", ONLINE, "openLoginWindow", { platform });
  },

  initLoginListeners: async () => {
    if (loginListenersReady || !isTauriRuntime()) return;
    loginListenersReady = true;

    await listen<LoginInfo>("music-login-success", (event) => {
      const info = event.payload;
      if (!info?.loggedIn) return;
      // provider 词表由后端单点归一（Q-20），前端直接采用线格式值
      const { provider } = info;
      const state = get();
      const currentLoggedIn = state.loginStatuses[state.currentPlatform]?.loggedIn;
      set({
        loginStatuses: { ...state.loginStatuses, [provider]: { ...info, provider } },
        // 当前平台未登录时收编到新登录的平台（NexBox 同语义）
        currentPlatform: currentLoggedIn ? state.currentPlatform : provider,
        loginError: null,
      });
      void state.loadUserPlaylists();
      void state.loadDailyRecommend();
      void state.loadRankList();
    });

    await listen<string>("music-login-failed", (event) => {
      set({ loginError: String(event.payload || "登录失败") });
    });
  },

  loginWithCookie: async (platform, cookie) => {
    try {
      const result = await invoke<LoginInfo>("music_login_cookie", ONLINE, "loginWithCookie", {
        platform,
        cookie,
      });
      set((state) => ({
        loginStatuses: { ...state.loginStatuses, [platform]: result },
      }));
    } catch (e) {
      console.error("Login failed:", e);
      throw e;
    }
  },

  logout: async (platform) => {
    try {
      await invoke("music_logout", ONLINE, "logout", { platform });
      set((state) => {
        const newStatuses = { ...state.loginStatuses };
        newStatuses[platform] = null;
        return { loginStatuses: newStatuses };
      });
    } catch (e) {
      console.error("Logout failed:", e);
      throw e;
    }
  },

  search: async (keywords, platform) => {
    if (!keywords.trim()) return;
    const targetPlatform = platform || get().currentPlatform;
    set({ searching: true, searchKeyword: keywords });

    try {
      const results = await invoke<SearchResults>("music_search", ONLINE, "search", {
        keywords,
        platform: targetPlatform,
        limit: 30,
      });
      if (get().currentPlatform !== targetPlatform) return;
      set({
        searchResults: results,
        searchHistory: [keywords, ...get().searchHistory.filter((k) => k !== keywords)].slice(
          0,
          20,
        ),
      });
    } catch (e) {
      if (get().currentPlatform !== targetPlatform) return;
      console.error("Search failed:", e);
      set({ searchResults: emptySearch });
    } finally {
      if (get().currentPlatform === targetPlatform) set({ searching: false });
    }
  },

  loadUserPlaylists: async () => {
    set({ loadingPlaylists: true });
    const platform = get().currentPlatform;
    try {
      const playlists = await invoke<Playlist[]>(
        "music_user_playlist",
        ONLINE,
        "loadUserPlaylists",
        {
          platform,
        },
      );
      if (get().currentPlatform !== platform) return;
      set({ userPlaylists: playlists });
    } catch (e) {
      if (get().currentPlatform !== platform) return;
      console.error("Load playlists failed:", e);
      set({ userPlaylists: [] });
    } finally {
      if (get().currentPlatform === platform) set({ loadingPlaylists: false });
    }
  },

  loadPlaylistTracks: async (id) => {
    set({ loadingPlaylistTracks: true });
    const platform = get().currentPlatform;
    try {
      const [playlist, tracks] = await invoke<[Playlist, Song[]]>(
        "music_playlist_tracks",
        ONLINE,
        "loadPlaylistTracks",
        { platform, playlistId: id },
      );
      if (get().currentPlatform !== platform) return;
      set({ currentPlaylist: playlist, playlistTracks: tracks });
    } catch (e) {
      if (get().currentPlatform !== platform) return;
      console.error("Load playlist tracks failed:", e);
      set({ currentPlaylist: null, playlistTracks: [] });
    } finally {
      if (get().currentPlatform === platform) set({ loadingPlaylistTracks: false });
    }
  },

  loadDailyRecommend: async () => {
    set({ loadingRecommendations: true });
    const platform = get().currentPlatform;
    try {
      if (platform === "netease") {
        // 网易云：每日推荐歌曲（当前为占位桩，返回空列表）
        const songs = await invoke<Song[]>(
          "music_recommend_songs",
          ONLINE,
          "loadDailyRecommendSongs",
          {
            platform,
          },
        );
        if (get().currentPlatform !== platform) return;
        set({ dailyRecommendations: songs, recommendPlaylists: [] });
      } else {
        // QQ：推荐歌单（无需登录）
        const playlists = await invoke<Playlist[]>(
          "music_recommend_resource",
          ONLINE,
          "loadDailyRecommendPlaylists",
          { platform },
        );
        if (get().currentPlatform !== platform) return;
        set({ dailyRecommendations: [], recommendPlaylists: playlists });
      }
    } catch (e) {
      if (get().currentPlatform !== platform) return;
      console.error("Load daily recommendations failed:", e);
      set({ dailyRecommendations: [], recommendPlaylists: [] });
    } finally {
      if (get().currentPlatform === platform) set({ loadingRecommendations: false });
    }
  },

  loadRankList: async () => {
    set({ loadingRankLists: true });
    const platform = get().currentPlatform;
    try {
      const ranks = await invoke<Playlist[]>("music_rank_list", ONLINE, "loadRankLists", {
        platform,
      });
      if (get().currentPlatform !== platform) return;
      set({ rankLists: ranks });
    } catch (e) {
      if (get().currentPlatform !== platform) return;
      console.error("Load rank lists failed:", e);
      set({ rankLists: [] });
    } finally {
      if (get().currentPlatform === platform) set({ loadingRankLists: false });
    }
  },

  loadRankSongs: async (rankId) => {
    set({ loadingPlaylistTracks: true });
    const platform = get().currentPlatform;
    try {
      const songs = await invoke<Song[]>("music_rank_songs", ONLINE, "loadRankSongs", {
        platform,
        rankId,
        limit: 30,
      });
      if (get().currentPlatform !== platform) return;
      set({ currentPlaylist: null, playlistTracks: songs });
    } catch (e) {
      if (get().currentPlatform !== platform) return;
      console.error("Load rank songs failed:", e);
      set({ playlistTracks: [] });
    } finally {
      if (get().currentPlatform === platform) set({ loadingPlaylistTracks: false });
    }
  },

  setSearchKeyword: (keyword) => set({ searchKeyword: keyword }),

  setCurrentPlatform: (platform) =>
    set({
      currentPlatform: platform,
      // 切平台同时清掉上一平台的歌单详情：否则下方"歌单"视图仍驻留旧平台内容
      currentPlaylist: null,
      playlistTracks: [],
    }),
}));
