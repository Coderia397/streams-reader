// Vector rebuild of the Streams Reader screens the reel needs (phone: library, add sheet, reader + dock, voice,
// speed and sleep sheets; laptop: library), built from the copy in strings/<lang>.json and styled by motion.css.
// Structure and copy follow crates/web/src (dock.rs, voice_row.rs, sheets/*.rs, library/*.rs) and the locales.

const ICONS = ["arrow-left-s-line", "more-2-fill", "play-large-fill", "pause-large-fill", "replay-15-line",
  "forward-15-line", "moon-clear-line", "play-list-2-line", "arrow-down-s-line", "book-shelf-line", "user-voice-line",
  "settings-4-line", "volume-up-line", "check-line", "link-m", "file-text-line", "upload-2-line", "add-line",
  "search-line", "article-line", "file-pdf-2-line", "book-2-line", "headphone-line", "close-line", "loader-4-line"];
const svg = {};

export async function loadAssets() {
  await Promise.all(ICONS.map(async (n) => {
    svg[n] = await (await fetch(`/crates/web/assets/icons/remix/${n}.svg`)).text();
  }));
  svg.mark = await (await fetch("/brand/src/mark.txt")).text();
  svg.wordmark = await (await fetch("/brand/wordmark/streams-reader-wordmark-on-dark.svg")).text();
}

export const icon = (n, cls = "") => `<span class="icon ${cls}">${svg[n] || ""}</span>`;
const esc = (s) => String(s).replace(/[&<>"]/g, (c) => ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;" }[c]));
const initials = (t) => t.replace(/^(the|a|an)\s+/i, "").split(/\s+/).filter((w) => /^[A-Z0-9]/.test(w))
  .slice(0, 2).map((w) => w[0]).join("") || t[0];
const cover = (d, size) => `<span class="cover cover--${size}" style="--hue-a:${d.hues[0]};--hue-b:${d.hues[1]}">
  <span class="cover__initials">${esc(initials(d.title))}</span></span>`;
const orb = (v, small) => `<span class="orb${small ? " orb--small" : ""}" style="--hue-a:${v.hues[0]};--hue-b:${v.hues[1]}"></span>`;

const STATUS = `<div class="status"><span>9:41</span><span style="display:flex;gap:6px;align-items:center">
  <svg viewBox="0 0 18 12"><g fill="currentColor"><rect x="0" y="8" width="3" height="4" rx="1"/><rect x="5" y="5.5" width="3" height="6.5" rx="1"/><rect x="10" y="3" width="3" height="9" rx="1"/><rect x="15" y="0" width="3" height="12" rx="1"/></g></svg>
  <svg viewBox="0 0 16 12"><path fill="currentColor" d="M8 2.6c2.2 0 4.2.8 5.7 2.2l1.2-1.2C13.1 1.9 10.7.9 8 .9S2.9 1.9 1.1 3.6l1.2 1.2C3.8 3.4 5.8 2.6 8 2.6zm0 3.4c1.3 0 2.5.5 3.4 1.3l1.2-1.2C11.4 4.9 9.8 4.3 8 4.3s-3.4.6-4.6 1.8l1.2 1.2C5.5 6.5 6.7 6 8 6zm0 3.4c.5 0 .9.2 1.2.5L8 11.1 6.8 9.9c.3-.3.7-.5 1.2-.5z"/></svg>
  <svg viewBox="0 0 27 12"><rect x=".5" y=".5" width="22" height="11" rx="3.2" fill="none" stroke="currentColor" opacity=".45"/><rect x="2" y="2" width="16" height="8" rx="2" fill="currentColor"/><rect x="24" y="4" width="2" height="4" rx="1" fill="currentColor" opacity=".45"/></svg>
  </span></div>`;

// ---------- the reading text: words as inline-blocks so lines can be staggered and the pill can be measured
function flowHTML(S) {
  let wi = 0;
  const para = (p, k) => {
    const words = p.split(" ").map((w) => `<span class="t" data-w="${wi++}">${esc(w)}</span>`).join(" ");
    return `<p class="r-p" data-p="${k}">${k === 0 ? `<span class="s">${words}</span>` : words}</p>`;
  };
  return `<div class="flow"><div class="word-pill"></div>${S.doc.paragraphs.map(para).join("")}</div>`;
}

export function phoneReader(S) {
  const v0 = S.voices[0];
  return `<div class="view reader">
    <div class="reader-bar">${`<span class="icon-btn">${icon("arrow-left-s-line")}</span>`}
      <span class="reader-bar__title">${esc(S.doc.title)}</span><span class="icon-btn">${icon("more-2-fill")}</span></div>
    <div class="reading">
      <div class="reading__head" data-line="h">${cover(S.doc, "l")}<div>
        <div class="reading__title">${esc(S.doc.title)}</div><div class="reading__author">${esc(S.doc.author)}</div></div></div>
      <div class="r-h" data-line="c">${esc(S.doc.chapter)}</div>
      ${flowHTML(S)}
    </div>
    <div class="dock">
      <div class="dock__top"><span class="chip dock__chapter">${icon("play-list-2-line")}<span>${esc(S.doc.chapter)}</span></span></div>
      <div class="scrubber"><div class="scrubber__track"><div class="scrubber__fill"></div><div class="scrubber__thumb"></div></div>
        <div class="scrubber__times"><span class="t-pos">0:00</span><span>${esc(S.doc.left)}</span></div></div>
      <div class="dock__transport">
        <span class="icon-btn icon-btn--lg dock__sleep">${icon("moon-clear-line")}<span class="dock__badge" style="opacity:0"></span></span>
        <span class="icon-btn icon-btn--lg">${icon("replay-15-line")}</span>
        <span class="play-btn"><span class="ring"></span>${icon("play-large-fill", "i-play")}${icon("pause-large-fill", "i-pause")}</span>
        <span class="icon-btn icon-btn--lg">${icon("forward-15-line")}</span>
        <span class="icon-btn icon-btn--lg dock__speed"><span class="speed-wrap">1×</span></span>
      </div>
      <div class="dock__tools">
        <span class="chip chip--voice"><span class="orb-wrap">${orb(v0, true)}</span>
          <span class="name-wrap"><span class="name n0">${esc(v0.name)}</span><span class="name n1"></span></span>
          <span class="eq">${"<i></i>".repeat(3)}</span>${icon("arrow-down-s-line")}</span>
        <span class="icon-btn">${icon("more-2-fill")}</span>
      </div>
    </div>
  </div>`;
}

export function phoneLibrary(S) {
  const [first, ...rest] = S.library;
  const kind = { book: "book-2-line", article: "article-line", pdf: "file-pdf-2-line", text: "file-text-line" };
  return `<div class="view library"><div class="page">
    <div class="page-head"><span class="brandline">${markSVG(1.5)}${wordmark(17)}</span>
      <span style="display:flex;gap:4px"><span class="icon-btn">${icon("search-line")}</span>
      <span class="icon-btn" style="background:var(--color-brand);color:#fff">${icon("add-line")}</span></span></div>
    <div class="page-title">${esc(S.ui.library)}</div>
    <div class="hero"><div class="hero__top">${cover(first, "l")}<div class="hero__text">
      <span class="overline">${esc(S.ui.continue)}</span><span class="hero__title">${esc(first.title)}</span>
      <span class="hero__author">${esc(first.author)}</span><span class="hero__meta">${esc(first.meta)}</span></div></div>
      <div class="hero__foot"><div class="progress"><span class="progress__fill" style="width:${first.pct}%"></span></div>
      <span class="play-btn play-btn--small">${icon("play-large-fill")}</span></div></div>
    <div class="list-head overline">${esc(S.ui.all)}</div>
    <ul class="doc-list">${rest.map((d) => `<li class="doc-row" data-row>${cover(d, "s")}<span class="doc-row__body">
      <span class="doc-row__title">${esc(d.title)}</span>
      <span class="doc-row__meta">${icon(kind[d.kind] || "file-text-line")}<span>${esc(d.meta)}</span></span>
      <span class="progress progress--thin"><span class="progress__fill" style="width:${d.pct}%"></span></span></span>
      <span class="icon-btn">${icon("more-2-fill")}</span></li>`).join("")}</ul>
    </div>
    <div class="tabbar"><span class="tab is-on">${icon("book-shelf-line")}${esc(S.ui.library)}</span>
      <span class="tab">${icon("user-voice-line")}${esc(S.ui.voices)}</span><span class="tab">${icon("settings-4-line")}${esc(S.ui.settings)}</span></div>
  </div>`;
}

export function addSheet(S) {
  return `<div class="sheet add-sheet"><div class="sheet__head"><span class="sheet__handle"></span>
    <span class="sheet__title">${esc(S.ui.add_title)}</span><span class="icon-btn">${icon("close-line")}</span></div>
    <div class="sheet__body">
      <div class="add-opts">
        <div class="add-opt is-active"><span class="ic">${icon("link-m")}</span><span class="tx"><b>${esc(S.ui.add_link)}</b><span>${esc(S.ui.add_link_hint)}</span></span></div>
        <div class="add-opt"><span class="ic">${icon("file-text-line")}</span><span class="tx"><b>${esc(S.ui.add_paste)}</b><span>${esc(S.ui.add_paste_hint)}</span></span></div>
        <div class="add-opt"><span class="ic">${icon("upload-2-line")}</span><span class="tx"><b>${esc(S.ui.add_file)}</b><span>${esc(S.ui.add_file_hint)}</span></span></div>
      </div>
      <div class="field">${icon("link-m")}<span class="ph">https://</span><span class="val"></span><span class="caret"></span></div>
      <div class="stage-line"><span class="stage-ic" style="display:inline-flex">${icon("loader-4-line")}</span><span class="stage-tx"></span></div>
      <div class="progress" style="margin:10px 0 16px"><span class="progress__fill add-prog" style="width:0%"></span></div>
      <span class="btn btn--primary btn--block add-btn">${esc(S.ui.add_link_submit)}</span>
    </div></div>`;
}

export function voiceSheet(S) {
  return `<div class="sheet voice-sheet"><div class="sheet__head"><span class="sheet__handle"></span>
    <span class="sheet__title">${esc(S.ui.voice_title)}</span><span class="icon-btn">${icon("close-line")}</span></div>
    <div class="sheet__body"><div class="v-section"><div class="overline">${esc(S.ui.standard)}</div>
      <div class="v-hint">${esc(S.ui.standard_hint)}</div></div>
      <div class="v-list"><div class="v-sel"></div>${S.voices.map((v, i) => `<div class="v-row" data-v="${i}">${orb(v)}
        <div class="tx"><div class="nm">${esc(v.name)}</div><div class="meta">${esc(v.meta)}</div><div class="desc">${esc(v.desc)}</div></div>
        <span class="wave">${"<i></i>".repeat(5)}</span><span class="icon chk">${svg["check-line"]}</span>
        <span class="spk">${icon("volume-up-line")}</span></div>`).join("")}</div>
    </div></div>`;
}

export function controlSheet(S) {
  const chips = S.ui.speeds.map((s) => `<span class="chip">${esc(s)}</span>`).join("");
  const sleeps = S.ui.sleeps.map((s) => `<span class="chip">${esc(s)}</span>`).join("");
  return `<div class="sheet ctl-sheet"><div class="sheet__head"><span class="sheet__handle"></span>
    <span class="sheet__title"><span class="ctl-title-wrap" style="position:relative;display:block;height:23px;overflow:hidden">
      <span class="ctl-t0" style="display:block">${esc(S.ui.speed_title)}</span><span class="ctl-t1" style="display:block;position:absolute;top:0">${esc(S.ui.sleep_title)}</span></span></span>
    <span class="icon-btn">${icon("close-line")}</span></div>
    <div class="sheet__body"><div class="sheet-pages" style="height:206px">
      <div class="sheet-page p-speed" style="position:absolute;inset:0">
        <div class="speed__value">1×</div>
        <div class="speed__row"><span class="icon-btn" style="background:var(--color-surface-3);color:var(--color-text);font-size:22px">−</span>
          <div class="slider"><div class="slider__fill"></div><div class="slider__thumb"></div></div>
          <span class="icon-btn" style="background:var(--color-surface-3);color:var(--color-text)">${icon("add-line")}</span></div>
        <div class="chip-row speed-chips"><span class="glider"></span>${chips}</div>
        <div class="hint">${esc(S.ui.speed_hint)}</div></div>
      <div class="sheet-page p-sleep" style="position:absolute;inset:0">
        <div class="chip-row sleep-chips"><span class="glider"></span>${sleeps}</div>
        <div class="hint">${esc(S.ui.sleep_hint)}</div></div>
    </div></div></div>`;
}

export function laptopLibrary(S) {
  return `<div class="desk"><div class="desk__side"><span class="brandline">${markSVG(1.75)}${wordmark(19)}</span>
      <div class="desk__nav"><div class="is-on">${icon("book-shelf-line")}${esc(S.ui.library)}</div>
      <div>${icon("user-voice-line")}${esc(S.ui.voices)}</div><div>${icon("settings-4-line")}${esc(S.ui.settings)}</div></div></div>
    <div class="desk__main"><div class="page-head" style="height:44px"><span></span>
      <span style="display:flex;gap:10px"><span class="icon-btn" style="background:var(--color-surface-2)">${icon("search-line")}</span>
      <span class="btn btn--primary">${icon("add-line")}${esc(S.ui.add)}</span></span></div>
      <div class="page-title">${esc(S.ui.library)}</div>
      <div class="overline" style="margin-bottom:14px">${esc(S.ui.continue)}</div>
      <div class="desk__grid">${S.library.slice(0, 4).map((d) => `<div class="desk__card" data-card>${cover(d, "s")}
        <b>${esc(d.title)}</b><span>${esc(d.author || d.meta)}</span>
        <span class="progress progress--thin"><span class="progress__fill" style="width:${d.pct}%"></span></span></div>`).join("")}</div>
    </div>
    <div class="mini">${cover(S.library[0], "xs")}<span class="t" style="flex:1"><b>${esc(S.library[0].title)}</b><span>${esc(S.doc.chapter)} · ${esc(S.voices[1].name)}</span></span>
      <span class="icon-btn">${icon("replay-15-line")}</span><span class="play-btn play-btn--small">${icon("pause-large-fill")}</span>
      <span class="icon-btn">${icon("forward-15-line")}</span><span class="eq" style="margin-left:8px">${"<i></i>".repeat(3)}</span></div>
  </div>`;
}

// The outlined wordmark (brand/wordmark), never live text.
export const wordmark = (h) => svg.wordmark.replace(/width="[^"]+" height="[^"]+"/, `height="${h}" width="${(h * 894.699 / 109.131).toFixed(1)}"`);

// The pixel mark drawn from brand/src/mark.txt with whole cells; the voice arc (cols 12-13, rows 4-11) is grouped
// so it alone may pulse (brand/README.md: only the voice arc may move).
export function markSVG(cell = 12, color = "var(--color-brand)", ember = false) {
  const rows = svg.mark.split("\n").filter((l) => /^[.#o]{16}$/.test(l.trim())).map((l) => l.trim());
  let body = "", arc = "";
  rows.forEach((r, y) => [...r].forEach((ch, x) => {
    if (ch === ".") return;
    const fill = ch === "o" && ember ? "var(--color-ember)" : color;
    const rect = `<rect x="${x}" y="${y}" width="1" height="1" fill="${fill}"/>`;
    if ((x === 12 || x === 13) && y >= 4 && y <= 11) arc += rect; else body += rect;
  }));
  const s = cell * 16;
  return `<svg class="mark" width="${s}" height="${s}" viewBox="0 0 16 16" shape-rendering="crispEdges">${body}<g class="arc">${arc}</g></svg>`;
}

export function endCard(S, scale) {
  return `<div class="endcard">${markSVG(Math.round(12 * scale), "var(--color-brand)", true)}
    <div class="wordmark" style="width:${Math.round(520 * scale)}px">${svg.wordmark.replace(/width="[^"]+" height="[^"]+"/, 'width="100%" height="auto"')}</div>
    <div class="tag" style="font-size:${Math.round(40 * scale)}px">${esc(S.end.tag)}</div>
    <div class="url" style="font-size:${Math.round(20 * scale)}px">${esc(S.end.url)}</div></div>`;
}
