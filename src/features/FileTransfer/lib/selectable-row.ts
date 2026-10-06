/* 行选择的键盘可达契约（2026-10-06 UI 审查 B2）。
 *
 * 起因：文件互传的对端行与任务行原先只有 `div onClick`，没有 tabIndex / role / 按键处理
 * ⇒ 只用键盘的用户**选不到设备也选不到任务**，「发送到所选设备」永远禁用 —— 这是本模块
 * 主功能而非边角（违反 §八 8.1 键盘可达、8.2 不得用 div 冒充控件，以及 §九.2 键盘冒烟验收）。
 *
 * 为什么取 `role=button` + `aria-pressed`，而不是 listbox/option：
 *   button 天然是**单一 Tab 停靠点**，不附带方向键导航契约；listbox/option 则要求实现
 *   ↑↓ + Home/End + aria-activedescendant 那一整套，做半套比不做更糟（读屏会以为有方向键）。
 *   §八 8.1 的 ↑↓ 规则只约束 `table`，本页两处是自定义卡片列表，不在其列。
 *   与 FileBackup 行内既有的 `role=button` + `aria-pressed` 写法同语系（不另立第三种）。
 *
 * Space 必须 preventDefault：否则选中行的同时浏览器滚动一屏。
 * 其余按键（含 Tab）**不吞**：焦点顺序仍由浏览器按 DOM 决定，符合「焦点顺序 = 视觉顺序」。
 */
export function selectableRow(selected: boolean, onSelect: () => void) {
  return {
    role: "button",
    tabIndex: 0,
    "aria-pressed": selected,
    onClick: onSelect,
    onKeyDown: (e: { key: string; preventDefault(): void }) => {
      if (e.key === "Enter" || e.key === " ") {
        e.preventDefault();
        onSelect();
      }
    },
  };
}
