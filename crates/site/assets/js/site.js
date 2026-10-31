// Streams Reader website script: scroll reveals (fallback), the condensing top bar, the mobile menu, OS-aware
// download labels, the hero film, the voice demo, the silent follow-along mock, cover and voice previews, and the
// classics marquee. Loaded with `defer` from a file (strict CSP: no inline script). Every part is progressive
// enhancement; the pages read and work without it.
(function () {
  "use strict";
  var doc = document, root = doc.documentElement;
  var reduce = matchMedia("(prefers-reduced-motion: reduce)");
  var $ = function (sel, el) { return (el || doc).querySelector(sel); };
  var $$ = function (sel, el) { return Array.prototype.slice.call((el || doc).querySelectorAll(sel)); };
  var hasIO = "IntersectionObserver" in window;

  // ---- one sound at a time: starting any clip stops the others
  var current = null;
  function claim(owner) { if (current && current !== owner) current.stop(); current = owner; }
  function release(owner) { if (current === owner) current = null; }
  doc.addEventListener("visibilitychange", function () { if (doc.hidden && current) current.stop(); });

  // ---- in view: reveals where scroll-driven animation is missing, and [data-inview] mocks (meters, cleanup)
  function watch(els, cb, opts) {
    if (!hasIO) { els.forEach(function (el) { cb(el, true); }); return; }
    var io = new IntersectionObserver(function (entries) {
      entries.forEach(function (e) { cb(e.target, e.isIntersecting, io); });
    }, opts || { rootMargin: "0px 0px -12% 0px" });
    els.forEach(function (el) { io.observe(el); });
  }
  if (!(window.CSS && CSS.supports && CSS.supports("animation-timeline: view()")) && hasIO && !reduce.matches) {
    root.classList.add("io");
    watch($$(".reveal, .stagger"), function (el, on, io) { if (on) { el.classList.add("is-in"); io.unobserve(el); } });
  }
  watch($$("[data-inview]"), function (el, on) { el.classList.toggle("is-in", on); }, { threshold: 0.35 });

  // ---- top bar condenses once the page scrolls
  var bar = $("[data-topbar]"), sentinel = $("[data-sentinel]");
  if (bar && sentinel && hasIO) {
    new IntersectionObserver(function (e) { bar.classList.toggle("is-scrolled", !e[0].isIntersecting); }).observe(sentinel);
  }

  // ---- mobile menu: close on a link, on Escape, or when the screen grows
  var menu = $("[data-menu]");
  if (menu) {
    menu.addEventListener("click", function (e) { if (e.target.closest("a")) menu.open = false; });
    doc.addEventListener("keydown", function (e) {
      if (e.key === "Escape" && menu.open) { menu.open = false; $("summary", menu).focus(); }
    });
    matchMedia("(min-width: 900px)").addEventListener("change", function () { menu.open = false; });
  }

  // ---- which operating system is this?
  function detectOs() {
    var uaData = navigator.userAgentData, ua = navigator.userAgent.toLowerCase();
    var p = ((uaData && uaData.platform) || navigator.platform || "").toLowerCase();
    if (/android/.test(ua) || p === "android") return "android";
    if (/iphone|ipad|ipod/.test(ua) || (/mac/.test(p) && navigator.maxTouchPoints > 1)) return "ios";
    if (/cros/.test(ua) || p === "chrome os") return "chromeos";
    if (/mac/.test(p) || /mac os x/.test(ua)) return "macos";
    if (/win/.test(p) || /windows/.test(ua)) return "windows";
    if (/linux|x11/.test(p) || /linux/.test(ua)) return "linux";
    return "";
  }
  (function applyOs(os) {
    if (!os) return;
    $$("[data-os-download]").forEach(function (el) {
      var label = el.getAttribute("data-label-" + os), target = $("[data-os-label]", el), href = el.getAttribute("data-href-" + os);
      if (label && target) target.textContent = label;
      // iOS has no app coming: "Add to Home Screen" leads to the install steps, without a "Coming soon" badge.
      if (href) { el.href = href; var badge = $(".badge", el); if (badge) badge.remove(); }
    });
    // Download page: the visitor's platform card first. Landing: the matching tile first, marked "Your device".
    var desktop = os === "macos" || os === "windows" || os === "linux";
    $$(".platform[data-os='" + os + "']").forEach(function (card) {
      card.classList.add("is-detected");
      var list = card.parentElement;
      if (list && list.hasAttribute("data-detected-first")) list.insertBefore(card, list.children[1] || null);
    });
    var tiles = $("[data-tiles]");
    if (tiles) {
      var key = desktop ? "desktop" : os === "android" ? "android" : "web";
      var tile = $(".tile[data-os='" + key + "']", tiles);
      if (tile) { tile.classList.add("is-detected"); tiles.insertBefore(tile, tiles.firstChild); }
      if (os === "chromeos") { var a = $(".tile[data-os='android']", tiles); if (a) tiles.insertBefore(a, tile.nextSibling); }
    }
  })(detectOs());

  // ---- the hero film: poster first; the silent loop loads after the page, only in view, never on Save-Data,
  // slow connections or reduced motion (then a play button starts it).
  var film = $("[data-film]");
  if (film) setupFilm(film);
  function setupFilm(fig) {
    var video = $("video", fig), btn = $("[data-film-toggle]", fig), line = $("[data-film-progress]", fig);
    var labels = { play: fig.getAttribute("data-label-play"), pause: fig.getAttribute("data-label-pause") };
    var conn = navigator.connection || {}, loaded = false, inView = false, wanted = true, raf = 0;
    var auto = !reduce.matches && !conn.saveData && !/2g/.test(conn.effectiveType || "");
    function load() {
      if (loaded) return;
      loaded = true;
      $$("source", video).forEach(function (s) { s.src = s.getAttribute("data-src"); });
      video.load();
    }
    function show(playing) {
      btn.setAttribute("aria-pressed", playing ? "true" : "false");
      btn.setAttribute("aria-label", playing ? labels.pause : labels.play);
    }
    function tick() {
      if (video.duration) line.style.transform = "scaleX(" + (video.currentTime / video.duration) + ")";
      if (!video.paused) raf = requestAnimationFrame(tick);
    }
    function play() { load(); var p = video.play(); if (p && p.catch) p.catch(function () { show(false); }); }
    video.addEventListener("playing", function () { fig.classList.add("is-playing"); show(true); tick(); });
    video.addEventListener("pause", function () { show(false); cancelAnimationFrame(raf); });
    btn.addEventListener("click", function () {
      if (video.paused) { wanted = true; play(); } else { wanted = false; video.pause(); }
    });
    btn.hidden = false;
    show(false);
    watch([fig], function (el, on) {
      inView = on;
      if (!on) video.pause(); else if (wanted && auto && loaded) play();
    }, { threshold: 0.25 });
    doc.addEventListener("visibilitychange", function () {
      if (doc.hidden) video.pause(); else if (inView && wanted && auto && loaded) play();
    });
    if (auto) {
      var start = function () { setTimeout(function () { if (inView || !hasIO) play(); else load(); }, 300); };
      if (doc.readyState === "complete") start(); else addEventListener("load", start);
    }
  }

  // ---- word timings: a pill that glides over the spoken word
  function pillMover(box, pill) {
    var lineTop = null;
    return function place(word) {
      if (!word) { pill.classList.remove("is-on"); lineTop = null; return; }
      var q = box.getBoundingClientRect(), r = word.getBoundingClientRect(), pad = 3, top = r.top - q.top;
      pill.classList.toggle("is-jump", lineTop !== null && Math.abs(top - lineTop) > 2);
      pill.style.width = r.width + pad * 2 + "px";
      pill.style.height = r.height + "px";
      pill.style.transform = "translate(" + (r.left - q.left - pad) + "px," + top + "px)";
      pill.classList.add("is-on");
      lineTop = top;
    };
  }
  function wordAt(words, ms) {
    var lo = 0, hi = words.length - 1, found = -1;
    while (lo <= hi) { var mid = (lo + hi) >> 1; if (words[mid][0] <= ms) { found = mid; lo = mid + 1; } else hi = mid - 1; }
    return found;
  }
  var cache = {};
  function timings(url) {
    if (!cache[url]) cache[url] = fetch(url).then(function (r) { if (!r.ok) throw new Error(r.status); return r.json(); });
    return cache[url];
  }
  function clock(ms) { var s = Math.ceil(ms / 1000); return Math.floor(s / 60) + ":" + ("0" + (s % 60)).slice(-2); }

  // ---- the voice demo
  var demo = $("[data-demo]");
  if (demo) setupDemo(demo);
  function setupDemo(card) {
    var base = card.getAttribute("data-base");
    var tabs = $$("[role=tab]", card), voices = $$("[role=radio]", card);
    var playBtn = $("[data-demo-play]", card), speedBtn = $("[data-demo-speed]", card);
    var wave = $("[data-wave]", card), bars = $$("i", wave), head = $(".wave__head", wave);
    var timeEl = $("[data-demo-time]", card), status = $("[data-demo-status]", card), notes = $$("[data-note]", card);
    var body = $(".demo__body", card), place = pillMover(body, $(".demo__pill", card));
    var labels = { play: card.getAttribute("data-label-play"), pause: card.getAttribute("data-label-pause") };
    var speeds = [1, 1.25, 1.5, 2], rate = 1;
    var st = { text: tabs[0].getAttribute("data-text"), voice: voices[0].getAttribute("data-voice"), audio: null, clip: null, index: -1, raf: 0 };
    var owner = { stop: function () { if (st.audio) st.audio.pause(); } };

    function words() { return $$(".w", $("#demo-p-" + st.text)); }
    function has(text, voice) {
      var t = tabs.filter(function (x) { return x.getAttribute("data-text") === text; })[0];
      return t && t.getAttribute("data-clips").split(" ").indexOf(voice) >= 0;
    }
    // All captions sit stacked in one grid cell (its height is the tallest), so showing another never moves the page;
    // the visually hidden status node carries the text to screen readers.
    function say(key) {
      notes.forEach(function (p) {
        var on = p.getAttribute("data-note") === (key || "caption");
        p.classList.toggle("off", !on);
        if (on) status.textContent = p.textContent;
      });
    }
    function levels(a) { bars.forEach(function (b, i) { b.className = "l" + (a[i] || 3) + (b.classList.contains("on") ? " on" : ""); }); }
    function progress(ms, dur) {
      var f = dur ? Math.min(1, ms / dur) : 0, n = Math.round(f * bars.length);
      bars.forEach(function (b, i) { b.classList.toggle("on", i < n); });
      head.style.transform = "translateX(" + f * wave.clientWidth + "px)";
      if (dur) timeEl.textContent = clock(Math.max(0, dur - ms));
    }
    function reset() {
      st.index = -1; place(null); progress(0, st.clip ? st.clip.d : 0);
      words().forEach(function (w) { w.classList.remove("is-done"); });
    }
    function setPlaying(on) {
      playBtn.setAttribute("aria-pressed", on ? "true" : "false");
      playBtn.setAttribute("aria-label", on ? labels.pause : labels.play);
      card.classList.toggle("is-playing", on);
      if (on) loop(); else cancelAnimationFrame(st.raf);
    }
    function loop() {
      var a = st.audio, c = st.clip;
      if (!a || !c) return;
      var ms = a.currentTime * 1000, i = wordAt(c.w, ms), ws = words();
      if (i !== st.index) {
        ws.forEach(function (w, k) { w.classList.toggle("is-done", k <= i); });
        place(ws[i]);
        st.index = i;
      }
      progress(ms, c.d);
      if (!a.paused) st.raf = requestAnimationFrame(loop);
    }
    function load() {
      var key = st.text + "-" + st.voice, clip;
      if (st.audio && st.audio.dataset.key === key) return Promise.resolve();
      if (st.pending && st.pendingKey === key) return st.pending;
      if (st.audio) { st.audio.pause(); }
      st.pendingKey = key;
      return st.pending = timings(base + key + ".json").then(function (c) {
        clip = c;
        st.pending = null;
        if (st.text + "-" + st.voice !== key) return;
        var a = new Audio(base + key + ".webm");
        a.dataset.key = key;
        a.preload = "auto";
        a.playbackRate = rate;
        a.addEventListener("playing", function () { card.classList.remove("is-loading"); setPlaying(true); });
        a.addEventListener("pause", function () { setPlaying(false); });
        a.addEventListener("ended", function () {
          setPlaying(false); release(owner); progress(clip.d, clip.d);
          setTimeout(function () { if (a.paused && st.audio === a) { a.currentTime = 0; reset(); } }, 1200);
        });
        a.addEventListener("error", function () { card.classList.remove("is-loading"); setPlaying(false); say("error"); });
        st.audio = a; st.clip = c; levels(c.a || []); reset();
      });
    }
    function play() {
      claim(owner);
      card.classList.add("is-loading");
      load().then(function () {
        if (!st.audio) { card.classList.remove("is-loading"); return; }
        if (st.audio.ended) st.audio.currentTime = 0;
        return st.audio.play();
      }).catch(function () { card.classList.remove("is-loading"); setPlaying(false); say("error"); });
    }
    function toggle() { if (st.audio && !st.audio.paused) st.audio.pause(); else play(); }

    function choose(list, el, attr) {
      list.forEach(function (x) {
        var on = x === el;
        x.setAttribute(attr, on ? "true" : "false");
        x.tabIndex = on ? 0 : -1;
      });
    }
    function selectText(tab, keepPlaying) {
      var text = tab.getAttribute("data-text");
      if (text === st.text) return;
      var was = keepPlaying && st.audio && !st.audio.paused;
      if (st.audio) st.audio.pause();
      reset();
      st.text = text; st.audio = null; st.clip = null;
      choose(tabs, tab, "aria-selected");
      $$("[role=tabpanel]", card).forEach(function (p) { p.hidden = p.getAttribute("data-panel") !== text; });
      if (!has(text, st.voice)) selectVoice(voices[0], false);
      say();
      load().then(function () { if (was) play(); });
    }
    function selectVoice(v, keepPlaying) {
      if (v.hasAttribute("data-locked")) { say("locked"); return; }
      var voice = v.getAttribute("data-voice");
      var was = keepPlaying && st.audio && !st.audio.paused;
      choose(voices, v, "aria-checked");
      if (voice === st.voice) return;
      if (st.audio) st.audio.pause();
      st.voice = voice; st.audio = null; st.clip = null;
      if (!has(st.text, voice)) {
        var t = tabs.filter(function (x) { return has(x.getAttribute("data-text"), voice); })[0];
        if (t) { selectText(t, false); say("switch"); }
      } else say();
      reset();
      load().then(function () { if (was) play(); });
    }
    // Arrow keys move within the tab list and the voice group (roving tabindex).
    function arrows(list, pick) {
      list.forEach(function (el, i) {
        el.addEventListener("keydown", function (e) {
          var rtl = getComputedStyle(el).direction === "rtl", n = list.length, j = -1;
          if (e.key === "ArrowRight" || e.key === "ArrowDown") j = (i + (rtl && e.key === "ArrowRight" ? -1 : 1) + n) % n;
          if (e.key === "ArrowLeft" || e.key === "ArrowUp") j = (i + (rtl && e.key === "ArrowLeft" ? 1 : -1) + n) % n;
          if (e.key === "Home") j = 0;
          if (e.key === "End") j = n - 1;
          if (j < 0) return;
          e.preventDefault();
          list[j].focus();
          pick(list[j], true);
        });
      });
    }
    tabs.forEach(function (t) { t.addEventListener("click", function () { selectText(t, true); }); });
    voices.forEach(function (v) { v.addEventListener("click", function () { selectVoice(v, true); }); });
    arrows(tabs, selectText);
    arrows(voices, selectVoice);
    playBtn.addEventListener("click", toggle);
    speedBtn.addEventListener("click", function () {
      rate = speeds[(speeds.indexOf(rate) + 1) % speeds.length];
      speedBtn.textContent = rate + "×";
      if (st.audio) st.audio.playbackRate = rate;
    });
    // Space toggles playback while the card (not one of its buttons) has focus.
    card.addEventListener("keydown", function (e) {
      if ((e.key === " " || e.code === "Space") && !e.target.closest("button, a")) { e.preventDefault(); toggle(); }
    });
    $$("[data-try]").forEach(function (link) {
      link.addEventListener("click", function () {
        var t = tabs.filter(function (x) { return x.getAttribute("data-text") === link.getAttribute("data-try"); })[0];
        if (t) selectText(t, false);
        setTimeout(function () { playBtn.focus({ preventScroll: true }); }, 400);
      });
    });
    addEventListener("resize", function () { if (st.index >= 0) place(words()[st.index]); if (st.clip && st.audio) progress(st.audio.currentTime * 1000, st.clip.d); });
    playBtn.disabled = false;
  }

  // ---- chapter 02: the reader follows along with real timings, silently, while in view
  $$("[data-follow]").forEach(function (mock) {
    var ws = $$(".w", mock), text = $(".reader__text", mock), place = pillMover(text, $(".demo__pill", mock));
    var clip = null, t0 = 0, raf = 0, last = -1;
    function frame(now) {
      var d = clip.d + 1500, ms = (now - t0) % d, i = wordAt(clip.w, ms);
      if (i !== last) { place(ws[i]); last = i; }
      raf = requestAnimationFrame(frame);
    }
    watch([mock], function (el, on) {
      cancelAnimationFrame(raf);
      if (!on) return;
      if (reduce.matches) { place(ws[4]); return; }
      timings(mock.getAttribute("data-follow")).then(function (c) { clip = c; t0 = performance.now(); raf = requestAnimationFrame(frame); });
    }, { threshold: 0.4 });
  });

  // ---- cover and voice previews: one short clip each, fetched on the first press
  $$("[data-preview]").forEach(function (btn) {
    var audio = null;
    var owner = { stop: function () { if (audio) audio.pause(); } };
    btn.addEventListener("click", function () {
      if (audio && !audio.paused) { audio.pause(); return; }
      if (!audio) {
        audio = new Audio(btn.getAttribute("data-preview"));
        audio.addEventListener("play", function () { btn.setAttribute("aria-pressed", "true"); });
        audio.addEventListener("pause", function () { btn.setAttribute("aria-pressed", "false"); release(owner); });
        audio.addEventListener("ended", function () { audio.currentTime = 0; });
      }
      claim(owner);
      audio.play().catch(function () { btn.setAttribute("aria-pressed", "false"); });
    });
  });

  // ---- a scroll row is a tab stop (keyboards reach every card) only while it scrolls
  $$("[data-scroller]").forEach(function (el) {
    function fit() { el.tabIndex = el.scrollWidth > el.clientWidth + 1 ? 0 : -1; }
    fit();
    addEventListener("resize", fit);
  });

  // ---- the classics marquee: touch pauses it; with reduced motion it is a scroll row with arrows, and so it becomes
  // once a keyboard user tabs into it (.is-manual), so the focused cover is scrolled into view, never off screen.
  // inline: "start" matches the row's snap points (scroll-snap-align: start), so the snap never pulls a cover back out.
  $$("[data-marquee]").forEach(function (m) {
    var view = $("[data-marquee-viewport]", m);
    m.addEventListener("pointerdown", function (e) { if (e.pointerType !== "mouse") m.classList.add("is-paused"); });
    var steps = $$("[data-marquee-step]", m);
    // An arrow at its end of the row says so (aria-disabled keeps it focusable, so focus never drops to the page).
    function ends() {
      var x = Math.abs(view.scrollLeft), max = view.scrollWidth - view.clientWidth - 2;
      steps.forEach(function (b) { b.setAttribute("aria-disabled", (b.getAttribute("data-marquee-step") < 0 ? x < 1 : x > max) ? "true" : "false"); });
    }
    function manual() { m.classList.toggle("is-manual", reduce.matches); ends(); }
    manual();
    reduce.addEventListener("change", manual);
    view.addEventListener("scroll", ends, { passive: true });
    m.addEventListener("focusin", function (e) {
      if (!e.target.matches(":focus-visible")) return;
      var el = e.target, first = !m.classList.contains("is-manual");
      m.classList.add("is-manual");
      el.scrollIntoView({ block: "nearest", inline: "start" });
      // The row has just changed layout: place the cover again once it is laid out, mid-screen, ring included.
      if (first) requestAnimationFrame(function () { el.scrollIntoView({ block: "center", inline: "start" }); ends(); });
    });
    doc.addEventListener("pointerdown", function (e) { if (!m.contains(e.target)) { m.classList.remove("is-paused"); manual(); } });
    steps.forEach(function (b) {
      b.addEventListener("click", function () {
        if (b.getAttribute("aria-disabled") === "true") return;
        var dir = Number(b.getAttribute("data-marquee-step")) * (getComputedStyle(m).direction === "rtl" ? -1 : 1);
        view.scrollBy({ left: dir * view.clientWidth * 0.8, behavior: reduce.matches ? "auto" : "smooth" });
      });
    });
  });
})();
