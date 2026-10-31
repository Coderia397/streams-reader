// @ts-check
// Unit tests for the extension's pure modules, run in a normal page (no extension APIs). tests/e2e.py loads this page
// in headless Chrome and reads window.__unit; it can also be opened by hand from any static server.
import { MAX_HTML_BYTES, MAX_TEXT_CHARS, PROTOCOL_VERSION } from "../src/config.js";
import { buildPayload, cleanLang, cleanTitle, httpUrl, isScriptable, utf8Length, validatePayload } from "../src/payload.js";
import { createMutex, createStore } from "../src/handoff-store.js";
import { trustedAppTab } from "../src/claim.js";
import { extractPage, extractSelection } from "../src/extract.js";
import { collect, looksLikePdf } from "../src/collect.js";

/** @type {string[]} */
const failures = [];
let passed = 0;

/** @param {string} name @param {() => unknown | Promise<unknown>} fn */
async function test(name, fn) {
  try {
    await fn();
    passed++;
  } catch (e) {
    failures.push(`${name}: ${e instanceof Error ? e.message : e}`);
  }
}

/** @param {unknown} cond @param {string} [msg] */
function ok(cond, msg = "expected true") {
  if (!cond) throw new Error(msg);
}
/** @param {unknown} a @param {unknown} b */
function eq(a, b) {
  const sa = JSON.stringify(a), sb = JSON.stringify(b);
  if (sa !== sb) throw new Error(`${sa} !== ${sb}`);
}

/** In-memory stand-in for chrome.storage.session with an optional byte quota. */
function memoryArea(quota = Infinity) {
  /** @type {Map<string, unknown>} */
  const m = new Map();
  const size = () => [...m.entries()].reduce((n, [k, v]) => n + k.length + JSON.stringify(v).length, 0);
  return {
    m,
    async get(/** @type {null | string | string[]} */ keys) {
      if (keys === null) return Object.fromEntries(m);
      const list = Array.isArray(keys) ? keys : [keys];
      return Object.fromEntries(list.filter((k) => m.has(k)).map((k) => [k, m.get(k)]));
    },
    async set(/** @type {Record<string, unknown>} */ items) {
      const before = new Map(m);
      for (const [k, v] of Object.entries(items)) m.set(k, structuredClone(v));
      if (size() > quota) {
        m.clear();
        before.forEach((v, k) => m.set(k, v));
        throw new Error("QUOTA_BYTES quota exceeded");
      }
    },
    async remove(/** @type {string | string[]} */ keys) {
      for (const k of Array.isArray(keys) ? keys : [keys]) m.delete(k);
    },
  };
}

let seq = 0;
/** @param {Partial<import("../src/payload.js").Payload>} [over] */
function payload(over = {}) {
  seq++;
  const id = `00000000-0000-4000-8000-${String(seq).padStart(12, "0")}`;
  return { v: 1, id, kind: "selection", createdAt: 1000 + seq, url: "https://a.example/x", title: "T", lang: "en", text: "hello", ...over };
}

// payload.js
await test("httpUrl keeps http(s) only", () => {
  eq(httpUrl("https://a.example/p?q=1"), "https://a.example/p?q=1");
  eq(httpUrl("http://a.example"), "http://a.example/");
  eq(httpUrl("javascript:alert(1)"), null);
  eq(httpUrl("chrome://settings"), null);
  eq(httpUrl("file:///etc/passwd"), null);
  eq(httpUrl("https://a.example/" + "x".repeat(9000)), null);
  eq(httpUrl(42), null);
});
await test("isScriptable", () => {
  ok(isScriptable("https://news.example/a"));
  ok(isScriptable("file:///Users/me/a.html"));
  ok(!isScriptable("chrome://version"));
  ok(!isScriptable("https://chromewebstore.google.com/detail/x"));
  ok(!isScriptable("https://chrome.google.com/webstore/detail/x"));
  ok(isScriptable("https://chrome.google.com/search"));
  ok(!isScriptable(undefined));
});
await test("cleanLang", () => {
  eq(cleanLang(" en-GB "), "en-GB");
  eq(cleanLang("zh-Hans-CN"), "zh-Hans-CN");
  eq(cleanLang("en_US"), null);
  eq(cleanLang('"><script>'), null);
  eq(cleanLang(""), null);
});
await test("cleanTitle collapses space and caps length", () => {
  eq(cleanTitle("  a \n b  "), "a b");
  const long = cleanTitle("x".repeat(400));
  eq(long.length, 300);
  ok(long.endsWith("…"));
  eq(cleanTitle(null), "");
});
await test("utf8Length matches TextEncoder", () => {
  for (const s of ["", "abc", "é", "مرحبا", "中文", "😀a", "\ud800"]) eq(utf8Length(s), new TextEncoder().encode(s).length);
});
await test("buildPayload page", () => {
  const r = buildPayload("page", { url: "https://a.example/", title: " T ", lang: "en", html: "<p>x</p>" }, { id: "00000000-0000-4000-8000-000000000001", now: 5 });
  ok(r.ok);
  if (r.ok) eq(r.payload, { v: PROTOCOL_VERSION, id: "00000000-0000-4000-8000-000000000001", kind: "page", createdAt: 5, url: "https://a.example/", title: "T", lang: "en", html: "<p>x</p>" });
});
await test("buildPayload drops non-http page address", () => {
  const r = buildPayload("page", { url: "file:///a.html", title: "", lang: "", html: "<p>x</p>" });
  ok(r.ok && r.payload.url === null && r.payload.lang === null);
});
await test("buildPayload passes known page errors, maps unknown ones", () => {
  eq(buildPayload("page", { error: "too_large" }), { ok: false, error: "too_large" });
  eq(buildPayload("page", { error: "weird" }), { ok: false, error: "failed" });
});
await test("buildPayload limits", () => {
  eq(buildPayload("page", { html: "   " }), { ok: false, error: "empty" });
  eq(buildPayload("page", { html: "é".repeat(MAX_HTML_BYTES / 2 + 1) }), { ok: false, error: "too_large" });
  eq(buildPayload("selection", { text: "x".repeat(MAX_TEXT_CHARS + 1) }), { ok: false, error: "too_large" });
  eq(buildPayload("selection", { text: "\n " }), { ok: false, error: "empty" });
  eq(buildPayload("link", { url: "chrome://x" }), { ok: false, error: "restricted" });
  ok(buildPayload("link", { url: "https://a.example/b" }).ok);
});
await test("validatePayload rejects extras and mixed kinds", () => {
  eq(validatePayload(payload()), null);
  eq(validatePayload({ ...payload(), extra: 1 }), "failed");
  eq(validatePayload({ ...payload(), html: "<p>" }), "failed");
  eq(validatePayload({ ...payload({ kind: "link" }) }), "failed");
  eq(validatePayload({ ...payload(), v: 2 }), "failed");
  eq(validatePayload({ ...payload(), id: "../../x" }), "failed");
  eq(validatePayload({ ...payload(), url: "javascript:1" }), "failed");
  eq(validatePayload(null), "failed");
});

// handoff-store.js
/** @param {Partial<{quota: number, alive: Set<number>, clock: {t: number}}>} [o] */
function store(o = {}) {
  const area = memoryArea(o.quota);
  const clock = o.clock ?? { t: 0 };
  const alive = o.alive ?? new Set([1, 2, 3]);
  /** @type {Array<number | null>} */
  const sweeps = [];
  const s = createStore({
    area,
    ttlMs: 1000,
    leaseMs: 100,
    now: () => clock.t,
    tabExists: async (id) => alive.has(id),
    scheduleSweep: (when) => {
      sweeps.push(when);
    },
  });
  return { s, area, clock, alive, sweeps };
}
await test("claim is bound to the target tab; ack deletes", async () => {
  const { s, area } = store();
  const p = payload();
  await s.put(p, 1);
  eq(await s.claim(2), null);
  eq(await s.claim(1), p);
  eq(await s.claim(2), null);
  eq(await s.ack(2, p.id), false);
  eq(await s.ack(1, p.id), true);
  eq(area.m.size, 0);
  eq(await s.claim(1), null);
  eq(await s.ack(1, p.id), false);
});
await test("the tab holding the lease gets the handoff again at once (its reply was lost)", async () => {
  const { s, clock } = store();
  const p = payload();
  await s.put(p, 1);
  eq(await s.claim(1), p);
  clock.t = 10;
  eq(await s.claim(1), p);
  eq(await s.ack(1, p.id), true);
  eq(await s.claim(1), null);
});
await test("an unacknowledged lease keeps other tabs out until it runs out", async () => {
  const { s, clock, alive } = store();
  const p = payload();
  await s.put(p, 7);
  alive.delete(7);
  eq(await s.claim(1), p);
  clock.t = 99;
  eq(await s.claim(2), null);
  clock.t = 100;
  eq(await s.claim(2), p);
  eq(await s.ack(1, p.id), false);
  eq(await s.ack(2, p.id), true);
});
await test("put arms the sweep for the earliest expiry; sweep re-arms or cancels", async () => {
  const { s, clock, sweeps, area } = store();
  await s.put(payload(), 1);
  eq(sweeps, [2000]);
  clock.t = 400;
  await s.put(payload(), 2);
  eq(sweeps.at(-1), 2000);
  clock.t = 1001;
  await s.sweep();
  eq(area.m.size, 1);
  eq(sweeps.at(-1), 2400);
  clock.t = 1401;
  await s.sweep();
  eq(area.m.size, 0);
  eq(sweeps.at(-1), null);
});
await test("a failing scheduler does not fail the put", async () => {
  const area = memoryArea();
  const s = createStore({
    area,
    ttlMs: 1000,
    leaseMs: 100,
    tabExists: async () => true,
    scheduleSweep: async () => {
      throw new Error("no alarms");
    },
  });
  const p = payload();
  await s.put(p, 1);
  eq(await s.claim(1), p);
});
await test("a lease taken by another tab keeps the entry from its target tab", async () => {
  const { s, alive, clock } = store();
  const p = payload();
  await s.put(p, 7);
  alive.delete(7);
  eq(await s.claim(2), p);
  eq(await s.claim(3), null);
  eq(await s.ack(3, p.id), false);
  clock.t = 200;
  eq(await s.claim(3), p);
});
await test("orphaned handoff goes to any app tab", async () => {
  const { s, alive } = store();
  const p = payload();
  await s.put(p, 7);
  alive.delete(7);
  eq(await s.claim(2), p);
});
await test("expired handoffs are dropped", async () => {
  const { s, clock, area } = store();
  await s.put(payload(), 1);
  clock.t = 1000;
  eq(await s.claim(1), null);
  eq(area.m.size, 0);
});
await test("newest handoff for a tab wins", async () => {
  const { s } = store();
  const a = payload(), b = payload();
  await s.put(a, 1);
  await s.put(b, 1);
  eq((await s.claim(1))?.id, b.id);
  eq(await s.ack(1, b.id), true);
  eq((await s.claim(1))?.id, a.id);
});
await test("quota pressure evicts the oldest pending handoff", async () => {
  const { s, area } = store({ quota: 700 });
  const a = payload({ text: "a".repeat(200) }), b = payload({ text: "b".repeat(200) }), c = payload({ text: "c".repeat(200) });
  await s.put(a, 1);
  await s.put(b, 2);
  await s.put(c, 3);
  ok(!area.m.has("handoff:" + a.id), "oldest kept");
  ok(area.m.has("handoff:" + c.id), "newest missing");
});
await test("a payload larger than the quota fails", async () => {
  const { s } = store({ quota: 10 });
  let threw = false;
  try {
    await s.put(payload(), 1);
  } catch {
    threw = true;
  }
  ok(threw);
});
await test("mutex runs in order and survives a failure", async () => {
  const m = createMutex();
  /** @type {number[]} */
  const log = [];
  const a = m.run(async () => { await new Promise((r) => setTimeout(r, 20)); log.push(1); });
  const b = m.run(async () => { log.push(2); throw new Error("x"); });
  const c = m.run(async () => { log.push(3); return 3; });
  await a;
  await b.catch(() => {});
  eq(await c, 3);
  eq(log, [1, 2, 3]);
});

// collect.js
/**
 * @param {Partial<import("../src/payload.js").PageResult> | Error} result  What the page script returns, or throws.
 * @returns {{ run: import("../src/collect.js").RunExtractor, calls: unknown[][] }}
 */
function extractor(result) {
  /** @type {unknown[][]} */
  const calls = [];
  return {
    calls,
    run: async (...args) => {
      calls.push(args);
      if (result instanceof Error) throw result;
      return result;
    },
  };
}
const ARTICLE_TAB = { id: 5, url: "https://news.example/a", title: "A" };
await test("collect refuses the Web Store without running the page script or sending a link", async () => {
  for (const url of ["https://chromewebstore.google.com/detail/x", "https://chrome.google.com/webstore/detail/x"]) {
    for (const kind of /** @type {const} */ (["page", "selection"])) {
      const x = extractor({ type: "text/html", html: "<p>x</p>", text: "x" });
      eq(await collect({ id: 5, url, title: "Store" }, { kind, selectionText: "x" }, x.run), { ok: false, error: "restricted" });
      eq(x.calls.length, 0);
    }
  }
});
await test("collect refuses pages whose address the extension cannot see or script", async () => {
  for (const url of [undefined, "chrome://version/", "chrome-extension://abc/page.html"]) {
    const x = extractor({ type: "text/html", html: "<p>x</p>" });
    eq(await collect({ id: 5, url, title: "" }, { kind: "page" }, x.run), { ok: false, error: "restricted" });
  }
});
await test("collect sends the page script's HTML", async () => {
  const x = extractor({ url: ARTICLE_TAB.url, title: "A", lang: "en", type: "text/html", html: "<p>x</p>" });
  const r = await collect(ARTICLE_TAB, { kind: "page" }, x.run);
  ok(r.ok && r.payload.kind === "page" && r.payload.html === "<p>x</p>");
  eq(x.calls, [[5, 0, "page"]]);
});
await test("collect sends the PDF viewer as a link", async () => {
  const tab = { id: 5, url: "https://papers.example/view?id=3", title: "Paper" };
  const r = await collect(tab, { kind: "page" }, extractor({ type: "application/pdf", error: "restricted" }).run);
  ok(r.ok && r.payload.kind === "link" && r.payload.url === tab.url, JSON.stringify(r));
});
await test("collect sends a .pdf address as a link when the viewer cannot be scripted", async () => {
  const tab = { id: 5, url: "https://papers.example/a/Report.PDF", title: "Report" };
  const r = await collect(tab, { kind: "page" }, extractor(new Error("Cannot access contents of the page")).run);
  ok(r.ok && r.payload.kind === "link" && r.payload.url === tab.url, JSON.stringify(r));
  const file = await collect({ id: 5, url: "file:///Users/me/a.pdf", title: "" }, { kind: "page" }, extractor(new Error("x")).run);
  eq(file, { ok: false, error: "restricted" });
  eq(await collect(ARTICLE_TAB, { kind: "page" }, extractor(new Error("x")).run), { ok: false, error: "failed" });
});
await test("collect falls back to Chrome's selection text when the page cleared its selection", async () => {
  const x = extractor({ url: ARTICLE_TAB.url, title: "A", lang: "fr", error: "empty" });
  const r = await collect(ARTICLE_TAB, { kind: "selection", frameId: 2, selectionText: "Bonjour" }, x.run);
  ok(r.ok && r.payload.kind === "selection" && r.payload.text === "Bonjour" && r.payload.lang === "fr", JSON.stringify(r));
  eq(x.calls, [[5, 2, "selection"]]);
  eq(await collect(ARTICLE_TAB, { kind: "selection", selectionText: "  " }, x.run), { ok: false, error: "empty" });
  const big = extractor({ error: "too_large" });
  eq(await collect(ARTICLE_TAB, { kind: "selection", selectionText: "x" }, big.run), { ok: false, error: "too_large" });
});
await test("collect falls back to Chrome's selection text when the frame cannot be scripted", async () => {
  const r = await collect(ARTICLE_TAB, { kind: "selection", selectionText: "hi" }, extractor(new Error("x")).run);
  ok(r.ok && r.payload.text === "hi");
  eq(await collect(ARTICLE_TAB, { kind: "selection" }, extractor(new Error("x")).run), { ok: false, error: "failed" });
});
await test("collect sends links without running the page script", async () => {
  const x = extractor(new Error("must not run"));
  const r = await collect(ARTICLE_TAB, { kind: "link", linkUrl: "https://b.example/c" }, x.run);
  ok(r.ok && r.payload.kind === "link" && r.payload.url === "https://b.example/c");
  eq(x.calls.length, 0);
});
await test("looksLikePdf", () => {
  ok(looksLikePdf("https://a.example/x.pdf?dl=1"));
  ok(!looksLikePdf("https://a.example/pdf"));
  ok(!looksLikePdf(undefined));
});

// claim.js
const APP = "https://listen.streams.watch";
/** @param {object} over */
const sender = (over) => ({ id: "ext", frameId: 0, origin: APP, url: APP + "/app/import?from=extension", tab: { id: 9 }, ...over });
await test("trustedAppTab accepts the app's top frame", () => {
  eq(trustedAppTab(/** @type {any} */ (sender({})), "ext", APP), 9);
  eq(trustedAppTab(/** @type {any} */ (sender({ url: APP + "/app" })), "ext", APP), 9);
});
await test("trustedAppTab rejects everything else", () => {
  for (const over of [
    { id: "other" },
    { frameId: 3 },
    { tab: undefined },
    { origin: "https://evil.example" },
    { url: "https://evil.example/app/" },
    { url: APP + "/" },
    { url: APP + "/application" },
    { origin: "http://listen.streams.watch" },
  ]) eq(trustedAppTab(/** @type {any} */ (sender(over)), "ext", APP), null);
});

// extract.js (runs against this page)
await test("extractPage strips scripts, styles and svg but keeps the article", () => {
  const r = extractPage(MAX_HTML_BYTES);
  ok(r.html, r.error);
  const html = r.html ?? "";
  ok(html.includes("UNIT_ARTICLE_MARKER"));
  ok(!html.includes("UNIT_SCRIPT_MARKER"), "script survived");
  ok(!html.includes("color: red"), "style survived");
  ok(html.startsWith("<!DOCTYPE html>"));
  eq(r.lang, "en-GB");
  ok(document.querySelector("script"), "live page was modified");
});
await test("extractPage falls back to <article> when the page is too big", () => {
  const big = document.createElement("div");
  big.textContent = "z".repeat(50_000);
  document.body.append(big);
  const r = extractPage(20_000);
  big.remove();
  ok(r.html && r.html.includes("UNIT_ARTICLE_MARKER") && !r.html.includes("zzzz"));
  eq(extractPage(10).error, "too_large");
});
await test("extractPage keeps article text inside open shadow roots, slots filled as rendered", () => {
  const host = document.createElement("div");
  host.id = "shadow-host";
  host.innerHTML = '<span slot="byline">UNIT_SLOT_MARKER</span><span>UNIT_UNSLOTTED_MARKER</span>';
  const root = host.attachShadow({ mode: "open" });
  root.innerHTML =
    '<style>.y { color: blue; }</style><article><h2>Shadow</h2><p>UNIT_SHADOW_MARKER readable words.</p>' +
    '<slot name="byline">fallback</slot><slot name="none">UNIT_FALLBACK_MARKER</slot><div id="inner"></div></article>';
  const inner = /** @type {HTMLElement} */ (root.getElementById("inner")).attachShadow({ mode: "open" });
  inner.innerHTML = "<p>UNIT_NESTED_MARKER</p><script>window.UNIT_SHADOW_SCRIPT = 1;<\/script>";
  const closed = document.createElement("div");
  closed.attachShadow({ mode: "closed" }).innerHTML = "<p>UNIT_CLOSED_MARKER</p>";
  document.body.append(host, closed);
  try {
    const html = extractPage(MAX_HTML_BYTES).html ?? "";
    for (const m of ["UNIT_ARTICLE_MARKER", "UNIT_SHADOW_MARKER", "UNIT_SLOT_MARKER", "UNIT_FALLBACK_MARKER", "UNIT_NESTED_MARKER"]) {
      ok(html.includes(m), `${m} missing`);
    }
    ok(!html.includes("UNIT_UNSLOTTED_MARKER"), "unassigned light child was kept");
    ok(!html.includes("UNIT_CLOSED_MARKER"), "closed shadow root was read");
    ok(!html.includes("shadowrootmode") && !html.includes("<slot"), "shadow markup left in the HTML");
    ok(!html.includes("color: blue") && !html.includes("UNIT_SHADOW_SCRIPT") && !html.includes("UNIT_SCRIPT_MARKER"), "style or script survived");
    ok(/<article><h2>Shadow<\/h2>/.test(html), "shadow article not in place");
    ok(html.includes('lang="en-GB"'), "html attributes lost");
    ok(document.querySelector("script") && root.querySelector("style"), "live page was modified");

    // Chrome before 125 has no getHTML: the light DOM is still sent.
    const proto = /** @type {any} */ (Element.prototype);
    const getHTML = proto.getHTML;
    proto.getHTML = undefined;
    try {
      const old = extractPage(MAX_HTML_BYTES).html ?? "";
      ok(old.includes("UNIT_ARTICLE_MARKER") && !old.includes("UNIT_SHADOW_MARKER"));
    } finally {
      proto.getHTML = getHTML;
    }
  } finally {
    host.remove();
    closed.remove();
  }
});
await test("extractSelection reads the selection and text fields", () => {
  const sel = /** @type {Selection} */ (window.getSelection());
  sel.removeAllRanges();
  eq(extractSelection(100).error, "empty");
  const range = document.createRange();
  range.selectNodeContents(/** @type {Node} */ (document.getElementById("para")));
  sel.addRange(range);
  eq(extractSelection(1000).text, "UNIT_ARTICLE_MARKER Some readable words for the extractor.");
  eq(extractSelection(5).error, "too_large");
  sel.removeAllRanges();
  const field = /** @type {HTMLTextAreaElement} */ (document.getElementById("field"));
  field.focus();
  field.setSelectionRange(6, 10);
  eq(extractSelection(100).text, "beta");
});

const out = document.getElementById("out");
if (out) out.textContent = failures.length ? `FAIL\n${failures.join("\n")}` : `PASS ${passed}`;
// @ts-ignore test hook read by e2e.py
window.__unit = { passed, failures };
