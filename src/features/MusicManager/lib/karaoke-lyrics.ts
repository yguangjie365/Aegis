// MU-3 歌词解析与进度算法 —— 移植自 NexBox `src/lib/karaoke-lyrics.ts`（GPL-3.0 同源衍生，
// 参考 Mineradio 的 YRC 逐字实现）。
// 适配差异（规格 07 v1.2 口径，NexBox 缺失部分按规格补写）：
// ① F-MU-22：解析入口清洗零宽/不可见字符（平台时间轴常见占位符）；
// ② F-MU-19：整篇无时间戳 → 不丢弃，转纯文本整段显示（NexBox 直接丢行）；
// ③ F-MU-20：当前行判定 = 最后一条开始时间 ≤ 歌词位置 + 0.05s 前瞻容差；
// ④ F-MU-23：歌词位置统一回拨 0.5s 对齐听感（进度条不受影响）——回拨常量在此导出，
//   由展示层对 audio.currentTime 应用；在线取词（F-MU-19 第三优先级）随 MU-4/5 接入。

// ── 类型（NexBox types/music.ts L85-110 移植；在线字段随 MU-4/5 启用）──

export interface Lyrics {
  lyric: string;
  translation?: string;
  roma?: string;
  /** YRC 逐字歌词（在线平台专用；本地来源恒缺省） */
  yrc?: string;
}

/** 逐词数据 */
export interface LyricWord {
  text: string;
  /** 开始时间（秒） */
  t: number;
  /** 持续时间（秒） */
  d: number;
  /** 在整行文本中的起始字符索引 */
  c0: number;
  /** 在整行文本中的结束字符索引 */
  c1: number;
}

/** 卡拉OK歌词行 */
export interface KaraokeLine {
  /** 行开始时间（秒） */
  time: number;
  /** 行持续时间（秒） */
  duration: number;
  /** 整行文本 */
  text: string;
  /** 译文 */
  translation?: string;
  /** 逐词数据（有字级时间信息时存在） */
  words?: LyricWord[];
  charCount: number;
  /** 是否有逐字数据 */
  hasKaraoke: boolean;
}

/** F-MU-23：歌词位置统一回拨量（秒）——仅作用于歌词，进度条不经此常量 */
export const LYRIC_REWIND_S = 0.5;
/** F-MU-20：当前行判定的前瞻容差（秒） */
const ACTIVE_LOOKAHEAD_S = 0.05;

/** F-MU-22：零宽/不可见字符清洗（ZWSP/ZWNJ/ZWJ/BOM/不换行空格/变体选择符/行末杂散控制符） */
const INVISIBLE_RE = /[\u200B-\u200F\u2028\u2029\u202A-\u202E\uFEFF\u00A0\uFE00-\uFE0F\u2060]/g;

export function cleanInvisible(text: string): string {
  return text.replace(INVISIBLE_RE, "");
}

/** 构建歌词行集合；返回 null = 整篇无任何带时间戳行（F-MU-19 → 调用方转纯文本整段显示） */
export function buildLines(lyrics: Lyrics | null | undefined): KaraokeLine[] | null {
  if (!lyrics) return null;
  const cleaned: Lyrics = {
    lyric: cleanInvisible(lyrics.lyric ?? ""),
    translation: lyrics.translation ? cleanInvisible(lyrics.translation) : undefined,
    roma: lyrics.roma,
    yrc: lyrics.yrc ? cleanInvisible(lyrics.yrc) : undefined,
  };
  // 优先逐字歌词（在线平台取词，MU-4/5 数据面；本地来源天然缺省走 LRC）
  if (cleaned.yrc) {
    const yrcLines = parseYrc(cleaned.yrc);
    if (yrcLines.length > 0) {
      mergeTranslation(yrcLines, cleaned.translation);
      return yrcLines;
    }
  }
  if (!cleaned.lyric) return null;
  const lines = parseLrcEnhanced(cleaned.lyric, cleaned.translation);
  return lines.length > 0 ? lines : null;
}

// ═══════════════════ YRC 逐字歌词解析（NexBox 移植）═══════════════════

/**
 * 解析 YRC 逐字歌词格式
 * 格式: [lineStartMs,lineDurMs](wordStartMs,wordDurMs,0)word(wordStartMs,wordDurMs,0)word...
 */
export function parseYrc(yrcText: string): KaraokeLine[] {
  if (!yrcText || !yrcText.trim()) return [];

  const lines: KaraokeLine[] = [];

  for (const rawLine of yrcText.split(/\r?\n/)) {
    // 匹配行头: [startMs,durMs]
    const lineMatch = rawLine.match(/^\[(\d+),(\d+)\](.*)$/);
    if (!lineMatch) continue;

    const lineStartMs = parseInt(lineMatch[1] ?? "0", 10) || 0;
    const lineDurMs = parseInt(lineMatch[2] ?? "0", 10) || 0;
    const body = lineMatch[3] || "";

    // 匹配每个词: (startMs,durMs,0)text
    const wordRegex = /\((\d+),(\d+),\d+\)([^()]*)/g;
    const words: LyricWord[] = [];
    let fullText = "";
    let wm: RegExpExecArray | null;

    while ((wm = wordRegex.exec(body)) !== null) {
      const txt = (wm[3] || "").replace(/\s+/g, " ");
      if (!txt) continue;

      const rawStart = parseInt(wm[1] ?? "0", 10) || 0;
      const rawDur = parseInt(wm[2] ?? "0", 10) || 0;
      // 判断是绝对时间戳还是相对行头的偏移
      const absStartMs = rawStart >= lineStartMs - 500 ? rawStart : lineStartMs + rawStart;

      const c0 = fullText.length;
      fullText += txt;

      words.push({
        text: txt,
        t: absStartMs / 1000,
        d: Math.max(0.06, rawDur / 1000),
        c0,
        c1: fullText.length,
      });
    }

    // 如果没有匹配到词，尝试提取纯文本
    if (!fullText) {
      fullText = body.replace(/\(\d+,\d+,\d+\)/g, "").replace(/\s+/g, " ");
    }

    // 清理文本
    const leading = (fullText.match(/^\s+/) || [""])[0].length;
    fullText = fullText.replace(/\s+/g, " ").trim();
    if (!fullText) continue;

    // 修正词的字符偏移
    if (words.length) {
      words.forEach((w) => {
        w.c0 = Math.max(0, Math.min(fullText.length, w.c0 - leading));
        w.c1 = Math.max(w.c0, Math.min(fullText.length, w.c1 - leading));
      });
      // 过滤掉无效的词
      const validWords = words.filter((w) => w.c1 > w.c0);
      if (validWords.length === 0) continue;
      lines.push({
        time: lineStartMs / 1000,
        duration: lineDurMs / 1000,
        text: fullText,
        words: validWords,
        charCount: Math.max(1, fullText.length),
        hasKaraoke: true,
      });
    } else {
      lines.push({
        time: lineStartMs / 1000,
        duration: lineDurMs / 1000,
        text: fullText,
        charCount: Math.max(1, fullText.length),
        hasKaraoke: false,
      });
    }
  }

  return finalizeLineDurations(lines);
}

// ═══════════════════ LRC 逐行歌词解析（增强版，NexBox 移植）═══════════════════

/** F-MU-21：时间戳行精确命中译文，否则容差 0.5s 内取最近；匹配不到返回 undefined 不强行挂靠 */
function matchTranslation(transMap: Map<number, string>, time: number): string | undefined {
  const exact = transMap.get(time);
  if (exact !== undefined) return exact;
  let bestMatch: string | undefined;
  let bestDiff = Infinity;
  for (const [t, txt] of transMap) {
    const diff = Math.abs(t - time);
    if (diff < bestDiff && diff < 0.5) {
      bestDiff = diff;
      bestMatch = txt;
    }
  }
  return bestMatch;
}

/** 解析 LRC 格式歌词，附带翻译（F-MU-21 就近挂靠） */
export function parseLrcEnhanced(lyric: string, translation?: string): KaraokeLine[] {
  if (!lyric) return [];

  const lines: KaraokeLine[] = [];
  const transMap = parseTimestampMap(translation);

  // 解析主歌词（多时间戳行逐时间戳展开）
  const mainLines = lyric.split(/\r?\n/);
  for (const line of mainLines) {
    const matches = [...line.matchAll(/\[(\d+):(\d+(?:\.\d+)?)\]/g)];
    if (matches.length === 0) continue;
    const text = line.replace(/\[\d+:\d+(?:\.\d+)?\]/g, "").trim();
    if (!text) continue;
    for (const m of matches) {
      const time = parseInt(m[1] ?? "0", 10) * 60 + parseFloat(m[2] ?? "0");
      lines.push({
        time,
        duration: 0,
        text,
        translation: matchTranslation(transMap, time),
        charCount: Math.max(1, text.length),
        hasKaraoke: false,
      });
    }
  }

  return finalizeLineDurations(lines);
}

/** 时间戳 → 文本 映射（译文解析共用） */
function parseTimestampMap(text: string | undefined): Map<number, string> {
  const map = new Map<number, string>();
  if (!text) return map;
  for (const line of text.split(/\r?\n/)) {
    const match = line.match(/\[(\d+):(\d+(?:\.\d+)?)\]/);
    if (!match) continue;
    const time = parseInt(match[1] ?? "0", 10) * 60 + parseFloat(match[2] ?? "0");
    const body = line.replace(/\[\d+:\d+(?:\.\d+)?\]/, "").trim();
    if (body) map.set(time, body);
  }
  return map;
}

/** F-MU-21：译文按时间就近挂靠主行（精确时间命中，否则容差 0.5s 内取最近；匹配不到不强行挂靠） */
function mergeTranslation(lines: KaraokeLine[], translation: string | undefined) {
  if (!translation) return;
  const transMap = parseTimestampMap(translation);
  lines.forEach((line) => {
    const matched = matchTranslation(transMap, line.time);
    if (matched) line.translation = matched;
  });
}

// ═══════════════════ 时间轴健壮性（F-MU-22，NexBox 移植）═══════════════════

/** 排序、由下一行推断缺省行时长（0.45–12s，默认约 4.8s）、确保 charCount 有效 */
function finalizeLineDurations(lines: KaraokeLine[]): KaraokeLine[] {
  lines.sort((a, b) => a.time - b.time);

  for (let i = 0; i < lines.length; i++) {
    const cur = lines[i];
    if (!cur) continue;
    const next = lines[i + 1];
    const inferred = next && next.time > cur.time ? next.time - cur.time : 4.8;
    if (!isFinite(cur.duration) || cur.duration <= 0) {
      cur.duration = inferred;
    }
    cur.duration = Math.max(0.45, Math.min(12, cur.duration));
    cur.charCount = Math.max(1, cur.charCount || String(cur.text || "").length);
  }

  return lines;
}

// ═══════════════════ 当前行判定（F-MU-20 规格补写）═══════════════════

/**
 * 当前行 = 最后一条 time ≤ 歌词位置 + 0.05s 前瞻容差 的行（乱序输入已在构建期排序）。
 * 传入的 lyricPosition 应为已回拨位置（见 LYRIC_REWIND_S 用法）。
 */
export function findActiveIndex(lines: KaraokeLine[], lyricPosition: number): number {
  if (lines.length === 0) return -1;
  let idx = -1;
  for (const line of lines) {
    if (line.time <= lyricPosition + ACTIVE_LOOKAHEAD_S) idx += 1;
    else break;
  }
  return idx;
}

// ═══════════════════ 行进度计算（NexBox 移植）═══════════════════

/**
 * 计算当前歌词行的填充进度 (0~1)。
 * - 有逐字数据时：遍历每个词，精确计算已唱到的字符位置
 * - 无逐字数据时：行级线性插值 + smoothstep 缓动
 * 传入的 currentTime 应为已回拨的歌词位置（F-MU-23）。
 */
export function getLineProgress(
  line: KaraokeLine | undefined,
  nextLine: KaraokeLine | undefined,
  currentTime: number,
): number {
  if (!line) return 0;

  // 微小偏移，让进度略超前于实际音频（视觉补偿）
  const now = currentTime + (line.words && line.words.length ? 0.03 : 0.02);

  // 有逐字数据：精确计算已唱字符位置
  if (line.words && line.words.length && line.charCount > 0) {
    let lastP = 0;
    for (const w of line.words) {
      const ws = w.t;
      const we = w.t + Math.max(0.08, w.d || 0.24);
      if (now < ws) return lastP;
      const local = now >= we ? 1 : (now - ws) / Math.max(0.08, we - ws);
      const clamped = Math.max(0, Math.min(1, local));
      const p = (w.c0 + (w.c1 - w.c0) * clamped) / line.charCount;
      lastP = Math.max(lastP, p);
      if (now < we) return lastP;
    }
    return 1;
  }

  // 无逐字数据：行级线性插值 + smoothstep 缓动
  const nextT =
    nextLine && nextLine.time > line.time ? nextLine.time : line.time + (line.duration || 4.8);
  const span = Math.max(0.75, nextT - line.time);
  const prog = Math.max(0, Math.min(1, (now - line.time) / span));
  return prog * prog * (3 - 2 * prog); // smoothstep
}

// ═══════════════════ 超长歌词水平滚动（NexBox 移植）═══════════════════

/**
 * 超长歌词水平滚动偏移量计算（参考 Mineradio updateLyricScroll）：
 * 前 8% 静止 → 中段 smoothstep 缓动 → 78% 后保持在最远位。
 */
export function calculateScrollOffset(progress: number, limit: number): number {
  if (limit <= 0) return 0;

  const p = Math.max(0, Math.min(1, progress));
  const startGate = 0.08;
  const endGate = 0.78;

  if (p < startGate) return 0;
  if (p >= endGate) return -limit;

  const t = (p - startGate) / (endGate - startGate);
  const eased = t * t * (3 - 2 * t);
  return -limit * eased;
}
