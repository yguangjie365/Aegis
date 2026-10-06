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
import { useTranslation } from "react-i18next";

import { invoke } from "@/lib/tauri/invoke";
import { FailureDetail, failureDetail, failureReason } from "@/ui/components/failure-detail";
import { steamDirFor, type SteamApiKeyStatus } from "@/features/Settings/lib/steam-dir";

/**
 * 🔴 Chakra `Button` 的默认字重是 **600**，而 §一.4 只允许 400/700 两档（真数据审查 T5）。
 * 不在主题层改：`theme.components.Button.baseStyle` 会连带命中**复刻域约 116 处** Chakra 按钮，
 * 而「复刻域视觉语系整体让位 NexBox」是 2026-10-07 的用户裁决（规范 §一 #1 例外条款）。
 * 也不换成 AppButton：本文件 4 个按钮里 3 个用了 `isLoading`，换过去会丢加载态。
 * 彻底修法挂 §八 C4「AppButton 与 Chakra Button 收敛为单一实现」。
 */
const BTN_WEIGHT = 700;

type StatusState =
  | { phase: "loading" }
  | { phase: "ready"; status: SteamApiKeyStatus }
  /**
   * 🔴 两段拆开（2026-10-06 UI 审查 D7）：`message` 是给用户读的原因，
   * `detail` 是收进「详情」折叠的技术标识（后端命令名），没有则为 null。
   * 存字符串而不是存 error：本组件还有「未安装 Steam」这一**非异常**错误来源，
   * 它没有 error 对象，硬造一个反而假。
   */
  | { phase: "error"; message: string; detail: string | null };

export function SteamApiKeySection() {
  const [state, setState] = useState<StatusState>({ phase: "loading" });
  const [draft, setDraft] = useState("");
  const [revealed, setRevealed] = useState(false);
  const [saving, setSaving] = useState(false);
  const [testing, setTesting] = useState(false);
  const [flash, setFlash] = useState<{ kind: "ok" | "err"; text: string } | null>(null);
  const inputRef = useRef<HTMLInputElement>(null);
  /* 🔴 本区文案全部走 i18n（2026-10-06 UI 审查 M9）：原先整段硬编码中文、
     也没有 settings 命名空间 ⇒ 该区换不了语言，纯图标按钮的 aria-label 也是裸中文。 */
  const { t } = useTranslation("settings");

  /* 深色分支（2026-10-05 令牌化）：原为 #1a1a1a / #333333 / #a0a0a0 三个裸值。
   * 前者与 canvas #0d0d0d 的层级差靠记忆维持、后两者在 canvas 上仅 1.54:1（装饰线豁免）
   * —— 一律改读 aegis 语义键，换包自动跟随。useColorModeValue 结构保留：
   * 浅色包将来交付时只需改第一个参数。 */
  const cardBg = useColorModeValue("white", "aegis.surface-soft");
  const borderColor = useColorModeValue("gray.200", "aegis.hairline");
  const subText = useColorModeValue("gray.600", "aegis.mute");

  const load = useCallback(async () => {
    setState({ phase: "loading" });
    try {
      const dir = await steamDirFor();
      if (!dir) {
        setState({
          phase: "error",
          message: t("steam.noClientLong"),
          detail: null,
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
      setState({ phase: "error", message: failureReason(e), detail: failureDetail(e) });
    }
  }, [t]);

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
          setFlash({ kind: "err", text: t("steam.noClient") });
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
          text: key.trim() ? t("steam.saved") : t("steam.cleared"),
        });
        await load();
      } catch (e) {
        setFlash({ kind: "err", text: e instanceof Error ? e.message : String(e) });
      } finally {
        setSaving(false);
      }
    },
    [load, t],
  );

  /** 测试连接：直接调在线库存命令，失败原因原样透出（不美化、不吞错） */
  const test = useCallback(async () => {
    setTesting(true);
    setFlash(null);
    try {
      const dir = await steamDirFor();
      if (!dir) {
        setFlash({ kind: "err", text: t("steam.noClient") });
        return;
      }
      const res = await invoke<{ source: string; total: number; error: string | null }>(
        "get_steam_inventory_online",
        "Steam",
        "FetchInventoryOnline",
        { steamDir: dir },
      );
      if (res.source === "online") {
        setFlash({ kind: "ok", text: t("steam.testOk", { count: res.total }) });
      } else {
        setFlash({ kind: "err", text: res.error ?? t("steam.testFail") });
      }
    } catch (e) {
      /* 🔴 单行 flash 没有展开的地方，故只给人话原因（命令名由 invoke 的 JSONL 日志
         留痕，action=FetchInventoryOnline）—— 与主线同口径，UI 审查 D7。 */
      setFlash({ kind: "err", text: failureReason(e) });
    } finally {
      setTesting(false);
    }
  }, [t]);

  return (
    <Box
      bg={cardBg}
      border="1px solid"
      borderColor={borderColor}
      borderRadius="var(--aegis-radius-sm)"
      p="lg"
    >
      <HStack spacing="sm" mb={1}>
        <KeyRound size={16} strokeWidth={1.5} color="var(--aegis-accent)" aria-hidden />
        <Text fontSize="14" fontWeight={700}>
          {t("steam.title")}
        </Text>
      </HStack>
      <Text fontSize="12" color={subText} mb="md">
        {t("steam.intro")}
      </Text>

      {state.phase === "loading" && (
        <HStack spacing="xs" color={subText} fontSize="12">
          <Loader2 size={14} strokeWidth={1.5} className="spin" aria-hidden />
          {t("steam.loading")}
        </HStack>
      )}

      {state.phase === "error" && (
        <HStack
          spacing="xs"
          wrap="wrap"
          fontSize="12"
          color="var(--aegis-error)"
          p="sm"
          border="1px solid"
          borderColor="var(--aegis-error)"
          borderRadius="var(--aegis-radius-sm)"
        >
          <AlertTriangle size={14} strokeWidth={1.5} aria-hidden />
          <Text>{state.message}</Text>
          {/* 🔴 命令名 get_steam_install_info 收进折叠（UI 审查 D7）；wrap 让它展开时
              另起一行，而不是把本行的图标与文字挤成竖排。 */}
          <FailureDetail detail={state.detail} />
        </HStack>
      )}

      {state.phase === "ready" && (
        <VStack spacing="sm" align="stretch">
          {/* 状态行 */}
          <HStack spacing="sm" fontSize="12" flexWrap="wrap">
            {state.status.configured ? (
              <HStack spacing="xs" color="var(--aegis-success)" fontWeight={700}>
                <Check size={14} strokeWidth={1.5} aria-hidden />
                <Text>{t("steam.configured")}</Text>
              </HStack>
            ) : (
              <HStack spacing="xs" color={subText} fontWeight={700}>
                <X size={14} strokeWidth={1.5} aria-hidden />
                <Text>{t("steam.notConfigured")}</Text>
              </HStack>
            )}

            {state.status.account_name && (
              <Text color={subText}>
                {t("steam.currentAccount", { name: state.status.account_name })}
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
                state.status.configured ? t("steam.keyHintConfigured") : t("steam.keyHintEmpty")
              }
              value={draft}
              onChange={(e) => setDraft(e.target.value)}
              fontFamily="mono"
              spellCheck={false}
              autoComplete="off"
            />
            <Button
              size="sm"
              fontWeight={BTN_WEIGHT}
              variant="outline"
              aria-label={revealed ? t("steam.hide") : t("steam.show")}
              onClick={() => setRevealed((v) => !v)}
              flex="none"
            >
              {revealed ? (
                <EyeOff size={14} strokeWidth={1.5} aria-hidden="true" />
              ) : (
                <Eye size={14} strokeWidth={1.5} aria-hidden="true" />
              )}
            </Button>
            <Button
              size="sm"
              fontWeight={BTN_WEIGHT}
              onClick={() => void save(draft)}
              isLoading={saving}
              isDisabled={draft.trim() === ""}
              flex="none"
            >
              {t("steam.save")}
            </Button>
          </HStack>

          {/* 操作 + 提示行 */}
          <Flex align="center" gap="sm" flexWrap="wrap">
            <Button
              size="sm"
              fontWeight={BTN_WEIGHT}
              variant="outline"
              onClick={() => void test()}
              isLoading={testing}
              isDisabled={!state.status.configured}
              flex="none"
            >
              {t("steam.test")}
            </Button>
            {state.status.configured && (
              <Button
                size="sm"
                fontWeight={BTN_WEIGHT}
                variant="ghost"
                color="var(--aegis-error)"
                onClick={() => void save("")}
                isLoading={saving}
                flex="none"
              >
                {t("steam.clear")}
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
            {t("steam.howTo")}
          </Text>
        </VStack>
      )}
    </Box>
  );
}
