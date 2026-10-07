/**
 * 卡片排序持久化存储（模块私有）。
 *
 * 🔴 与 NexBox 的差异（刻意为之，原因见 README「与 NexBox 的差异」第 3 条）：
 *   NexBox 两页共用音乐域的 `settings.json`（LazyStore），Aegis 本模块**独立文件**，
 *   避免与音乐域写同一份 settings.json 造成跨域耦合（同源 localStorage 键亦已改名）。
 *   排序读取优先级与 NexBox 一致：localStorage 优先，store 为副。
 */
import { LazyStore } from "@tauri-apps/plugin-store";

/** 本模块私有存储实例（排序顺序持久化） */
export const store = new LazyStore("system-optimization.json");
