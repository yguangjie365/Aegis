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

let current: BrandContent = {};
const listeners = new Set<() => void>();

/** 🔴 注册内容**连同它属于哪个路由**一起存** —— 渲染侧据此校验归属，
 *  避免「漏注册的页面显示上一个注册者的副标题」（2026-10-05 实机发现，见 BrandBar 内注释）。 */
let currentOwner = "";

export function useRegisterBrand(route: string, content: BrandContent): void {
  const nextTitle = content.title;
  const nextSub = content.sub;
  const nextActions = content.actions;
  useLayoutEffect(() => {
    current = { title: nextTitle, sub: nextSub, actions: nextActions };
    currentOwner = route;
    publish();
  }, [nextTitle, nextSub, nextActions, route]);
}

function subscribe(cb: () => void): () => void {
  listeners.add(cb);
  return () => listeners.delete(cb);
}

/* 🔴 snapshot 必须带上 `owner`（= 注册者路由）。
   `useSyncExternalStore` 靠**引用比较**判断是否变化 —— 若只返回 `current`（内容不变时
   引用相同），owner 变化不会触发重渲染，归属校验就会读到过期值。 */
interface Snapshot {
  content: BrandContent;
  owner: string;
}
let snapshot: Snapshot = { content: {}, owner: "" };
const getSnapshot = () => snapshot;

function publish() {
  snapshot = { content: current, owner: currentOwner };
  listeners.forEach((l) => l());
}

/* ── 样式 ──────────────────────────────────────────────────────────────────── */

const Bar = styled.header`
  /* 🔴 72 是定值（规范 §三）。flex:none 保证不被内容挤压。 */
  /* 🔴 靠右只由「最右那个有内容的元素」承担 margin-left:auto。
     TextCol 有 flex:1吃掉剩余空间，所以 Acts 与 PortalSlot **都**写 auto
     也不会平分（auto 只作用于「auto 边距与 flex 剩余空间」的计算，
     flex:1 已把剩余全部吃掉 ⇒ 后面的 auto 得 0）。
     ⚠️ 曾反过来把 TextCol 的 flex 去掉 ⇒ Acts 的 auto 把整列推到 690，主标题错位。 */
  flex: none;
  height: 72px;
  display: flex;
  align-items: center;
  /* 🔴 2026-10-05 最终裁决：底色与主界面背景同色 = **不画底色**
     （10-04 同色裁决 → 10-05 早曾短暂改浅 chrome-elevated，同日用户改回，勿再改）。
     层次只由 24/13 字号差建立；不加分隔线。
     （历史教训存档：画底色的两轮里，background 默认画 padding-box / content-box
     分别造成左缘、右缘各 20px 错位，被用户三次截图纠正 —— 无底色即无此类问题。） */

  /* 🔴 padding-left 20 —— 主标题左缘 = 104 = safe-x 20 + 胶囊 64 + main-pad-x 20，
     与内容区左缘**严格同一条竖线**（规范 §三 / mockup 标注）。
     🔴 该值与 shell-layout 的 Main padding-left 是**同一个数**（20），
     **两者必须同步**：改一个不改另一个 ⇒ 标题与内容错位 20px（2026-10-05 踩过）。
     ⚠️ 曾误把它改 0 想「贴边框」，结果标题跑到 84 而内容仍在 104，反而不对齐 ——
        「对齐」的前提是**两个容器同值**，不是「都改成 0」。
     ⚠️ 本注释位于模板字符串内部，**禁止出现反引号**（会报 TS1005，本项目已踩 8 次）。 */
  padding-left: 20px;

  /* 🔴 2026-10-05 用户明示等式：品牌区宽度 = 内容区宽度 = 1280-20-64-20-20 = 1156。
     padding-right = **0**（原 20 已推翻 —— 它把可见右缘截短在 1240，与内容区 1260
     错 20px，用户截图红框第三次纠正）。
     现行契约：内容盒 = Main 内容盒 = [104, 1260]；动作槽右缘 1260 与卡片描边
     同一条竖线（距窗口边仍有 SAFE_RIGHT 20）。
     🔴 改本行必须连同上方 padding-left 一起复核（两者共同决定内容盒 [104,1260]）。 */
  padding-right: 0;
`;

const TextCol = styled.div`
  min-width: 0;
  /* 🔴 flex:1 —— 必须占据剩余空间，否则右侧的 Acts / PortalSlot（都带 margin-left:auto）
     会把本列挤到右边，主标题跑出 x=104（实测整列被推到 690）。
     本列是「标题+副标题」的唯一容器，flex:1 让它吃掉所有剩余宽度，
     auto 左边距就自然落在它右侧的槽位上。 */
  flex: 1 1 auto;
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

/* 🔴 2026-10-05：复刻域（音乐）**不能 import 壳层** —— `tsconfig.music-replica.json`
   的 include 只有「音乐域 + types/music.ts」，任何外部 import 都撞 TS6307
   （试过 `@/shell/brand-bar` 与 `@/shared` 两条路，门面本身也在清单外）。
   ⇒ 音乐页无法调 useRegisterBrand。**解法：portal 槽位**——
   壳层在这里挂空容器并给出稳定 id，复刻域用 `createPortal` 把内容投进去。
   为什么这不违反 TS6307：复刻域只依赖 **react-dom**（第三方）与 **字符串 id**，
   不 import 任何本仓文件 ⇒ 类型系统看不到跨域依赖。
   ⚠️ 槽位必须**无条件渲染**（哪怕没有注册者），否则复刻域投不进来。
   🔴🔴 **不能用 `display: contents`** —— 实测浏览器会把它的子节点**提升到父级**，
      于是 `slot.children.length === 0`，DOM 上看不到投进来的内容（而 React 认为已挂载），
      表现为「品牌区右侧空着、内容却在页内」。改用 flex + 空时塌陷。
   🔴 槽位**分两个**：`sub` 位在标题下方（左缘与标题对齐），
      `actions` 位在右侧靠右。副标题必须走 sub 位 —— 放进 actions 位就变成
      「和平台标签并排」，违反规范（实测曾如此，副标题跑到 x=886）。 */
const PortalSlot = styled.div`
  margin-left: auto;
  display: flex;
  align-items: center;
  gap: var(--aegis-space-sm);
  flex: none;

  /* 无内容时完全塌陷（不留 20px 空白）：:empty 只在**真的没有子节点**时命中，
     而 React portal 投进来的节点会计入 children ⇒ 空槽位确实匹配。 */
  &:empty {
    display: none;
  }
`;

/** 副标题位的 portal 槽位。样式与 Sub 一致（13px / mute / 单行省略），
 *  这样复刻域投进来的文字与壳层自己渲染的副标题**视觉完全同构**。 */
const PortalSubSlot = styled.div`
  font-size: 13px;
  color: var(--aegis-mute);
  margin-top: 2px;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
  min-width: 0;

  &:empty {
    display: none;
  }
`;

/* ── 组件 ──────────────────────────────────────────────────────────────────── */

/** 品牌区 portal 槽位的 DOM id。
 *  🔴 复刻域**只能复制这个字符串**，不得 import 本文件（TS6307，见 PortalSlot 注释）。 */
export const BRAND_PORTAL_ID = "aegis-brand-portal";
/** 副标题位的 portal 槽位 id。副标题**必须**走这个位（在标题下方），不是右侧动作位。 */
export const BRAND_SUB_PORTAL_ID = "aegis-brand-sub-portal";

export function BrandBar() {
  const { t } = useTranslation("shell");
  const { pathname } = useLocation();
  const { content: brand, owner } = useSyncExternalStore(subscribe, getSnapshot, getSnapshot);

  // 主标题优先用页面注册值（404 等不在模块表里的路由），否则从模块表按路由派生
  const mod = SHELL_MODULES.find((m) => m.route === pathname);
  const title = brand.title ?? (mod ? t(mod.nameKey) : "");

  /* 🔴🔴 2026-10-05 修复「品牌区串页」——**漏注册页面会显示上一页的副标题与动作**。
   *
   * 现象：游戏管理页与音乐管理页的副标题都显示 `BlueSky · Windows 11 Pro · 运行 1天`，
   *      右侧动作也都是概览页的 `LIVE · 每 2 秒 / 导出报告 / 立即刷新`。
   *
   * 根因（设计缺陷，非笔误）：`useRegisterBrand` 里的「路由变了就清空」在**函数体内**，
   *   只有**主动调用**的页面才触发。SteamPage / MusicPage 从未调用 ⇒ 清空永远不执行
   *   ⇒ 上一个注册者的内容一直留在 store 里。
   *
   * 修法（把归属校验放到消费侧，不依赖页面自觉）：
   *   注册时记下 `route`，渲染时比对 `pathname` —— 不匹配就不渲染 sub/actions。
   *   漏注册的后果从「显示别人的内容」降级为「自己不显示副标题」= 安全失败。
   *   主标题不受影响（它本来就从 `modules.ts` 按路由派生，与注册无关）。
   */
  const owns = owner === pathname;

  return (
    <Bar>
      <TextCol>
        <Title>{title}</Title>
        {owns && brand.sub ? <Sub>{brand.sub}</Sub> : null}
        {/* 🔴 副标题位 portal 槽（供复刻域投副标题）。样式与 Sub 同构 ⇒ 视觉一致。
            ⚠️ 副标题**只能**放这里；放进右侧动作位会变成「与平台标签并排」，违反规范。 */}
        <PortalSubSlot id={BRAND_SUB_PORTAL_ID} />
      </TextCol>
      {owns && brand.actions ? <Acts>{brand.actions}</Acts> : null}
      {/* 🔴 portal 槽位（见 PortalSlot 注释）：供音乐管理等**复刻域**注入品牌区内容。
          槽位自带 margin-left:auto；只有它与 Acts **同时**存在时需特殊处理
          （见下方 style 规则），否则两个 auto 会平分剩余空间、间距错乱。 */}
      <PortalSlot id={BRAND_PORTAL_ID} />
    </Bar>
  );
}
