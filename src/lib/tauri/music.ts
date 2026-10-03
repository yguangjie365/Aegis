// music 域前端门面（MU-1）：曲库命令 + 扫描进度事件 + 目录选择器包装。
// 🔴 业务代码不得裸调 tauriInvoke/listen/open —— 一律经本文件（01 专题「前端不触引擎」白名单侧）。
// ⚠️ 以下 interface 是 Rust 生成物的临时手写形状（沿 invoke.ts CmdEnvelope 先例）；
// W0-07 ts-rs 生成物落地后由 src/types/ 生成类型替换，禁止继续扩张手写镜像。

import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { open } from "@tauri-apps/plugin-dialog";
import { invoke, isTauriRuntime } from "./invoke";

export interface LocalSong {
  id: string;
  name: string;
  path: string;
  size: number;
  extension: string;
  title: string;
  artist: string;
  album: string;
  albumArtist: string;
  genre: string;
  track: number;
  year: number;
  durationMs: number;
  bitrate: number;
  sampleRate: number;
  channels: number;
  modifiedAt: number;
  coverPath: string;
  coverSource: string;
  hasEmbeddedCover: boolean;
  hasEmbeddedLyric: boolean;
  hasLyric: boolean;
}

export interface Library {
  roots: string[];
  songs: LocalSong[];
}

export interface ScanProgress {
  phase: "collecting" | "parsing" | "done" | "cancelled" | "failed";
  processed: number;
  total: number;
  percent: number;
  songs: number;
  failed: number;
  unreachable: number;
}

export interface ImportRootResult {
  correlationId: string;
  added: boolean;
  roots: string[];
  scanStarted: boolean;
}

export interface LibraryView {
  correlationId: string;
  library: Library;
  scanning: boolean;
  cacheWarning: boolean;
}

export interface ScanAck {
  correlationId: string;
  started: boolean;
}

const MUSIC = "Music" as const;

/** 目录选择器（原生对话框）；用户取消返回 null。浏览器模式下返回 null（不抛错，选择是纯宿主动作）。 */
export async function pickFolder(): Promise<string | null> {
  if (!isTauriRuntime()) return null;
  const picked = await open({ directory: true, multiple: false });
  return typeof picked === "string" ? picked : null;
}

export function musicScanImportRoot(path: string): Promise<ImportRootResult> {
  return invoke<ImportRootResult>("music_scan_import_root", MUSIC, "musicScanImportRoot", { path });
}

export function musicScanStart(): Promise<ScanAck> {
  return invoke<ScanAck>("music_scan_start", MUSIC, "musicScanStart", null);
}

export function musicScanCancel(): Promise<ScanAck> {
  return invoke<ScanAck>("music_scan_cancel", MUSIC, "musicScanCancel", null);
}

export function musicLibraryGet(): Promise<LibraryView> {
  return invoke<LibraryView>("music_library_get", MUSIC, "musicLibraryGet", null);
}

export function musicLyricRead(path: string): Promise<{ correlationId: string; text: string }> {
  return invoke("music_lyric_read", MUSIC, "musicLyricRead", { path });
}

/**
 * 在线歌词返回（`services::music::models::Lyrics` 的线格式镜像，serde camelCase）。
 * `yrc` 为网易云逐字歌词（`KaraokeLine.words` 的数据源），`translation`/`roma` 为可选项。
 */
export interface OnlineLyrics {
  lyric: string;
  translation?: string;
  roma?: string;
  yrc?: string;
}

/**
 * 在线歌词（`music_lyric_online`）。QQ 音乐侧后端以 **mid** 作入参
 * （见 `services/src/music/online.rs` 的 `music_lyric_online`：`qqmusic::lyric(&song_id, "0", ..)`），
 * 故 QQ 传 mid、网易云传 id——调用方口径见 `features/MusicManager/lib/online-compat.ts`。
 */
export function musicLyricOnline(songId: string, platform: string): Promise<OnlineLyrics> {
  return invoke("music_lyric_online", MUSIC, "musicLyricOnline", { songId, platform });
}

/**
 * 本地代理端口（music_get_proxy_port）。0 = 不可用：浏览器预览模式或后端未起
 * （图片退回直链，非静默兜底——端口缺席时 coverProxyUrl 原样透传）。
 */
export async function musicProxyPort(): Promise<number> {
  if (!isTauriRuntime()) return 0;
  try {
    return await invoke<number>("music_get_proxy_port", MUSIC, "musicProxyPort");
  } catch (e) {
    console.error("Get proxy port failed:", e);
    return 0;
  }
}

/**
 * 远程图片 → 本地代理 URL（照搬 NexBox `stores/music-store.ts` 的 `coverProxyUrl`，GPL-3.0 同源衍生）。
 * 用途：头像与封面——图床有防盗链/跨源限制，后端 `/cover` 路由补 Referer 与 CORS 头（限额 8MB）。
 */
export function coverProxyUrl(url: string, proxyPort: number): string {
  if (!url) return "";
  if (url.startsWith("data:") || url.startsWith("blob:")) return url;
  // asset 协议 URL（convertFileSrc 生成的本地文件直链）仅在 WebView 内可访问，
  // 无法被后端代理解析，原样返回即可
  if (url.startsWith("http://asset.localhost/") || url.startsWith("https://asset.localhost/"))
    return url;
  if (!proxyPort) return url;
  return `http://127.0.0.1:${proxyPort}/cover?url=${encodeURIComponent(url)}`;
}

/** 订阅扫描进度事件（music:scan-progress）；返回解订函数。浏览器模式静默不订阅。 */
export function onScanProgress(handler: (p: ScanProgress) => void): Promise<UnlistenFn> {
  if (!isTauriRuntime()) {
    return Promise.resolve(() => {});
  }
  return listen<ScanProgress>("music:scan-progress", (e) => handler(e.payload));
}
