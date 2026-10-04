/**
 * 设置页 · 平台辅助（当前只服务 Steam 在线库存配置）
 * ============================================================================
 * 背景：后端 `steam_api_key_status` / `steam_save_api_key` / `get_steam_inventory_online`
 * 三个命令都接收 `steamDir` 形参（注册表读取是后端职责，services crate 的
 * `steam.rs::get_steam_path` 是唯一口径，前端不复刻）。
 *
 * 前端要拿到这个路径，唯一途径是调后端 —— 而 `get_steam_install_info`
 * 恰好已返回 `install_path`，**不新增命令**。
 */
import { invoke } from "@/lib/tauri/invoke";

interface SteamInstallInfo {
  installed: boolean;
  install_path: string | null;
  is_running: boolean;
}

/**
 * `steam_api_key_status` 命令的返回值（后端 `SteamApiKeyStatus` 的前端投影）。
 * 字段名与 Rust 侧逐字一致（serde 默认 camelCase 不改，故保持 snake_case）。
 */
export interface SteamApiKeyStatus {
  /** 是否已配置且格式合法 */
  configured: boolean;
  /** 本机可读到的当前账号 SteamID64（`null` = 未登录任何账号） */
  steam_id64: string | null;
  /** 当前账号昵称 */
  account_name: string | null;
  /** 配置了但拿不到账号 ID 时的原因（如实交代，不静默） */
  warning: string | null;
}

/**
 * 取 Steam 安装目录；未安装返回 `null`。
 *
 * 🔴 失败**如实抛出**（不静默返回 null）—— 与项目「禁静默失败」纪律一致：
 * 读不到路径是真异常，调用方须区分「未安装」与「读失败」。
 */
export async function steamDirFor(): Promise<string | null> {
  const info = await invoke<SteamInstallInfo>(
    "get_steam_install_info",
    "Steam",
    "FetchInstallInfo",
  );
  return info.installed ? info.install_path : null;
}
