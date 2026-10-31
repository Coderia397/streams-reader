// @ts-check
/**
 * Content script on the web app's own pages (APP_ORIGIN/app/*), top frame only, from document_start. It is the only
 * place the payload enters the app, and it does so only when the app asks:
 *
 *   app  → window.postMessage({ source: "streams-reader-app", type: "handoff:ready", v: 1, requestId }, origin)
 *   here → chrome.runtime.sendMessage({ type: "handoff:claim" })   (the worker checks this tab, origin and path)
 *   here → window.postMessage({ source: "streams-reader-extension", type: "handoff:payload" | "handoff:none",
 *                               v: 1, requestId, payload? }, origin)
 *   app  → window.postMessage({ source: "streams-reader-app", type: "handoff:ack", v: 1, id: payload.id }, origin)
 *   here → chrome.runtime.sendMessage({ type: "handoff:ack", id })      (deletes it; without an ack the same tab
 *                                                                        can claim it again at once, other tabs
 *                                                                        after the 30 s lease)
 *
 * It also marks <html data-streams-reader-extension="<version>"> so the app can tell the extension is installed.
 * Content scripts cannot be ES modules, so this file imports nothing.
 */
(() => {
  const VERSION = 1;
  const origin = location.origin;

  try {
    document.documentElement.dataset.streamsReaderExtension = chrome.runtime.getManifest().version;
  } catch {
    // Orphaned after an extension update: the page reloads with a fresh script.
  }

  /**
   * @param {"handoff:payload" | "handoff:none"} type
   * @param {unknown} requestId
   * @param {unknown} [payload]
   */
  const reply = (type, requestId, payload) => {
    /** @type {Record<string, unknown>} */
    const msg = { source: "streams-reader-extension", type, v: VERSION, requestId };
    if (payload !== undefined) msg.payload = payload;
    window.postMessage(msg, origin);
  };

  window.addEventListener("message", (event) => {
    // Only the app's own top window may ask; iframes and other origins are ignored.
    if (event.source !== window || event.origin !== origin) return;
    const d = event.data;
    if (!d || typeof d !== "object" || d.source !== "streams-reader-app") return;
    if (d.type === "handoff:ack") {
      if (d.v === VERSION && typeof d.id === "string") {
        try {
          Promise.resolve(chrome.runtime.sendMessage({ type: "handoff:ack", id: d.id.slice(0, 36) })).catch(() => {});
        } catch {
          // Orphaned content script: the lease simply runs out.
        }
      }
      return;
    }
    if (d.type !== "handoff:ready") return;
    if (d.v !== VERSION) {
      reply("handoff:none", d.requestId);
      return;
    }
    const requestId = typeof d.requestId === "string" ? d.requestId.slice(0, 100) : null;
    let sent;
    try {
      sent = chrome.runtime.sendMessage({ type: "handoff:claim" });
    } catch {
      reply("handoff:none", requestId);
      return;
    }
    Promise.resolve(sent)
      .then((res) => {
        if (res && res.ok && res.payload) reply("handoff:payload", requestId, res.payload);
        else reply("handoff:none", requestId);
      })
      .catch(() => reply("handoff:none", requestId));
  });
})();
