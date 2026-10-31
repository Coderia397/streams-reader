// @ts-check
// Mock web app for tests/e2e.py: waits for a hostile cross-origin frame to try first, then claims the handoff twice
// (the second claim must find nothing) and publishes what happened on window.__mockApp. Uses receiver.js exactly as
// the real app should.
import { extensionVersion, requestHandoff } from "./receiver.js";

/** @type {string[]} */
const foreignReplies = [];
/** @type {string[]} */
const log = [];
const t0 = performance.now();
// @ts-ignore test hook
window.__mockLog = log;
window.addEventListener("message", (e) => {
  const d = e.data;
  if (d && typeof d === "object") log.push(`${Math.round(performance.now() - t0)} ${d.source} ${d.type} ${d.requestId}`);
  if (d && d.source === "streams-reader-extension" && d.requestId === "spoof") foreignReplies.push(d.type);
});

const frame = document.createElement("iframe");
frame.src = `http://127.0.0.1:${location.port}/fixtures/spoof.html`;
const framed = new Promise((r) => frame.addEventListener("load", r, { once: true }));
document.body.append(frame);

await framed;
await new Promise((r) => setTimeout(r, 400));

// Lost-reply scenario (set by e2e.py): give up on the first reply at once, so it is lost unacknowledged, then reload
// right away, well inside the 30 s lease. The reloaded page (same tab) must receive the same handoff.
let lost = false;
try {
  lost = localStorage.getItem("sr-lose-first") === "1";
  localStorage.removeItem("sr-lose-first");
} catch {
  // Storage blocked: run the normal scenario.
}
if (lost) {
  await requestHandoff({ timeoutMs: 0 });
  location.reload();
  await new Promise(() => {}); // Never finish this run.
}

const first = await requestHandoff();
const second = await requestHandoff();
const out = document.getElementById("out");
if (out) out.textContent = first ? `${first.kind}: ${first.title}` : "nothing received";
// @ts-ignore test hook
window.__mockApp = { done: true, log, extension: extensionVersion(), href: location.href, first, second, foreignReplies };
