// 在线音乐登录模态框（MU-4.13 / MU-4.15）—— 复用 NexBox MusicLoginSection 核心逻辑
// 主路径：点击平台 → 打开官方登录窗口（页面默认即扫码登录）→ 等后端事件回传；
// 备用路径：手动粘贴 Cookie（Tauri WebView 不支持 window.prompt，用模态内文本框）

import { useEffect, useState } from "react";
import {
  Modal,
  ModalOverlay,
  ModalContent,
  ModalHeader,
  ModalCloseButton,
  ModalBody,
  VStack,
  HStack,
  Button,
  Text,
  Box,
  Badge,
  Spinner,
  Textarea,
} from "@chakra-ui/react";
import { QrCode } from "lucide-react";
import type { MusicProvider } from "@/types/music";
import { useOnlineStore } from "../../store/online-store";

const PROVIDERS: { id: MusicProvider; name: string; desc: string; color: string }[] = [
  { id: "netease", name: "网易云音乐", desc: "海量曲库 · 歌单 · 云村社区", color: "#C20C0C" },
  { id: "qq", name: "QQ 音乐", desc: "海量曲库 · Hi-Res · 精准推荐", color: "#FEC135" },
];

interface LoginModalProps {
  isOpen: boolean;
  onClose: () => void;
}

export function LoginModal({ isOpen, onClose }: LoginModalProps) {
  const store = useOnlineStore();
  const [target, setTarget] = useState<MusicProvider | null>(null);
  const [waiting, setWaiting] = useState(false);
  const [manual, setManual] = useState(false);
  const [cookieInput, setCookieInput] = useState("");
  const [submitting, setSubmitting] = useState(false);
  const [error, setError] = useState("");

  const targetLoggedIn = target ? store.loginStatuses[target]?.loggedIn : false;

  const close = () => {
    setTarget(null);
    setWaiting(false);
    setManual(false);
    setCookieInput("");
    setError("");
    onClose();
  };

  // 登录窗口事件回传成功（loginStatuses 更新）后自动关闭模态框
  useEffect(() => {
    if (isOpen && waiting && targetLoggedIn) {
      setTarget(null);
      setWaiting(false);
      setManual(false);
      setCookieInput("");
      setError("");
      onClose();
    }
  }, [isOpen, waiting, targetLoggedIn, onClose]);

  // 打开官方登录窗口（页面默认即扫码登录）
  const startLogin = async (platform: MusicProvider) => {
    setTarget(platform);
    setManual(false);
    setWaiting(false);
    setError("");
    try {
      await store.openLoginWindow(platform);
      setWaiting(true);
    } catch (e) {
      setError(String(e));
    }
  };

  const handleProviderClick = (p: (typeof PROVIDERS)[number]) => {
    if (store.loginStatuses[p.id]?.loggedIn) {
      // 已登录 → 切换播放源并关闭
      store.setCurrentPlatform(p.id);
      void store.loadUserPlaylists();
      void store.loadDailyRecommend();
      void store.loadRankList();
      close();
      return;
    }
    void startLogin(p.id);
  };

  const handleSubmit = () => {
    if (!target) return;
    const cookie = cookieInput.trim();
    if (!cookie) {
      setError("请粘贴 Cookie 字符串");
      return;
    }
    setSubmitting(true);
    setError("");
    store
      .loginWithCookie(target, cookie)
      .then(() => close())
      .catch((e) => setError(String(e)))
      .finally(() => setSubmitting(false));
  };

  const waitingError = error || store.loginError || "";

  return (
    <Modal isOpen={isOpen} onClose={close} isCentered>
      <ModalOverlay />
      <ModalContent
        bg="var(--aegis-canvas)"
        border="1px solid"
        borderColor="var(--aegis-hairline)"
        maxW="440px"
        mx={4}
      >
        <ModalHeader fontSize="md">选择平台登录</ModalHeader>
        <ModalCloseButton />
        <ModalBody pb={5}>
          {!target ? (
            <VStack spacing={3} align="stretch">
              {PROVIDERS.map((p) => {
                const loggedIn = store.loginStatuses[p.id]?.loggedIn;
                return (
                  <Button
                    key={p.id}
                    variant="outline"
                    h="auto"
                    justifyContent="flex-start"
                    px={4}
                    py={3}
                    borderColor="var(--aegis-hairline)"
                    _hover={{ bg: `${p.color}12`, borderColor: p.color }}
                    onClick={() => handleProviderClick(p)}
                  >
                    <HStack spacing={3} w="100%">
                      {/* Logo 占位 */}
                      <Box
                        w="40px"
                        h="40px"
                        borderRadius="lg"
                        overflow="hidden"
                        flexShrink={0}
                        bg="white"
                        display="flex"
                        alignItems="center"
                        justifyContent="center"
                      >
                        <Text fontSize="xs" color={p.color} fontWeight="bold">
                          {p.name[0]}
                        </Text>
                      </Box>
                      <VStack spacing={0} align="start" flex={1}>
                        <Text fontSize="sm" fontWeight="semibold" color="var(--aegis-ink)">
                          {p.name}
                        </Text>
                        <Text fontSize="xs" color="var(--aegis-mute)">
                          {p.desc}
                        </Text>
                      </VStack>
                      <Badge
                        variant="subtle"
                        colorScheme={loggedIn ? "green" : "gray"}
                        fontSize="10px"
                      >
                        {loggedIn ? "已登录" : "未登录"}
                      </Badge>
                    </HStack>
                  </Button>
                );
              })}
              <Text fontSize="xs" color="var(--aegis-mute)">
                点击平台后将打开官方登录窗口，扫码即可登录
              </Text>
            </VStack>
          ) : manual ? (
            /* 备用：手动粘贴 Cookie */
            <VStack spacing={3} align="stretch">
              <HStack>
                <Text fontSize="sm" fontWeight="semibold" color="var(--aegis-ink)">
                  {PROVIDERS.find((p) => p.id === target)?.name}
                </Text>
                <Badge variant="subtle" colorScheme="purple" fontSize="10px">
                  Credential Manager 加密保存
                </Badge>
              </HStack>
              <Textarea
                value={cookieInput}
                onChange={(e) => setCookieInput(e.target.value)}
                placeholder={
                  target === "netease"
                    ? "粘贴含 MUSIC_U=… 的 Cookie"
                    : "粘贴含 uin / music_key / p_skey 的 Cookie"
                }
                size="sm"
                h="120px"
                bg="var(--aegis-surface)"
                borderColor="var(--aegis-hairline)"
                _focus={{ borderColor: "var(--aegis-accent)" }}
              />
              {error && (
                <Text fontSize="xs" color="red.400">
                  {error}
                </Text>
              )}
              <HStack justify="flex-end">
                <Button size="sm" variant="ghost" onClick={() => setManual(false)}>
                  返回扫码
                </Button>
                <Button size="sm" colorScheme="red" isLoading={submitting} onClick={handleSubmit}>
                  保存登录
                </Button>
              </HStack>
            </VStack>
          ) : (
            /* 主路径：等待登录窗口内扫码完成 */
            <VStack spacing={3} align="stretch">
              <HStack>
                <Text fontSize="sm" fontWeight="semibold" color="var(--aegis-ink)">
                  {PROVIDERS.find((p) => p.id === target)?.name}
                </Text>
                <Badge variant="subtle" colorScheme="green" fontSize="10px">
                  官方页面扫码登录
                </Badge>
              </HStack>
              <Box
                py={5}
                borderRadius="lg"
                bg="var(--aegis-surface)"
                border="1px solid"
                borderColor="var(--aegis-hairline)"
                textAlign="center"
              >
                {waiting ? (
                  <>
                    <HStack justify="center" spacing={2}>
                      <QrCode size={18} color="var(--aegis-accent)" />
                      <Spinner size="sm" color="var(--aegis-accent)" />
                    </HStack>
                    <Text mt={3} fontSize="sm" color="var(--aegis-ink)">
                      已打开登录窗口，请用手机扫码完成登录
                    </Text>
                    <Text mt={1} fontSize="xs" color="var(--aegis-mute)">
                      登录成功后此处会自动关闭，账号加密保存到本机
                    </Text>
                  </>
                ) : (
                  <Text fontSize="sm" color="var(--aegis-mute)">
                    正在打开登录窗口…
                  </Text>
                )}
              </Box>
              {waitingError && (
                <Text fontSize="xs" color="red.400">
                  {waitingError}
                </Text>
              )}
              <HStack justify="space-between">
                <Button size="sm" variant="ghost" onClick={() => void startLogin(target)}>
                  重新打开登录窗口
                </Button>
                <Button
                  size="sm"
                  variant="ghost"
                  color="var(--aegis-mute)"
                  onClick={() => setManual(true)}
                >
                  手动输入 Cookie
                </Button>
              </HStack>
            </VStack>
          )}
        </ModalBody>
      </ModalContent>
    </Modal>
  );
}
