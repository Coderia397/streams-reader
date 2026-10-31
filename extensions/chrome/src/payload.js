// @ts-check
/**
 * The handoff payload: built from what the page script returned, validated before it is stored. Pure functions,
 * unit tested in tests/unit.js. The web app must apply the same checks on receipt (it cannot trust the sender).
 */

import {
  MAX_HTML_BYTES,
  MAX_TEXT_CHARS,
  MAX_TITLE_CHARS,
  MAX_URL_CHARS,
  PROTOCOL_VERSION,
} from "./config.js";

/** @typedef {"page" | "selection" | "link"} Kind */

/**
 * @typedef {object} Payload
 * @property {number} v          Protocol version.
 * @property {string} id         Random UUID of this handoff.
 * @property {Kind} kind
 * @property {number} createdAt  Milliseconds since the epoch.
 * @property {string | null} url Page (or link) address; http(s) only, else null.
 * @property {string} title      Page title, trimmed; may be empty.
 * @property {string | null} lang BCP 47 tag from `<html lang>`, or null.
 * @property {string} [html]     kind "page": the page's rendered HTML with scripts and styles removed.
 * @property {string} [text]     kind "selection": the selected text.
 */

/** Error codes shown to the user (each has a `error_<code>` message in _locales). */
export const Errors = Object.freeze({
  restricted: "restricted",
  empty: "empty",
  tooLarge: "too_large",
  failed: "failed",
});

const LANG_RE = /^[A-Za-z]{2,3}(?:-[A-Za-z0-9]{1,8}){0,4}$/;

/**
 * @param {unknown} value
 * @returns {string | null} The URL if it is http(s) and not too long, else null.
 */
export function httpUrl(value) {
  if (typeof value !== "string" || value.length === 0 || value.length > MAX_URL_CHARS) return null;
  try {
    const u = new URL(value);
    return u.protocol === "https:" || u.protocol === "http:" ? u.href : null;
  } catch {
    return null;
  }
}

/**
 * Pages no extension may script (browser UI, the Web Store, other extensions). The read flow refuses them; their
 * address is not sent as a link either, because the app's link import cannot read them. Unknown addresses (the
 * extension may not see the URL) count as not scriptable.
 * @param {string | undefined} url
 */
export function isScriptable(url) {
  if (!url) return false;
  let u;
  try {
    u = new URL(url);
  } catch {
    return false;
  }
  if (u.protocol === "file:") return true;
  if (u.protocol !== "http:" && u.protocol !== "https:") return false;
  const host = u.hostname;
  return !(host === "chromewebstore.google.com" || (host === "chrome.google.com" && u.pathname.startsWith("/webstore")));
}

/**
 * @param {unknown} value
 * @returns {string | null}
 */
export function cleanLang(value) {
  if (typeof value !== "string") return null;
  const tag = value.trim();
  return LANG_RE.test(tag) ? tag : null;
}

/**
 * @param {unknown} value
 * @returns {string}
 */
export function cleanTitle(value) {
  if (typeof value !== "string") return "";
  const t = value.replace(/\s+/g, " ").trim();
  return t.length > MAX_TITLE_CHARS ? t.slice(0, MAX_TITLE_CHARS - 1).trimEnd() + "…" : t;
}

/**
 * UTF-8 length of a string without allocating the encoded bytes.
 * @param {string} s
 */
export function utf8Length(s) {
  let n = 0;
  for (let i = 0; i < s.length; i++) {
    const c = s.charCodeAt(i);
    if (c < 0x80) n += 1;
    else if (c < 0x800) n += 2;
    else if (c >= 0xd800 && c <= 0xdbff && i + 1 < s.length) {
      n += 4;
      i++;
    } else n += 3;
  }
  return n;
}

/**
 * @typedef {object} PageResult  What the page script returns (see extract.js).
 * @property {string} [url]
 * @property {string} [title]
 * @property {string} [lang]
 * @property {string} [html]
 * @property {string} [text]
 * @property {string} [type]   document.contentType (page only).
 * @property {string} [error]
 */

/**
 * Builds a payload from the page script's result, or returns an error code.
 * @param {Kind} kind
 * @param {PageResult} r
 * @param {{ id?: string, now?: number }} [opts]
 * @returns {{ ok: true, payload: Payload } | { ok: false, error: string }}
 */
export function buildPayload(kind, r, opts = {}) {
  if (r.error) {
    const known = /** @type {string[]} */ (Object.values(Errors)).includes(r.error);
    return { ok: false, error: known ? r.error : Errors.failed };
  }
  /** @type {Payload} */
  const payload = {
    v: PROTOCOL_VERSION,
    id: opts.id ?? crypto.randomUUID(),
    kind,
    createdAt: opts.now ?? Date.now(),
    url: httpUrl(r.url),
    title: cleanTitle(r.title),
    lang: cleanLang(r.lang),
  };
  if (kind === "page") payload.html = r.html;
  if (kind === "selection") payload.text = r.text;
  const err = validatePayload(payload);
  return err ? { ok: false, error: err } : { ok: true, payload };
}

/**
 * Checks a payload; returns null when it is valid, else an error code. Unknown fields are rejected so nothing
 * unexpected travels to the app.
 * @param {unknown} p
 * @returns {string | null}
 */
export function validatePayload(p) {
  if (!p || typeof p !== "object") return Errors.failed;
  const o = /** @type {Record<string, unknown>} */ (p);
  const allowed = new Set(["v", "id", "kind", "createdAt", "url", "title", "lang", "html", "text"]);
  if (Object.keys(o).some((k) => !allowed.has(k))) return Errors.failed;
  if (o.v !== PROTOCOL_VERSION) return Errors.failed;
  if (typeof o.id !== "string" || !/^[0-9a-f-]{36}$/.test(o.id)) return Errors.failed;
  if (typeof o.createdAt !== "number" || !Number.isFinite(o.createdAt)) return Errors.failed;
  if (typeof o.title !== "string" || o.title.length > MAX_TITLE_CHARS) return Errors.failed;
  if (o.lang !== null && cleanLang(o.lang) !== o.lang) return Errors.failed;
  if (o.url !== null && httpUrl(o.url) !== o.url) return Errors.failed;
  switch (o.kind) {
    case "page":
      if ("text" in o) return Errors.failed;
      if (typeof o.html !== "string" || o.html.trim() === "") return Errors.empty;
      if (utf8Length(o.html) > MAX_HTML_BYTES) return Errors.tooLarge;
      return null;
    case "selection":
      if ("html" in o) return Errors.failed;
      if (typeof o.text !== "string" || o.text.trim() === "") return Errors.empty;
      if (o.text.length > MAX_TEXT_CHARS) return Errors.tooLarge;
      return null;
    case "link":
      if ("html" in o || "text" in o) return Errors.failed;
      return o.url === null ? Errors.restricted : null;
    default:
      return Errors.failed;
  }
}
