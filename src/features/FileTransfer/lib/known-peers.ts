// 已知设备清单的纯逻辑（F-FT-10）。
//
// 单独成纯函数而不是写在组件里：F-FT-10 的四条口径都是有边界的行为
// （按 IP+端口去重、非法端口不落库、编辑只作用于选中行、添加默认命名并选中），
// 用单测钉住比在界面上点出来可靠。

import type { KnownPeer } from "@/types/transfer";

/** 新增时的默认名（F-FT-10 原文："手动『添加』默认命名「新设备」"）。 */
export const NEW_DEVICE_NAME = "新设备";

/** 端口合法区间（与后端 `settings::MIN_PORT` / 65535 一致）。 */
export const MIN_PORT = 1024;
export const MAX_PORT = 65535;

/** 端口文本是否合法（界面用字符串收输入，故这里判文本）。 */
export function isValidPort(text: string): boolean {
  // 只接受纯数字：`parseInt("12abc")` 会得 12，那会把用户的笔误当成合法值
  if (!/^\d+$/.test(text.trim())) return false;
  const port = Number.parseInt(text.trim(), 10);
  return Number.isInteger(port) && port >= MIN_PORT && port <= MAX_PORT;
}

/**
 * 按 **IP + 端口** 判定"是不是同一条"（F-FT-10 的去重口径）。
 *
 * 🔴 不看名称：同一台机器换个显示名仍然是同一台；反之同名不同地址是两台。
 */
export function isSamePeer(a: KnownPeer, b: KnownPeer): boolean {
  return a.ip.trim() === b.ip.trim() && a.port === b.port;
}

/**
 * 加入一条已知设备。
 *
 * 🔴 **重复加入不产生第二条**（F-FT-10 原文）：命中已有条目时**保留原有名称**并返回原数组，
 * 而不是用新名称覆盖——用户手动起的名字不该被一次误点「加入已知」冲掉。
 * 返回的 `added` 为 `false` 时调用方应提示"已加入"（页面已有该 i18n key）而不是再报成功。
 */
export function addKnownPeer(
  list: readonly KnownPeer[],
  candidate: KnownPeer,
): { list: KnownPeer[]; added: boolean } {
  const existing = list.find((peer) => isSamePeer(peer, candidate));
  if (existing !== undefined) {
    return { list: [...list], added: false };
  }
  return { list: [...list, { ...candidate }], added: true };
}

/** 移除一条（按 IP + 端口定位；不存在则原样返回）。 */
export function removeKnownPeer(list: readonly KnownPeer[], target: KnownPeer): KnownPeer[] {
  return list.filter((peer) => !isSamePeer(peer, target));
}

/**
 * 更新**选中行**的名称（F-FT-10："编辑"仅作用于当前选中行）。
 *
 * 定位仍按 IP + 端口（改动名称不该改变行的身份）。
 */
export function renameKnownPeer(
  list: readonly KnownPeer[],
  target: KnownPeer,
  name: string,
): KnownPeer[] {
  return list.map((peer) => (isSamePeer(peer, target) ? { ...peer, name } : peer));
}

/**
 * 新建一条（F-FT-10："默认命名「新设备」并**自动选中该行**"）。
 *
 * 返回新数组与**新行的索引**：调用方要拿它去选中那一行。
 * 🔴 端口默认给 `18889`（本工程默认传输端口）而不是 0：0 是不合法值，
 * 会让"添加"后立刻处于行内报错状态，那是很差的第一次体验。
 */
export function createKnownPeerRow(
  list: readonly KnownPeer[],
  defaultPort = 18889,
): { list: KnownPeer[]; index: number } {
  const row: KnownPeer = { name: NEW_DEVICE_NAME, ip: "", port: defaultPort };
  const next = [...list, row];
  return { list: next, index: next.length - 1 };
}
