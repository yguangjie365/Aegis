/**
 * 构建期标志占位。
 *
 * 🔴 NexBox 原文用 `IS_STORE_BUILD` 决定「商店版（MSIX）是否隐藏部分工具栏目」
 *   （其商店政策 10.1.5 / 10.2.3 禁止引导到商店外获取软件）。
 *   **Aegis 无 MSIX 商店版**，该标志恒为 false ⇒ 不搬运条件分支，
 *   内置工具恒为完整 23 项（差异登记见 README）。
 *   保留本文件而非删除：后续若出商店版，只需在此处接真实判定即可恢复原行为。
 */
export const IS_STORE_BUILD = false;
