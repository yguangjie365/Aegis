// 文件备份页（M-06）：左栏规则列表 + 右栏所选规则的快照表格 + 运行期进度条。
//
// 布局按已审稿的 `.worknotes/fb-layout-draft.md` 落地。要点：
// · 停用规则**降透明度**而非隐藏（隐藏会让人以为规则丢了）
// · 卷影/定时开关**禁用 + 就地说明原因**（F-FB-21 要求用户理解「勾了为何无效」）
// · 进度区在无事件通道时只说「进行中」，**不编造百分比**
// · 失败项在表格行内展开，不弹独立窗口（避免打断批量流程）

import { useEffect, useMemo, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import styled from "@emotion/styled";
import { Menu, MenuButton, MenuItem, MenuList, Portal } from "@chakra-ui/react";
import { EllipsisVertical } from "lucide-react";
import { open as openDialog, save as saveDialog } from "@tauri-apps/plugin-dialog";
import { useRegisterBrand } from "@/shared";
import { AppButton } from "@/ui/components/app-button";
// 🔴 2026-10-07：记录损坏警告 Panel 删除后，`PanelNote` 在本页已无人使用，
//   故从import 里一并移除（留着会同时触发 TS6133 与 ESLint no-unused-vars）。
import { Panel } from "@/ui/components/panel";
import { SectionTitle } from "@/ui/components/section-title";
import { DataTable, Tbody, Td, Th, Thead, Tr } from "@/ui/components/panel";
import { EmptyStateBlock, ErrorStateBlock, LoadingSkeleton } from "@/ui/components/state-block";
import { failureDetail, failureReason } from "@/ui/components/failure-detail";
import { useConfirmDialog } from "@/ui/components/confirm-dialog";
import { RuleEditorDialog } from "../components/rule-editor-dialog";
import { RestoreDialog } from "../components/restore-dialog";
import { DraftBadge, EnabledBadge, IntegrityBadge, StatusBadge } from "../components/status-badge";
import { BackupProgress } from "../components/backup-progress";
import { LogPanel } from "../components/log-panel";
import { useLogStore } from "../store/log-store";
import {
  asBackupError,
  useBackupRules,
  useCancelBackup,
  useDeleteRule,
  useDeleteSnapshot,
  useExportRules,
  useImportRules,
  useOpenSnapshotDir,
  useRefreshRules,
  useReorderRules,
  useSetRuleEnabled,
  useSnapshots,
  useStartBackup,
  useVerifySnapshot,
} from "../hooks/use-backup";
import { formatBytes, formatCount, formatSnapshotTime } from "../lib/format";
import type { RuleView, SnapshotView } from "@/types/backup";

/* ══ 页面骨架 ══════════════════════════════════════════════════════════ */

/**
 * 页面容器。
 *
 * 🔴 **`width: 100%` 不是冗余，是修一个实测到的塌陷**（2026-10-05，CDP 取证）：
 * 壳层 `Main` 是 `display: flex; flex-direction: column`，本容器是它的直接子元素。
 * `margin: 0 auto` 的 **auto 外边距优先于 `align-items: stretch`** ——
 * flex 规范里 auto margin 会吃掉剩余空间，于是子项从「填满 1146」变成
 * 「收缩到内容宽再居中」。
 *
 * 实测（视口 1280，`.worknotes/layout-fixture.html` 四变体对照）：
 * | 写法 | 页面宽 | 右栏剩余 |
 * |---|---|---|
 * | `max-width + margin: 0 auto`（原） | 374 | **18px**（塌了） |
 * | `+ width: 100%`（现） | 1109 | 753px ✅ |
 * | `+ align-self: stretch` | 374 | 18px（**无效**） |
 *
 * ⇒ 要「超宽时居中 + 正常时撑满」，`width: 100%` 是唯一正解。
 *
 * ⚠️ 同款写法在 `Overview/pages/overview-page.tsx:34` 也有一处，
 *    它的病被 `repeat(auto-fit, minmax(228px,1fr))` 网格掩盖了（网格自己会吃满），
 *    但根因相同 —— 要不要一并修由你定（本批只动本域）。
 */
const Page = styled.div`
  max-width: 1280px;
  /* 🔴 见上方实测：没有这一行，页面会塌到内容宽并居中 */
  width: 100%;
  margin: 0 auto;
  display: flex;
  flex-direction: column;
  /* 🔴 垂直节奏的唯一来源（同 Overview 页口径） */
  gap: var(--aegis-space-lg);
`;

/* 🔴 2026-10-06 删除 `BrandActions` 包装层（原声明 gap: sm + nowrap）。
   品牌区动作槽的**间距与换行只由壳层 `Acts` 声明**（规范 §三 brand-bar 行：
   项间距 12 / nowrap / 按钮 h=28 / 非按钮项 h=24）—— 留着它，本行就是 8 而
   Overview 是 12，同一排两套节奏。下面 `brandActions` 因此直接传 Fragment。 */

/**
 * 「⋯」更多菜单的触发钮（2026-10-06 用户裁决：导入/导出规则收进隐藏按钮）。
 *
 * 只改内边距：`AppButton` 的 sm 档是 `height 28 / padding 0 12`，图标-only 时
 * 左右各 12 会让它比文字按钮「刷新」(53px) 还宽，看起来像个空按钮 ⇒ 收到 8。
 * 其余（高度、圆角、ghost 配色、sm 档命中区 `::after` 补偿）全部继承 AppButton，
 * 不另写一套 —— 控件规范 §5.1 的「视觉 28 / 命中 ≥32」靠的就是那个伪元素。
 *
 * 同款图形在 `GameManager/SteamPage.tsx:455` 已用过（IconButton + EllipsisVertical），
 * 但那里是卡片行内的 `variant="outline"`；品牌区这一排是 ghost/secondary，
 * 用 outline 会凭空多一圈描边 ⇒ 这里用 ghost。
 */
const MoreTrigger = styled(AppButton)`
  padding: 0 var(--aegis-space-sm);
`;

/**
 * 两栏：左栏规则 340px 固定，右栏自适应（布局稿 §一）。
 *
 * 🔴 `align-items` **不能用 `start`**（2026-10-05 实测修）：
 * 那样两栏各自按内容高收缩，右栏在「无快照/加载中」时只有 34px 高
 * （实测值），视觉上是个瘪窄条，而它其实是主工作面（快照表格）。
 * 改为默认 `stretch` 让两栏等高；高度由 `flex: 1` 从 Page 的剩余空间来。
 */
const Columns = styled.div`
  display: grid;
  grid-template-columns: 340px minmax(0, 1fr);
  gap: var(--aegis-space-lg);
  /* 🔴 占满 Page 的剩余高度：Page 是 column flex，本项须显式 flex:1 才会被拉伸 */
  flex: 1 1 0;
  min-height: 0;

  /* 🔴 窗口宽 1240（最小）时内容区不足 1156 ⇒ 左栏 340 + 间距 16 会把右栏
     压到 800 以下，快照表格（5 列）开始折行。窄屏下改为单列纵向排列。
     断点取 1100：低于它就单列，宁可多滚一屏也不要让表格挤成两行。
     ⚠️ 单列时两栏**不能**再各占满高度（否则页面高度翻倍）⇒ 回到 auto。 */
  @media (max-width: 1100px) {
    grid-template-columns: minmax(0, 1fr);
    flex: 0 0 auto;
    min-height: 0;
  }
`;

/* ══ 规则卡片 ══════════════════════════════════════════════════════════ */

/**
 * 栏容器：把 `Panel` 变成「标题固定 + 内容区独立滚动」的整高栏。
 *
 * 🔴 **为什么不直接改 `Panel`**：它是全站共用语汇（Overview 五个面板都在用），
 *   给它加 `display:flex; height:100%` 会波及那些**本就该按内容高**的面板。
 *   只在本页包一层，作用域可控。
 *
 * 🔴 `min-height: 0` 是 flex 列滚动的前提：没有它，内容的 min-content 高
 *   会把容器顶开，`overflow-y: auto` 永远不生效（这是 flex 布局的经典坑）。
 */
const Column = styled.div`
  display: flex;
  flex-direction: column;
  min-height: 0;
  height: 100%;

  > section {
    display: flex;
    flex-direction: column;
    min-height: 0;
    /* 🔴 默认**按内容收高**（2026-10-06 UI 审查 m3 的落点）。
       原先无条件 1 1 auto ⇒ 空态 / 失败态下卡片仍被顶满整栏，中部出现
       实测 247px（800 高窗口）/289px（取证窗口）的空白，把卡片劈成两截。 */
    flex: 0 1 auto;
  }

  /* 🔴 只有**栏里真的是那条规则列表（ul）**时才吃满剩余高度：这时列表才需要
     「撑满卡片 + 内部滚动」，底栏也才该贴在卡底。用 DOM 形状当判据而不是用
     isPending/isError/isSuccess 分支去摆高度 —— 状态怎么组合都不需要再改这里。
     :has 与本项目已在用的 color-mix 同代（WebView2 / Chromium 105+）。
     🔴 data-grow（2026-10-07）：右栏的日志面板不是列表，但它是**唯一**该吃
     剩余空间的分区（用户裁决「日志区下面的空间给日志区」）。上面那条 m3 通配
     （flex: 0 1 auto）的特异性 (0,1,1) 高于日志组件自己写的 (0,1,0) ⇒ 组件内
     声明被静默吃掉。与 data-fixed 同一套办法：**由布局属主显式放行**，
     不在被放行方那边用更高特异性硬压。 */
  > section:has(> ul),
  > section[data-grow] {
    flex: 1 1 auto;
  }

  /* 🔴 标题固定、内容滚动：直接给「SectionTitle 之后的那个兄弟节点」加滚动。
     用 \`> section > *:not(h2)\` 选中内容块，标题（h2）自然留在原地。
     规则少的页面内容不足一屏时 max-height 也不会造成空白滚动条。
     「:not([data-fixed])」（2026-10-06）：本条是通配选择器，**特异性 (0,1,2) 高于
     styled 组件自己的 (0,1,0)** —— 固定操作条若被它命中，「padding-right」与
     「overflow-y」在组件内**改不掉**，居中会偏 2px 且条身被当成滚动区。
     故在此处排除，而不是在组件里用更高特异性硬压。
     ⚠️ 本注释内不得出现反引号（会提前终止 template literal 报 TS1005）。 */
  > section > *:not(h2):not([data-fixed]) {
    overflow-y: auto;
    min-height: 0;
    /* 🔴 滚动容器需要自己的 padding 承载，Panel 的 padding 在外层不会跟着内容滚 */
    padding-right: var(--aegis-space-xs);
  }
`;

const RuleList = styled.ul`
  list-style: none;
  margin: 0;
  padding: 0;
  display: flex;
  flex-direction: column;
  gap: var(--aegis-space-sm);
  /* 🔴 由**列表自己**吃掉栏内剩余高度（2026-10-06 UI 审查 m3，与 Column 的
     :has(> ul) 配套成两半）：卡片在有列表时才吃满整栏，此时剩余高度由本元素吸收，
     底栏照旧贴卡底 —— 与改前逐像素一致。
     此前是操作条用 margin-top auto 去顶到底部，而卡片又无条件撑满整栏 ⇒
     空态 / 失败态 / 只有一条规则时同样成立，于是把卡片从中间劈开：实测底栏
     上方空白 **247px**（800 高窗口）/ 289px（报告取证时的窗口）。
     ⚠️ 本注释内不得出现反引号（会提前终止 template literal 报 TS1005）。 */
  flex: 1 1 0;
  min-height: 0;
`;

const RuleCard = styled.li<{ $selected: boolean; $disabled: boolean; $isDropTarget: boolean }>`
  /* 🔴 停用态只降透明度、不隐藏 —— 隐藏会让人以为规则丢了（布局稿 §五.1） */
  opacity: ${(p) => (p.$disabled ? 0.6 : 1)};
  border: 1px solid ${(p) => (p.$selected ? "var(--aegis-accent-strong)" : "var(--aegis-hairline)")};
  border-radius: var(--aegis-radius-sm);
  background: var(--aegis-canvas);
  padding: var(--aegis-space-sm) var(--aegis-space-md);
  cursor: pointer;
  display: flex;
  flex-direction: column;
  gap: var(--aegis-space-xs);
  position: relative;
  transition:
    border-color var(--aegis-motion-duration-fast) var(--aegis-motion-easing-standard),
    background-color var(--aegis-motion-duration-fast) var(--aegis-motion-easing-standard);

  &:hover {
    background: var(--aegis-surface-soft);
  }
  /* 🔴 焦点环用 focus-ring 语义键（UI 规范 §八.1：禁 outline:none 无替代） */
  &:focus-visible {
    outline: 2px solid var(--aegis-focus-ring);
    outline-offset: 2px;
  }

  /* 🔴 拖拽落位指示（F-FB-08 的可发现性）：此前整张卡draggable 却**零反馈**，
     用户根本不知道能拖。
     用 ::before 画顶部 2px accent 条而不是改 border-top —— 后者会让卡片
     「长高 2px」，拖动过程中整列跟着抖动。规范 §一.3 亦禁阴影，故不走 inset。*/
  &::before {
    content: "";
    position: absolute;
    top: -1px;
    left: var(--aegis-space-sm);
    right: var(--aegis-space-sm);
    height: 2px;
    background: transparent;
    pointer-events: none;
  }

  ${(p) =>
    p.$isDropTarget &&
    `
    &::before {
      background: var(--aegis-accent);
    }
  `}
`;

const RuleHead = styled.div`
  display: flex;
  align-items: center;
  gap: var(--aegis-space-sm);
`;

const RuleName = styled.span`
  font-weight: 700;
  font-size: var(--aegis-type-body-size);
  color: var(--aegis-ink);
  /* 长规则名省略而不是撑破卡片 */
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  flex: 1 1 auto;
`;

const RuleMeta = styled.div`
  display: flex;
  align-items: center;
  gap: var(--aegis-space-xs);
  flex-wrap: wrap;
  font-size: var(--aegis-type-caption-size);
  color: var(--aegis-mute);
`;

/* ══ 快照表格 ══════════════════════════════════════════════════════════ */

const FailureList = styled.ul`
  list-style: none;
  margin: var(--aegis-space-xs) 0 0 0;
  padding: 0;
  display: flex;
  flex-direction: column;
  gap: 2px;
  font-size: var(--aegis-type-caption-size);
  color: var(--aegis-error);
`;

const PathText = styled.span`
  color: var(--aegis-mute);
  font-family: var(--font-mono);
`;

/** 标题行：标题 + 右侧计数（母本形态）。 */
const TitleRow = styled.div`
  display: flex;
  align-items: baseline;
  justify-content: space-between;
  gap: var(--aegis-space-sm);
`;

const CountNote = styled.span`
  font-size: var(--aegis-type-caption-size);
  color: var(--aegis-mute);
  font-variant-numeric: tabular-nums;
`;

/**
 * 快照表上方的操作条（作用于**选中行**）。
 *
 * 🔴 为什么把「校验 / 删除」从行内挪上来：① 行内 4 个按钮挤在 280px 里，
 *   「打开目录」文字被裁（用户截图实证）；② 破坏性操作（删除）贴在每一行旁边，
 *   密集列表里是误点高危区；③ 顺带让「选中行」有真实使用场景 ——
 *   规范 §三 `table` 的「选中行 = 左侧 2px accent-strong 竖条」此前全站无处落地。
 */
const SnapToolbar = styled.div`
  display: flex;
  align-items: center;
  gap: var(--aegis-space-xs);
  flex-wrap: nowrap;
  margin-bottom: var(--aegis-space-sm);
`;

/** 工具条右侧的「当前操作对象」提示 —— 没有它用户不知道按钮作用在哪一行。 */
const SnapTarget = styled.span`
  margin-left: auto;
  font-size: var(--aegis-type-caption-size);
  color: var(--aegis-mute);
  font-variant-numeric: tabular-nums;
`;

/** 页头工具条里的竖向分隔线（母本同款：把「对数据做动作」与「对规则集合做动作」分开）。 */
/* 动作槽里的分组分隔线（「对已有数据做动作」‖「对规则集合做动作」）。
   🔴 间距算术要写清：壳层 Acts 的 gap 是 **12**，本线左右各再要 **4**
   ⇒ 按钮到线 16、线到按钮 16，而组内只有 12（实测 2026-10-06），分组节奏成立。
   `align-self: stretch` 在 Acts（align-items: center）里让本线拉到行高 28，
   min-height 20 是行内无按钮时的兜底。 */
const ToolbarDivider = styled.div`
  width: 1px;
  align-self: stretch;
  min-height: 20px;
  background: var(--aegis-hairline);
  margin: 0 var(--aegis-space-xs);
`;

/**
 * 规则卡片区**底部**的固定操作条（母本同款）。
 *
 * 🔴 用户 2026-10-06 裁决：操作按钮从卡片里挪出来。
 *   卡片内四个按钮会把每张卡片撑高一截，一屏放不下几条规则；
 *   挪到列表下方后，卡片只剩「名称 + 状态 + 计数」两行，一屏能多看一倍的规则。
 *   代价是「操作对象」从「这张卡片」变成「当前选中项」—— 按钮因此统一置灰逻辑
 *   （没有选中规则时可点但无意义，故 disabled）。
 */
const RuleFooterActions = styled.div`
  display: flex;
  align-items: center;
  /* 三键居中（用户 2026-10-06 裁决）。 */
  justify-content: center;
  gap: var(--aegis-space-sm);
  flex-wrap: nowrap;
  /* 🔴 2026-10-06 UI 审查 m3：**随内容**，不再钉底。
     生长权交给 RuleList（理由与实测空白数见那里）：有列表时它把本条顶到栏底，
     与改前逐像素一致；空态 / 失败态时本条紧跟内容，不再把卡片从中间劈开。
     ⚠️ 本注释内不得出现反引号（会提前终止 template literal 报 TS1005）。 */
  margin-top: var(--aegis-space-sm);
  /* 不参与收缩：本栏唯一可滚的是内容块，操作条高度恒定。 */
  flex: none;
  padding-top: var(--aegis-space-sm);
  border-top: 1px solid var(--aegis-hairline);
`;

/**
 * 右栏的快照区容器。
 *
 * 🔴 日志面板移进右栏后（用户 2026-10-06 裁决：日志不该横贯整个内容区），
 *   右栏变成「快照表格 + 日志（占剩余）」两段。
 *   `min-height: 0` 缺了会让快照区**撑开**而不是收缩 —— 这是 flex 子项的经典塌陷，
 *   本项目已踩过（见 `Column` 的注释）。
 *
 * 🔴 **高度不再按剩余空间拉伸**（2026-10-06 用户裁决：「快照区高度固定，
 *   高度设置为标题行高度 + 七条内容行高度」）。此前它是 `flex: 1 1 0` 吃掉
 *   日志之外的全部剩余 ⇒ 实测 307px，而 307px 只装得下约 4 条数据行，
 *   且空态/错误态时那 307px 里大半是空白 —— 高度由**偶然**决定，不由内容决定。
 *   改为**按内容高**（= 下面算出的固定预算），既不抢日志的空间、也不被日志抢；
 *   溢出全部由日志吸收（`log-panel.tsx` 的 Shell 同步改为可收缩项）。
 *
 * 🔴 **2026-10-07 用户复核：预算里不该有状态条**。裁决原文是「标题行 + 7 条内容行」，
 *   而 2026-10-06 落地时把状态条（37px）也算进了预算 ⇒ 面板比口径多一行。
 *   状态条整块已删（四个结局各自的归宿见组件区 2026-10-07 那条注释），预算回到 429。
 *   ⚠️ 本注释内不得出现反引号（会提前终止 template literal 报 TS1005）。
 */
const SnapshotArea = styled.div`
  /* ── 五个分量全部由**被测量对象的 CSS 声明**推得，不是量 rect 得来的
     （rect 在本机 150% 缩放下有 1/3px 抖动：如表头实测 34.33 而非 35、标题 29.67 而非 30，
     故下面每个分量都取"声明值之和"，再拿实测校验 —— 2026-10-07 就是靠这一步发现 row 预算写小了）。
     chrome  34 = Panel 上下 padding 16+16 + 上下边框 1+1
     title   30 = h2 行高 20×1.25=25 + padding-bottom 4 + 下边框 1
     toolbar 36 = AppButton sm 高 28 + SnapToolbar margin-bottom 8
     head    35 = Th 上下 padding 8+8 + caption 行高 12×1.5=18 + thead 下边框 1
     row     42 = Td 上下 padding 8+8 + **行盒 25** + tr 下边框 1
                  🔴 行盒不是 21：状态/校验两列放的是 20px 高徽章，
                  display:inline-flex 配 vertical-align:baseline ⇒ 徽章底缘落在基线上，
                  行盒要再吃下约 4px 下伸空间（实测 25，非 21）。
                  本值曾写 38（按纯文字行推），真数据实测行高 41.7 才暴露——
                  预算写小 ⇒ 「七行」实际只装得下约 6.3 行，表格比设计更早开始滚动。
                  （末行按 Tbody 规则无边框，42 于是留了 1px 余量 —— 宁多不少，
                   写小会立刻长出滚动条。）
     合计 34+30+36+35+7×42 = **429**。
     ⚠️ 本注释内不得出现反引号（会提前终止 template literal 报 TS1005）。 */
  --snap-chrome-h: 34px;
  --snap-title-h: 30px;
  --snap-toolbar-h: 36px;
  --snap-head-h: 35px;
  --snap-row-h: 42px;
  /* 一屏要装下的数据行数 —— 改这一个数就能改口径（用户裁决当前为 7）。 */
  --snap-rows: 7;

  flex: 0 0 auto;
  min-height: 0;
  /* 🔴 **不参与收缩**（2026-10-06 实测纠正）：先写成「0 1 auto」，日志灌满 40 条时
     两个子项按基数比例一起缩 —— 快照区被从 438 压回 307，「固定 7 行」当场失效。
     优先级必须是**快照区恒定、日志吸收溢出**，故这里 grow/shrink 全 0，
     让日志（Shell 的「flex: 0 1 auto」）成为唯一可收缩项。
     「max-height: 100%」是极端矮窗口（640 高时内容区仅约 504）的兜底：
     收缩权交给面板自己（下方「> *」规则的 overflow-y），不让它顶穿右栏压在日志上。
     ⚠️ 本注释内不得出现反引号（会提前终止 template literal 报 TS1005）。 */
  max-height: 100%;
  display: flex;
  flex-direction: column;

  > * {
    /* 不再 grow：面板高 = 上面的预算，而不是「剩余空间」。
       固定高覆盖**两种**状态（用户 2026-10-06 裁决「同一高度」）：
       ① 有内容行 —— 表格本来的样子；② 取数中 —— 「还不知道有几行」不等于「没有行」，
       塌下去会在每次切规则时弹一下（主工作面抖动）。
       高度钉在面板上而不是钉在表格里，故骨架屏期间轮廓也完整。
       🔴 「已确认没有内容行」（空 / 失败 / 未选）另走下面的 data-empty：
       预算说的是「7 条内容行的工作区」，那三种状态压根没有内容行可言。 */
    flex: 0 1 auto;
    min-height: 0;
    height: calc(
      var(--snap-chrome-h) + var(--snap-title-h) + var(--snap-toolbar-h) + var(--snap-head-h) +
        var(--snap-rows) * var(--snap-row-h)
    );
    /* 🔴 极端矮窗口（最小 1240×640 时内容区实测约 504px）下的兜底：
       预算高于可用高度时由**面板自己**收缩并内部滚动，
       不让它顶穿右栏、压在日志上。 */
    max-height: 100%;
    overflow-y: auto;
  }

  /* 🔴 m7 修正（2026-10-07 实测）：**无数据时按内容高，而不是按一条算错的固定预算**。
     原写法把「空态高」写成 chrome+title+toolbar+head = 135，但空态里根本**没有**
     toolbar 与表头 —— 那里渲染的是 EmptyStateBlock（图标 + 一句 + CTA，实测内容 226）。
     135 于是把 CTA 裁掉、并长出滚动条（真机 04:31 删除全部规则后当场复现）。
     改为 auto：m7 的本意（不预留 7 行的空洞）仍然成立，且不需要猜空态有多高。
     ⚠️ 本注释内不得出现反引号（会提前终止 template literal 报 TS1005）。 */
  &[data-empty="true"] > * {
    height: auto;
  }
`;

/* ══ 组件 ══════════════════════════════════════════════════════════════ */

export function FileBackupPage() {
  const { t } = useTranslation("backup");
  const [selectedId, setSelectedId] = useState<string | null>(null);
  const [editorState, setEditorState] = useState<{ open: boolean; rule: RuleView | null }>({
    open: false,
    rule: null,
  });
  const [restoreTarget, setRestoreTarget] = useState<SnapshotView | null>(null);
  /**
   * 当前选中的**快照行**（表格操作的对象）。
   *
   * 🔴 2026-10-06 布局整改：行内 4 个按钮（打开目录/校验/恢复/删除）把操作列
   *   挤到文字被裁切（截图里「打开目录」被切掉右边）。改为「整行选中 + 表上方工具条」：
   *   ① 行内零按钮 ⇒ 表格干净且按钮文字永不被裁；
   *   ② 顺带落实规范 §三 `table` 的「选中行：左侧 2px accent-strong 竖条」——
   *      该条此前全站没有任何表格用得上（表格行此前不可选中）。
   */
  const [selectedSnapshot, setSelectedSnapshot] = useState<SnapshotView | null>(null);
  /**
   * 最近一次发起的备份对象名（供结果日志用）。
   *
   * 🔴 后端 `BackupOutcome` 不回传对象名，而日志必须说清「是哪条规则的结果」——
   *   没有它只能写「备份完成」，规则一多就无法分辨是哪一条。
   */
  const [lastBackupLabel, setLastBackupLabel] = useState("");
  /** 行内展开的失败明细：key = `ruleId/snapshotDir` */
  const [expanded, setExpanded] = useState<Record<string, FailureRow[]>>({});

  const sub = useMemo(() => t("page.sub", { ns: "backup" }), [t]);
  /* 🔴 动作槽要引用下面定义的 handler，注册调用一并挪到 `return` 之前
     （见 brandActions）；只留副标题在这里算。 */

  const rules = useBackupRules();
  const selected = useMemo<RuleView | null>(() => {
    const list = rules.data?.rules ?? [];
    if (selectedId !== null) {
      const hit = list.find((r) => r.id === selectedId);
      if (hit) return hit;
    }
    // 🔴 选中的规则被删了 ⇒ 回落到第一条，而不是让右栏空着
    return list[0] ?? null;
  }, [rules.data, selectedId]);

  const snapshots = useSnapshots(selected ? selected.id : null);

  // 🔴 换规则 / 重新取数后，若选中的快照已不在列表里则清空 ——
  //   否则工具条的「删除」会作用在一份当前不可见的快照上（用户以为删的是眼前这份）。
  useEffect(() => {
    setSelectedSnapshot((prev) => {
      if (prev === null) return null;
      const list = snapshots.data ?? [];
      return list.some((s) => s.displayPath === prev.displayPath) ? prev : null;
    });
  }, [snapshots.data]);

  // 🔴 规则列表变化后，若当前选中已不存在则清空选中态
  useEffect(() => {
    if (selectedId !== null && !(rules.data?.rules ?? []).some((r) => r.id === selectedId)) {
      setSelectedId(null);
    }
  }, [rules.data, selectedId]);

  const start = useStartBackup();
  const cancel = useCancelBackup();
  const removeRule = useDeleteRule();
  const removeSnapshot = useDeleteSnapshot();
  const verify = useVerifySnapshot();
  const refresh = useRefreshRules();
  const openDir = useOpenSnapshotDir();
  const toggleEnabled = useSetRuleEnabled();
  const exportRulesMutation = useExportRules();
  const importRulesMutation = useImportRules();
  const reorder = useReorderRules();
  // 🔴 拖拽源（F-FB-08）。**刻意不做乐观更新**：界面永远显示磁盘上的顺序 ——
  //   落盘失败时不需要任何「回退」逻辑（天然就是对的），
  //   也不会出现「界面顺序变了、磁盘没变」的中间态。
  const [dragId, setDragId] = useState<string | null>(null);
  /** 当前悬停的落点卡片（F-FB-08 可发现性：拖拽时要有「会插到这里」的指示）。 */
  const [dragOverId, setDragOverId] = useState<string | null>(null);
  /** 「刚刚发生过一次拖拽」标记：用来吃掉拖拽结束后浏览器补发的那次 click。 */
  const justDraggedRef = useRef(false);
  // 🔴 zustand 的 `push` 引用稳定（不随 entries 变化）⇒ 可安全放进任何依赖数组。
  const logPush = useLogStore((s) => s.push);

  const running = start.isPending;

  /**
   * 备份结果 → 操作日志（F-FB-48：备份是这一页最需要事后回看的动作）。
   *
   * 🔴 四个结局各有各的措辞，**不合并**：
   *   · 取消 → `info`（F-FB-19「取消不是失败」，落 error 会让用户以为出错了）；
   *   · 硬失败（命令抛错）→ `error`；
   *   · 部分失败 → `error` 且带失败项数（R-25：快照不完整就是失败，不能显示成成功）；
   *   · 全部成功 → `success` 并带文件数与体量。
   */
  useEffect(() => {
    if (lastBackupLabel === "") return;
    if (start.isError) {
      logPush(
        "error",
        t("log.backupFailed", {
          name: lastBackupLabel,
          message: asBackupError(start.error).message,
        }),
      );
      return;
    }
    if (!start.isSuccess) return;
    const outcome = start.data;
    if (outcome === undefined) return;
    if (outcome.cancelled) {
      logPush("info", t("log.backupCancelled", { name: lastBackupLabel }));
      return;
    }
    if (outcome.failureCount > 0) {
      logPush(
        "error",
        t("log.backupDoneWithFailures", {
          name: lastBackupLabel,
          failureCount: outcome.failureCount,
        }),
      );
    } else {
      logPush(
        "success",
        t("log.backupDone", {
          name: lastBackupLabel,
          fileCount: formatCount(outcome.fileCount),
          size: formatBytes(outcome.totalBytes),
        }),
      );
    }
    /* 🔴 保留策略裁掉的旧快照（2026-10-07 随状态条一并迁入日志）：这条信息原本**只**
       长在状态条上，删状态条时若不搬走，「备份顺手删了 N 份旧快照」就变成静默副作用 ——
       而 F-FB-31 的口径恰恰是「逐条留痕」。快照被裁后表格会少一行，日志要说清为什么。 */
    if (outcome.pruned.length > 0) {
      logPush("info", t("snapshot.pruned", { count: outcome.pruned.length }));
    }
  }, [start.isError, start.error, start.isSuccess, start.data, lastBackupLabel, logPush, t]);

  // 🔴 2026-10-07 用户裁决「记录损坏提示不要直接铺在界面上」：页面顶部的
  //   损坏警告 Panel（每条各占一整行）**移除**，改由操作日志（F-FB-48）承载。
  //
  // 🔴 **移除前必须先补上这条留痕**，否则「损坏」信息会彻底消失：
  //   先前 `rules.data.warnings` 只在 JSX 里渲染，从未写入日志。
  //   用 `useRef` 记已记过的条目，避免每次回源重复刷屏。
  const loggedWarningsRef = useRef<string[]>([]);
  useEffect(() => {
    const warnings = rules.data?.warnings ?? [];
    const seen = loggedWarningsRef.current;
    const fresh = warnings.filter((w) => !seen.includes(w));
    if (fresh.length === 0) return;
    loggedWarningsRef.current = [...seen, ...fresh];
    for (const warning of fresh) {
      logPush("error", `${t("warnings.title")}：${warning}`);
    }
  }, [rules.data?.warnings, logPush, t]);
  // ⚠️ 记录损坏按用户裁决**只进日志**（页面上不留痕）—— 这是对 R-18
  //   「记录损坏必须可见」的**显式豁免**。查阅路径：右栏「操作日志」面板
  //   （F-FB-48，永久留痕、可回看）。

  const handleVerify = (snapshot: SnapshotView): void => {
    verify.mutate(
      { ruleId: snapshot.ruleId, snapshotDir: snapshot.displayPath },
      {
        onSuccess: (report) => {
          logPush(
            "success",
            t("log.verifyDone", {
              snapshot: snapshot.displayPath,
              compared: report.compared,
              total: report.total,
            }),
          );
          setExpanded((prev) => ({
            ...prev,
            [`${snapshot.ruleId}/${snapshot.displayPath}`]: [
              {
                code: "REPORT",
                text: t("status.verifySummary", {
                  compared: report.compared,
                  total: report.total,
                }),
                path:
                  report.mismatched.length + report.missing.length === 0
                    ? t("status.integrity.passed")
                    : t("status.verifyMismatch", {
                        count: report.mismatched.length + report.missing.length,
                      }),
              },
            ],
          }));
        },
        onError: (error) => {
          const parsed = asBackupError(error);
          logPush(
            "error",
            t("log.verifyFailed", {
              snapshot: snapshot.displayPath,
              message: parsed.message,
            }),
          );
          setExpanded((prev) => ({
            ...prev,
            [`${snapshot.ruleId}/${snapshot.displayPath}`]: [
              { code: parsed.code, text: parsed.message, path: "" },
            ],
          }));
        },
      },
    );
  };

  /** 打开快照目录（F-FB-27）。成功无需提示（资源管理器已经弹出来了），失败必须留痕。 */
  const handleOpenDir = (snapshot: SnapshotView): void => {
    openDir.mutate(
      { ruleId: snapshot.ruleId, snapshotDir: snapshot.displayPath },
      {
        onError: (error) => {
          logPush("error", t("log.openDirFailed", { message: asBackupError(error).message }));
        },
      },
    );
  };

  /**
   * 单键切换规则启停（F-FB-07）。
   *
   * 🔴 日志措辞按**点击后**的目标态取（`rule.enabled` 是点击前的值）——
   *   写反了会让日志与磁盘状态相反，比不记还糟。
   */
  const handleToggleEnabled = (rule: RuleView): void => {
    toggleEnabled.mutate(
      { ruleId: rule.id, enabled: !rule.enabled },
      {
        onSuccess: () => {
          logPush(
            "success",
            rule.enabled
              ? t("log.ruleDisabled", { name: rule.name })
              : t("log.ruleEnabled", { name: rule.name }),
          );
        },
        onError: (error) => {
          logPush(
            "error",
            t("log.ruleToggleFailed", {
              name: rule.name,
              message: asBackupError(error).message,
            }),
          );
        },
      },
    );
  };

  /**
   * 删除一份快照（F-FB-30：二次确认列出路径、声明不可撤销）。
   *
   * 🔴 从行内挪到表上方工具条后，**操作对象变成了「选中的那一行」**，
   *   所以确认文案里必须带上时间 —— 只说「删除这份快照」时，
   *   用户在 8 行的表里分辨不出指的是哪一份。
   */
  /* 🔴 确认对话框的唯一承载件（§三 dialog 行；UI 审查 B3）。
     本文件原有 3 处原生 `window.confirm`（删除快照 / 覆盖导入 / 删除规则）已全部改调它 ——
     原生框不可令牌化、焦点与朗读不受控、遮罩下仍可 Tab 到，承担不了「危险操作四步」里
     「确认」这一步的红线承载角色。 */
  const { ask, dialog } = useConfirmDialog();

  const handleDeleteSnapshot = (snapshot: SnapshotView): void => {
    /* 🔴 原生 window.confirm 已下线（2026-10-06 UI 审查 B3）：删除快照不可撤销，
       必须走 §三 dialog 承载件 —— 初始焦点落「取消」、Esc=取消不直通执行、遮罩下不可 Tab，
       且确认文案里的**路径预览**留在正文（F-FB-30 的口径不变）。 */
    ask({
      title: t("snapshot.deleteConfirmTitle"),
      body: t("snapshot.deleteConfirmBody", { path: snapshot.displayPath }),
      confirmLabel: t("dialog.delete"),
      cancelLabel: t("dialog.cancel"),
      tone: "danger",
      onConfirm: () => {
        removeSnapshot.mutate(
          { ruleId: snapshot.ruleId, snapshotDir: snapshot.displayPath },
          {
            onSuccess: () => {
              logPush("success", t("log.snapshotDeleted", { snapshot: snapshot.displayPath }));
              setSelectedSnapshot(null);
            },
            onError: (error) => {
              logPush(
                "error",
                t("log.snapshotDeleteFailed", {
                  snapshot: snapshot.displayPath,
                  message: asBackupError(error).message,
                }),
              );
            },
          },
        );
      },
    });
  };

  /**
   * 导出规则（F-FB-14）：对话框选路径 → 落盘 → 日志报**条数与路径**。
   *
   * 🔴 默认文件名给 `.json`：后端只接受该扩展名并会拒绝其它值，
   *   而用户从对话框里选一个 `.txt` 会得到一句拒绝 —— 不如先把默认值给对。
   */
  const handleExport = async (): Promise<void> => {
    const picked = await saveDialog({
      title: t("export.choose", { ns: "backup" }),
      defaultPath: "aegis-backup-rules.json",
      filters: [{ name: "JSON", extensions: ["json"] }],
    });
    if (picked === null) return;
    exportRulesMutation.mutate(
      { targetPath: picked },
      {
        onSuccess: (result) => {
          logPush(
            "success",
            t("log.exported", { count: formatCount(result.count), path: result.path }),
          );
        },
        onError: (error) => {
          logPush("error", t("log.exportFailed", { message: asBackupError(error).message }));
        },
      },
    );
  };

  /**
   * 导入规则（F-FB-15，**合并式**）。
   *
   * 🔴 合并会**覆盖同标识的既有规则** ⇒ 必须先把后果讲清再确认，
   *   不能只弹一句「确定吗」。
   */
  const handleImport = async (): Promise<void> => {
    const picked = await openDialog({
      title: t("import.choose", { ns: "backup" }),
      multiple: false,
      filters: [{ name: "JSON", extensions: ["json"] }],
    });
    if (typeof picked !== "string") return;
    /* 🔴 原生 window.confirm 已下线（UI 审查 B3）。导入会**覆盖同标识规则**且无回滚 ⇒
       按「不可撤销」归 danger 实心（本文件口径：会销毁/覆盖既有数据 = danger，仅改状态 = primary）。
       三条合并口径原样留在正文，充当四步里的「预览」步。 */
    ask({
      title: t("dialog.importTitle"),
      body: t("import.confirmBody", { path: picked }),
      confirmLabel: t("dialog.importConfirm"),
      cancelLabel: t("dialog.cancel"),
      tone: "danger",
      onConfirm: () => {
        importRulesMutation.mutate(
          { sourcePath: picked },
          {
            onSuccess: (result) => {
              logPush(
                "success",
                t("log.imported", {
                  updated: formatCount(result.updated),
                  appended: formatCount(result.appended),
                  renamed: formatCount(result.renamed),
                }),
              );
              // 🔴 逐条问题**必须留痕**（F-FB-15「逐条问题给明细」）——
              //   静默导入等于骗人：用户不知道哪条被改了名。
              for (const issue of result.issues) logPush("info", issue);
            },
            onError: (error) => {
              logPush("error", t("log.importFailed", { message: asBackupError(error).message }));
            },
          },
        );
      },
    });
  };

  /** 拖拽落到目标规则上：重排后交后端持久化（F-FB-08）。 */
  const handleDrop = (targetId: string): void => {
    const source = dragId;
    setDragId(null);
    setDragOverId(null);
    if (source === null) return;
    const ids = (rules.data?.rules ?? []).map((r) => r.id);
    const from = ids.indexOf(source);
    const to = ids.indexOf(targetId);
    if (from < 0 || to < 0 || from === to) return;
    const next = [...ids];
    // 🔴 用 `[0]` + 显式判空而不是解构：`noUncheckedIndexedAccess` 下解构出
    //   `string | undefined`，而 `splice` 的插入位不接受它。
    const moved = next.splice(from, 1)[0];
    if (moved === undefined) return;
    next.splice(to, 0, moved);
    reorder.mutate(
      { orderedIds: next },
      {
        onError: (error) => {
          logPush("error", t("log.reorderFailed", { message: asBackupError(error).message }));
        },
      },
    );
  };

  /**
   * 备份**选中**的规则（母本「备份选中」，F-FB-16）。
   *
   * 🔴 从规则卡片挪到这里：卡片里塞四个按钮会把每张卡片撑高、一屏放不下几条规则
   *   （用户 2026-10-06 裁决）。「立即备份」的本质是「对选中项执行」，归到页头
   *   与「备份全部」并列才说得通。
   */
  const handleBackupSelected = (): void => {
    if (selected === null) return;
    logPush("info", t("log.backupStart", { name: selected.name }));
    setLastBackupLabel(selected.name);
    start.mutate({ ruleId: selected.id });
  };

  /**
   * 恢复选中规则的**最新一份**快照（母本「恢复选中」）。
   *
   * 快照列表是**倒序**的（F-FB-26）⇒ `[0]` 即最新。没有快照时如实报一条日志，
   * 不静默无反应 —— 按钮灰着和点了没反应同样让人困惑，所以保持可点、给出解释。
   */
  const handleRestoreSelected = (): void => {
    if (selected === null) return;
    const latest = snapshots.data?.[0];
    if (latest === undefined) {
      logPush("error", t("log.restoreNoSnapshot", { name: selected.name }));
      return;
    }
    setRestoreTarget(latest);
  };

  const enabledRules = (rules.data?.rules ?? []).filter((r) => r.enabled && !r.isDraft);
  const canBackupAll = enabledRules.length > 1 && !running;

  /* 🔴 2026-10-07 用户裁决：删掉快照表下方那条状态条（常驻一行「就绪」）。
     它承载的四个结局**每一处都另有归宿**，删掉不丢信息：
     · 进行中 → BackupProgress（运行期实时块，本就在快照区上方）
     · 成功 / 部分失败 / 取消 → 运行日志（F-FB-48 永久留痕，见上面的 useEffect）
     · 「就绪」本身零信息量 —— 它只是「什么都没有」的一种说法，却常年占一行高度，
       并被算进了快照区的固定预算，使面板比「标题行 + 7 条内容行」的口径多一行。
     唯一原本**只**活在状态条里的是「按保留策略清理了 N 份」，已折进日志（见 useEffect）。 */

  /* ── 品牌区动作槽（2026-10-06 用户裁决：操作按钮移到品牌区）────────────────
     原页头工具栏整组上移：「刷新 / 备份选中 / 恢复选中 / 备份全部 ‖ 新建 / 导入 / 导出」。
     页头那一行随之消失 ⇒ 内容区上移 44px（按钮 28 + Page gap 16）；
     快照区是固定高（见 SnapshotArea），多出来的高度落在下方的日志面板上。

     🔴 **这里不套 useMemo**（Overview 页那处套了）：本文件的 handler
        （refresh / handleBackupSelected / handleRestoreSelected / handleExport /
        handleImport）全是每次渲染新建的普通函数，`enabledRules` 也是每次重新
        filter 出来的新数组 ⇒ 依赖数组必然每次都不相等，memo 只会给出**假的安全感**。
        代价：页面每次渲染，BrandBar 跟着重渲染一次（store 是单向订阅，不会成环）。
        将来若给这些 handler 补上 useCallback，再回来加 memo。 */
  const brandActions = (
    <>
      <AppButton variant="ghost" size="sm" onClick={() => void refresh()}>
        {t("summary.refresh")}
      </AppButton>
      {/* 母本操作组：「备份选中 / 恢复选中 / 备份全部 ‖ 新建 / 导入 / 导出」。
          🔴 竖线分隔的是「对已有数据做动作」与「对规则集合做动作」两类操作。 */}
      <AppButton
        variant="secondary"
        size="sm"
        disabled={selected === null || running}
        onClick={handleBackupSelected}
      >
        {t("summary.backupSelected")}
      </AppButton>
      <AppButton
        variant="secondary"
        size="sm"
        disabled={selected === null || (snapshots.data?.length ?? 0) === 0}
        onClick={handleRestoreSelected}
      >
        {t("summary.restoreSelected")}
      </AppButton>
      <AppButton
        variant="secondary"
        size="sm"
        disabled={!canBackupAll}
        title={canBackupAll ? undefined : t("progress.stageUnknown")}
        onClick={() => {
          // 🔴 批量串行执行，标签带 [i/N] 以便日志追溯（F-FB-17）
          enabledRules.forEach((rule, index) => {
            const label = `[${index + 1}/${enabledRules.length}] ${rule.name}`;
            window.setTimeout(() => {
              start.mutate({ ruleId: rule.id, label });
            }, index * 50);
          });
        }}
      >
        {t("summary.backupAll")}
      </AppButton>

      <ToolbarDivider />

      {/* 规则集合操作（母本右半组：新建 / 导入 / 导出） */}
      <AppButton
        variant="primary"
        size="sm"
        onClick={() => setEditorState({ open: true, rule: null })}
      >
        {t("summary.newRule")}
      </AppButton>
      {/* 规则导入 / 导出（F-FB-14/15）—— 2026-10-06 用户裁决收进「⋯」隐藏按钮。
          两项的禁用条件与处理函数**原样保留**，只是从常显按钮变成菜单项：
          · 导入：正在导入时禁用
          · 导出：正在导出 或 一条规则都没有时禁用
          菜单项文案沿用 `import.button` / `export.button`（不新造键）。
          🔴 `Portal` 必须有：品牌区是 72px 定值高的 flex 条，菜单就地渲染会被
             条带裁掉（且 `Acts` 是 `flex:none`，不会为它长高）。 */}
      <Menu>
        <MenuButton
          as={MoreTrigger}
          variant="ghost"
          size="sm"
          aria-label={t("summary.more")}
          title={t("summary.more")}
        >
          <EllipsisVertical size={16} strokeWidth={1.5} aria-hidden="true" />
        </MenuButton>
        <Portal>
          <MenuList fontSize="sm" zIndex={9999}>
            <MenuItem
              isDisabled={importRulesMutation.isPending}
              onClick={() => {
                void handleImport();
              }}
            >
              {t("import.button", { ns: "backup" })}
            </MenuItem>
            <MenuItem
              isDisabled={exportRulesMutation.isPending || (rules.data?.ruleCount ?? 0) === 0}
              onClick={() => {
                void handleExport();
              }}
            >
              {t("export.button", { ns: "backup" })}
            </MenuItem>
          </MenuList>
        </Portal>
      </Menu>
    </>
  );
  useRegisterBrand("/file-backup", { sub, actions: brandActions });

  return (
    <Page>
      {/* 页头不再有工具栏：操作组已上移到品牌区（见上方 brandActions）。
          原先那行「规则 N 条 · 快照 N 份」汇总于 2026-10-06 按用户裁决删除 ——
          规则条数 = 左栏卡片数、每条规则的快照数写在卡片上，不必再汇总一遍。 */}
      {/* 🔴 2026-10-07 用户裁决**删除**此处「记录损坏」警告 Panel（原逐条各占一整行）。
          原因同概览页横幅：常驻铺在内容上方会把下方两栏整体下移。
          改由右栏「操作日志」承载 —— 见上方 effect：warnings 先写进日志再删掉这条渲染。
          ⚠️ 这是对 R-18「记录损坏必须可见」的**显式豁免**（用户拍板「只进日志，页面不留痕」）。*/}

      {/* ── 备份运行期进度区 ── */}
      {/* 🔴 用户 2026-10-06 裁决：备份完成的横幅**删除** —— 它横贯页头下方、
          白占一整行高度，挤的是「规则列表 + 快照表格」的可用空间。
          结果信息改由两处承载：① 快照表格下方的状态条（常驻、只一行）；
          ② 操作日志（F-FB-48，永久留痕、可回看）。
          此处只保留**运行中**的进度区 —— 那条是必要的实时反馈，不是结果陈列。
          ⚠️ 2026-10-06 修：这同一个 `{running && <BackupProgress …>}` 此前**写了两遍**
          （两个块条件、props 全同），备份运行中会上下叠出两块进度区。
          🟡 浏览器预览模式无后端，`running` 恒 false ⇒ 该缺陷在预览里看不见，是读代码发现的。 */}
      {running && (
        <BackupProgress onCancel={() => cancel.mutate()} cancelPending={cancel.isPending} />
      )}

      {/* ── 两栏主体 ── */}
      <Columns>
        {/* 左栏：规则列表 */}
        <Column>
          <Panel as="section">
            {/* 🔴 标题是「这一栏是什么」，不是按钮文案 —— 早前误用了
              summary.newRule，读起来像在标题位置放了个按钮。 */}
            <SectionTitle>{t("summary.ruleListTitle")}</SectionTitle>
            {rules.isPending && <LoadingSkeleton rows={3} />}
            {rules.isError && (
              <>
                <ErrorStateBlock
                  title={t("errors.loadRules")}
                  /* 🔴 主线只说「为什么」；后端命令名 backup_list_rules 收进折叠
                     （2026-10-06 UI 审查 D7 / 报告 m6）。 */
                  body={failureReason(rules.error)}
                  detail={failureDetail(rules.error)}
                  onRetryLabel={t("errors.retry")}
                  onRetry={() => void refresh()}
                  /* 🔴 描边重试（2026-10-06 UI 审查 M3）：品牌区已有实心绿「新建规则」，
                     失败态再给实心就是同屏两枚主 CTA 争权重（§一 #1 实心绿 ≤ 1）。
                     语义不变、仍可点，只是让位给工具栏 —— prop 与 Overview 页同用法。 */
                  retryVariant="secondary"
                />
                {/* 🔴 失败态保留列表轮廓（UI 审查 m3）：错误块下面接三根**静止**占位行，
                    读屏不参与（骨架件在失败态不带 aria-busy）。它顶住的是「这一栏本该是一列规则」
                    的版面，而不是谎称在加载 —— 所以 busy=false。 */}
                <LoadingSkeleton rows={3} busy={false} />
              </>
            )}
            {rules.isSuccess && rules.data.rules.length === 0 && (
              <EmptyStateBlock
                titleKey="rule.empty.title"
                bodyKey="rule.empty.body"
                ns="backup"
                actionLabel={t("rule.empty.action")}
                onAction={() => setEditorState({ open: true, rule: null })}
                /* 品牌区已有实心「新建规则」，同屏不能再有第二枚实心绿（§一 #1 / D8）
                   ⇒ 空态动作降描边：语义不变、仍可点，只是不抢主 CTA。 */
                actionVariant="secondary"
              />
            )}
            {rules.isSuccess && rules.data.rules.length > 0 && (
              <RuleList>
                {rules.data.rules.map((rule) => (
                  <RuleCard
                    key={rule.id}
                    $selected={selected?.id === rule.id}
                    $disabled={!rule.enabled}
                    $isDropTarget={dragOverId === rule.id && dragId !== rule.id}
                    tabIndex={0}
                    role="button"
                    aria-pressed={selected?.id === rule.id}
                    // 🔴 拖拽排序（F-FB-08）。`draggable` 让整张卡片可拖，
                    //   拖到另一张上即插到它之前；`onDragEnd` 兜底清源（拖到列表外也要清）。
                    draggable
                    onDragStart={(e) => {
                      justDraggedRef.current = true;
                      setDragId(rule.id);
                      e.dataTransfer.effectAllowed = "move";
                    }}
                    onDragOver={(e) => {
                      // 🔴 必须 preventDefault，否则浏览器不接受 drop（默认不允许放）。
                      e.preventDefault();
                      e.dataTransfer.dropEffect = "move";
                      setDragOverId(rule.id);
                    }}
                    onDrop={(e) => {
                      e.preventDefault();
                      handleDrop(rule.id);
                    }}
                    onDragEnd={() => {
                      setDragId(null);
                      setDragOverId(null);
                    }}
                    onClick={() => {
                      // 🔴 拖拽结束后浏览器仍会补一次 click —— 直接跳过一次，
                      //   否则「把 A 拖到 B 上」会顺带把 B 选中（意外的选中切换）。
                      if (justDraggedRef.current) {
                        justDraggedRef.current = false;
                        return;
                      }
                      setSelectedId(rule.id);
                    }}
                    onKeyDown={(e) => {
                      if (e.key === "Enter" || e.key === " ") {
                        e.preventDefault();
                        setSelectedId(rule.id);
                      }
                    }}
                  >
                    <RuleHead>
                      <RuleName title={rule.name}>{rule.name}</RuleName>
                      <EnabledBadge enabled={rule.enabled} />
                      {rule.isDraft && <DraftBadge />}
                    </RuleHead>
                    <RuleMeta>
                      <span>{t("rule.sourceCount", { count: rule.sources.length })}</span>
                      <span>·</span>
                      <span>{t("rule.keepCount", { count: rule.keepCount })}</span>
                      <span>·</span>
                      {/* 🔴 逐规则口径（2026-10-06 补）：这条规则**自己**攒了几份、
                          最近什么时候备的。此前卡片只有配置项（来源数 / 保留份数），
                          用户判断「该管哪条规则」只能点进去看 —— 而页头那个是总数，
                          除以条数毫无意义。后端已在扫目录时一并算出（零额外 IO）。 */}
                      <span>{t("rule.snapshotCount", { count: rule.snapshotCount })}</span>
                      <span>·</span>
                      <span>
                        {rule.latestSnapshotAt > 0
                          ? t("rule.latestBackup", {
                              time: formatSnapshotTime(rule.latestSnapshotAt),
                            })
                          : t("rule.neverBackedUp")}
                      </span>
                      {rule.isDraft && <span>· {t("rule.draftHint")}</span>}
                    </RuleMeta>
                  </RuleCard>
                ))}
              </RuleList>
            )}

            {/* 🔴 规则卡片区底部的固定操作条（母本同款）。
                  作用于**当前选中**的规则 —— 卡片里不再放按钮，一屏能多看一倍的规则。
                  「立即备份」不在这里：它归到页头的「备份选中」（与「备份全部」并列更合语义）。 */}
            <RuleFooterActions data-fixed>
              <AppButton
                variant="ghost"
                size="sm"
                disabled={selected === null}
                onClick={() => {
                  if (selected !== null) setEditorState({ open: true, rule: selected });
                }}
              >
                {t("rule.edit")}
              </AppButton>
              {/* F-FB-07：单键切换启停。停用**不隐藏**规则（隐藏会让人以为规则丢了），
                    只把卡片降透明度 —— 见 `RuleCard` 的 `$disabled`。 */}
              <AppButton
                variant="ghost"
                size="sm"
                disabled={selected === null || toggleEnabled.isPending}
                onClick={() => {
                  if (selected !== null) handleToggleEnabled(selected);
                }}
              >
                {selected?.enabled === true ? t("rule.disableAction") : t("rule.enableAction")}
              </AppButton>
              <AppButton
                variant="danger"
                size="sm"
                disabled={selected === null || running}
                onClick={() => {
                  if (selected === null) return;
                  /* 🔴 原生 window.confirm 已下线（UI 审查 B3）。F-FB-06 的口径不变：
                     「删除规则**不删快照**」必须留在正文里当预览步。 */
                  ask({
                    title: t("rule.deleteConfirmTitle", { name: selected.name }),
                    body: t("rule.deleteConfirmBody"),
                    confirmLabel: t("dialog.delete"),
                    cancelLabel: t("dialog.cancel"),
                    tone: "danger",
                    onConfirm: () => {
                      removeRule.mutate(selected, {
                        onSuccess: () => {
                          logPush("success", t("log.ruleDeleted", { name: selected.name }));
                        },
                        onError: (error) => {
                          logPush(
                            "error",
                            t("log.ruleDeleteFailed", {
                              name: selected.name,
                              message: asBackupError(error).message,
                            }),
                          );
                        },
                      });
                    },
                  });
                }}
              >
                {t("rule.delete")}
              </AppButton>
            </RuleFooterActions>
          </Panel>
        </Column>

        {/* 右栏：所选规则的快照表格 + 操作日志（两段） */}
        <Column>
          {/* 🔴 data-empty 的判据（2026-10-07 重算，三条缺一不可）：
              ① `!isFetching`：**取数中不是「没有内容行」，是「还不知道有几行」**。
                 少了这条，切规则时面板会先塌到骨架高再弹回 429 —— 主工作面每次点击都跳一次。
              ② `snapshots.data?.length ?? 0`：原写法 `=== 0` 在首帧 `data === undefined` 时为
                 false ⇒ 真·无数据被算成「有数据」，照样预留 7 行空洞（m7 想治的就是这个）。
              ③ `!selected`：没选规则时右栏本来就没有表格可言。
              三条合起来 = 「**已经知道这一栏没有内容行**」，与下面 data-empty 分支的 height auto 配对。 */}
          <SnapshotArea
            data-empty={!snapshots.isFetching && (!selected || (snapshots.data?.length ?? 0) === 0)}
          >
            <Panel as="section">
              {selected ? (
                <>
                  <TitleRow>
                    <SectionTitle>{t("snapshot.title", { name: selected.name })}</SectionTitle>
                    {/* 🔴 母本在标题旁给「共 N 个备份完成」的计数 ——
                      没有它用户无法判断这一屏是全部还是被滚动截断了。 */}
                    <CountNote>
                      {t("snapshot.totalCount", {
                        count: formatCount(snapshots.data?.length ?? 0),
                      })}
                    </CountNote>
                  </TitleRow>
                  {/* 🔴 `isPending` 在 `enabled:false`（未选规则）时**恒为 true** ——
                  TanStack Query 把「禁用且无数据」也算 pending。
                  所以骨架屏必须再加一层「确实在取」的条件，
                  否则未选规则时会永久显示一块假loading（本轮踩过）。
                  真正的取数中判定用 `isFetching`（它只看是否在飞）。 */}
                  {snapshots.isFetching && <LoadingSkeleton rows={4} />}
                  {snapshots.isError && (
                    <ErrorStateBlock
                      title={t("errors.loadSnapshots")}
                      body={failureReason(snapshots.error)}
                      detail={failureDetail(snapshots.error)}
                      onRetryLabel={t("errors.retry")}
                      onRetry={() => void snapshots.refetch()}
                      /* 同上：品牌区已有实心绿主操作，失败态重试走描边（§一 #1、M3） */
                      retryVariant="secondary"
                    />
                  )}
                  {snapshots.isSuccess && snapshots.data.length === 0 && (
                    /* 🔴 §三 空态三件套 = 图标 + 一句 + 主 CTA。原先只给图标+一句，
                       文案还指向一个**不存在的按钮**（「立即备份」——品牌区实际叫「备份选中」）。
                       现补上动作，且**沿用品牌区的同一个词**（一个动作只有一个名字）。
                       降描边理由同规则列表空态（§一 #1 / D8）。 */
                    <EmptyStateBlock
                      titleKey="snapshot.empty.title"
                      bodyKey="snapshot.empty.body"
                      ns="backup"
                      actionLabel={t("summary.backupSelected")}
                      onAction={handleBackupSelected}
                      actionVariant="secondary"
                    />
                  )}
                  {snapshots.isSuccess && snapshots.data.length > 0 && (
                    <>
                      <SnapToolbar>
                        {/* 🔴 收尾 2026-10-06 的既有裁决（见本文件 :477 与 :312 的整改记录）：
                            那次把"行内 4 个按钮挤到文字被裁"改成「整行选中 + 表上方工具条」，
                            但**只搬了校验 / 删除两个**，打开目录 / 恢复 仍留在行里 ⇒
                            操作列实测内容需 164px+，七列总需 **724px** > 最小窗口表格可用
                            **711px**，列宽怎么排都装不下（真数据审查 T1/T2 的算术根因）。
                            本次补齐后：① 「路径」列从被挤到的 71px 拿回 289px（原先显示成
                            2026…）；② 行高 44.7 → 41.7（行内 28px 按钮额外撑高 3px），并因此
                            暴露 --snap-row-h 预算写的 38 与真实 42 不符 —— 已在预算处改正；
                            ③ 交互模型与本条既有按钮一致。
                            ⚠️ 与品牌区「恢复选中」不是同一动作：那个恢复**最新一份**
                            （handleRestoreSelected 取 data[0]），本按钮恢复**当前选中那一份**，
                            是恢复非最新快照的唯一入口。 */}
                        <AppButton
                          variant="ghost"
                          size="sm"
                          disabled={selectedSnapshot === null || openDir.isPending}
                          onClick={() => {
                            if (selectedSnapshot !== null) handleOpenDir(selectedSnapshot);
                          }}
                        >
                          {t("snapshot.openDir")}
                        </AppButton>
                        <AppButton
                          variant="ghost"
                          size="sm"
                          disabled={selectedSnapshot === null || verify.isPending}
                          onClick={() => {
                            if (selectedSnapshot !== null) handleVerify(selectedSnapshot);
                          }}
                        >
                          {verify.isPending ? t("snapshot.verifyRunning") : t("snapshot.verify")}
                        </AppButton>
                        <AppButton
                          variant="secondary"
                          size="sm"
                          disabled={selectedSnapshot === null}
                          onClick={() => {
                            if (selectedSnapshot !== null) setRestoreTarget(selectedSnapshot);
                          }}
                        >
                          {t("snapshot.restore")}
                        </AppButton>
                        <AppButton
                          variant="danger"
                          size="sm"
                          disabled={selectedSnapshot === null || removeSnapshot.isPending}
                          onClick={() => {
                            if (selectedSnapshot !== null) handleDeleteSnapshot(selectedSnapshot);
                          }}
                        >
                          {t("snapshot.delete")}
                        </AppButton>
                        <SnapTarget>
                          {selectedSnapshot === null
                            ? t("snapshot.noneSelected")
                            : formatSnapshotTime(selectedSnapshot.createdUnixMs)}
                        </SnapTarget>
                      </SnapToolbar>
                      <DataTable>
                        <Thead>
                          <tr>
                            {/* 🔴 前五列宽度**全部由实测内容需求推得**（留 2~6px 呼吸），
                                「路径」列刻意不写宽度：`table-layout: fixed` 下未声明的列
                                吃掉余量 —— 路径长度无上界，本就该是唯一可伸缩的一列。
                                T1 的成因正是"六列定宽 + 一列自适应"把自适应列挤到 71px，
                                故这里同时约束**定宽总和 ≤ 最小窗口余量**（462 ≤ 711-144）。 */}
                            <Th $width="146px">{t("snapshot.columns.time")}</Th>
                            <Th $width="66px" $numeric>
                              {t("snapshot.columns.files")}
                            </Th>
                            <Th $width="78px" $numeric>
                              {t("snapshot.columns.size")}
                            </Th>
                            <Th $width="76px">{t("snapshot.columns.status")}</Th>
                            <Th $width="96px">{t("snapshot.columns.integrity")}</Th>
                            <Th>{t("snapshot.columns.path")}</Th>
                          </tr>
                        </Thead>
                        <Tbody>
                          {snapshots.data.map((snapshot) => {
                            const key = `${snapshot.ruleId}/${snapshot.displayPath}`;
                            const rows = expanded[key];
                            return (
                              <Tr
                                key={key}
                                $selected={selectedSnapshot?.displayPath === snapshot.displayPath}
                                tabIndex={0}
                                role="button"
                                aria-pressed={
                                  selectedSnapshot?.displayPath === snapshot.displayPath
                                }
                                onClick={() => setSelectedSnapshot(snapshot)}
                                onKeyDown={(e) => {
                                  // 🔴 规范 §8.1：键盘可达「以真实按键为准」——
                                  //   行必须能用 Enter/Space 选中，否则上方的
                                  //   校验/删除对键盘用户等于不存在。
                                  if (e.key === "Enter" || e.key === " ") {
                                    e.preventDefault();
                                    setSelectedSnapshot(snapshot);
                                  }
                                }}
                              >
                                <Td>
                                  {formatSnapshotTime(snapshot.createdUnixMs)}
                                  <FailureList>
                                    {rows?.map((row, i) => (
                                      <li key={`${row.code}-${i}`}>
                                        {row.text}
                                        {row.path && (
                                          <>
                                            {" "}
                                            <PathText>{row.path}</PathText>
                                          </>
                                        )}
                                      </li>
                                    ))}
                                  </FailureList>
                                </Td>
                                <Td $numeric>{formatCount(snapshot.fileCount)}</Td>
                                <Td $numeric>{formatBytes(snapshot.totalBytes)}</Td>
                                <Td>
                                  <StatusBadge status={snapshot.status} />
                                  {snapshot.failedCount > 0 && (
                                    <>
                                      {" "}
                                      <AppButton
                                        variant="ghost"
                                        size="sm"
                                        onClick={(e) => {
                                          // 🔴 必须止冒泡：否则「展开失败明细」会连带
                                          //   触发行选中，用户点了想看明细却改变了操作对象。
                                          e.stopPropagation();
                                          setExpanded((prev) => ({ ...prev, [key]: [] }));
                                        }}
                                      >
                                        {t("snapshot.failuresTitle", {
                                          count: snapshot.failedCount,
                                        })}
                                      </AppButton>
                                    </>
                                  )}
                                </Td>
                                <Td>
                                  <IntegrityBadge integrity={snapshot.integrity} />
                                </Td>
                                {/* 🔴 路径列（规格 §3.1 / F-FB-26）：完整落点，`title` 给悬停看全。
                                `Td` 自带 ellipsis，长路径不会把表格撑破。 */}
                                <Td title={snapshot.displayPath}>{snapshot.displayPath}</Td>
                              </Tr>
                            );
                          })}
                        </Tbody>
                      </DataTable>
                    </>
                  )}
                </>
              ) : (
                /* 🔴 右栏在「规则加载失败」时**不重复报空态**：
               左栏已经给出「读取规则失败 + 重试」，这里再补一句
               「还没有备份规则」会让用户以为是真的没有规则（两态互斥）。
               只有「加载成功且确实一条都没有」才显示引导新建的空态。 */
                rules.isSuccess && (
                  <EmptyStateBlock
                    titleKey="rule.empty.title"
                    bodyKey="rule.empty.body"
                    ns="backup"
                    actionLabel={t("rule.empty.action")}
                    onAction={() => setEditorState({ open: true, rule: null })}
                    /* 🔴 同 D8：这块的动作与品牌区「新建规则」是**同一个动作、同一个词**，
                       两处都实心 = 同屏两枚主 CTA。降描边（左栏那块同名空态早已带此 prop）。
                       🟡 预览不可达（要后端返回"加载成功且零条规则"），本条只有源码证据。 */
                    actionVariant="secondary"
                  />
                )
              )}
              {/* 🔴 2026-10-06 批次④：上面那块 `: (rules.isSuccess && …)` 的写法在
                  `rules.isError` 时**什么都不渲染** —— 快照区因此变成一块 438px 高的纯空洞，
                  既不空也不错，用户只看到"这块地方是空的"（违反「禁止静默失败」）。
                  现补第三种状态：明确说明快照为何缺席，并给同一次重试的入口。
                  放在 `selected` 三元之外、Panel 之内：未选规则 ⟂ 规则不可用是两回事。 */}
              {!selected && rules.isError && (
                <EmptyStateBlock
                  titleKey="snapshot.notLoaded.title"
                  bodyKey="snapshot.notLoaded.body"
                  ns="backup"
                  actionLabel={t("errors.retry")}
                  onAction={() => void refresh()}
                  /* 🔴 本块是 2026-10-06 补「第三种状态」时新加的，漏带 D8 那条既有裁定的
                     actionVariant="secondary"（同文件另四处状态块都已带）：品牌区已有实心绿
                     「新建规则」，分区内状态块再出实心就是同屏两枚主 CTA（§一 #1）。
                     截图实测见本批记录。 */
                  actionVariant="secondary"
                />
              )}
            </Panel>
          </SnapshotArea>

          {/* 🔴 操作日志面板（F-FB-48）。
              用户 2026-10-06 裁决：日志**不再横贯整个内容区** ——
              横贯的日志左边缘会与左栏对齐，而上方快照表的左边缘在更右边，
              两条左边缘对不齐，一眼看去就像「日志比表格宽出一截」。
              放进右栏后左右边缘与快照表严格对齐（母本同款）。 */}
          <LogPanel />
        </Column>
      </Columns>

      {/* ── 三个弹窗 ── */}
      <RuleEditorDialog
        open={editorState.open}
        rule={editorState.rule}
        onClose={() => setEditorState({ open: false, rule: null })}
      />
      <RestoreDialog snapshot={restoreTarget} onClose={() => setRestoreTarget(null)} />
      {/* 确认对话框（B3）：全站共用一个实例，由 ask(...) 携带各自的标题/正文/动作 */}
      {dialog}
    </Page>
  );
}

interface FailureRow {
  code: string;
  text: string;
  path: string;
}

export default FileBackupPage;
