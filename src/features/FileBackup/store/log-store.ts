// 操作日志（F-FB-48）。
//
// 🔴 **为什么在前端记，而不是读后端 `tracing`**：规格要求留痕的是
//   「规则、备份、校验、恢复、定时的**动作与结果**」—— 这些动作全部由界面发起、
//   结果也由命令返回值带回界面。后端 `tracing` 是**进程内**日志，且当前
//   subscriber 尚未安装（W0-07 欠账，见 kickoff 变更记录），拿它当界面日志源
//   既做不到、也绕远。两者是互补：界面日志给用户看，`tracing` 给排障看。
//
// 🔴 **上限滚动清理**（规格 F-FB-48 明文「上限滚动清理」）：超出 [`MAX_ENTRIES`]
//   丢弃最旧的。没有这条，长时间挂着页面会把条目无限堆积。
//
// 🔴 三条纪律（规格「失败、取消、警告一律显式呈现，禁止静默」）：
//   ① 每个用户动作**至少**落一条（成功落 success、失败落 error）；
//   ② 「取消」落 info 而非 error（F-FB-19：取消不是失败）；
//   ③ 调用方负责给出**可读中文**，store 不做任何措辞加工。

import { create } from "zustand";

/** 日志级别（与界面过滤标签一一对应）。 */
export type LogLevel = "info" | "success" | "error";

/** 一条日志。 */
export interface LogEntry {
  /** 单调递增序号（同时充当 React key）。 */
  id: number;
  level: LogLevel;
  /** 可直接展示的中文（调用方已组装好）。 */
  text: string;
  /** 写入时刻（Unix 毫秒）。 */
  at: number;
}

/** 滚动上限。 */
const MAX_ENTRIES = 200;

/** 序号源：模块级而非 state，避免每次 push 都触发额外的 set。 */
let sequence = 0;

interface LogStore {
  entries: LogEntry[];
  push: (level: LogLevel, text: string) => void;
  clear: () => void;
}

export const useLogStore = create<LogStore>((set) => ({
  entries: [],
  push: (level, text) =>
    set((state) => {
      sequence += 1;
      const next = [...state.entries, { id: sequence, level, text, at: Date.now() }];
      return { entries: next.length > MAX_ENTRIES ? next.slice(-MAX_ENTRIES) : next };
    }),
  clear: () => set({ entries: [] }),
}));

/**
 * 便捷写入 —— 供**非组件上下文**（如 mutation 回调之外的工具函数）调用。
 *
 * 🔴 组件内请直接用 `useLogStore` 的选择器，不要走这里：
 *   这三个函数会无条件读一次 store，而组件本可以只订阅 `push` 引用。
 */
export function logInfo(text: string): void {
  useLogStore.getState().push("info", text);
}

export function logSuccess(text: string): void {
  useLogStore.getState().push("success", text);
}

export function logError(text: string): void {
  useLogStore.getState().push("error", text);
}
