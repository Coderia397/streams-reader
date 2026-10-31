// @ts-check
/**
 * Functions injected into the page with chrome.scripting.executeScript. Chrome serialises each function's source and
 * runs it in an isolated world, so each must be self-contained: no imports, no outer variables, arguments and return
 * value JSON-compatible. Error strings must match `Errors` in payload.js.
 *
 * The page's own HTML is sent as rendered (so content loaded by script and pages behind the reader's own sign-in
 * work, and text inside open shadow roots is flattened in); the web app's importer (crates/import, readability via
 * dom_smoothie) finds the article. Elements that never carry article text are removed here only to keep the handoff
 * small.
 */

/**
 * @param {number} maxBytes  Largest HTML to return, in UTF-8 bytes.
 * @returns {{ url: string, title: string, lang: string, type: string, html?: string, error?: string }}
 */
export function extractPage(maxBytes) {
  const DROP =
    "script,style,noscript,template,iframe,frame,object,embed,canvas,video,audio,source,track,map,dialog[open]," +
    "link[rel~=stylesheet],link[rel~=preload],link[rel~=modulepreload],link[rel~=prefetch],link[rel~=icon]";
  const base = {
    url: location.href,
    title: document.title,
    lang: document.documentElement.lang || "",
    type: document.contentType,
  };
  // Chrome's PDF viewer: nothing to read in the DOM; the caller hands the address to the app instead.
  if (document.contentType === "application/pdf") return { ...base, error: "restricted" };

  /** @returns {ShadowRoot[]} the page's open shadow roots, nested ones included */
  const openRoots = () => {
    /** @type {ShadowRoot[]} */
    const roots = [];
    /** @param {Document | ShadowRoot} root */
    const scan = (root) => {
      for (const el of root.querySelectorAll("*")) {
        if (el.shadowRoot) {
          roots.push(el.shadowRoot);
          scan(el.shadowRoot);
        }
      }
    };
    scan(document);
    return roots;
  };

  /**
   * Replaces each `<template shadowrootmode>` under `root` with the shadow tree it describes, each <slot> filled with
   * the host's light children assigned to it (or its fallback content), which is what the reader sees. Light
   * children assigned to no slot are not rendered and are dropped.
   * @param {Document | DocumentFragment} root
   */
  const expand = (root) => {
    for (const tpl of [...root.querySelectorAll("template[shadowrootmode]")]) {
      const host = tpl.parentElement;
      const tree = /** @type {HTMLTemplateElement} */ (tpl).content;
      tpl.remove();
      if (!host) continue;
      expand(tree);
      const light = [...host.childNodes];
      /** @param {Node} n */
      const slotName = (n) =>
        n.nodeType === Node.ELEMENT_NODE ? (/** @type {Element} */ (n).getAttribute("slot") ?? "") : "";
      for (const slot of [...tree.querySelectorAll("slot")]) {
        if (!tree.contains(slot)) continue; // Inside the fallback of a slot already filled.
        const name = slot.getAttribute("name") ?? "";
        const assigned = light.filter((n) => n.parentNode === host && slotName(n) === name);
        slot.replaceWith(...(assigned.length ? assigned : [...slot.childNodes]));
      }
      host.replaceChildren(tree);
    }
  };

  /**
   * A detached copy of the page to trim. Content inside open shadow roots (web components) is part of what the
   * reader sees, so when there is any it is serialised with getHTML (Chrome 125+) and flattened into a parsed,
   * inert document (DOMParser runs no scripts and loads nothing). Without getHTML that content is lost, as before.
   * @returns {Element}
   */
  const snapshot = () => {
    const live = document.documentElement;
    const roots = openRoots();
    const getHTML = /** @type {any} */ (live).getHTML;
    if (roots.length === 0 || typeof getHTML !== "function") return /** @type {Element} */ (live.cloneNode(true));
    const inner = getHTML.call(live, { serializableShadowRoots: true, shadowRoots: roots });
    const doc = new DOMParser().parseFromString(`<!DOCTYPE html><html>${inner}</html>`, "text/html");
    expand(doc);
    for (const a of live.attributes) doc.documentElement.setAttribute(a.name, a.value);
    return doc.documentElement;
  };

  /** @param {string} s */
  const bytes = (s) => new Blob([s]).size;
  /** @param {Element} page @param {string} bodyHtml */
  const wrap = (page, bodyHtml) => {
    const head = page.querySelector("head")?.outerHTML ?? "<head></head>";
    const lang = document.documentElement.getAttribute("lang");
    const langAttr = lang ? ` lang="${lang.replace(/[^A-Za-z0-9-]/g, "")}"` : "";
    return `<!DOCTYPE html>\n<html${langAttr}>${head}<body>${bodyHtml}</body></html>`;
  };

  if (!document.body) return { ...base, error: "empty" };
  const page = snapshot();
  page.querySelectorAll(DROP).forEach((n) => n.remove());
  // Inline SVG is mostly icons; keep <title>/<text> out of the article by dropping it whole.
  page.querySelectorAll("svg").forEach((n) => n.remove());
  let html = "<!DOCTYPE html>\n" + page.outerHTML;
  if (bytes(html) > maxBytes) {
    // Too big as a whole (long comment threads, huge inline data). Fall back to the main content region.
    const main = page.querySelector("article, main, [role=main]");
    if (!main) return { ...base, error: "too_large" };
    html = wrap(page, main.outerHTML);
    if (bytes(html) > maxBytes) return { ...base, error: "too_large" };
  }
  if ((document.body.innerText || "").trim() === "") return { ...base, error: "empty" };
  return { ...base, html };
}

/**
 * @param {number} maxChars  Largest selection returned, in UTF-16 code units.
 * @returns {{ url: string, title: string, lang: string, text?: string, error?: string }}
 */
export function extractSelection(maxChars) {
  const base = { url: location.href, title: document.title, lang: document.documentElement.lang || "" };
  const sel = window.getSelection();
  let text = sel ? sel.toString() : "";
  // Text fields keep their selection outside getSelection() in some engines.
  const active = document.activeElement;
  if (!text.trim() && active && (active instanceof HTMLTextAreaElement || active instanceof HTMLInputElement)) {
    const { selectionStart: a, selectionEnd: b } = active;
    if (a !== null && b !== null && b > a) text = active.value.slice(a, b);
  }
  if (!text.trim()) return { ...base, error: "empty" };
  if (text.length > maxChars) return { ...base, error: "too_large" };
  return { ...base, text };
}
