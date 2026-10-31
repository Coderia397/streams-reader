// Motion engine: builds the stage from a resolved timeline (render.py merges extends, strings, reading schedule and
// the narration envelope) and renders any time t as a pure function: motion.seek(t). Nothing animates on its own.
import * as UI from "./ui.js";

// ---------- easing (cubic-bezier solver + named eases; token curves from brand/tokens.toml)
function bezier(x1, y1, x2, y2) {
  const cx = 3 * x1, bx = 3 * (x2 - x1) - cx, ax = 1 - cx - bx;
  const cy = 3 * y1, by = 3 * (y2 - y1) - cy, ay = 1 - cy - by;
  const X = (t) => ((ax * t + bx) * t + cx) * t, Y = (t) => ((ay * t + by) * t + cy) * t;
  const dX = (t) => (3 * ax * t + 2 * bx) * t + cx;
  return (x) => {
    if (x <= 0) return 0; if (x >= 1) return 1;
    let t = x;
    for (let i = 0; i < 8; i++) { const d = dX(t); if (Math.abs(d) < 1e-6) break; t -= (X(t) - x) / d; }
    t = Math.min(1, Math.max(0, t));
    return Y(t);
  };
}
const EASE = {
  linear: (x) => x,
  out: bezier(0.2, 0.8, 0.2, 1),          // --ease-out
  inout: bezier(0.65, 0, 0.35, 1),        // --ease-inout
  in: bezier(0.4, 0, 1, 1),               // --ease-in
  expo: bezier(0.16, 1, 0.3, 1),          // type reveals
  glide: bezier(0.45, 0, 0.15, 1),        // camera: slow in, long settle
  sheet: bezier(0.32, 0.72, 0, 1),        // iOS-like sheet
  soft: bezier(0.4, 0, 0.2, 1),
};
const clamp = (x, a = 0, b = 1) => Math.min(b, Math.max(a, x));
const lerp = (a, b, p) => a + (b - a) * p;
const prog = (t, t0, dur) => clamp((t - t0) / Math.max(1e-6, dur));

// keyframe track: [{t, ...values, ease}] -> values at time t (ease of the key being approached)
function track(keys, t, defaults) {
  if (!keys || !keys.length) return { ...defaults };
  // every key inherits the values of the keys before it
  if (!keys._full) {
    let acc = { ...defaults };
    keys._full = keys.map((k) => (acc = { ...acc, ...k }));
  }
  const F = keys._full;
  if (t <= F[0].t) return { ...F[0] };
  for (let i = 1; i < F.length; i++) {
    if (t <= F[i].t) {
      const a = F[i - 1], b = F[i];
      const p = (EASE[keys[i].ease || "glide"])(prog(t, a.t, b.t - a.t));
      const out = {};
      for (const key of Object.keys(b)) out[key] = typeof b[key] === "number" && typeof a[key] === "number" ? lerp(a[key], b[key], p) : b[key];
      return out;
    }
  }
  return { ...F[F.length - 1] };
}

const $ = (sel, root = document) => root.querySelector(sel);
const $$ = (sel, root = document) => [...root.querySelectorAll(sel)];

let TL, S, W, H, D = {};

function headlineHTML(text, sub) {
  const lines = text.split("\n").map((line) => {
    const parts = line.split(/(\*[^*]+\*)/).filter(Boolean).flatMap((seg) => {
      const em = seg.startsWith("*");
      return seg.replace(/\*/g, "").split(/(\s+)/).filter((x) => x.length).map((w) =>
        /^\s+$/.test(w) ? " " : `<span class="w">${em ? `<em>${w}</em>` : w}</span>`);
    });
    return `<span class="line">${parts.join("")}</span>`;
  });
  return lines.join("") + (sub ? `<span class="sub">${sub}</span>` : "");
}

async function load(tl) {
  TL = tl; S = tl.strings;
  document.documentElement.dataset.theme = tl.theme || "ink";
  [W, H] = tl.size;
  await UI.loadAssets();
  const stage = $("#stage");
  stage.style.width = W + "px"; stage.style.height = H + "px";
  const L = tl.layout || {};
  const K = D.K = tl.hires ?? 2;   // devices are laid out K times larger and scaled back, so 3D layers raster sharp
  stage.innerHTML = `
    <div id="bg"><div class="grid"></div><div class="glow a"></div><div class="glow b"></div></div>
    <div id="world"><div id="cam">
      <div class="plane" id="p-laptop" style="left:${-760 * K}px;top:${-484 * K}px"><div class="laptop" style="zoom:${K}"><div class="lid"><div class="lcd">${UI.laptopLibrary(S)}</div></div>
        <div class="base"><div class="notch"></div></div></div></div>
      <div class="plane" id="p-phone" style="left:${-207 * K}px;top:${-434 * K}px"><div class="phone" style="zoom:${K}"><div class="screen">
        ${UI.phoneLibrary(S)}${UI.phoneReader(S)}<div class="dim" style="position:absolute;inset:0;background:#000;opacity:0;z-index:5;pointer-events:none"></div>
        <div class="scrim"></div>${UI.addSheet(S)}${UI.voiceSheet(S)}${UI.controlSheet(S)}
        <div class="island"></div>${""}<div class="homebar"></div></div></div></div>
    </div></div>
    <div id="type"></div>
    ${UI.endCard(S, L.end_scale || 1)}
    <div id="fade"></div>`;
  // status bar sits above both views
  $(".screen").insertAdjacentHTML("afterbegin", "");
  $$(".view").forEach((v) => v.insertAdjacentHTML("afterbegin", ""));
  $(".screen").insertAdjacentHTML("beforeend", `<div class="status-host" style="position:absolute;inset:0 0 auto 0;z-index:85">${statusBar()}</div>`);

  // headlines
  const type = $("#type");
  D.heads = (tl.headlines || []).map((h) => {
    const el = document.createElement("div");
    el.className = "headline";
    const text = S.headlines[h.id] ?? h.text ?? "";
    const sub = h.sub ? S.headlines[h.sub] : "";
    el.innerHTML = headlineHTML(text, sub);
    const st = { ...(L.headline || {}), ...(h.style || {}) };
    Object.assign(el.style, { left: st.x + "px", top: st.y + "px", fontSize: st.size + "px", width: (st.width || 900) + "px",
      textAlign: st.align || "left" });
    if (st.anchor === "middle") el.style.transform = "translateY(-50%)";
    type.appendChild(el);
    return { h, el, words: $$(".w", el), sub: $(".sub", el) };
  });
  // waveform strip
  const wv = L.wave;
  if (wv) {
    const n = wv.bars || 56;
    type.insertAdjacentHTML("beforeend", `<div class="strip" style="position:absolute;left:${wv.x}px;top:${wv.y}px;width:${wv.width}px;height:${wv.height}px;
      display:flex;align-items:center;justify-content:space-between;opacity:0;
      -webkit-mask-image:linear-gradient(90deg,transparent,#000 18%,#000 82%,transparent)">${"<i></i>".repeat(n)}</div>`);
    D.strip = $(".strip"); D.stripBars = $$(".strip i");
    D.stripBars.forEach((b) => Object.assign(b.style, { display: "block", width: (wv.bar || 4) + "px", borderRadius: "3px",
      background: "var(--color-brand-text)", height: "100%" }));
  }

  await document.fonts.load('450 19px "Literata"'); await document.fonts.load('600 19px "Literata"');
  await document.fonts.load('italic 480 40px "Literata"');
  for (const w of [450, 550, 620, 640]) await document.fonts.load(`${w} 16px "Inter"`);
  await document.fonts.ready;

  // measurements (offsets ignore transforms, so they are stable whatever the camera does)
  D.words = $$(".flow .t");
  D.flow = $(".flow");
  D.pill = $(".word-pill");
  D.rects = D.words.map((w) => ({ x: w.offsetLeft, y: w.offsetTop, w: w.offsetWidth, h: w.offsetHeight }));
  // doc lines for the build stagger: head, chapter, then each visual line of the paragraphs
  const groups = [[$(".reading__head")], [$(".r-h")]];
  let lastTop = null;
  D.words.forEach((w) => {
    const top = w.offsetTop + w.closest(".r-p").offsetTop * 1000;
    if (top !== lastTop) { groups.push([]); lastTop = top; }
    groups[groups.length - 1].push(w);
  });
  D.lines = groups;
  // chip rows
  for (const k of ["speed", "sleep"]) {
    const row = $(`.${k}-chips`);
    D[k] = { row, glider: $(".glider", row), chips: $$(".chip", row) };
    D[k].rects = D[k].chips.map((c) => ({ x: c.offsetLeft, y: c.offsetTop, w: c.offsetWidth }));
  }
  D.vrows = $$(".v-row");
  D.vsel = $(".v-sel");
  // press dots inside their targets
  for (const ev of tl.events || []) {
    if (ev.type !== "press") continue;
    const el = targetEl(ev);
    if (el && !$(":scope > .press-dot", el)) {
      if (getComputedStyle(el).position === "static") el.style.position = "relative";
      el.insertAdjacentHTML("beforeend", '<span class="press-dot" style="left:50%;top:50%"></span>');
    }
  }
  D.events = (tl.events || []).slice().sort((a, b) => a.t - b.t);
  return { fps: tl.fps, frames: Math.round(tl.duration * tl.fps), size: [W, H] };
}

function statusBar() {
  return `<div class="status"><span>9:41</span><span style="display:flex;gap:6px;align-items:center">
  <svg viewBox="0 0 18 12" style="height:12px"><g fill="currentColor"><rect x="0" y="8" width="3" height="4" rx="1"/><rect x="5" y="5.5" width="3" height="6.5" rx="1"/><rect x="10" y="3" width="3" height="9" rx="1"/><rect x="15" y="0" width="3" height="12" rx="1"/></g></svg>
  <svg viewBox="0 0 27 12" style="height:12px"><rect x=".5" y=".5" width="22" height="11" rx="3.2" fill="none" stroke="currentColor" opacity=".45"/><rect x="2" y="2" width="16" height="8" rx="2" fill="currentColor"/><rect x="24" y="4" width="2" height="4" rx="1" fill="currentColor" opacity=".45"/></svg>
  </span></div>`;
}

function targetEl(ev) {
  const list = $$(ev.target);
  return list[ev.index || 0];
}

// ---------- state per frame
function baseState() {
  const i = TL.init || {};
  return {
    view: i.view === "reader" ? 1 : 0,       // 0 library, 1 reader
    add: i.sheet === "add" ? 1 : 0, voice: 0, ctl: 0,
    paste: i.paste || 0, sel: 0, focus: i.sheet === "add" ? 1 : 0,
    stage: 0, stageP: 0, prog: 0, built: i.built ?? (i.view === "reader" ? 1 : 0), buildT: -1, buildDur: 1,
    playing: i.playing ? 1 : 0, ring: 0,
    vsel: i.voice || 0, vname: [i.voice || 0, i.voice || 0, 1],
    speed: i.speed ?? 1, speedLabel: [1, 1, 1], ctlPage: 0, sleepG: [i.sleep ?? 0, i.sleep ?? 0, 1], sleepOn: 0,
    press: [], end: 0, fade: 0, scrub: i.scrub ?? 0.012, washA: i.playing ? 1 : 0,
  };
}

const H_EV = {
  view(s, p, ev) { s.view = lerp(s.view, ev.to === "reader" ? 1 : 0, EASE.sheet(p)); },
  sheet(s, p, ev) { // {which:add|voice|ctl, open:true|false}
    const k = ev.which; s[k] = lerp(s[k], ev.open ? 1 : 0, (ev.open ? EASE.sheet : EASE.soft)(p));
    if (k === "add") s.focus = s.add;
  },
  paste(s, p) { s.paste = EASE.out(p); s.sel = p < 1 ? 1 : 1; },
  unselect(s, p) { s.sel = 1 - EASE.out(p); },
  clear(s, p) { s.paste = 1 - EASE.inout(p); s.stage = p > 0.5 ? 0 : s.stage; s.prog = s.prog * (1 - p); },
  import(s, p) { s.stage = p < 0.5 ? 1 : 2; s.prog = EASE.inout(p); },
  build(s, p, ev) { s.built = 1; s.buildT = ev.t; s.buildDur = ev.dur || 1.2; },
  unbuild(s, p) { if (p >= 1) { s.built = 0; s.buildT = -1; } },
  play(s, p, ev) { s.playing = ev.on === false ? 0 : 1; s.ring = p; s.washA = ev.on === false ? 1 - EASE.out(p) : Math.max(s.washA, EASE.out(p)); },
  press(s, p, ev) { s.press.push([ev, p]); },
  voice(s, p, ev) { s.vsel = lerp(ev.from, ev.to, EASE.inout(p)); s.vname = [ev.from, ev.to, EASE.inout(clamp(p * 1.4 - 0.2))]; },
  speed(s, p, ev) { s.speed = lerp(ev.from, ev.to, EASE.inout(p)); s.speedLabel = [ev.from, ev.to, EASE.inout(p)]; },
  page(s, p, ev) { s.ctlPage = lerp(s.ctlPage, ev.to, EASE.inout(p)); },
  sleep(s, p, ev) { s.sleepG = [ev.from, ev.to, EASE.inout(p)]; s.sleepOn = Math.max(s.sleepOn, EASE.out(p)); },
  end(s, p) { s.end = EASE.inout(p); },
  fade(s, p, ev) { s.fade = lerp(ev.from ?? s.fade, ev.to, EASE.inout(p)); },
  scrub(s, p, ev) { s.scrub = lerp(ev.from, ev.to, EASE.linear(p)); },
};

function stateAt(t) {
  const s = baseState();
  for (const ev of D.events) {
    if (t < ev.t) break;
    const fn = H_EV[ev.type];
    if (fn) fn(s, prog(t, ev.t, ev.dur ?? 0.5), ev, t);
  }
  return s;
}

function envAt(t) {
  const e = TL.env; if (!e || !e.length) return 0;
  const x = t * (TL.env_rate || TL.fps); const i = Math.floor(x);
  if (i < 0 || i >= e.length - 1) return 0;
  return lerp(e[i], e[i + 1], x - i);
}

const fmtSpeed = (r) => { const h = Math.round(r * 100), w = Math.floor(h / 100), f = h % 100;
  return f === 0 ? `${w}×` : f % 10 === 0 ? `${w}.${f / 10}×` : `${w}.${String(f).padStart(2, "0")}×`; };

// ---------- render
function render(t) {
  const s = stateAt(t);
  const L = TL.layout || {};
  // camera
  const c = track(TL.camera, t, { x: 0, y: 0, z: 1, rx: 0, ry: 0, rz: 0, sx: W / 2, sy: H / 2 });
  const drift = TL.drift ?? 1;
  const dx = Math.sin(t * 0.37) * 6 * drift, dy = Math.cos(t * 0.29) * 4 * drift;
  const dry = Math.sin(t * 0.23) * 0.6 * drift, drx = Math.cos(t * 0.31) * 0.4 * drift;
  $("#cam").style.transform = `translate(${c.sx + dx}px, ${c.sy + dy}px) scale(${c.z}) rotateX(${c.rx + drx}deg) rotateY(${c.ry + dry}deg) rotateZ(${c.rz}deg) translate(${-c.x}px, ${-c.y}px)`;
  for (const [id, def] of [["phone", { x: 0, y: 0, z: 0, rx: 0, ry: 0, rz: 0, s: 1, o: 1 }], ["laptop", { x: -900, y: 0, z: -200, rx: 0, ry: 0, rz: 0, s: 1, o: 0 }]]) {
    const p = track((TL.planes || {})[id], t, def);
    const el = $(`#p-${id}`);
    el.style.transform = `translate3d(${p.x}px, ${p.y}px, ${p.z}px) rotateX(${p.rx}deg) rotateY(${p.ry}deg) rotateZ(${p.rz}deg) scale(${p.s / D.K})`;
    el.style.opacity = p.o; el.style.visibility = p.o < 0.002 ? "hidden" : "visible";
  }
  // background
  const bg = track(TL.bg, t, { glow: 0.55, ax: 0.7, ay: 0.45, bx: 0.25, by: 0.8, grid: 0 });
  const ga = $("#bg .glow.a"), gb = $("#bg .glow.b"), size = Math.max(W, H) * 0.9;
  Object.assign(ga.style, { width: size + "px", height: size + "px", left: (bg.ax * W - size / 2 - c.x * 0.05) + "px", top: (bg.ay * H - size / 2) + "px", opacity: bg.glow });
  Object.assign(gb.style, { width: size * 0.7 + "px", height: size * 0.7 + "px", left: (bg.bx * W - size * 0.35) + "px", top: (bg.by * H - size * 0.35) + "px", opacity: bg.glow * 0.6 });
  $("#bg .grid").style.opacity = bg.grid;
  $("#bg .grid").style.transform = `translate(${(-c.x * 0.08) % 32}px, ${(-c.y * 0.08) % 32}px)`;

  // views
  const lib = $(".view.library"), rd = $(".view.reader");
  rd.style.transform = `translateX(${(1 - s.view) * 100}%)`;
  rd.style.visibility = s.view < 0.001 ? "hidden" : "visible";
  lib.style.transform = `translateX(${-s.view * 28}%)`;
  lib.style.visibility = s.view > 0.999 ? "hidden" : "visible";
  $(".dim").style.opacity = 0; lib.style.filter = `brightness(${1 - s.view * 0.5})`;

  // sheets
  const anySheet = Math.max(s.add, s.voice, s.ctl);
  $(".scrim").style.opacity = anySheet;
  $(".add-sheet").style.transform = `translateY(${(1 - s.add) * 105}%)`;
  $(".voice-sheet").style.transform = `translateY(${(1 - s.voice) * 105}%)`;
  $(".ctl-sheet").style.transform = `translateY(${(1 - s.ctl) * 105}%)`;

  // add sheet: paste, selection, stages
  const field = $(".add-sheet .field"), val = $(".add-sheet .val");
  if (!val.dataset.ready) { val.innerHTML = `<b>${S.link}</b>`; val.dataset.ready = 1; }
  field.classList.toggle("is-focus", s.focus > 0.5);
  $(".add-sheet .ph").style.display = s.paste > 0.001 ? "none" : "";
  const avail = field.clientWidth - 16 * 2 - 20 - 8 - 4;
  const tw = val.scrollWidth;
  val.style.clipPath = `inset(0 ${(1 - s.paste) * 100}% 0 0)`;
  val.style.transform = `translateX(${Math.min(0, avail - tw) * EASE.inout(clamp(s.paste * 1.2 - 0.1))}px)`;
  val.style.display = s.paste > 0.001 ? "inline-block" : "none";
  $("b", val).style.background = `rgba(110,155,255,${0.35 * s.sel})`;
  $(".add-sheet .caret").style.opacity = s.focus > 0.5 && s.stage === 0 ? (Math.floor(t * 1.9) % 2 === 0 ? 1 : 0) : 0;
  const stx = $(".add-sheet .stage-tx");
  stx.textContent = s.stage === 1 ? S.ui.stage_fetching : s.stage === 2 ? S.ui.stage_reading : "";
  $(".add-sheet .stage-ic").style.opacity = s.stage ? 1 : 0;
  $(".add-sheet .stage-ic").style.transform = `rotate(${t * 360}deg)`;
  $(".add-prog").style.width = s.prog * 100 + "%";
  $(".add-prog").parentElement.style.opacity = s.stage || s.prog > 0.001 ? 1 : 0.0;

  // reader: build stagger
  const now = t;
  D.lines.forEach((g, i) => {
    let p;
    if (!s.built) p = 0;
    else if (s.buildT < 0) p = 1;
    else p = EASE.expo(prog(now, s.buildT + i * (s.buildDur / Math.max(8, D.lines.length)), 0.7));
    for (const el of g) { el.style.opacity = p; el.style.transform = `translateY(${(1 - p) * 18}px)`; if (el.classList.contains("t")) el.style.display = "inline-block"; }
  });

  // play button / wash / pill
  const pb = $(".dock .play-btn");
  const pl = s.playing;
  $(".i-play", pb).style.cssText = `position:absolute;transform:scale(${pl ? 0.4 : 1});opacity:${pl ? 0 : 1}`;
  $(".i-pause", pb).style.cssText = `position:absolute;transform:scale(${pl ? 1 : 0.4});opacity:${pl ? 1 : 0}`;
  const ring = $(".ring", pb);
  ring.style.opacity = s.ring > 0 && s.ring < 1 ? (1 - s.ring) * 0.8 : 0;
  ring.style.transform = `scale(${1 + EASE.out(s.ring) * 0.55})`;
  const sent = $(".r-p .s");
  sent.style.background = `rgba(47,107,255,${0.14 * s.washA})`;
  sent.style.boxShadow = `0 0 0 3px rgba(47,107,255,${0.14 * s.washA})`;
  pill(t);

  // scrubber + time
  const sc = s.scrub;
  $(".scrubber__fill").style.width = sc * 100 + "%";
  $(".scrubber__thumb").style.left = sc * 100 + "%";
  const secs = Math.max(0, Math.floor((TL.clock0 || 0) + Math.max(0, t - (TL.clock_start ?? 1e9))));
  $(".t-pos").textContent = `${Math.floor(secs / 60)}:${String(secs % 60).padStart(2, "0")}`;

  // eq bars driven by the narration envelope
  const e = envAt(t);
  $$(".eq").forEach((eq, j) => $$("i", eq).forEach((b, k) => {
    const wob = 0.55 + 0.45 * Math.sin(t * (9 + k * 3.3) + k * 1.7 + j);
    const idle = 0.42 + 0.22 * Math.sin(t * (4.1 + k * 1.3) + k * 2.2 + j);   // playing between words: keep a gentle pulse
    b.style.transform = `scaleY(${clamp(Math.max(idle, 0.2 + e * 1.1 * wob), 0.25, 1)})`;
  }));
  $$(".chip--voice .eq").forEach((eq) => { eq.style.opacity = pl ? 1 : 0; eq.style.width = pl ? "13px" : "0px"; });

  // voice sheet
  const vs = s.vsel, rh = 78;
  D.vsel.style.top = vs * rh + "px"; D.vsel.style.height = 76 + "px";
  D.vrows.forEach((r, i) => {
    const on = clamp(1 - Math.abs(vs - i));
    $(".chk", r).style.opacity = on; $(".chk", r).style.transform = `scale(${0.6 + 0.4 * on})`;
    const wv = $(".wave", r); wv.style.opacity = on * (pl ? 1 : 0.0); wv.style.width = "0"; wv.style.overflow = "visible";
    $$("i", wv).forEach((b, k) => { b.style.transform = `scaleY(${clamp(0.15 + e * (0.6 + 0.4 * Math.sin(t * 11 + k * 2.1)), 0.12, 1)})`; });
    wv.style.display = "none";
  });
  // voice chip name roll + orb
  const [na, nb, np] = s.vname;
  const n0 = $(".chip--voice .n0"), n1 = $(".chip--voice .n1");
  n0.textContent = S.voices[na].name; n1.textContent = S.voices[nb].name;
  n0.style.transform = `translateY(${-np * 100}%)`; n1.style.transform = `translateY(${-np * 100}%)`;
  n0.style.opacity = 1 - np; n1.style.opacity = np;
  $(".chip--voice .name-wrap").style.width = Math.max(n0.offsetWidth, n1.offsetWidth) + "px";
  const ow = $(".chip--voice .orb-wrap");
  const want = np > 0.5 ? nb : na;
  if (ow.dataset.v !== String(want)) { const v = S.voices[want]; ow.innerHTML = `<span class="orb orb--small" style="--hue-a:${v.hues[0]};--hue-b:${v.hues[1]}"></span>`; ow.dataset.v = want; }
  ow.style.display = "inline-flex";

  // control sheet: pages, speed chips, sleep chips
  const pg = s.ctlPage;
  $(".p-speed").style.transform = `translateX(${-pg * 106}%)`; $(".p-sleep").style.transform = `translateX(${(1 - pg) * 106}%)`;
  $(".ctl-t0").style.transform = `translateY(${-pg * 100}%)`; $(".ctl-t1").style.transform = `translateY(${(1 - pg) * 100}%)`;
  $(".ctl-t0").style.opacity = 1 - pg; $(".ctl-t1").style.opacity = pg;
  const speeds = [0.8, 1, 1.2, 1.5, 2, 2.5];
  const idx = (r) => { for (let i = 0; i < speeds.length - 1; i++) if (r <= speeds[i + 1]) return i + (r - speeds[i]) / (speeds[i + 1] - speeds[i]); return speeds.length - 1; };
  const si = idx(s.speed);
  glide(D.speed, Math.floor(si), Math.min(Math.floor(si) + 1, speeds.length - 1), si - Math.floor(si));
  glide(D.sleep, ...s.sleepG);
  const sleepSel = s.sleepG[2] < 0.5 ? s.sleepG[0] : s.sleepG[1];
  D.speed.chips.forEach((ch, i) => { ch.style.color = Math.abs(idx(s.speed) - i) < 0.5 ? "var(--color-brand-text)" : "var(--color-text-2)"; ch.style.borderColor = Math.abs(idx(s.speed) - i) < 0.5 ? "transparent" : "var(--color-border)"; });
  D.sleep.chips.forEach((ch, i) => { ch.style.color = sleepSel === i ? "var(--color-brand-text)" : "var(--color-text-2)"; ch.style.borderColor = sleepSel === i ? "transparent" : "var(--color-border)"; });
  const [sa, sb, sp] = s.speedLabel;
  const label = fmtSpeed(sp < 0.5 ? sa : sb);
  $(".speed__value").textContent = label;
  $(".speed__value").style.opacity = 1 - Math.sin(sp * Math.PI) * 0.6;
  $(".dock__speed .speed-wrap").textContent = label;
  const frac = (s.speed - 0.5) / 3.0;
  $(".slider__fill").style.width = frac * 100 + "%"; $(".slider__thumb").style.left = frac * 100 + "%";
  $(".dock__sleep").classList.toggle("is-on", s.sleepOn > 0.5);

  // press feedback
  $$(".press-dot").forEach((d) => { d.style.opacity = 0; });
  for (const [ev, p] of s.press) {
    const el = targetEl(ev); if (!el) continue;
    const d = $(":scope > .press-dot", el); if (!d) continue;
    const a = p < 0.3 ? EASE.out(p / 0.3) : 1 - EASE.inout((p - 0.3) / 0.7);
    d.style.opacity = a * 0.9;
    d.style.transform = `scale(${0.6 + 0.5 * EASE.out(p)})`;
    el.style.transform = p < 1 ? `scale(${1 - 0.05 * Math.sin(Math.PI * clamp(p / 0.6))})` : "";
  }

  // headlines
  for (const hd of D.heads) {
    const { h, words, sub } = hd;
    const stg = h.stagger ?? 0.055;
    let vis = false;
    words.forEach((w, k) => {
      const pin = EASE.expo(prog(t, h.in + k * stg, h.dur_in ?? 0.9));
      const pout = h.out != null ? EASE.in(prog(t, h.out + k * 0.03, 0.5)) : 0;
      const y = (1 - pin) * 105 - pout * 105;
      w.style.transform = `translateY(${y}%)`;
      w.style.opacity = pin * (1 - pout);
      if (pin > 0 && pout < 1) vis = true;
    });
    if (sub) {
      const pin = EASE.out(prog(t, h.in + 0.35, 0.8)), pout = h.out != null ? EASE.in(prog(t, h.out, 0.4)) : 0;
      sub.style.opacity = pin * (1 - pout); sub.style.transform = `translateY(${(1 - pin) * 12}px)`;
    }
    hd.el.style.visibility = vis ? "visible" : "hidden";
  }

  // waveform strip: a scrolling history of the narration envelope
  if (D.strip) {
    const wv = track(TL.wave, t, { o: 0 });
    D.strip.style.opacity = wv.o;
    const n = D.stripBars.length, dt = 1 / 30;
    D.stripBars.forEach((b, k) => {
      const tt = t - (n - 1 - k) * dt * 0.6;
      const v = envAt(tt);
      const jig = 0.75 + 0.25 * Math.sin(k * 2.39 + tt * 3);
      b.style.transform = `scaleY(${clamp(0.06 + v * 1.25 * jig, 0.06, 1)})`;
      b.style.opacity = 0.35 + 0.65 * clamp(v * 1.4);
    });
  }

  // end card: the product fades away first, then mark, wordmark, line and address rise in turn
  const ec = $(".endcard");
  const endT = (D.events.find((e) => e.type === "end") || { t: 1e9 }).t;
  ec.style.opacity = 1;
  ec.style.visibility = t >= endT ? "visible" : "hidden";
  const mk = $(".endcard .mark");
  const pm = EASE.out(prog(t, endT + 0.45, 1.1));
  mk.style.opacity = pm; mk.style.transform = `translateY(${(1 - pm) * 18}px) scale(${0.97 + 0.03 * pm})`;
  const wm = $(".endcard .wordmark"), tg = $(".endcard .tag"), ur = $(".endcard .url");
  [wm, tg, ur].forEach((el, k) => { const p = EASE.expo(prog(t, endT + 0.7 + k * 0.2, 1.0)); el.style.opacity = p; el.style.transform = `translateY(${(1 - p) * 14}px)`; });
  // the voice arc breathes once (the only part of the mark that may move)
  const ap = prog(t, endT + 0.9, 2.4);
  $(".endcard .arc").style.opacity = 0.3 + 0.7 * EASE.out(clamp(ap * 2.2)) - 0.22 * Math.sin(Math.PI * clamp((ap - 0.45) / 0.55));
  $("#world").style.opacity = 1 - EASE.inout(prog(t, endT, 0.7));
  $("#world").style.visibility = t > endT + 0.75 ? "hidden" : "visible";
  $("#fade").style.opacity = s.fade;
}

function glide(d, ia, ib, p) {
  const a = d.rects[ia], b = d.rects[ib];
  const sameRow = a.y === b.y;
  const r = sameRow ? { x: lerp(a.x, b.x, p), y: a.y, w: lerp(a.w, b.w, p) } : p < 0.5 ? a : b;
  const op = sameRow ? 1 : Math.abs(p - 0.5) * 2;
  const sc = sameRow ? 1 : 0.85 + 0.15 * op;
  Object.assign(d.glider.style, { left: r.x + "px", top: r.y + "px", width: r.w + "px", opacity: op, transform: `scale(${sc})` });
}

// word pill: glides between words on the same line, re-forms at the start of a new line
function pill(t) {
  const W_ = TL.words || [];
  const P = D.pill;
  let k = -1;
  for (let i = 0; i < W_.length; i++) if (W_[i].s <= t) k = i; else break;
  if (k < 0) { P.style.opacity = 0; return; }
  const cur = W_[k];
  const gap = (W_[k + 1]?.s ?? cur.e + 0.8) - cur.e;
  // fade out after the last word of a reading segment
  const tail = cur.last ? 1 - EASE.out(prog(t, cur.e + 0.25, 0.4)) : 1;
  const r1 = D.rects[cur.i];
  const prev = k > 0 && W_[k - 1].i === cur.i - 1 && cur.s - W_[k - 1].e < 0.8 ? D.rects[W_[k - 1].i] : null;
  const g = EASE.out(prog(t, cur.s, 0.13));
  const lineH = r1.h, ph = Math.round(parseFloat(getComputedStyle(D.flow).fontSize) * 1.34);
  let x, w, op = 1, sx = 1;
  if (prev && prev.y === r1.y) { x = lerp(prev.x, r1.x, g); w = lerp(prev.w, r1.w, g); }
  else { x = r1.x; w = r1.w; op = prev ? g : EASE.out(prog(t, cur.s, 0.2)); sx = 0.7 + 0.3 * g; }
  const pad = 3;
  Object.assign(P.style, { left: x - pad + "px", top: r1.y + (lineH - ph) / 2 + "px", width: w + pad * 2 + "px", height: ph + "px",
    opacity: op * tail, transform: `scaleX(${sx})`, transformOrigin: "left center" });
  void gap;
}

window.motion = {
  load,
  async seek(t) { render(t); },
  async frame(i) { render(i / TL.fps); },
};
window.motionReady = true;
