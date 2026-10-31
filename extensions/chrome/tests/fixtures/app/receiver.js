// @ts-check
/**
 * Reference receiver for the extension handoff, as the web app should implement it (the real app does the same in
 * Rust/wasm-bindgen; see the extension handoff note). Used by the mock app in the end-to-end test.
 */

const MAX_HTML_BYTES = 6 * 1024 * 1024;
const MAX_TEXT_CHARS = 1024 * 1024;

/** @returns {string | null} the extension's version when its bridge is on this page */
export function extensionVersion() {
  return document.documentElement.dataset.streamsReaderExtension ?? null;
}

/**
 * Asks the extension for the pending handoff. Resolves to the validated payload, or null when there is none, the
 * extension is not installed, or it does not answer in time. A valid payload is acknowledged at once, which deletes
 * it in the extension; a reply that arrives after the timeout is not acknowledged, so the extension offers it again:
 * at once to the same tab (a reload, or a second ask), and to other tabs after its 30 s lease.
 * @param {{ timeoutMs?: number }} [opts]
 * @returns {Promise<object | null>}
 */
export function requestHandoff({ timeoutMs = 10000 } = {}) {
  if (!extensionVersion()) return Promise.resolve(null);
  const origin = location.origin;
  const requestId = crypto.randomUUID();
  return new Promise((resolve) => {
    /** @param {MessageEvent} e */
    const onMessage = (e) => {
      if (e.source !== window || e.origin !== origin) return;
      const d = e.data;
      if (!d || d.source !== "streams-reader-extension" || d.requestId !== requestId) return;
      const payload = d.type === "handoff:payload" ? validate(d.payload) : null;
      if (payload) window.postMessage({ source: "streams-reader-app", type: "handoff:ack", v: 1, id: payload.id }, origin);
      done(payload);
    };
    const timer = setTimeout(() => done(null), timeoutMs);
    /** @param {object | null} value */
    const done = (value) => {
      clearTimeout(timer);
      window.removeEventListener("message", onMessage);
      resolve(value);
    };
    window.addEventListener("message", onMessage);
    window.postMessage({ source: "streams-reader-app", type: "handoff:ready", v: 1, requestId }, origin);
  });
}

/**
 * The receiver's own checks; the extension already applies them, but the page cannot know who posted.
 * @param {any} p
 * @returns {object | null}
 */
export function validate(p) {
  if (!p || typeof p !== "object" || p.v !== 1 || typeof p.id !== "string") return null;
  if (!["page", "selection", "link"].includes(p.kind)) return null;
  if (typeof p.title !== "string" || p.title.length > 300) return null;
  if (p.url !== null && !isHttp(p.url)) return null;
  if (p.kind === "page" && (typeof p.html !== "string" || new Blob([p.html]).size > MAX_HTML_BYTES)) return null;
  if (p.kind === "selection" && (typeof p.text !== "string" || p.text.length > MAX_TEXT_CHARS)) return null;
  if (p.kind === "link" && p.url === null) return null;
  return p;
}

/** @param {unknown} u */
function isHttp(u) {
  try {
    const p = new URL(String(u)).protocol;
    return p === "https:" || p === "http:";
  } catch {
    return false;
  }
}
