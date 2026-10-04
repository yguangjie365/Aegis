// K-S01 测试 A：当前配置（dragDropEnabled:false）下，DOM 的 drop 能否拿到原生文件路径
// 手段：CDP Input.dispatchDragEvent 注入一条带 files 的拖放（files = 真实绝对路径）
import { setTimeout as sleep } from "node:timers/promises";

const PORT = 9222;
const SAMPLES = {
  nonAscii: String.raw`D:\MyProject\MyAPP\.ks01-test\中文文件名 带空格 与emoji 🚀.txt`,
  crossDrive: String.raw`C:\Users\BlueSky\AppData\Local\Temp\ks01-c-drive\跨盘样本-C盘.txt`,
};

const list = await (await fetch(`http://127.0.0.1:${PORT}/json/list`)).json();
const page = list.find((t) => t.type === "page" && t.webSocketDebuggerUrl);
if (!page) {
  console.error("no page target");
  process.exit(1);
}
console.log("目标: " + page.url);

const ws = new WebSocket(page.webSocketDebuggerUrl);
await new Promise((r, j) => {
  ws.onopen = r;
  ws.onerror = () => j(new Error("ws"));
});
let seq = 0;
const pend = new Map();
ws.onmessage = (e) => {
  const m = JSON.parse(e.data);
  if (m.id && pend.has(m.id)) {
    pend.get(m.id)(m);
    pend.delete(m.id);
  }
};
const send = (method, params = {}) =>
  new Promise((res) => {
    const i = ++seq;
    pend.set(i, res);
    ws.send(JSON.stringify({ id: i, method, params }));
  });
const js = async (x) => {
  const r = await send("Runtime.evaluate", {
    expression: x,
    returnByValue: true,
    awaitPromise: true,
  });
  if (r.result?.exceptionDetails)
    return "EXC:" + (r.result.exceptionDetails.exception?.description || "");
  return r.result?.result?.value;
};
await send("Runtime.enable");
await send("Input.enable" in {} ? "Input.enable" : "Runtime.enable");

/* ① 在页面里装监听器：把 drop 的全部可见信息记下来 */
console.log(
  "\n① 装 drop 监听器: " +
    (await js(`(() => {
  window.__ks01 = { events: [], dom: null };
  const cap = (e) => {
    const dt = e.dataTransfer;
    const out = { type: e.type, hasDT: !!dt, types: dt ? [...dt.types] : null,
                  itemCount: dt ? dt.items.length : -1, fileCount: dt ? dt.files.length : -1,
                  files: [], items: [], handles: null, plain: null, uriList: null };
    if (dt) {
      try { out.plain = dt.getData('text/plain'); } catch (ex) { out.plain = 'ERR:' + ex.message; }
      try { out.uriList = dt.getData('text/uri-list'); } catch (ex) { out.uriList = 'ERR:' + ex.message; }
      for (const f of dt.files) {
        out.files.push({
          name: f.name, size: f.size, type: f.type, lastModified: f.lastModified,
          hasPathProp: 'path' in f, pathValue: f.path ?? null,
          ownProps: Object.getOwnPropertyNames(f),
          protoPath: Object.getOwnPropertyDescriptor(Object.getPrototypeOf(f) || {}, 'path') ? 'yes' : 'no',
          webkitRelativePath: f.webkitRelativePath ?? null,
        });
      }
      for (const it of dt.items) out.items.push({ kind: it.kind, type: it.type });
      out.hasGetAsFileSystemHandle = typeof dt.items[0]?.getAsFileSystemHandle === 'function';
    }
    window.__ks01.events.push(out);
    if (e.type === 'drop') window.__ks01.dom = out;
  };
  ['dragenter','dragover','drop'].forEach(t => document.addEventListener(t, cap, true));
  document.addEventListener('drop', e => e.preventDefault(), true);
  document.addEventListener('dragover', e => e.preventDefault(), true);
  return 'listening';
})()`)),
);

/* ② CDP 注入拖放（files = 真实绝对路径） */
const rect = JSON.parse(
  await js(
    `(() => { const r = document.body.getBoundingClientRect(); return JSON.stringify({x: Math.round(r.width/2), y: Math.round(r.height/2)}); })()`,
  ),
);
const dragData = {
  items: [
    { mimeType: "text/uri-list", data: "file:///" + SAMPLES.nonAscii.replace(/\\/g, "/") },
    { mimeType: "text/plain", data: SAMPLES.nonAscii },
  ],
  files: [SAMPLES.nonAscii, SAMPLES.crossDrive],
  dragOperationsMask: 1, // COPY
};
console.log("\n② 注入拖放（2 个文件：非 ASCII + 跨盘）");
await send("Input.setInterceptDrags", { enabled: true });
for (const type of ["dragEnter", "dragOver", "drop"]) {
  const r = await send("Input.dispatchDragEvent", { type, x: rect.x, y: rect.y, data: dragData });
  if (r.error) console.log(`     ${type}: 错误 -> ${JSON.stringify(r.error)}`);
  else console.log(`     ${type}: 已派发`);
  await sleep(120);
}
await sleep(400);

/* ③ 读取捕获结果 */
console.log("\n③ 捕获结果:");
const dom = JSON.parse(await js(`JSON.stringify(window.__ks01.dom)`));
if (!dom) {
  console.log("   🔴 drop 事件未到达页面");
} else {
  console.log(`   types       = ${JSON.stringify(dom.types)}`);
  console.log(`   items       = ${JSON.stringify(dom.items)}`);
  console.log(`   fileCount   = ${dom.fileCount}`);
  console.log(`   text/plain  = ${JSON.stringify(dom.plain)}`);
  console.log(`   uri-list    = ${JSON.stringify(dom.uriList)}`);
  console.log(`   getAsFileSystemHandle 可用 = ${dom.hasGetAsFileSystemHandle}`);
  for (const f of dom.files) {
    console.log(`   ── 文件: ${f.name}`);
    console.log(`        size=${f.size} type=${f.type || "(空)"}`);
    console.log(`        hasPathProp=${f.hasPathProp}  pathValue=${JSON.stringify(f.pathValue)}`);
    console.log(`        webkitRelativePath=${JSON.stringify(f.webkitRelativePath)}`);
    console.log(`        ownProps=${JSON.stringify(f.ownProps)}`);
  }
}
console.log(
  "\n④ 全部事件序列: " +
    JSON.stringify(
      (JSON.parse(await js(`JSON.stringify(window.__ks01.events)`)) || []).map((e) => e.type),
    ),
);

/* ⑤ 顺带确认 Tauri 原生通道是否可用（不应触发，因为 dragDropEnabled:false） */
console.log(
  "\n⑤ Tauri 原生事件可达性: " +
    (await js(`(async () => {
  try {
    if (!window.__TAURI__) return 'window.__TAURI__ 不存在（withGlobalTauri 未开启）';
    const w = await import('@tauri-apps/api/webview');
    return 'tauri api 模块可用: ' + Object.keys(w).filter(k => /drag/i.test(k)).join(',');
  } catch (e) { return 'ERR: ' + e.message; }
})()`)),
);

await js(
  `['dragenter','dragover','drop'].forEach(t => document.removeEventListener(t, window.__ks01_cap, true));`,
);
process.exit(0);
