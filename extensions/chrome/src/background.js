// @ts-check
/**
 * Service worker: wires the toolbar button, the keyboard shortcut (Chrome routes `_execute_action` to
 * action.onClicked), the context menu and the app's claim message to the read flow, and the sweep alarm to the
 * store. Listeners are registered at the top level so Chrome can wake the worker for them.
 */

import { SWEEP_ALARM, lock, read, store } from "./actions.js";
import { claimListener } from "./claim.js";
import { MenuIds, installMenus, requestFor } from "./menus.js";

chrome.runtime.onInstalled.addListener(() => {
  installMenus();
});

chrome.runtime.onStartup.addListener(() => {
  store.sweep().catch(() => {});
});

// Removes handoffs the app never collected once they expire (the store arms the alarm on each put and re-arms it
// for the next expiry after each sweep).
chrome.alarms.onAlarm.addListener((alarm) => {
  if (alarm.name === SWEEP_ALARM) store.sweep().catch(() => {});
});

chrome.action.onClicked.addListener((tab) => {
  read(tab, { kind: "page" });
});

chrome.contextMenus.onClicked.addListener((info, tab) => {
  if (!tab || !Object.values(MenuIds).includes(/** @type {string} */ (info.menuItemId))) return;
  read(tab, requestFor(info));
});

chrome.runtime.onMessage.addListener(claimListener(store, lock));
