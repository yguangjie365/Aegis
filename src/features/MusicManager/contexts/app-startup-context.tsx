/**
 * app-startup-context —— NexBox 应用启动上下文 · 🎯 阶段一最小桩（2026-10-03 用户裁决）
 *
 * 🔴 复刻基线（`Docs/功能规格/07-音乐管理.md` §〇，v1.4）：音乐管理模块是 NexBox 的换皮版本。
 * 本文件**不是** NexBox 原件，而是其 1007 行 `contexts/app-startup-context.tsx` 的**接口等价桩**。
 *
 * 为什么写桩而不搬原件：
 *   ① NexBox 的 app-startup-context 是**全局壳层**，负责开机自启、硬件自检、更新检查、
 *      全局热键注册、首次启动引导等横切能力，与音乐模块无关；
 *   ② 整体搬入会把 NexBox 的硬件检测（`lib/hardware`）、开机自启、全局热键系统
 *      （`hotkey.rs` → 游戏域依赖链）一并拖入 —— 其中**全局热键已被用户明确裁决不做**；
 *   ③ 用户 2026-10-03 裁决：「基建也照搬，**壳层写最小桩**」。
 *
 * 接口面（已实测穷举音乐侧的用法）：`MusicPage.tsx:962` 是**唯一**调用点，仅解构 3 个字段：
 * ```tsx
 * const { musicPrevHotkey, musicNextHotkey, musicPlayPauseHotkey } = useAppStartup();
 * ```
 * 用于在播放器上显示"快捷键提示"文案。因此桩只需提供这三个热键字符串，
 * 取值**逐字沿用 NexBox 的默认常量**（`app-startup-context.tsx:21-23`），保证界面文案一致。
 *
 * 其余字段（isFirstLaunch / hardwareReport / 各类自检结果等）音乐侧未使用，故不在桩中提供；
 * 若阶段二或后续功能需要，须移植 NexBox 原组件而非扩张本桩。
 */
import { createContext, useContext, useMemo, type ReactNode } from "react";

/** NexBox `app-startup-context.tsx:21-23` 的音乐热键默认值，逐字沿用。 */
const DEFAULT_MUSIC_PREV_HOTKEY = "Alt+[";
const DEFAULT_MUSIC_NEXT_HOTKEY = "Alt+]";
const DEFAULT_MUSIC_PLAYPAUSE_HOTKEY = "Alt+Space";

export interface AppStartupStub {
  musicPrevHotkey: string;
  musicNextHotkey: string;
  musicPlayPauseHotkey: string;
  /** 桌面歌词按钮热键（DesktopLyricsSettingsModal 用；NexBox 默认空串 = 未绑定） */
  lyricsBtnHotkey: string;
  /** 保存桌面歌词热键（NexBox 默认 no-op；Aegis 桩不落盘设置，返回 null 表示未改动） */
  saveLyricsBtnHotkey: (shortcut: string) => Promise<string | null>;
}

const AppStartupContext = createContext<AppStartupStub>({
  musicPrevHotkey: DEFAULT_MUSIC_PREV_HOTKEY,
  musicNextHotkey: DEFAULT_MUSIC_NEXT_HOTKEY,
  musicPlayPauseHotkey: DEFAULT_MUSIC_PLAYPAUSE_HOTKEY,
  lyricsBtnHotkey: "",
  saveLyricsBtnHotkey: async () => null,
});

/** 与 NexBox `useAppStartup` 同名同义；本桩返回固定默认值，不含任何启动副作用。 */
export function useAppStartup(): AppStartupStub {
  return useContext(AppStartupContext);
}

export function AppStartupProvider({ children }: { children: ReactNode }) {
  const value = useMemo<AppStartupStub>(
    () => ({
      musicPrevHotkey: DEFAULT_MUSIC_PREV_HOTKEY,
      musicNextHotkey: DEFAULT_MUSIC_NEXT_HOTKEY,
      musicPlayPauseHotkey: DEFAULT_MUSIC_PLAYPAUSE_HOTKEY,
      lyricsBtnHotkey: "",
      saveLyricsBtnHotkey: async () => null,
    }),
    [],
  );
  return <AppStartupContext.Provider value={value}>{children}</AppStartupContext.Provider>;
}

export default AppStartupProvider;
