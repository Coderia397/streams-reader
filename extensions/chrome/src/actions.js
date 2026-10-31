// @ts-check
/**
 * The read flow: take the page (or selection, or link) from the tab the user acted on, store it as a pending
 * handoff, and open the app next to that tab. The app claims the handoff through app-bridge.js once it has loaded.
 */

import { BADGE_MS, HANDOFF_LEASE_MS, HANDOFF_TTL_MS, MAX_HTML_BYTES, MAX_TEXT_CHARS, appImportUrl } from "./config.js";
import { collect } from "./collect.js";
import { extractPage, extractSelection } from "./extract.js";
import { createMutex, createStore } from "./handoff-store.js";
import { Errors } from "./payload.js";

/** @typedef {import("./payload.js").Kind} Kind */
/** @typedef {import("./payload.js").PageResult} PageResult */
/** @typedef {import("./collect.js").ReadRequest} ReadRequest */

/** Name of the alarm that removes handoffs the app never collected. background.js listens for it. */
export const SWEEP_ALARM = "handoff-sweep";

export const store = createStore({
  area: chrome.storage.session,
  ttlMs: HANDOFF_TTL_MS,
  leaseMs: HANDOFF_LEASE_MS,
  scheduleSweep: async (when) => {
    if (when === null) await chrome.alarms.clear(SWEEP_ALARM);
    else await chrome.alarms.create(SWEEP_ALARM, { when });
  },
  tabExists: async (tabId) => {
    try {
      await chrome.tabs.get(tabId);
      return true;
    } catch {
      return false;
    }
  },
});

/** Serialises "open tab + store" against "claim", so a fast app tab never finds its handoff missing. */
export const lock = createMutex();

/**
 * Runs an extractor in one frame of the tab. Throws when the page cannot be scripted.
 * @param {number} tabId
 * @param {number} frameId
 * @param {Kind} kind
 * @returns {Promise<PageResult>}
 */
async function runExtractor(tabId, frameId, kind) {
  const [res] = await chrome.scripting.executeScript({
    target: { tabId, frameIds: [frameId] },
    func: kind === "page" ? extractPage : extractSelection,
    args: [kind === "page" ? MAX_HTML_BYTES : MAX_TEXT_CHARS],
    injectImmediately: true,
  });
  if (!res || !res.result) throw new Error("no result from page script");
  return /** @type {PageResult} */ (res.result);
}

/**
 * The whole flow for one user action. Resolves to the opened app tab's id, or to an error code after showing it.
 * @param {chrome.tabs.Tab} tab
 * @param {ReadRequest} req
 * @returns {Promise<{ ok: true, appTabId: number } | { ok: false, error: string }>}
 */
export async function read(tab, req) {
  if (tab.id === undefined || tab.id === chrome.tabs.TAB_ID_NONE) return { ok: false, error: Errors.failed };
  const sourceTabId = tab.id;
  const built = await collect(tab, req, runExtractor);
  if (!built.ok) {
    await showError(sourceTabId, built.error);
    return built;
  }
  const payload = built.payload;
  try {
    return await lock.run(async () => {
      const appTab = await chrome.tabs.create({
        url: appImportUrl(),
        index: tab.index + 1,
        openerTabId: sourceTabId,
        windowId: tab.windowId,
        active: true,
      });
      const appTabId = /** @type {number} */ (appTab.id);
      try {
        await store.put(payload, appTabId);
      } catch (e) {
        await chrome.tabs.remove(appTabId).catch(() => {});
        throw e;
      }
      return /** @type {const} */ ({ ok: true, appTabId });
    });
  } catch {
    await showError(sourceTabId, Errors.failed);
    return { ok: false, error: Errors.failed };
  }
}

/**
 * Pending badge resets, per tab, so a newer error is not cleared by an older error's timer.
 * @type {Map<number, ReturnType<typeof setTimeout>>}
 */
const badgeTimers = new Map();

/**
 * Marks the toolbar button on that tab with "!" and puts the reason in its tooltip for a few seconds, then restores
 * the manifest's tooltip.
 * @param {number} tabId
 * @param {string} code
 */
export async function showError(tabId, code) {
  const message = chrome.i18n.getMessage(`error_${code}`) || chrome.i18n.getMessage("error_failed");
  clearTimeout(badgeTimers.get(tabId));
  badgeTimers.delete(tabId);
  try {
    await chrome.action.setBadgeBackgroundColor({ tabId, color: "#B3261E" });
    await chrome.action.setBadgeText({ tabId, text: "!" });
    await chrome.action.setTitle({ tabId, title: message });
  } catch {
    return; // The tab closed.
  }
  // Another error on this tab may have started while the calls above were awaited; keep only the newest timer.
  clearTimeout(badgeTimers.get(tabId));
  badgeTimers.set(
    tabId,
    setTimeout(() => {
      badgeTimers.delete(tabId);
      chrome.action.setBadgeText({ tabId, text: "" }).catch(() => {});
      // An empty tab title would show the extension's name; restore the button's own tooltip instead.
      chrome.action.setTitle({ tabId, title: chrome.i18n.getMessage("action_title") }).catch(() => {});
    }, BADGE_MS),
  );
}
