/**
 * dynamic-island —— NexBox「灵动岛」提示层 · 🎯 阶段一最小桩（2026-10-03 用户裁决）
 *
 * 🔴 复刻基线（`Docs/功能规格/07-音乐管理.md` §〇，v1.4）：音乐管理模块是 NexBox 的换皮版本。
 * 本文件**不是** NexBox 原件，而是其 1695 行 `components/ui/dynamic-island.tsx` 的**接口等价桩**。
 *
 * 为什么写桩而不搬原件：
 *   ① NexBox 的 dynamic-island 是**全局壳层**（灵动岛胶囊 UI + 全局 store + 图标体系），
 *      服务于全部 20+ 页面，不属于音乐模块；整体搬入会把壳层依赖链拖进音乐域；
 *   ② 用户 2026-10-03 裁决：「基建也照搬，**壳层写最小桩**」。
 *
 * 接口面（已实测穷举 MusicPage.tsx / AudioEqPage.tsx / MusicLoginSection.tsx 的全部用法）：
 *   - `useDynamicIsland("music")` → 返回一个**可调用的 toast 函数**（Chakra useToast 兼容形态）
 *   - 音乐侧实际使用的 options 字段仅 3 个：`title` / `status` / `duration`
 *   - 音乐侧**未使用**任何附加方法（update / close / isActive / showPersistent 等）
 *   ��— 故桩实现只需覆盖上述面；`IslandOptions` 其余字段保留在类型里以保证调用侧逐字可编译。
 *
 * 视觉差异登记：原动态岛为顶部/底部胶囊浮层，本桩用 Chakra `useToast` 渲染默认 Toast。
 * 阶段二换皮时若需还原灵动岛外观，应移植 NexBox 原组件并替换本桩。
 */
import { useCallback, useMemo } from "react";
import { useToast, type UseToastOptions } from "@chakra-ui/react";

/** 与 NexBox `IslandStatus` 同义的语义状态（音乐侧实际用到 success / info / error / warning）。 */
export type IslandStatus = "success" | "info" | "warning" | "error" | "loading";

/** 与 NexBox `IslandOptions` 对齐的选项形态（仅注释，音乐侧未用字段保留以兼容逐字调用）。 */
export interface IslandOptions extends Omit<UseToastOptions, "status"> {
  id?: string;
  title?: string;
  description?: string;
  status?: IslandStatus;
  duration?: number | null;
  isClosable?: boolean;
  /** 兼容 Chakra toast 的 variant（如 "left-accent"）；本桩忽略该值 */
  variant?: string;
  icon?: React.ReactNode;
  onCloseComplete?: () => void;
  /** 持久基线：本桩不支持，映射为长 duration */
  persistent?: boolean;
  /** 0-100 进度：本桩不支持，仅透传 */
  progress?: number;
  onClick?: () => void;
}

/** 与 NexBox `IconKey` 同义的图标键（本桩不消费，仅保持类型可传）。 */
export type IconKey = string;

/** 与 NexBox `DynamicIslandToast` 同构：可调用 + 附加方法（本桩均为 no-op）。 */
export type DynamicIslandToast = ((options: IslandOptions) => void) & {
  update: (id: string, options: IslandOptions) => void;
  isActive: (id?: string) => boolean;
  close: (id?: string) => void;
  closeAll: () => void;
  showPersistent: (options: IslandOptions) => void;
  updatePersistent: (id: string, options: IslandOptions) => void;
  closePersistent: (id?: string) => void;
};

/**
 * 返回与 NexBox `useDynamicIsland` 接口等价的 toast 函数。
 *
 * @param _defaultIcon NexBox 用它给 success/info 自动配语义图标；本桩忽略（Toast 自行处理 status）。
 */
export function useDynamicIsland(_defaultIcon?: IconKey): DynamicIslandToast {
  const toast = useToast();

  const show = useCallback(
    (options: IslandOptions) => {
      // 持久提示：NexBox 是不自动关闭，本桩映射为较长 duration（上限 30s）
      const duration =
        options.duration === null
          ? options.persistent
            ? 30000
            : 5000
          : (options.duration ?? 5000);
      toast({
        id: options.id,
        title: options.title,
        description: options.description,
        status: options.status === "loading" ? "loading" : options.status,
        duration,
        isClosable: options.isClosable,
        icon: options.icon,
      });
    },
    [toast],
  );

  return useMemo<DynamicIslandToast>(
    () =>
      Object.assign(show, {
        update: () => {},
        isActive: () => false,
        close: () => {},
        closeAll: () => {},
        showPersistent: (options: IslandOptions) => show({ ...options, persistent: true }),
        updatePersistent: () => {},
        closePersistent: () => {},
      }),
    [show],
  );
}

export default useDynamicIsland;
