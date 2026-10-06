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
import { matchModule } from "./modules";

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
    /* 🔴 2026-10-06（代码审查 #6）：卸载时清场 —— 防「同路由换实例」的间隙里
       消费侧读到上一实例的陈旧 sub/actions（owns 校验只看路由，路由没变拦不住：
       如 Suspense 挂起、懒挂载重来）。清场只认自己的登记（currentOwner === route）：
       若同一次 commit 里新实例已抢先注册，则不动，避免误删后来者。
       ⚠️ 调用方须 memo 动作节点（现状全部 memo）—— 否则内容一变就是
       clear → set 两次 publish；虽同在 layout 相位无可见闪烁，但纯属白开销。 */
    return () => {
      if (currentOwner === route) {
        current = {};
        currentOwner = "";
        publish();
      }
    };
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

/* 🔴 动作槽 = 间距的**唯一**声明处（2026-10-06 用户裁决：品牌区按钮高度与间距统一）。
   统一后的契约（规范 §三 brand-bar 行）：
     · 可点控件恒 sm 档 **h=28**（AppButton size=sm 与 Chakra Button size=sm 都被
       令牌层锁在 28，见 theme.ts controlSizes）；非按钮项（状态点 / 徽章）恒 **h=24**。
     · 项间距恒 **12 = --aegis-space-md**，**页面不得再自带带 gap 的包装容器** ——
       此前 Overview 的 HeadTools 是 12、FileBackup 的 BrandActions 是 8，同一排两套节奏。
       页面要传就传 Fragment（或单个不带 gap 的元素），让本容器直接持有那些项。
   🔴 flex-wrap 必须 **nowrap**（原写在 FileBackup 的包装层里，现上收到这里，
     因为包装层已删）：本条带是 **72px 定值**高且 `flex:none`，动作换行 =
     28+8+28 = 64 另起一行 ⇒ 直接顶穿 72。迁进品牌区前它在页头 wrap 是无害的。 */
const Acts = styled.div`
  margin-left: auto;
  display: flex;
  align-items: center;
  gap: var(--aegis-space-md);
  flex-wrap: nowrap;
  flex: none;
`;

/* 🔴 2026-10-05：复刻域（音乐）**不能 import 壳层** —— `tsconfig.music-replica.json`
   的 include 只有「音乐域 + types/music.ts」，任何外部 import 都撞 TS6307
   （试过 `@/shell/brand-bar` 与 `@/shared` 两条路，门面本身也在清单外）。
   ⇒ 音乐页无法调 useRegisterBrand。**解法：portal 槽位**——
   壳层在这里挂空容器并给出稳定 id，复刻域用 `createPortal` 把内容投进去。
   为什么这不违反 TS6307：复刻域只依赖 **react-dom**（第三方）与 **字符串 id**，
   不 import 任何本仓文件 ⇒ 类型系统看不到跨域依赖。
   🔴🔴 2026-10-06（代码审查 #3，用户裁定）：原口径「槽位**无条件渲染**（哪怕没有
      注册者）、两路靠纪律不并投」作废，改为**结构排斥** —— 槽位 id 只在 hook 路径
      **未产出**时进 DOM（BrandBar 里 hasSub / hasActs 派生）⇒
      **槽位 id 的存在性即互斥信号**：复刻域 getElementById 命中 ⟺ hook 无内容，
      同页双源重叠在结构上不可能。
      🔴 冲突时的**降级方向（写死，勿再推敲）**：hook 优先 —— hook 有产出 ⇒ 槽位缺席
         ⇒ 复刻域 getElementById 取空 ⇒ **portal 投递静默失败、不报警**（复刻域无法
         import 壳层，告警无处可去）。取不到槽位 = **预期行为**，不是 bug、不要去"修"。
      ⚠️ 边界（明确不支持）：同页 hook + portal 混投（如 hook 出 actions、portal 投
         sub）—— 判为不支持；复刻域路由永远无 hook 内容 ⇒ 可用性不受影响。
         将来若确有需求，再设计显式优先级，现在不留模糊兼容窗口。
      ⚠️ 契约传递：复刻域读不到本文件（TS6307）⇒ 上述降级方向必须抄送在
         `features/MusicManager/pages/MusicPage.tsx` 的 portal 通道注释处，两边同步改。
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
  /* 🔴 与 Acts 同值（2026-10-06 统一）：复刻域投进来的项和页面注册的项走同一节奏。 */
  gap: var(--aegis-space-md);
  flex-wrap: nowrap;
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

  // 主标题优先用页面注册值（404 等不在模块表里的路由），否则按路由派生（子路由继承父模块）
  const mod = matchModule(pathname);
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

  /* 🔴 互斥门（2026-10-06 代码审查 #3）：hook 路径有内容时**槽位 id 不进 DOM**。
     两路若同时渲染会出现两份副标题 / 两组动作；「谁是唯一渲染者」现由结构承担 ——
     **槽位 id 的存在性即互斥信号**（复刻域 getElementById 命中 = hook 无内容）。 */
  const hasSub = owns && !!brand.sub;
  const hasActs = owns && !!brand.actions;

  return (
    <Bar>
      <TextCol>
        <Title>{title}</Title>
        {hasSub ? <Sub>{brand.sub}</Sub> : null}
        {/* 🔴 副标题位 portal 槽（供复刻域投副标题，**仅 hasSub=false 时渲染**）。样式与 Sub 同构。
            ⚠️ 副标题**只能**放这里；放进右侧动作位会变成「与平台标签并排」，违反规范。 */}
        {!hasSub ? <PortalSubSlot id={BRAND_SUB_PORTAL_ID} /> : null}
      </TextCol>
      {hasActs ? <Acts>{brand.actions}</Acts> : null}
      {/* 🔴 actions 位 portal 槽（**仅 hasActs=false 时渲染**；互斥契约见 PortalSlot 注释）。 */}
      {!hasActs ? <PortalSlot id={BRAND_PORTAL_ID} /> : null}
    </Bar>
  );
}
