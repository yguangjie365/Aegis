import { create } from "zustand";

// 外壳客户端状态（Zustand，总览 §二）。
//
// 🟡 2026-10-04 用户裁决「导航栏仅保留折叠态」：折叠/展开双态机制整体下线，
// 原 `navFolded` / `toggleNavFolded`（含 localStorage 兜底 FOLD_KEY）一并删除。
// 导航栏宽度固定 64px，无用户可切换的状态，也就不再需要持久化。
// 旧 profile 里的 `aegis.shell.nav-folded` 键会自然失效（读取方已删），不主动清理 ——
// 留着无害，删它反而引入一次性迁移代码。
interface ShellState {
  /** 占位：壳层状态目前全空，保留 store 形状以便后续接入（如导航位置偏好）。 */
  readonly _reserved?: never;
}

export const useShellStore = create<ShellState>(() => ({}));
