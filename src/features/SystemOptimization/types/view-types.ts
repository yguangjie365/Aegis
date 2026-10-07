/**
 * 工具卡片条目契约 —— 逐字复刻 NexBox `src/components/special/view-types.ts`
 * （GPL-3.0 同源衍生，Aegis = NexBox 换皮工程）。
 *
 * 复刻基线：Source/NexBox/src/components/special/view-types.ts（10 行，原文未改语义）
 * 适配：仅注释头；类型本体逐字保留。
 */
import type { ComponentType } from "react";

export interface ViewItem {
  id: string;
  path: string;
  icon: ComponentType<{ size?: number; strokeWidth?: number; color?: string }>;
  titleKey: string;
  descriptionKey: string;
  color: string;
  beta?: boolean;
}
