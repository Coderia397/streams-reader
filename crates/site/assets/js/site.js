// Streams Reader website: OS-aware download buttons and the hero demo (a pre-rendered clip with word timings).
// Loaded with `defer` from a file (strict CSP: no inline scripts). Everything here is optional progressive
// enhancement; the pages work without it.
(function () {
  "use strict";

  // ---- which operating system is this?
  function detectOs() {
    var uaData = navigator.userAgentData;
    var p = ((uaData && uaData.platform) || navigator.platform || "").toLowerCase();
    var ua = navigator.userAgent.toLowerCase();
    if (/android/.test(ua) || p === "android") return "android";
    if (/iphone|ipad|ipod/.test(ua) || (p === "macintel" && navigator.maxTouchPoints > 1)) return "ios";
    if (/mac/.test(p) || /mac os x/.test(ua)) return "macos";
    if (/win/.test(p) || /windows/.test(ua)) return "windows";
    if (/linux|x11|cros/.test(p) || /linux|cros/.test(ua)) return "linux";
    return "";
  }

  function applyOs(os) {
    if (!os) return;
    document.querySelectorAll("[data-os-download]").forEach(function (btn) {
      var label = btn.getAttribute("data-label-" + os);
      var target = btn.querySelector("[data-os-label]");
      if (label && target) target.textContent = label;
    });
    document.querySelectorAll(".platform[data-os='" + os + "']").forEach(function (card) {
      card.classList.add("is-detected");
      var list = card.parentElement;
      if (list && list.classList.contains("platforms--detail")) list.insertBefore(card, list.children[1] || null);
    });
  }

  // ---- hero demo
  function setupDemo(root) {
    var base = root.getAttribute("data-base");
    var playBtn = root.querySelector("[data-demo-play]");
    var progress = root.querySelector("[data-demo-progress]");
    var speedBtn = root.querySelector("[data-demo-speed]");
    var quote = root.querySelector(".demo__text");
    var pill = root.querySelector(".demo__pill");
    var words = Array.prototype.slice.call(root.querySelectorAll(".w"));
    var chips = Array.prototype.slice.call(root.querySelectorAll("[data-voice]"));
    var labels = { play: root.getAttribute("data-label-play"), pause: root.getAttribute("data-label-pause") };
    var speeds = [1, 1.25, 1.5, 2];
    var state = { voice: chips.length ? chips[0].getAttribute("data-voice") : "", timings: {}, audio: null, rate: 1, index: -1, lineTop: null, raf: 0 };

    function loadTimings(voice) {
      if (state.timings[voice]) return Promise.resolve(state.timings[voice]);
      return fetch(base + voice + ".json").then(function (r) {
        if (!r.ok) throw new Error("timings " + r.status);
        return r.json();
      }).then(function (doc) {
        state.timings[voice] = doc.words;
        return doc.words;
      });
    }

    function audioFor(voice) {
      var a = new Audio(base + voice + ".mp3");
      a.preload = "auto";
      a.playbackRate = state.rate;
      a.addEventListener("ended", function () { setPlaying(false); finish(); });
      a.addEventListener("pause", function () { setPlaying(false); });
      a.addEventListener("play", function () { setPlaying(true); });
      return a;
    }

    function setPlaying(on) {
      playBtn.setAttribute("aria-pressed", on ? "true" : "false");
      playBtn.setAttribute("aria-label", on ? labels.pause : labels.play);
      root.classList.toggle("is-playing", on);
      if (on) loop(); else cancelAnimationFrame(state.raf);
    }

    function wordAt(timings, t) {
      var lo = 0, hi = timings.length - 1, found = -1;
      while (lo <= hi) {
        var mid = (lo + hi) >> 1;
        if (timings[mid].s <= t) { found = mid; lo = mid + 1; } else { hi = mid - 1; }
      }
      return found;
    }

    function placePill(i) {
      if (i < 0 || !words[i]) { pill.classList.remove("is-on"); state.index = -1; return; }
      var q = quote.getBoundingClientRect();
      var r = words[i].getBoundingClientRect();
      var pad = 3;
      var top = r.top - q.top;
      var jump = state.lineTop !== null && Math.abs(top - state.lineTop) > 2;
      pill.classList.toggle("is-jump", jump);
      pill.style.width = (r.width + pad * 2) + "px";
      pill.style.height = r.height + "px";
      pill.style.transform = "translate(" + (r.left - q.left - pad) + "px," + top + "px)";
      pill.classList.add("is-on");
      state.lineTop = top;
      state.index = i;
    }

    function loop() {
      var a = state.audio, timings = state.timings[state.voice];
      if (!a || !timings) return;
      var i = wordAt(timings, a.currentTime);
      if (i !== state.index) placePill(i);
      if (a.duration) progress.style.transform = "scaleX(" + Math.min(1, a.currentTime / a.duration) + ")";
      if (!a.paused) state.raf = requestAnimationFrame(loop);
    }

    function finish() {
      progress.style.transform = "scaleX(1)";
      window.setTimeout(function () {
        if (state.audio && state.audio.paused) {
          pill.classList.remove("is-on");
          state.index = -1;
          state.lineTop = null;
          progress.style.transform = "scaleX(0)";
        }
      }, 900);
    }

    function start() {
      var still = root.querySelector(".w.is-still");
      if (still) still.classList.remove("is-still");
      loadTimings(state.voice).then(function () {
        if (!state.audio) state.audio = audioFor(state.voice);
        if (state.audio.ended) state.audio.currentTime = 0;
        return state.audio.play();
      }).catch(function () { setPlaying(false); });
    }

    playBtn.addEventListener("click", function () {
      if (state.audio && !state.audio.paused) state.audio.pause(); else start();
    });

    chips.forEach(function (chip) {
      chip.addEventListener("click", function () {
        var voice = chip.getAttribute("data-voice");
        if (voice === state.voice) return;
        var wasPlaying = state.audio && !state.audio.paused;
        if (state.audio) { state.audio.pause(); state.audio = null; }
        state.voice = voice;
        state.index = -1;
        state.lineTop = null;
        progress.style.transform = "scaleX(0)";
        chips.forEach(function (c) {
          var on = c === chip;
          c.setAttribute("aria-pressed", on ? "true" : "false");
          c.classList.toggle("is-selected", on);
        });
        if (wasPlaying) start(); else loadTimings(voice).catch(function () {});
      });
    });

    if (speedBtn) {
      speedBtn.addEventListener("click", function () {
        state.rate = speeds[(speeds.indexOf(state.rate) + 1) % speeds.length];
        speedBtn.textContent = state.rate + "×";
        if (state.audio) state.audio.playbackRate = state.rate;
      });
    }

    window.addEventListener("resize", function () {
      if (state.index >= 0) { state.lineTop = null; var i = state.index; state.index = -1; placePill(i); }
    });

    playBtn.disabled = false;
    playBtn.setAttribute("aria-pressed", "false");
  }

  applyOs(detectOs());
  var demo = document.querySelector("[data-demo]");
  if (demo) setupDemo(demo);
})();
