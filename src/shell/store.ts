import { create } from "zustand";

// 外壳客户端状态（Zustand，总览 §二）。折叠选择属慢变量：
// settings 服务落地（W0-08/W1-11 之后的持久化通道就绪）前先记 localStorage 兜预览体验，
// 届时迁移并删除本兜底——迁移点已在此登记，不留暗账。
const FOLD_KEY = "aegis.shell.nav-folded";

function readFolded(): boolean {
  // 无记录时默认折叠态（2026-10-02 用户裁定）；用户显式选择过则尊重其选择
  const saved = localStorage.getItem(FOLD_KEY);
  return saved === null ? true : saved === "1";
}

interface ShellState {
  navFolded: boolean;
  toggleNavFolded: () => void;
}

export const useShellStore = create<ShellState>((set, get) => ({
  navFolded: readFolded(),
  toggleNavFolded: () => {
    const next = !get().navFolded;
    localStorage.setItem(FOLD_KEY, next ? "1" : "0");
    set({ navFolded: next });
  },
}));
