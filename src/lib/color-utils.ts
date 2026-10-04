// 颜色工具（照搬 NexBox `lib/color-utils.ts` hexToRgba，GPL-3.0 同源衍生；仅引入当前使用面）

export function hexToRgba(hex: string, alpha: number): string {
  const cleanHex = hex.replace("#", "");
  const r = parseInt(cleanHex.slice(0, 2), 16);
  const g = parseInt(cleanHex.slice(2, 4), 16);
  const b = parseInt(cleanHex.slice(4, 6), 16);
  return `rgba(${r},${g},${b},${alpha})`;
}
