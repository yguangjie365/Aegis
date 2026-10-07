/**
 * 工具子页占位（批次 0 形态）。
 *
 * 🔴 为什么需要它：批次 0 只迁入两个入口页 + 共享组件，37 个子页按批次逐个迁入。
 *   若无占位，卡片点击会落到全局 `*` → NotFoundPage（死链）。
 *   本页**如实展示落地状态**，不假装可用（项目纪律：禁静默、已实现须伴随实码证据）。
 *
 * 状态语义与 `lib/tools.ts` 的 `ToolStatus` 一一对应：
 *   - `pending`         → 该工具的子页尚未迁入（批次 1~5）
 *   - `needsElevation`  → 子页已迁入，但操作需管理员权限；提权通道（W0-08）未落地
 *   - `blockedBySpec`   → 规格 08 §明确不做（游戏辅助类），保留卡片但不提供功能
 *   - `ported`          → 已接线可用（本批次内不会出现，留给后续批次替换本组件）
 */
import { useMemo } from "react";
import { useParams, Link } from "react-router-dom";
import { Box, Heading, Text, VStack, Button, Center } from "@chakra-ui/react";
import { useTranslation } from "react-i18next";
import { findTool } from "../lib/tools";

export function ToolPlaceholderPage() {
  const { toolId } = useParams<{ toolId: string }>();
  const { t } = useTranslation();
  const tool = useMemo(() => (toolId ? findTool(toolId) : undefined), [toolId]);

  const title = tool ? t(tool.titleKey) : t("sysOptimize.unknownTool");
  const description = tool ? t(tool.descriptionKey) : "";

  return (
    <Box pt={8}>
      <Center>
        <VStack
          align="start"
          spacing={4}
          maxW="640px"
          w="full"
          p={8}
          border="1px solid"
          borderColor="whiteAlpha.300"
          borderRadius="xl"
        >
          <Heading size="md">{title}</Heading>
          {description ? (
            <Text color="whiteAlpha.800" fontSize="sm">
              {description}
            </Text>
          ) : null}
          <Text color="whiteAlpha.700" fontSize="sm">
            {tool?.status === "blockedBySpec"
              ? t("sysOptimize.placeholderSpecBlocked")
              : tool?.status === "needsElevation"
                ? t("sysOptimize.placeholderNeedsElevation")
                : t("sysOptimize.placeholderPending")}
          </Text>
          <Center>
            <Button as={Link} to="/system-optimization" size="sm" variant="ghost">
              {t("sysOptimize.backToTools")}
            </Button>
          </Center>
        </VStack>
      </Center>
    </Box>
  );
}

export default ToolPlaceholderPage;
