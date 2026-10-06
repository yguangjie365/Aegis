import { describe, it, expect, vi } from "vitest";
import { selectableRow } from "./selectable-row";

/* 为什么单独给这个 helper 写测试（2026-10-06 UI 审查 B2）：
   行选择在**浏览器预览里没有数据行**（对端/任务列表都要后端），所以渲染探针拍不到它；
   而 §八 8.4 的 A11ySemanticsGuard 又尚未实装 ⇒ 这是「既看不见、又没守卫」的那类缺陷。
   故把可访问性契约钉在单元层：键盘能选中、Space 不滚页、语义与状态可读。 */

describe("selectableRow —— 行选择的键盘可达契约（§八 8.1 / 8.2）", () => {
  it("给出 button 语义 + 单一 Tab 停靠点 + 可读的按下态", () => {
    const p = selectableRow(true, () => {});
    expect(p.role).toBe("button");
    expect(p.tabIndex).toBe(0); // 🔴 只允许 0/-1（§八 8.1 禁正值）
    expect(p["aria-pressed"]).toBe(true);
    expect(typeof p.onClick).toBe("function");
  });

  it("Enter 与 Space 都触发选中（只用键盘也能完成选择）", () => {
    const onSelect = vi.fn();
    const p = selectableRow(false, onSelect);
    p.onKeyDown({ key: "Enter", preventDefault: () => {} });
    p.onKeyDown({ key: " ", preventDefault: () => {} });
    expect(onSelect).toHaveBeenCalledTimes(2);
  });

  it("🔴 Space 必须 preventDefault —— 否则选中同时把整页滚一屏", () => {
    const preventDefault = vi.fn();
    selectableRow(false, () => {}).onKeyDown({ key: " ", preventDefault });
    expect(preventDefault).toHaveBeenCalledTimes(1);
  });

  it("无关按键不吞事件（Tab 仍归浏览器管，焦点顺序不被打断）", () => {
    const onSelect = vi.fn();
    const preventDefault = vi.fn();
    selectableRow(false, onSelect).onKeyDown({ key: "Tab", preventDefault });
    expect(onSelect).not.toHaveBeenCalled();
    expect(preventDefault).not.toHaveBeenCalled();
  });

  it("点击路径与键盘路径行为一致（同一 onSelect）", () => {
    const onSelect = vi.fn();
    const p = selectableRow(false, onSelect);
    p.onClick();
    expect(onSelect).toHaveBeenCalledTimes(1);
  });
});
