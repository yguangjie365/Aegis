/**
 * 品牌区（2026-10-05 落地 vertical-geometry-spec.md §三 / §七 迁移表第 3 条）
 * ============================================================================
 * 拖动条之下、隐形框架之上的 72px 条带，由**壳层统一渲染**：主副标题 + 右侧动作槽。
 *
 * 为什么由壳层渲染而不是各页自己画：
 *   ① 一致性 —— 十个模块的标题基线、副标题色、动作槽右缘只应有一处定义；
 *   ② 垂直几何 —— 72 是从内容反推的定值（24px 标题 30 + 13px 副标题 21.5 = 51.5，
 *      余 20.5 分上下），任何页面自己画都会差几 px，十页差十种高度。
 *
 * 副标题**必须单行**：折两行 = 30 + 41 = 71，距 72 只剩 1px，任何行高微调就溢出。
 * 这不是偏好，是 72 这个数的数学后果（规范 §三）。
 */
import { useLayoutEffect, useSyncExternalStore } from "react";
import { useTranslation } from "react-i18next";
import { useLocation } from "react-router-dom";
import styled from "@emotion/styled";
import { SHELL_MODULES } from "./modules";

/* ── 页面注册契约 ────────────────────────────────────────────────────────────
 * 页面在渲染时调 useRegisterBrand({ sub, actions }) 上报自己的副标题与动作。
 *
 * 🔴 为什么用「外部 store + useSyncExternalStore」而不是 Context：
 *   Context 的 Provider state 一变，`<Outlet />` 整棵子树重渲染。而页面上报动作
 *   恰好发生在**它自己每次渲染时** ⇒ Provider state 每帧变 ⇒ 渲染风暴。
 *   外部 store 只通知订阅了 brand 切片的那一个组件，页面自身不受影响。
 *
 * 🔴 为什么用 useLayoutEffect 而不是 useEffect：
 *   useEffect 在 paint 之后跑 ⇒ 首帧会先渲染出「空标题」，再跳成真标题（可见闪动）。
 *   useLayoutEffect 在 paint 前同步完成，首帧就是最终态。
 */
export interface BrandContent {
  /**
   * 主标题覆盖（可选）。
   * 🔴 默认**不需要**传 —— 壳层按当前路由从 `modules.ts` 自动派生，十模块零重复声明。
   *    只有两类页面需要覆盖：
   *      ① 不在模块表里的路由（404 等异常页）；
   *      ② 同路由多视图、需要区分具体子页面时。
   *    规范 §三「页面内不得再出现 h1 级主标题」仍然成立 —— 覆盖也是经品牌区渲染。
   */
  title?: string;
  /** 副标题。必须单行 —— 渲染侧用 CSS 强制 nowrap + ellipsis 兜底。 */
  sub?: React.ReactNode;
  /** 右侧动作槽（按钮组 / 平台头像等） */
  actions?: React.ReactNode;
}

let currentRoute = "";
let current: BrandContent = {};
const listeners = new Set<() => void>();

const emit = () => listeners.forEach((l) => l());

export function useRegisterBrand(route: string, content: BrandContent): void {
  // 路由切换时先清空，避免上一个页面的副标题/动作在新页面上闪一帧
  if (currentRoute !== route) {
    currentRoute = route;
    current = {};
  }
  const nextTitle = content.title;
  const nextSub = content.sub;
  const nextActions = content.actions;
  useLayoutEffect(() => {
    current = { title: nextTitle, sub: nextSub, actions: nextActions };
    emit();
  }, [nextTitle, nextSub, nextActions]);
}

function subscribe(cb: () => void): () => void {
  listeners.add(cb);
  return () => listeners.delete(cb);
}

const getSnapshot = () => current;

/* ── 样式 ──────────────────────────────────────────────────────────────────── */

const Bar = styled.header`
  /* 🔴 72 是定值（规范 §三）。flex:none 保证不被内容挤压。 */
  flex: none;
  height: 72px;
  display: flex;
  align-items: center;
  /* 无底色、无分隔线（用户裁决「品牌区底色与内容区同色」）：层级靠 24/13 的字号字重差建立 */
  background: none;
  /* 🔴 padding-left 20 —— 主标题左缘必须 = 104 = safe-x 20 + 胶囊 64 + main-pad-x 20，
     与内容区左缘**严格同一条竖线**（规范 §三 / mockup 标注）。
     漏这一条的后果：BrandBar 贴在 Column 左缘 84 上，比内容左缘少 20px，
     首屏左上角两列错开（实测截图才发现，断言当时没守着这条）。
     🔴 该值与 shell-layout 的 Main padding-left 是**同一个数**（20），
     改「胶囊→内容」间距时必须同改这里。 */
  padding-left: 20px;
`;

const TextCol = styled.div`
  min-width: 0;
`;

const Title = styled.h1`
  font-size: 24px;
  font-weight: 700;
  line-height: 1.25;
  color: var(--aegis-ink);
`;

const Sub = styled.div`
  font-size: 13px;
  color: var(--aegis-mute);
  margin-top: 2px;
  /* 🔴 副标题必须单行：72 = 30(标题) + 21.5(副) = 51.5，余 20.5 分上下。
     折两行 = 71 ⇒ 距 72 只剩 1px，任何字距/行高微调就溢出（规范 §三硬约束）。 */
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
`;

const Acts = styled.div`
  margin-left: auto;
  display: flex;
  align-items: center;
  gap: var(--aegis-space-sm);
  flex: none;
`;

/* ── 组件 ──────────────────────────────────────────────────────────────────── */

export function BrandBar() {
  const { t } = useTranslation("shell");
  const { pathname } = useLocation();
  const brand = useSyncExternalStore(subscribe, getSnapshot, getSnapshot);

  // 主标题优先用页面注册值（404 等不在模块表里的路由），否则从模块表按路由派生
  const mod = SHELL_MODULES.find((m) => m.route === pathname);
  const title = brand.title ?? (mod ? t(mod.nameKey) : "");

  return (
    <Bar>
      <TextCol>
        <Title>{title}</Title>
        {brand.sub ? <Sub>{brand.sub}</Sub> : null}
      </TextCol>
      {brand.actions ? <Acts>{brand.actions}</Acts> : null}
    </Bar>
  );
}
