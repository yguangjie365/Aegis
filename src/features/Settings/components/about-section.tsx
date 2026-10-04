/**
 * 「关于」区块 —— 2026-10-04 壳层改版配套
 * ============================================================================
 * 状态栏被移除后，规范 §三 要求「常驻红线呈现」的**版本号 + 零遥测声明**
 * 从底部黑条迁到这里（用户裁决：状态栏彻底移除，信息进设置页）。
 *
 * 为什么必须留：零遥测是隐私合规声明，删掉等于放弃红线。
 * 改「常驻」为「常在」—— 声明始终可查，只是不再占用底部 26px 视高。
 */
import { Box, HStack, Text, VStack } from "@chakra-ui/react";
import { useTranslation } from "react-i18next";
import { Info } from "lucide-react";

export function AboutSection() {
  const { t } = useTranslation("shell");
  return (
    <Box
      bg="white"
      border="1px solid"
      borderColor="gray.200"
      borderRadius="var(--aegis-radius-sm)"
      p={4}
    >
      <HStack spacing={2} mb={3}>
        <Info size={16} strokeWidth={1.5} />
        <Text fontSize="sm" fontWeight="bold">
          {t("about.title")}
        </Text>
      </HStack>
      <VStack spacing={0} align="stretch">
        <HStack justify="space-between" py={2} borderBottom="1px solid" borderColor="gray.100">
          <Text fontSize="xs" color="gray.600">
            {t("about.versionLabel")}
          </Text>
          <Text fontSize="xs" fontFamily="var(--font-mono)">
            {t("status.version")}
          </Text>
        </HStack>
        <HStack justify="space-between" py={2}>
          <Text fontSize="xs" color="gray.600">
            {t("about.telemetryLabel")}
          </Text>
          <Text fontSize="xs" fontFamily="var(--font-mono)" color="green.700">
            {t("status.noTelemetry")}
          </Text>
        </HStack>
      </VStack>
      <Text fontSize="xs" color="gray.500" mt={3}>
        {t("about.note")}
      </Text>
    </Box>
  );
}

export default AboutSection;
