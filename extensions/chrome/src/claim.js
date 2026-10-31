// @ts-check
/**
 * Answers app-bridge.js when the app claims or acknowledges its handoff. Only the top frame of a tab on the app
 * origin, under /app/, running this extension's own content script, is answered.
 */

import { APP_ORIGIN } from "./config.js";

/**
 * @param {chrome.runtime.MessageSender} sender
 * @param {string} extensionId
 * @param {string} appOrigin
 * @returns {number | null} the sender's tab id when it may claim, else null
 */
export function trustedAppTab(sender, extensionId, appOrigin = APP_ORIGIN) {
  if (sender.id !== extensionId) return null;
  if (sender.frameId !== 0 || !sender.tab || typeof sender.tab.id !== "number") return null;
  if (sender.origin !== appOrigin || typeof sender.url !== "string") return null;
  let u;
  try {
    u = new URL(sender.url);
  } catch {
    return null;
  }
  if (u.origin !== appOrigin || !(u.pathname === "/app" || u.pathname.startsWith("/app/"))) return null;
  return sender.tab.id;
}

/**
 * Builds the runtime.onMessage listener.
 * @param {{ claim(tabId: number): Promise<unknown>, ack(tabId: number, id: string): Promise<boolean> }} store
 * @param {{ run<T>(fn: () => Promise<T>): Promise<T> }} lock
 */
export function claimListener(store, lock) {
  /**
   * @param {unknown} message
   * @param {chrome.runtime.MessageSender} sender
   * @param {(response: unknown) => void} sendResponse
   */
  return (message, sender, sendResponse) => {
    const m = /** @type {{ type?: unknown, id?: unknown } | null} */ (message);
    if (!m || (m.type !== "handoff:claim" && m.type !== "handoff:ack")) return false;
    const tabId = trustedAppTab(sender, chrome.runtime.id);
    if (tabId === null) {
      sendResponse({ ok: false });
      return false;
    }
    if (m.type === "handoff:ack") {
      const id = m.id;
      if (typeof id !== "string" || !/^[0-9a-f-]{36}$/.test(id)) {
        sendResponse({ ok: false });
        return false;
      }
      lock
        .run(() => store.ack(tabId, id))
        .then((ok) => sendResponse({ ok }))
        .catch(() => sendResponse({ ok: false }));
      return true;
    }
    lock
      .run(() => store.claim(tabId))
      .then((payload) => sendResponse(payload ? { ok: true, payload } : { ok: false }))
      .catch(() => sendResponse({ ok: false }));
    return true; // Responds asynchronously.
  };
}
