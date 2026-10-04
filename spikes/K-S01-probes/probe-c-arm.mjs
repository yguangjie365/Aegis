// K-S01 测试 C（准备）：武装监听器，等用户做一次真实拖放
// 同时挂「Tauri 原生拖放事件」与「DOM drop」，一次拖放即可判定：
//   ① dragDropEnabled:true 时 Tauri 原生事件能否交付**原生绝对路径**
//   ② 该配置下 DOM 是否还能收到 drop（验证"会拦截 HTML5 拖放"这一假设）
import { setTimeout as sleep } from "node:timers/promises";

const PORT = 9222;
const list = await (await fetch(`http://127.0.0.1:${PORT}/json/list`)).json();
const page = list.find((t) => t.type === "page" && t.webSocketDebuggerUrl);
if (!page) {
  console.error("no page target");
  process.exit(1);
}
console.log("目标页: " + page.url);

const ws = new WebSocket(page.webSocketDebuggerUrl);
await new Promise((r, j) => {
  ws.onopen = r;
  ws.onerror = () => j(new Error("ws"));
});
let seq = 0;
const pend = new Map();
const consoleErrs = [];
ws.onmessage = (e) => {
  const m = JSON.parse(e.data);
  if (m.id && pend.has(m.id)) {
    pend.get(m.id)(m);
    pend.delete(m.id);
  }
  if (m.method === "Log.entryAdded" && m.params.entry.level === "error")
    consoleErrs.push(m.params.entry.text);
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
await send("Log.enable");
await send("Page.enable");

/* 页面健康度（配置翻面后不得有回归） */
console.log("\n① 页面健康度");
console.log(
  "  " +
    (await js(`JSON.stringify({
  url: location.pathname,
  navItems: document.querySelectorAll('nav a').length,
  bodyChildren: document.body.children.length,
  hasReactRoot: !!document.getElementById('root')
})`)),
);

/* 武装监听器 */
console.log("\n② 武装监听器");
console.log(
  "  " +
    (await js(`(async () => {
  window.__ks01 = { armed: false, tauri: [], dom: [], raw: [] };
  // DOM 侧（验证 dragDropEnabled 是否真的拦住了 HTML5 drop）
  const capDom = (e) => {
    const dt = e.dataTransfer;
    window.__ks01.dom.push({ type: e.type, types: dt ? [...dt.types] : null,
      fileCount: dt ? dt.files.length : -1,
      firstFileName: dt && dt.files[0] ? dt.files[0].name : null,
      firstHasPath: dt && dt.files[0] ? ('path' in dt.files[0]) : null });
  };
  ['dragenter','dragover','drop'].forEach(t => document.addEventListener(t, capDom, true));
  document.addEventListener('drop', e => e.preventDefault(), true);
  document.addEventListener('dragover', e => e.preventDefault(), true);

  // Tauri 原生侧：先挂 raw 事件（不依赖 webview 包装层）
  const ev = await import('/node_modules/@tauri-apps/api/event.js');
  window.__ks01.rawNames = [];
  for (const name of ['tauri://drag-enter','tauri://drag-over','tauri://drag-drop','tauri://drag-leave']) {
    await ev.listen(name, (e) => { window.__ks01.raw.push({ name, payload: e.payload, at: Date.now() }); });
    window.__ks01.rawNames.push(name);
  }
  // 再挂官方包装层（对照）
  const wv = await import('/node_modules/@tauri-apps/api/webview.js');
  window.__ks01.unlistenWrapper = await wv.getCurrentWebview().onDragDropEvent((e) => {
    window.__ks01.tauri.push({ type: e.payload && e.payload.type, payload: e.payload, at: Date.now() });
  });
  window.__ks01.armed = true;
  return JSON.stringify({ armed: true, rawListens: window.__ks01.rawNames });
})()`)),
);

await sleep(500);
console.log("\n③ 武装状态复核");
console.log(
  "  " +
    (await js(
      `JSON.stringify({ armed: window.__ks01?.armed, rawCount: window.__ks01?.raw.length, domCount: window.__ks01?.dom.length })`,
    )),
);

console.log(
  "\n④ 控制台错误（配置翻面后）: " +
    (consoleErrs.length ? consoleErrs.slice(0, 3).join(" | ") : "无"),
);
console.log("\n✅ 监听器已就绪 —— 现在可以拖放了（本进程会保留，直到页面刷新）");
process.exit(0);
