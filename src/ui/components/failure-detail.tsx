import styled from "@emotion/styled";
import { ChevronDown } from "lucide-react";
import i18n from "@/i18n";
import { backendOfflineCommand, isBackendOffline } from "@/lib/tauri/invoke";

/* 失败态文案的两段拆分（2026-10-06 UI 审查 D7，报告条目 m6）。
 *
 * 🔴 起因：整屏 5 处用户可见文案直接把后端命令名摊在第一行
 *   （backup_list_rules / overview_live_sample / transfer_peers_list /
 *    get_steam_install_info / backup_get_settings）。命令名是**排查标识**，
 *   不是给用户读的话：它既非中文、又占掉一行的注意力，还让人以为是自己点错了什么。
 *   实测改前文案（/file-backup 主线）：
 *   「读取规则失败 | 后端不可用（当前不在 Tauri 运行时内，浏览器预览模式）：无法调用 backup_list_rules」
 *
 * 🔴 口径：**主线只说「为什么」，技术标识收进「详情」折叠**。
 *   折叠不是删除 —— 用户要报障时那串命令名仍是唯一的定位手段，日志里也有它。
 *   故这里拆成两个纯函数 + 一个折叠件，而不是把信息丢掉。
 *
 * 两个函数都不接 `t` 形参：它们要的词是**跨页共用**的失败态词，取 i18n 单例即可，
 * 且在渲染期调用 —— 换语言时宿主组件本就重渲染（都挂了 useTranslation），不会读到旧词。
 */

/** 取错误的可读原文（后端 reject 出来的是 `{ code, message }`，不是 Error）。 */
function messageOf(error: unknown): string {
  if (error instanceof Error) return error.message;
  if (typeof error === "object" && error !== null) {
    const m = (error as { message?: unknown }).message;
    if (typeof m === "string") return m;
  }
  return String(error);
}

/**
 * 失败态**主线**文案：后端没在跑时说「后端未运行」，其余照原样给错误文本。
 *
 * 🟢 为什么其余情况不也翻译一下：真后端错误的 message 是 Rust 侧按 01 专题 §2.3
 * 生成的中文原因句，本就面向用户；再套一层前端词条会变成「翻译的翻译」，
 * 且前端并不知道后端将来会回哪些原因。
 */
export function failureReason(error: unknown): string {
  return isBackendOffline(error) ? i18n.t("common:failure.backendOffline") : messageOf(error);
}

/**
 * 失败态**详情**文案（技术标识）；没有可折叠内容时返回 `null`。
 *
 * 🔴 参数须传**原始 error**，不要传 `as*Error()` 的归一化结果 ——
 * 归一化会丢掉命令名（只剩整句 message），届时这里退化成整句原文，
 * 折叠行会比主线还长。各失败态调用点本就持有原始 error，没有理由传归一化的。
 */
export function failureDetail(error: unknown): string | null {
  if (!isBackendOffline(error)) return null;
  const command = backendOfflineCommand(error);
  if (command === null) return messageOf(error);
  return i18n.t("common:failure.detailCommand", { command });
}

const Box = styled.details`
  /* 🔴 两种宿主都要成立：整页/整栏的失败块（块级流）与**行内**失败条（flex 行）。
     收起时跟着主线文字排在同一行；展开时独占一行，让长命令名换行而不是把行挤爆。
     flex 属性在非 flex 父级里自然无效 ⇒ 不需要为两种宿主写两套件。 */
  flex: 0 0 auto;
  min-width: 0;

  &[open] {
    flex: 1 1 100%;
  }

  summary {
    display: inline-flex;
    align-items: center;
    gap: 2px;
    cursor: pointer;
    font-size: var(--aegis-type-caption-size);
    /* 🔴 24px 行高 = 行内控件档（与 Overview 分区失败条的 SectionRetry 同档），
       不是随手补的：12px 文字撑出来的原高只有 18px，是本域最矮的可点目标。
       纵向 padding 不写、用 min-height 顶开 ⇒ 收起时不会把主线行高拉高。 */
    min-height: 24px;
    /* mute 而非 stone：§六 #6 已把 stone 降级为 UI-only（3.64:1），本项是**文字**。 */
    color: var(--aegis-mute);
    list-style: none;
    /* 🔴 不写 outline: none（§八）—— 焦点环由全局 :focus-visible 提供。 */
    user-select: none;
  }

  summary svg {
    transition: transform var(--aegis-motion-duration-fast) var(--aegis-motion-easing-standard);
  }

  &[open] summary svg {
    transform: rotate(180deg);
  }

  @media (prefers-reduced-motion: reduce) {
    summary svg {
      transition: none;
    }
  }
`;

const Line = styled.div`
  font-family: var(--font-mono);
  font-size: var(--aegis-type-caption-size);
  line-height: 1.5;
  color: var(--aegis-mute);
  margin-top: var(--aegis-space-xs);
  /* 命令名是无空格长串：overflow-wrap 让它**只在整行放不下时**才断开。
     原先写 word-break: break-all 会连中文一起按字符硬断（实测 340px 栏内
     「当前不 / 在 Tauri」这样断行），中文本就允许任意字间换行，不需要它。 */
  overflow-wrap: anywhere;
`;

/**
 * 「详情」折叠件：把后端命令标识收起来。
 *
 * 🔴 用原生 details/summary 而不是 useState 控制开合：键盘可达（Tab 到 summary、
 *   Enter/Space 切换）与 `aria-expanded` 语义由浏览器免费提供，自研一套反而容易漏
 *   （本项目 §八 的门禁是「键盘能完成全部操作」，不是「看起来能」）。
 */
export function FailureDetail({ detail }: { detail: string | null }) {
  if (detail === null) return null;
  return (
    <Box>
      <summary>
        {i18n.t("common:failure.detail")}
        {/* 14 档：§七 行内微图标（12px 文字行内须低于字高才不撑行）。 */}
        <ChevronDown size={14} strokeWidth={1.5} aria-hidden="true" />
      </summary>
      <Line>{detail}</Line>
    </Box>
  );
}
