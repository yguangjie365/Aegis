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

/** 订阅扫描进度事件（music:scan-progress）；返回解订函数。浏览器模式静默不订阅。 */
export function onScanProgress(handler: (p: ScanProgress) => void): Promise<UnlistenFn> {
  if (!isTauriRuntime()) {
    return Promise.resolve(() => {});
  }
  return listen<ScanProgress>("music:scan-progress", (e) => handler(e.payload));
}
