// MusicManager 本地曲库状态（MU-1）——数据流移植自 NexBox music-store.ts 本地切片，
// 🔴 改接 Aegis 后端命令面（importRoot/scanStart/scanCancel/libraryGet + music:scan-progress 事件），
// 曲库持久化在 Rust 侧（library.json），前端不再走 localStorage store（与 NexBox 单文件导入模型的差异点）。
// GPL-3.0 同源衍生，来源：Source/NexBox/src/stores/music-store.ts。

import { create } from "zustand";
import i18n from "@/i18n";
import {
  musicLibraryGet,
  musicScanImportRoot,
  musicScanStart,
  musicScanCancel,
  onScanProgress,
  pickFolder,
  type LocalSong,
  type ScanProgress,
} from "@/lib/tauri/music";
import { BackendUnavailableError } from "@/lib/tauri/invoke";

export type LocalPanelView = "home" | "local";

export interface StatusLine {
  tone: "info" | "success" | "warning" | "error";
  text: string;
}

interface MusicUiState {
  view: LocalPanelView;
  loading: boolean;
  /** 曲库加载失败原因（R-08：空态必须写明原因，禁假数据静默兜底） */
  error: string | null;
  songs: LocalSong[];
  roots: string[];
  scanning: boolean;
  /** 扫描中实时进度；null = 尚无事件（刚发起） */
  progress: ScanProgress | null;
  /** 扫描结果/降级警告状态行（F-MU-02/03/07） */
  status: StatusLine | null;
  importing: boolean;
  keyword: string;

  open: () => Promise<void>;
  setView: (v: LocalPanelView) => void;
  setKeyword: (k: string) => void;
  importFolder: () => Promise<void>;
  rescan: () => Promise<void>;
  cancelScan: () => Promise<void>;
  refresh: () => Promise<void>;
}

/** 扫描进度事件只订阅一次（StrictMode 双挂载防护，沿 NexBox listenersRegistered 手法） */
let eventsAttached = false;
let unlistenProgress: (() => void) | null = null;

function friendlyError(e: unknown): string {
  if (e instanceof BackendUnavailableError) {
    return i18n.t("music:error.backendUnavailable");
  }
  return e instanceof Error ? e.message : String(e);
}

/** 扫描终态 → 状态行文案（F-MU-02 完成口径 / F-MU-03 取消保留成果） */
function terminalStatus(p: ScanProgress): StatusLine | null {
  if (p.phase === "done") {
    const parts: string[] = [i18n.t("music:status.scanned", { n: p.songs })];
    if (p.failed > 0) parts.push(i18n.t("music:status.failed", { n: p.failed }));
    if (p.unreachable > 0) parts.push(i18n.t("music:status.unreachable", { n: p.unreachable }));
    return { tone: p.failed + p.unreachable > 0 ? "warning" : "success", text: parts.join("，") };
  }
  if (p.phase === "cancelled") {
    return {
      tone: "warning",
      text: i18n.t("music:status.cancelled", { processed: p.processed }),
    };
  }
  if (p.phase === "failed") {
    return { tone: "error", text: i18n.t("music:status.failedPhase") };
  }
  return null;
}

export const useMusicStore = create<MusicUiState>((set, get) => ({
  view: "home",
  loading: true,
  error: null,
  songs: [],
  roots: [],
  scanning: false,
  progress: null,
  status: null,
  importing: false,
  keyword: "",

  open: async () => {
    if (!eventsAttached) {
      eventsAttached = true;
      void onScanProgress((p) => {
        const terminal = p.phase === "done" || p.phase === "cancelled" || p.phase === "failed";
        set({
          scanning: !terminal,
          progress: p,
          status: terminal ? terminalStatus(p) : get().status,
        });
        if (terminal) {
          // 终态后回读曲库拿最终入库结果（Rust 侧才是曲库单源）
          void get().refresh();
        }
      }).then((fn) => {
        unlistenProgress = fn;
      });
    }
    await get().refresh();
  },

  refresh: async () => {
    set({ loading: true, error: null });
    try {
      const view = await musicLibraryGet();
      set({
        songs: view.library.songs,
        roots: view.library.roots,
        scanning: view.scanning,
        loading: false,
        status: view.cacheWarning
          ? { tone: "warning", text: i18n.t("music:status.cacheReset") }
          : get().status,
      });
    } catch (e) {
      set({ loading: false, error: friendlyError(e) });
    }
  },

  setView: (v) => set({ view: v }),
  setKeyword: (k) => set({ keyword: k }),

  importFolder: async () => {
    if (get().importing) return;
    set({ importing: true, error: null });
    try {
      const folder = await pickFolder();
      if (folder === null) return; // 取消选择 = 无副作用（F-MU-01）
      const result = await musicScanImportRoot(folder);
      if (!result.added) {
        set({
          status: { tone: "info", text: i18n.t("music:status.rootExists", { folder }) },
        });
        if (!result.scanStarted) await get().refresh();
      } else {
        set({
          scanning: true,
          progress: null,
          status: null,
          roots: result.roots,
          view: "local",
        });
      }
    } catch (e) {
      set({ error: friendlyError(e) });
    } finally {
      set({ importing: false });
    }
  },

  rescan: async () => {
    if (get().scanning || get().roots.length === 0) return;
    try {
      const ack = await musicScanStart();
      if (ack.started) set({ scanning: true, progress: null, status: null, view: "local" });
    } catch (e) {
      set({ error: friendlyError(e) });
    }
  },

  cancelScan: async () => {
    try {
      await musicScanCancel();
    } catch (e) {
      set({ error: friendlyError(e) });
    }
  },
}));

/** 卸载页面时释放事件订阅（模块级句柄，配合 eventsAttached 复位允许再进页面重订） */
export function detachScanEvents() {
  if (unlistenProgress) {
    unlistenProgress();
    unlistenProgress = null;
  }
  eventsAttached = false;
}
