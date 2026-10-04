/**
 * 设置页 · Steam 在线库存（API Key 配置区）
 * ============================================================================
 * 用户裁决（2026-10-03）：
 * > 「库存补全的 Steam API Key 输入 UI 放到设置页面，不要放到游戏管理页面」
 *
 * 背景：SystemToolkit 把这个弹窗放在游戏管理页内（`SteamApiKeyWindow.xaml`），
 * Aegis 明确不做。凭据属全局配置，与「看游戏」是两件事 —— 放设置页更贴切。
 *
 * 能力对应（后端 `services/src/steam/inventory_online.rs`）：
 * - `steam_api_key_status`  → 读配置状态（是否已配置 / 本机当前账号 / 缺失原因）
 * - `steam_save_api_key`    → 保存（空串 = 清除）；格式预检 32 位十六进制
 * - `get_steam_inventory_online` → 游戏管理页用，本页只提供「测试连接」
 *
 * 🔴 凭据落盘位置：Steam 目录下的 `aegis_steam_api_key.txt`（明文）。
 * 为什么不加密：与 SystemToolkit 一致（`ISteamApiKeyStore` 存用户配置目录明文）。
 * Steam Web API Key 本身是公开的客户端凭据（非账号密码），泄漏风险限于被他人
 * 冒用配额。**本项目不存任何 Steam 密码**（密码由 Steam 客户端自己管）。
 * 如需更强的保护，阶段二可改走 Windows Credential Manager（音乐模块已有 keyring 依赖）。
 */
import {
  Box,
  Button,
  Flex,
  HStack,
  Input,
  Text,
  VStack,
  useColorModeValue,
} from "@chakra-ui/react";
import { AlertTriangle, Check, Eye, EyeOff, KeyRound, Loader2, X } from "lucide-react";
import { useCallback, useEffect, useRef, useState } from "react";

import { invoke } from "@/lib/tauri/invoke";
import { steamDirFor, type SteamApiKeyStatus } from "@/features/Settings/lib/steam-dir";

type StatusState =
  | { phase: "loading" }
  | { phase: "ready"; status: SteamApiKeyStatus }
  | { phase: "error"; message: string };

export function SteamApiKeySection() {
  const [state, setState] = useState<StatusState>({ phase: "loading" });
  const [draft, setDraft] = useState("");
  const [revealed, setRevealed] = useState(false);
  const [saving, setSaving] = useState(false);
  const [testing, setTesting] = useState(false);
  const [flash, setFlash] = useState<{ kind: "ok" | "err"; text: string } | null>(null);
  const inputRef = useRef<HTMLInputElement>(null);

  const cardBg = useColorModeValue("white", "#1a1a1a");
  const borderColor = useColorModeValue("gray.200", "#333333");
  const subText = useColorModeValue("gray.600", "#a0a0a0");

  const load = useCallback(async () => {
    setState({ phase: "loading" });
    try {
      const dir = await steamDirFor();
      if (!dir) {
        setState({
          phase: "error",
          message: "本机未检测到 Steam 客户端，无法配置在线库存。",
        });
        return;
      }
      const status = await invoke<SteamApiKeyStatus>(
        "steam_api_key_status",
        "Steam",
        "ApiKeyStatus",
        { steamDir: dir },
      );
      setState({ phase: "ready", status });
    } catch (e) {
      setState({ phase: "error", message: e instanceof Error ? e.message : String(e) });
    }
  }, []);

  useEffect(() => {
    void load();
  }, [load]);

  const save = useCallback(
    async (key: string) => {
      setSaving(true);
      setFlash(null);
      try {
        const dir = await steamDirFor();
        if (!dir) {
          setFlash({ kind: "err", text: "本机未检测到 Steam 客户端" });
          return;
        }
        await invoke("steam_save_api_key", "Steam", "SaveApiKey", {
          steamDir: dir,
          apiKey: key,
        });
        setDraft("");
        setRevealed(false);
        setFlash({
          kind: "ok",
          text: key.trim() ? "API Key 已保存" : "API Key 已清除",
        });
        await load();
      } catch (e) {
        setFlash({ kind: "err", text: e instanceof Error ? e.message : String(e) });
      } finally {
        setSaving(false);
      }
    },
    [load],
  );

  /** 测试连接：直接调在线库存命令，失败原因原样透出（不美化、不吞错） */
  const test = useCallback(async () => {
    setTesting(true);
    setFlash(null);
    try {
      const dir = await steamDirFor();
      if (!dir) {
        setFlash({ kind: "err", text: "本机未检测到 Steam 客户端" });
        return;
      }
      const res = await invoke<{ source: string; total: number; error: string | null }>(
        "get_steam_inventory_online",
        "Steam",
        "FetchInventoryOnline",
        { steamDir: dir },
      );
      if (res.source === "online") {
        setFlash({ kind: "ok", text: `连接成功，账号共拥有 ${res.total} 款游戏` });
      } else {
        setFlash({ kind: "err", text: res.error ?? "在线库存获取失败" });
      }
    } catch (e) {
      setFlash({ kind: "err", text: e instanceof Error ? e.message : String(e) });
    } finally {
      setTesting(false);
    }
  }, []);

  return (
    <Box
      bg={cardBg}
      border="1px solid"
      borderColor={borderColor}
      borderRadius="var(--aegis-radius-sm)"
      p="lg"
    >
      <HStack spacing="sm" mb={1}>
        <KeyRound size={16} color="var(--aegis-accent)" aria-hidden />
        <Text fontSize="14" fontWeight={700}>
          Steam 在线库存
        </Text>
      </HStack>
      <Text fontSize="12" color={subText} mb="md">
        填写 Steam Web API Key 后，游戏管理页可显示「账号拥有的全部游戏」（含从未在本机安装的）。
        未配置时该区显示本机缓存数据，功能其余部分不受影响。
      </Text>

      {state.phase === "loading" && (
        <HStack spacing="xs" color={subText} fontSize="12">
          <Loader2 size={14} className="spin" aria-hidden />
          正在读取配置状态…
        </HStack>
      )}

      {state.phase === "error" && (
        <HStack
          spacing="xs"
          fontSize="12"
          color="var(--aegis-error)"
          p="sm"
          border="1px solid"
          borderColor="var(--aegis-error)"
          borderRadius="var(--aegis-radius-sm)"
        >
          <AlertTriangle size={14} aria-hidden />
          <Text>{state.message}</Text>
        </HStack>
      )}

      {state.phase === "ready" && (
        <VStack spacing="sm" align="stretch">
          {/* 状态行 */}
          <HStack spacing="sm" fontSize="12" flexWrap="wrap">
            {state.status.configured ? (
              <HStack spacing="xs" color="var(--aegis-success)" fontWeight={700}>
                <Check size={14} aria-hidden />
                <Text>已配置</Text>
              </HStack>
            ) : (
              <HStack spacing="xs" color={subText} fontWeight={700}>
                <X size={14} aria-hidden />
                <Text>未配置</Text>
              </HStack>
            )}

            {state.status.account_name && (
              <Text color={subText}>
                当前账号：{state.status.account_name}
                {state.status.steam_id64 ? `（${state.status.steam_id64}）` : ""}
              </Text>
            )}

            {state.status.warning && (
              <Text color="var(--aegis-warning)">{state.status.warning}</Text>
            )}
          </HStack>

          {/* 输入行 */}
          <HStack spacing="sm">
            <Input
              ref={inputRef}
              size="sm"
              type={revealed ? "text" : "password"}
              placeholder={
                state.status.configured
                  ? "已配置；输入新值可覆盖，留空并点「清除」可删除"
                  : "粘贴 32 位 Steam Web API Key"
              }
              value={draft}
              onChange={(e) => setDraft(e.target.value)}
              fontFamily="mono"
              spellCheck={false}
              autoComplete="off"
            />
            <Button
              size="sm"
              variant="outline"
              aria-label={revealed ? "隐藏" : "显示"}
              onClick={() => setRevealed((v) => !v)}
              flex="none"
            >
              {revealed ? <EyeOff size={14} /> : <Eye size={14} />}
            </Button>
            <Button
              size="sm"
              onClick={() => void save(draft)}
              isLoading={saving}
              isDisabled={draft.trim() === ""}
              flex="none"
            >
              保存
            </Button>
          </HStack>

          {/* 操作 + 提示行 */}
          <Flex align="center" gap="sm" flexWrap="wrap">
            <Button
              size="sm"
              variant="outline"
              onClick={() => void test()}
              isLoading={testing}
              isDisabled={!state.status.configured}
              flex="none"
            >
              测试连接
            </Button>
            {state.status.configured && (
              <Button
                size="sm"
                variant="ghost"
                color="var(--aegis-error)"
                onClick={() => void save("")}
                isLoading={saving}
                flex="none"
              >
                清除
              </Button>
            )}

            {flash && (
              <Text
                fontSize="12"
                color={flash.kind === "ok" ? "var(--aegis-success)" : "var(--aegis-error)"}
                fontWeight={700}
              >
                {flash.text}
              </Text>
            )}
          </Flex>

          <Text fontSize="11" color={subText}>
            获取方式：登录 Steam → 访问 store.steampowered.com/account/api/apikeys → 生成新 Key。Key
            属公开客户端凭据，本项目明文存于 Steam 目录下的 aegis_steam_api_key.txt（写入前自动备份
            .bak）。
          </Text>
        </VStack>
      )}
    </Box>
  );
}
