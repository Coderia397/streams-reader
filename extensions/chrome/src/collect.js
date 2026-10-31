// @ts-check
/**
 * Decides what one user action sends: the page's HTML, the selection, a link, or an error code. The page script
 * runner is injected so tests/unit.js can drive every branch without the extension APIs.
 *
 * Rules:
 * - Pages no extension may script (browser UI, the Web Store, other extensions' pages) are refused.
 * - Chrome's PDF viewer is the only page sent as a link (the app fetches the PDF). It is recognised by the page
 *   script reporting `application/pdf`, or, when the script cannot run in the viewer, by a `.pdf` path.
 * - A selection falls back to Chrome's own copy of the selected text when the page script cannot see it (the frame
 *   cannot be scripted, or the page cleared its selection on mousedown or contextmenu).
 */

import { Errors, buildPayload, isScriptable } from "./payload.js";

/** @typedef {import("./payload.js").Kind} Kind */
/** @typedef {import("./payload.js").PageResult} PageResult */
/** @typedef {ReturnType<typeof buildPayload>} Built */

/**
 * @typedef {object} ReadRequest
 * @property {Kind} kind
 * @property {number} [frameId]      Frame of the selection (context menu), default the top frame.
 * @property {string} [selectionText] Chrome's copy of the selection, used when the page script cannot see it.
 * @property {string} [linkUrl]      kind "link": the link's address.
 */

/**
 * Runs the page script for `kind` in one frame of the tab; throws when the frame cannot be scripted.
 * @typedef {(tabId: number, frameId: number, kind: Kind) => Promise<PageResult>} RunExtractor
 */

/**
 * @param {string | undefined} url
 * @returns {boolean} true when the address names a PDF file
 */
export function looksLikePdf(url) {
  try {
    return /\.pdf$/i.test(new URL(String(url)).pathname);
  } catch {
    return false;
  }
}

/**
 * Collects what the user asked for from `tab`. Never throws; returns an error code instead.
 * @param {{ id?: number, url?: string, title?: string }} tab
 * @param {ReadRequest} req
 * @param {RunExtractor} runExtractor
 * @returns {Promise<Built>}
 */
export async function collect(tab, req, runExtractor) {
  if (req.kind === "link") {
    return buildPayload("link", { url: req.linkUrl, title: "", lang: "" });
  }
  if (!isScriptable(tab.url)) return { ok: false, error: Errors.restricted };

  /** @param {string} lang @returns {Built | null} */
  const chromeSelection = (lang) =>
    req.kind === "selection" && req.selectionText?.trim()
      ? buildPayload("selection", { url: tab.url, title: tab.title, lang, text: req.selectionText })
      : null;

  let viewer = false;
  try {
    const result = await runExtractor(/** @type {number} */ (tab.id), req.frameId ?? 0, req.kind);
    if (result.type === "application/pdf") {
      viewer = true;
    } else {
      if (result.error === Errors.empty) {
        const fallback = chromeSelection(result.lang ?? "");
        if (fallback) return fallback;
      }
      return buildPayload(req.kind, result);
    }
  } catch {
    // The frame is cross-origin, the page blocks scripting, the tab navigated meanwhile, or this is the PDF viewer.
    viewer = looksLikePdf(tab.url);
  }
  const fallback = chromeSelection("");
  if (fallback) return fallback;
  if (req.kind === "page" && viewer) {
    // The app fetches the PDF itself. A file: PDF has no address the app can open, so this is "restricted".
    return buildPayload("link", { url: tab.url, title: tab.title, lang: "" });
  }
  return { ok: false, error: Errors.failed };
}
