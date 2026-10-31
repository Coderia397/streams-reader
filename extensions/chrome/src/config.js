// @ts-check
/**
 * Build-time settings. `package.py --app-origin` rewrites APP_ORIGIN for staging and test builds; keep it on one
 * line in this exact form so the rewrite finds it.
 */

export const APP_ORIGIN = "https://listen.streams.watch";

/** Where the app is opened. Only a fixed marker goes in the query; page content never does. */
export const APP_IMPORT_PATH = "/app/import?from=extension";

/** Version of the handoff message format (see notes, "Extension handoff"). */
export const PROTOCOL_VERSION = 1;

/**
 * Largest page HTML sent, in UTF-8 bytes. chrome.storage.session holds 10 MiB in total and stores JSON, so 6 MiB
 * leaves room for escaping and a second pending handoff.
 */
export const MAX_HTML_BYTES = 6 * 1024 * 1024;

/** Largest selected text sent, in UTF-16 code units. */
export const MAX_TEXT_CHARS = 1024 * 1024;

/** Longest title kept. */
export const MAX_TITLE_CHARS = 300;

/** Longest URL accepted (Chrome's own limit is 2 MiB; real article URLs are far below this). */
export const MAX_URL_CHARS = 8 * 1024;

/** A handoff not claimed by the app within this time is dropped. */
export const HANDOFF_TTL_MS = 5 * 60 * 1000;

/** A claimed handoff not acknowledged by the app within this time can be claimed again. */
export const HANDOFF_LEASE_MS = 30 * 1000;

/** How long the error badge stays on the toolbar button. */
export const BADGE_MS = 6000;

/** @returns {string} */
export function appImportUrl() {
  return APP_ORIGIN + APP_IMPORT_PATH;
}
