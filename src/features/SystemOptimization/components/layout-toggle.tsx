/**
 * 网格 / 列表布局切换 —— 逐字复刻 NexBox
 * `src/components/special/layout-toggle.tsx`（GPL-3.0 同源衍生）。
 * 原文 50 行，除文件头注释外**逐字未改**（仅 Chakra + lucide + i18next，无跨模块依赖）。
 */
import { HStack, IconButton, Tooltip, useColorModeValue } from "@chakra-ui/react";
import { LayoutGrid, List } from "lucide-react";
import { useTranslation } from "react-i18next";

export type LayoutMode = "grid" | "list";

interface LayoutToggleProps {
  mode: LayoutMode;
  onChange: (mode: LayoutMode) => void;
}

export function LayoutToggle({ mode, onChange }: LayoutToggleProps) {
  const { t } = useTranslation();
  const activeBg = useColorModeValue("blackAlpha.100", "whiteAlpha.200");
  const inactiveColor = useColorModeValue("gray.400", "gray.500");

  const options: {
    mode: LayoutMode;
    icon: React.ComponentType<{ size?: number }>;
    labelKey: string;
  }[] = [
    { mode: "grid", icon: LayoutGrid, labelKey: "view.grid" },
    { mode: "list", icon: List, labelKey: "view.list" },
  ];

  return (
    <HStack spacing={1}>
      {options.map((opt) => {
        const Icon = opt.icon;
        const isActive = mode === opt.mode;
        const label = t(opt.labelKey);
        return (
          <Tooltip key={opt.mode} label={label} placement="top">
            <IconButton
              aria-label={label}
              icon={<Icon size={18} />}
              size="sm"
              variant="ghost"
              borderRadius="md"
              color={isActive ? undefined : inactiveColor}
              bg={isActive ? activeBg : "transparent"}
              onClick={() => onChange(opt.mode)}
              _hover={{ bg: activeBg }}
            />
          </Tooltip>
        );
      })}
    </HStack>
  );
}
