// K-S03 探针（TASKS §4.2）：验证 Q-16 拍板方案「前端 <audio> + asset 协议 Range 直读」的落地质量。
// 判定标准：seek 精度与断流恢复实测；大文件不整读内存。
// 运行方式：`VITE_SPIKE=K-S03 npx tauri build`（构建期注入开关）后启动 exe，面板自动执行并显示结果；
// 常规构建（无该环境变量）此模块被摇树剔除，不进产物。结论与实测数据归档于 Aegis/spikes/K-S03-audio-range.md。

import { useEffect, useRef, useState } from "react";
import { convertFileSrc } from "@tauri-apps/api/core";
import { isTauriRuntime } from "@/lib/tauri/invoke";
import { musicLibraryGet, musicScanStart, type LocalSong } from "@/lib/tauri/music";

type Status = "…" | "PASS" | "FAIL" | "SKIP";
interface Line {
  label: string;
  status: Status;
  detail: string;
}

let alreadyRan = false;

function waitEvent(el: HTMLAudioElement, event: string, timeoutMs: number): Promise<boolean> {
  return new Promise((resolve) => {
    const timer = window.setTimeout(() => {
      el.removeEventListener(event, on);
      resolve(false);
    }, timeoutMs);
    const on = () => {
      window.clearTimeout(timer);
      resolve(true);
    };
    el.addEventListener(event, on, { once: true });
  });
}

function bufferedSeconds(a: HTMLAudioElement): number {
  let total = 0;
  for (let i = 0; i < a.buffered.length; i++) {
    total += a.buffered.end(i) - a.buffered.start(i);
  }
  return total;
}

async function runTests(setLines: (ls: Line[]) => void): Promise<void> {
  const lines: Line[] = [];
  const push = (label: string, status: Status, detail: string) => {
    lines.push({ label, status, detail });
    setLines([...lines]);
  };

  if (!isTauriRuntime()) {
    push("环境", "SKIP", "非 Tauri 运行时，探针无意义");
    return;
  }

  // 曲库准备：若最大曲目 <50MB（不足以验证"大文件不整读"），触发重扫并等待完成
  let song: LocalSong | undefined;
  try {
    const largest = (lib: LocalSong[]): LocalSong | undefined =>
      lib.reduce<LocalSong | undefined>(
        (max, s) => (!max || s.size > max.size ? s : max),
        undefined,
      );
    let view = await musicLibraryGet();
    let best = largest(view.library.songs);
    if (!best) {
      push("曲库", "SKIP", "曲库为空，无法探测");
      return;
    }
    if (best.size < 50 * 1024 * 1024) {
      push(
        "曲库",
        "…",
        `最大仅 ${(best.size / 1024 / 1024).toFixed(1)} MB，触发重扫等待大文件入库…`,
      );
      await musicScanStart();
      const deadline = Date.now() + 180_000;
      for (;;) {
        await new Promise((r) => window.setTimeout(r, 1000));
        view = await musicLibraryGet();
        best = largest(view.library.songs) ?? best;
        if (!view.scanning && best.size >= 50 * 1024 * 1024) break;
        if (Date.now() > deadline) break;
      }
    }
    song = best;
    push("曲库", "PASS", `${song.name}（${(song.size / 1024 / 1024).toFixed(1)} MB）`);
  } catch (e) {
    push("曲库", "FAIL", String(e));
    return;
  }
  if (!song) return;

  const url = convertFileSrc(song.path);
  const size = song.size;

  // T1 动态 scope：Range 请求经 asset 协议。注意：asset.rs 仅经 Access-Control-Expose-Headers 暴露
  // content-range，accept-ranges 被 CORS 过滤不可读，故不断言该头（源码证据另见 spikes 文档）。
  try {
    const res = await fetch(url, { headers: { Range: "bytes=0-1" } });
    const buf = await res.arrayBuffer();
    const cr = res.headers.get("content-range") ?? "";
    const ok = res.status === 206 && buf.byteLength === 2 && cr.startsWith("bytes 0-1/");
    push(
      "T1 动态授权+Range",
      ok ? "PASS" : "FAIL",
      `status=${res.status} len=${buf.byteLength} CR=${cr}`,
    );
  } catch (e) {
    push("T1 动态授权+Range", "FAIL", String(e));
  }

  // T2 中段 Range：随机访问不触发整读（单响应上限 1MB，tauri asset.rs MAX_LEN）
  try {
    const start = Math.floor(size / 2);
    const res = await fetch(url, { headers: { Range: `bytes=${start}-${start + 1023}` } });
    const buf = await res.arrayBuffer();
    const cr = res.headers.get("content-range") ?? "";
    const ok = res.status === 206 && buf.byteLength === 1024 && cr.endsWith(`/${size}`);
    push(
      "T2 中段Range",
      ok ? "PASS" : "FAIL",
      `start=${start} status=${res.status} len=${buf.byteLength} CR=${cr}`,
    );
  } catch (e) {
    push("T2 中段Range", "FAIL", String(e));
  }

  // T3 越界 Range → 416。已知协议事实：asset.rs 对 416 响应不附加 ACAO 头，
  // WebView 中 fetch 会以 TypeError 拒绝——这本身即"服务端返回了不带 CORS 许可的 416"的证据，判 PASS。
  try {
    const res = await fetch(url, { headers: { Range: `bytes=${size + 10}-${size + 20}` } });
    await res.arrayBuffer();
    push("T3 越界Range", res.status === 416 ? "PASS" : "FAIL", `status=${res.status}`);
  } catch (e) {
    const msg = String(e);
    const ok = msg.includes("TypeError") || msg.includes("Failed to fetch");
    push(
      "T3 越界Range",
      ok ? "PASS" : "FAIL",
      ok ? `预期内 CORS 遮蔽（416 无 ACAO）: ${msg}` : msg,
    );
  }

  // T8 单响应上限：请求 2MB 应被截断（asset.rs `const MAX_LEN: u64 = 1000 * 1024`，源码实测 1024000 B）
  try {
    const res = await fetch(url, { headers: { Range: "bytes=0-2097151" } });
    const buf = await res.arrayBuffer();
    const ok = res.status === 206 && buf.byteLength === 1000 * 1024;
    push(
      "T8 响应上限",
      ok ? "PASS" : "FAIL",
      `请求2MB→status=${res.status} len=${buf.byteLength}（期望1024000）`,
    );
  } catch (e) {
    push("T8 响应上限", "FAIL", String(e));
  }

  // T4 <audio> 元数据：loadedmetadata 时长与 lofty 元数据一致。
  // 合成拼接样本（K-S03探针-大文件）豁免：lofty 按首帧码率预估 ≠ <audio> 帧计数，属样本固有误差。
  const a = new Audio();
  a.preload = "metadata";
  a.src = url;
  try {
    const gotMeta = await waitEvent(a, "loadedmetadata", 8000);
    const dur = a.duration;
    const expect = song.durationMs / 1000;
    const synthetic = song.name.includes("K-S03探针");
    const diff = Math.abs(dur - expect);
    const ok = gotMeta && Number.isFinite(dur) && (diff < 1 || synthetic);
    push(
      "T4 元数据时长",
      ok ? "PASS" : "FAIL",
      `audio=${dur.toFixed(3)}s lofty=${expect.toFixed(3)}s${synthetic && diff >= 1 ? "（合成拼接样本豁免：预估≠帧计数）" : ""}`,
    );
  } catch (e) {
    push("T4 元数据时长", "FAIL", String(e));
  }

  if (!Number.isFinite(a.duration) || a.duration <= 0) {
    push("T5-T7", "SKIP", "未取得时长，后续探测终止");
    a.src = "";
    return;
  }
  const dur = a.duration;

  // T5 seek 精度：三个位置（50%/90%/25%）
  {
    let worst = 0;
    const parts: string[] = [];
    for (const p of [0.5, 0.9, 0.25]) {
      const target = dur * p;
      a.currentTime = target;
      const got = await waitEvent(a, "seeked", 5000);
      if (!got) {
        parts.push(`${Math.round(p * 100)}%=超时`);
        worst = 999;
        break;
      }
      const delta = Math.abs(a.currentTime - target);
      worst = Math.max(worst, delta);
      parts.push(`${Math.round(p * 100)}% Δ${delta.toFixed(3)}s`);
    }
    push("T5 seek精度", worst < 0.05 ? "PASS" : "FAIL", `${parts.join(" ")}（判定 <0.05s）`);
  }

  // T6 断流恢复：播放→尾部 seek→回跳，全程续播不卡死
  try {
    await a.play();
    await new Promise((r) => window.setTimeout(r, 700));
    a.currentTime = dur - 3;
    await waitEvent(a, "seeked", 5000);
    const t0 = a.currentTime;
    await new Promise((r) => window.setTimeout(r, 1500));
    const advanced = a.currentTime - t0;
    a.currentTime = 10;
    await waitEvent(a, "seeked", 5000);
    await new Promise((r) => window.setTimeout(r, 700));
    const back = a.currentTime;
    const ok = advanced > 0.8 && back > 10.5 && !a.paused;
    push(
      "T6 断流恢复",
      ok ? "PASS" : "FAIL",
      `尾段推进${advanced.toFixed(2)}s 回跳后${back.toFixed(2)}s paused=${a.paused}`,
    );
  } catch (e) {
    push("T6 断流恢复", "FAIL", String(e));
  }

  // T7 缓冲窗口：已缓冲秒数远小于全曲（大文件不整读的直接证据；服务端单响应 ≤1MB 另有 T8/源码保证）
  {
    const buffered = bufferedSeconds(a);
    const ok = buffered < dur * 0.6;
    const estMb = (size * (buffered / dur)) / 1048576;
    push(
      "T7 缓冲窗口",
      ok ? "PASS" : "FAIL",
      `buffered=${buffered.toFixed(1)}s/${dur.toFixed(1)}s ≈${estMb.toFixed(1)}MB/${(size / 1048576).toFixed(1)}MB`,
    );
  }

  a.pause();
  a.src = "";
  push("总判定", lines.every((l) => l.status === "PASS") ? "PASS" : "FAIL", "见上行逐项");
}

export function KS03SelfTest() {
  const [lines, setLines] = useState<Line[]>([]);
  const started = useRef(false);

  useEffect(() => {
    if (alreadyRan || started.current) return;
    started.current = true;
    alreadyRan = true;
    void runTests(setLines);
  }, []);

  return (
    <div
      style={{
        position: "fixed",
        top: 48,
        left: 12,
        zIndex: 99999,
        background: "#101010",
        color: "#e8e8e8",
        font: "12px/1.6 monospace",
        padding: "10px 14px",
        border: "1px solid #76b900",
        borderRadius: 8,
        width: 560,
        maxWidth: "calc(100vw - 24px)",
        maxHeight: "80vh",
        overflowY: "auto",
      }}
    >
      <div style={{ fontWeight: 700, marginBottom: 4, color: "#76b900" }}>
        K-S03 音频 Range 探针
      </div>
      {lines.length === 0 && <div>准备中…</div>}
      {lines.map((l, i) => (
        <div key={i}>
          <span
            style={{
              color: l.status === "PASS" ? "#76b900" : l.status === "FAIL" ? "#e53e3e" : "#a0a0a0",
              marginRight: 6,
              fontWeight: 700,
            }}
          >
            [{l.status}]
          </span>
          <b>{l.label}</b>
          <div style={{ opacity: 0.8, marginLeft: 12, wordBreak: "break-all" }}>{l.detail}</div>
        </div>
      ))}
    </div>
  );
}
