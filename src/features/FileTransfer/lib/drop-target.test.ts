import { describe, expect, it } from "vitest";
import {
  DROP_DEDUP_WINDOW_MS,
  dropKey,
  hitTestDropRow,
  isDuplicateDrop,
  toCssPoint,
  type DropRowRect,
} from "./drop-target";

const ROW_A: DropRowRect = { id: "a", left: 100, top: 200, right: 400, bottom: 230 };
const ROW_B: DropRowRect = { id: "b", left: 100, top: 240, right: 400, bottom: 270 };

describe("toCssPoint（K-S01 实测的坐标系差异）", () => {
  it("1x 缩放时原样返回", () => {
    expect(toCssPoint({ x: 300, y: 215 }, 1)).toEqual({ x: 300, y: 215 });
  });

  it("🔴 非 1x 缩放必须换算（否则只在缩放机器上错行）", () => {
    expect(toCssPoint({ x: 125, y: 250 }, 1.25)).toEqual({ x: 100, y: 200 });
    expect(toCssPoint({ x: 300, y: 400 }, 2)).toEqual({ x: 150, y: 200 });
    expect(toCssPoint({ x: 240, y: 360 }, 1.5)).toEqual({ x: 160, y: 240 });
  });

  it("🔴 dpr 非法一律按 1（宁可退化也不能让命中测试恒落空）", () => {
    for (const bad of [0, -1, Number.NaN, Number.POSITIVE_INFINITY]) {
      expect(toCssPoint({ x: 10, y: 20 }, bad)).toEqual({ x: 10, y: 20 });
    }
  });
});

describe("hitTestDropRow（R-7：不许猜测）", () => {
  const rows = [ROW_A, ROW_B];

  it("落在行内即命中该行", () => {
    expect(hitTestDropRow(rows, { x: 200, y: 215 }, 1)).toBe("a");
    expect(hitTestDropRow(rows, { x: 200, y: 255 }, 1)).toBe("b");
  });

  it("边界包含（左/右/上/下四条边都算命中）", () => {
    expect(hitTestDropRow(rows, { x: 100, y: 200 }, 1)).toBe("a");
    expect(hitTestDropRow(rows, { x: 400, y: 230 }, 1)).toBe("a");
  });

  it("🔴 落在行外返回 null，**不回退到任何选中行**", () => {
    expect(hitTestDropRow(rows, { x: 99, y: 215 }, 1)).toBeNull();
    expect(hitTestDropRow(rows, { x: 200, y: 235 }, 1)).toBeNull();
    expect(hitTestDropRow(rows, { x: 200, y: 271 }, 1)).toBeNull();
  });

  it("空列表恒 null", () => {
    expect(hitTestDropRow([], { x: 200, y: 215 }, 1)).toBeNull();
  });

  it("🔴 放大缩放下也命中同一行（物理坐标 + dpr 的组合）", () => {
    // 1.25 倍下，行 a 的物理范围是 125..500 / 250..287.5
    expect(hitTestDropRow(rows, { x: 250, y: 270 }, 1.25)).toBe("a");
    // 若不换算，x=160 会落到行外（160 < 100 之外无行…此处置于 b 行区间）
    expect(hitTestDropRow(rows, { x: 250, y: 320 }, 1.25)).toBe("b");
  });

  it("重叠时取数组里第一个（调用方按 DOM 顺序传入）", () => {
    const overlapping: DropRowRect[] = [
      { id: "upper", left: 0, top: 0, right: 100, bottom: 100 },
      { id: "lower", left: 0, top: 0, right: 100, bottom: 100 },
    ];
    expect(hitTestDropRow(overlapping, { x: 50, y: 50 }, 1)).toBe("upper");
  });
});

describe("dropKey / isDuplicateDrop（K-S01：一次拖放投递两条事件）", () => {
  it("同一批路径 + 同一点得到同一个键", () => {
    expect(dropKey(["C:\\a.txt"], { x: 10, y: 20 })).toBe(dropKey(["C:\\a.txt"], { x: 10, y: 20 }));
  });

  it("路径或落点任一不同即不同键", () => {
    expect(dropKey(["C:\\a.txt"], { x: 10, y: 20 })).not.toBe(
      dropKey(["C:\\b.txt"], { x: 10, y: 20 }),
    );
    expect(dropKey(["C:\\a.txt"], { x: 10, y: 20 })).not.toBe(
      dropKey(["C:\\a.txt"], { x: 11, y: 20 }),
    );
  });

  it("🔴 键相同且在窗口内 => 判为重复（第二次投递必须丢弃）", () => {
    const key = dropKey(["C:\\a.txt"], { x: 10, y: 20 });
    expect(isDuplicateDrop(key, 1_000, key, 1_000)).toBe(true);
    expect(isDuplicateDrop(key, 1_000, key, 1_000 + DROP_DEDUP_WINDOW_MS)).toBe(true);
  });

  it("键相同但已过窗口 => 不是重复（用户确实又拖了一次）", () => {
    const key = dropKey(["C:\\a.txt"], { x: 10, y: 20 });
    expect(isDuplicateDrop(key, 1_000, key, 1_001 + DROP_DEDUP_WINDOW_MS)).toBe(false);
  });

  it("键不同 => 不是重复，哪怕同一毫秒", () => {
    expect(isDuplicateDrop("k1", 1_000, "k2", 1_000)).toBe(false);
  });

  it("首次（没有上次）=> 不是重复", () => {
    expect(isDuplicateDrop(null, 0, "k", 0)).toBe(false);
  });

  it("🔴 时间倒流时按不重复处理：宁可多处理一次，也不要永远吞掉后续 drop", () => {
    expect(isDuplicateDrop("k", 5_000, "k", 1_000)).toBe(false);
  });
});
