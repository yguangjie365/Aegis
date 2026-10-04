// K-S01 测试 C（读取）：把用户真实拖放捕获到的数据全部 dump 出来
const PORT = 9222;
const list = await (await fetch(`http://127.0.0.1:${PORT}/json/list`)).json();
const page = list.find((t) => t.type === "page" && t.webSocketDebuggerUrl);
if (!page) {
  console.error("no page target");
  process.exit(1);
}
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

const raw = await js(`JSON.stringify(window.__ks01 ? window.__ks01 : null)`);
if (!raw || raw === "null") {
  console.log("🔴 页面上下文里没有捕获数据（可能被刷新过）");
  process.exit(1);
}
const d = JSON.parse(raw);
const base = d.raw.length ? d.raw[0].at : Date.now();

console.log("════ 一、Tauri 原生事件（tauri://drag-*） ════");
console.log(`  捕获条数 = ${d.raw.length}`);
const byName = {};
for (const e of d.raw) byName[e.name] = (byName[e.name] || 0) + 1;
console.log("  按事件名: " + JSON.stringify(byName));

// 只打印 drag-drop（最有价值）的全部载荷
const drops = d.raw.filter((e) => e.name === "tauri://drag-drop");
console.log(`\n  ── tauri://drag-drop 共 ${drops.length} 次 ──`);
drops.forEach((e, i) => {
  console.log(`  [${i + 1}] +${e.at - base}ms  payload = ${JSON.stringify(e.payload)}`);
});

const enters = d.raw.filter((e) => e.name === "tauri://drag-enter");
const leaves = d.raw.filter((e) => e.name === "tauri://drag-leave");
const overs = d.raw.filter((e) => e.name === "tauri://drag-over");
console.log(`\n  ── 其它 ──`);
console.log(
  `  drag-enter ${enters.length} 次` +
    (enters[0] ? `  首个 payload=${JSON.stringify(enters[0].payload)}` : ""),
);
console.log(
  `  drag-over  ${overs.length} 次` +
    (overs[0] ? `  首个 payload=${JSON.stringify(overs[0].payload)}` : ""),
);
console.log(
  `  drag-leave ${leaves.length} 次` +
    (leaves[0] ? `  首个 payload=${JSON.stringify(leaves[0].payload)}` : ""),
);

console.log("\n════ 二、官方包装层 onDragDropEvent ════");
console.log(`  捕获条数 = ${d.tauri.length}`);
const wrapped = {};
for (const e of d.tauri) wrapped[e.type || "(无 type)"] = (wrapped[e.type || "(无 type)"] || 0) + 1;
console.log("  按 type: " + JSON.stringify(wrapped));

console.log("\n════ 三、DOM 事件（验证是否被拦住 HTML5） ════");
console.log(`  捕获条数 = ${d.dom.length}`);
const domByType = {};
for (const e of d.dom) domByType[e.type] = (domByType[e.type] || 0) + 1;
console.log("  按类型: " + JSON.stringify(domByType));
if (d.dom.length) {
  const drop = d.dom.find((e) => e.type === "drop");
  console.log(
    "  首个 drop: " + (drop ? JSON.stringify(drop) : "（无 drop，只有 dragenter/dragover）"),
  );
}

console.log("\n════ 四、判定 ════");
const dropPaths = drops.flatMap((e) => (e.payload && e.payload.paths) || []);
console.log(
  `  T10 原生路径交付      : ${dropPaths.length ? "✅ PASS" : "🔴 FAIL"}  （共 ${dropPaths.length} 条路径）`,
);
if (dropPaths.length) {
  dropPaths.forEach((p, i) => console.log(`        [${i + 1}] len=${[...p].length}  ${p}`));
  const nonAscii = dropPaths.some((p) => /[\u4e00-\u9fa5]/.test(p));
  const emoji = dropPaths.some((p) => /[\u{1F300}-\u{1FAFF}]/u.test(p));
  console.log(`        含中文 = ${nonAscii}   含 emoji = ${emoji}`);
}
console.log(
  `  T11 HTML5 是否被拦    : ${d.dom.length === 0 ? "✅ 是（DOM 完全没收到事件，与文档一致）" : `⚠️ 否（DOM 仍收到 ${d.dom.length} 条）`}`,
);
process.exit(0);
