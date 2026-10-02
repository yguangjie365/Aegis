// 在线音乐类型定义（MU-4）—— 🔴 与后端 services::music::models / online.rs 的 serde camelCase 线格式逐字段对齐，
// 改后端结构体字段时必须同步本文件（契约单源在 Rust 侧，这里只是消费镜像）。

export type MusicProvider = "netease" | "qq";

// 🔴 provider 词表唯一归一在 Rust 侧（`services::music::auth::MusicPlatform::from_slug` / `slug`）：
// 命令层入站接受别名、出站一律规范值。本类型是线格式的镜像，前端不再做别名归一（Q-20）。

export interface ArtistInfo {
  id?: string;
  mid?: string;
  name: string;
  picUrl?: string;
  musicSize?: number;
}

export interface Song {
  provider: MusicProvider;
  id: string;
  mid?: string;
  mediaMid?: string;
  name: string;
  artist: string;
  artists: ArtistInfo[];
  album: string;
  cover: string;
  duration: number; // 毫秒
  fee: number; // 0=免费, 1=VIP, 2=数字专辑, 4=付费单曲
  playable: boolean;
  language?: number;
  hash?: string;
  albumId?: string;
  albumAudioId?: string;
  hqHash?: string;
  sqHash?: string;
  resHash?: string;
  qqSongId?: number;
  contentId?: string;
}

export interface Playlist {
  provider: MusicProvider;
  id: string;
  name: string;
  cover: string;
  trackCount: number;
  creator: string;
  subscribed?: boolean;
}

export interface LoginInfo {
  provider: MusicProvider;
  loggedIn: boolean;
  userId: string;
  nickname: string;
  avatar: string;
  vipType: number;
  vipLevel: string;
  isVip: boolean;
  isSvip: boolean;
}

// music_search 返回 SearchResult { correlationId, songs }
export interface SearchResults {
  correlationId: string;
  songs: Song[];
}

// music_song_url 返回 SongUrlResultWrapper
export interface SongUrlResult {
  correlationId: string;
  url?: string;
  quality?: string;
  trial: boolean;
  reason?: string;
}
