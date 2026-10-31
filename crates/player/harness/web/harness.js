// Player harness page. Instruments MediaSource, SourceBuffer, the media element and the Media Session before the
// player exists, serves fixture clips through the player's Provider (with optional per-segment delays and failures),
// and exposes window.__harness for harness/run.py (Chrome DevTools Protocol) or a person at the console.
//
// URL query: ?delay=<ms for every segment>&delays=<seg>:<ms>,<seg>:<ms>&fail=<seg>:<times>&fatal=<seg>:<times>
// &autoload=1&nomse=1. `fail` answers with a retryable error (PlayerError::Load), `fatal` with a final one
// (PlayerError::Other). `nomse` hides MediaSource so the player takes its fallback path (one blob URL per segment, as
// on iPhone Safari); the log then holds one synthetic build of fixture-length appends and media().ct is the document
// time of the blob the element holds, so run.py's ground truth still applies (its gapless check does not).
import init, { Harness } from './pkg/harness.js';

const now = () => performance.now();
const log = {
  events: [], media: [], builds: [], appends: [], removes: [], provider: [], position: [], playback_state: [],
  commands: [], samples: [], errors: [], sb_events: [],
};
let build = 0;
let audio = null;
let player = null;
let manifest = null;
const clips = new Map();
const segOf = new WeakMap();
const config = { delay_ms: 0, delays: {}, fail: {}, fatal: {} };
const q = new URLSearchParams(location.search);
const NOMSE = Boolean(q.get('nomse'));
// Fallback path: blob URL -> segment (fixture sizes are distinct), and each segment's start in the document.
const urlSegment = new Map();
const segmentOfSize = new Map();
const startOf = [];

function ranges(tr) {
  const out = [];
  if (!tr) return out;
  for (let i = 0; i < tr.length; i++) out.push([tr.start(i), tr.end(i)]);
  return out;
}

function currentTime() {
  if (!NOMSE) return audio.currentTime;
  const segment = urlSegment.get(audio.src);
  return segment === undefined ? null : startOf[segment] + audio.currentTime;
}

function media() {
  if (!audio) return { ct: null, build, ranges: [], paused: null, seeking: null, ready: null, rate: null };
  return {
    ct: currentTime(), build, ranges: NOMSE ? [] : ranges(audio.buffered), paused: audio.paused,
    seeking: audio.seeking, ready: audio.readyState, rate: audio.playbackRate, ended: audio.ended,
  };
}

// --- query --------------------------------------------------------------------------------------------------------
if (q.get('delay')) config.delay_ms = Number(q.get('delay'));
for (const [key, field] of [['delays', 'delays'], ['fail', 'fail'], ['fatal', 'fatal']]) {
  for (const pair of (q.get(key) || '').split(',').filter(Boolean)) {
    const [s, v] = pair.split(':').map(Number);
    config[field][s] = v;
  }
}

// --- MediaSource / SourceBuffer -------------------------------------------------------------------------------------
const NativeMediaSource = window.MediaSource;
class LoggedMediaSource extends NativeMediaSource {
  constructor() {
    super();
    build += 1;
    this.__build = build;
    log.builds.push({ t: now(), build });
  }
  addSourceBuffer(mime) {
    const sb = super.addSourceBuffer(mime);
    sb.__build = this.__build;
    sb.__current = null;
    // Registered before the player's own listeners, so these run first in every dispatch: the player's next append
    // (started from a microtask after its listener) cannot overtake the bookkeeping of the one that just finished.
    // `error` and `abort` are followed by `updateend`, which closes the entry.
    sb.addEventListener('error', () => { if (sb.__current) sb.__current.ok = 'error'; });
    sb.addEventListener('abort', () => { if (sb.__current) sb.__current.ok = 'abort'; });
    sb.addEventListener('updateend', () => {
      const entry = sb.__current;
      log.sb_events.push({ t: now(), build: sb.__build, type: 'updateend', segment: entry ? entry.segment : null });
      sb.__current = null;
      if (!entry) return;
      entry.t1 = now();
      entry.ok = entry.ok || 'ok';
      entry.tso_after = sb.timestampOffset;
      entry.end = bufferedEnd(sb);
      try { entry.ranges = ranges(sb.buffered); } catch { entry.ranges = null; }
    });
    return sb;
  }
}
window.MediaSource = NOMSE ? undefined : LoggedMediaSource;
if (NOMSE) {
  const createObjectURL = URL.createObjectURL;
  URL.createObjectURL = function (obj) {
    const url = createObjectURL.call(URL, obj);
    if (obj instanceof Blob && segmentOfSize.has(obj.size)) urlSegment.set(url, segmentOfSize.get(obj.size));
    return url;
  };
}

function bufferedEnd(sb) {
  try {
    const r = sb.buffered;
    return r.length ? r.end(r.length - 1) : 0;
  } catch {
    return null;
  }
}

const appendBuffer = SourceBuffer.prototype.appendBuffer;
SourceBuffer.prototype.appendBuffer = function (data) {
  // In sequence mode a segment starts where the buffer ends. timestampOffset is kept for reference: after an append
  // it holds the offset applied to that append (its start), not its end.
  const entry = {
    build: this.__build, segment: segOf.has(data) ? segOf.get(data) : null, t0: now(), start: bufferedEnd(this),
    tso_before: this.timestampOffset, mode: this.mode, bytes: data.byteLength,
  };
  log.appends.push(entry);
  try {
    appendBuffer.call(this, data);
  } catch (err) {
    entry.ok = `throw:${err && err.name}`;
    throw err;
  }
  this.__current = entry;
};
const remove = SourceBuffer.prototype.remove;
SourceBuffer.prototype.remove = function (start, end) {
  log.removes.push({ t: now(), build: this.__build, start, end });
  remove.call(this, start, end);
  this.__current = null;
};

// --- Media Session --------------------------------------------------------------------------------------------------
const handlers = {};
function hookSession(ms, proto) {
  const target = proto || ms;
  const setPos = target.setPositionState;
  target.setPositionState = function (state) {
    log.position.push({ t: now(), ...(state || { cleared: true }), ...media() });
    if (setPos) return setPos.call(this, state);
  };
  const setHandler = target.setActionHandler;
  target.setActionHandler = function (action, handler) {
    handlers[action] = handler;
    if (setHandler) {
      try { return setHandler.call(this, action, handler); } catch { /* unknown action in this engine */ }
    }
  };
  const desc = proto && Object.getOwnPropertyDescriptor(proto, 'playbackState');
  if (desc && desc.set) {
    Object.defineProperty(proto, 'playbackState', {
      configurable: true,
      get() { return desc.get.call(this); },
      set(v) { log.playback_state.push({ t: now(), value: v, ...media() }); desc.set.call(this, v); },
    });
  }
}
if ('mediaSession' in navigator && typeof MediaSession !== 'undefined') {
  hookSession(navigator.mediaSession, MediaSession.prototype);
} else {
  const fake = { metadata: null, _state: 'none' };
  Object.defineProperty(fake, 'playbackState', {
    get() { return this._state; },
    set(v) { log.playback_state.push({ t: now(), value: v, ...media() }); this._state = v; },
  });
  fake.setPositionState = () => {};
  fake.setActionHandler = () => {};
  hookSession(fake, null);
  Object.defineProperty(navigator, 'mediaSession', { value: fake, configurable: true });
  if (typeof window.MediaMetadata === 'undefined') window.MediaMetadata = class { constructor(m) { Object.assign(this, m); } };
}

// --- provider -------------------------------------------------------------------------------------------------------
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));
async function clip(segment) {
  if (!clips.has(segment)) {
    const meta = manifest.segments[segment];
    const resp = await fetch(`../fixtures/${meta.file}`);
    clips.set(segment, await resp.arrayBuffer());
  }
  return clips.get(segment);
}
async function provide(segment) {
  const delay = segment in config.delays ? config.delays[segment] : config.delay_ms;
  const entry = { t: now(), segment, delay };
  log.provider.push(entry);
  if (delay > 0) await sleep(delay);
  entry.served = now();
  if (config.fail[segment] > 0) {
    config.fail[segment] -= 1;
    entry.failed = true;
    return { error: `harness: induced failure for segment ${segment}` };
  }
  if (config.fatal[segment] > 0) {
    config.fatal[segment] -= 1;
    entry.failed = 'fatal';
    return { error: `harness: induced final failure for segment ${segment}`, fatal: true };
  }
  const meta = manifest.segments[segment];
  if (!meta) return { error: `no fixture for segment ${segment}`, fatal: true };
  const bytes = new Uint8Array((await clip(segment)).slice(0));
  segOf.set(bytes, segment);
  return { bytes, duration_ms: meta.duration_ms, words: meta.words, ends_section: meta.ends_section };
}

function onEvent(ev) {
  const entry = { t: now(), ...ev, ...media() };
  log.events.push(entry);
}

// --- sampling -------------------------------------------------------------------------------------------------------
let sampler = null;
function sample() {
  if (!player) return;
  log.samples.push({ t: now(), snap: player.snapshot(), ...media() });
}

// --- player ---------------------------------------------------------------------------------------------------------
/** Logs the events of the player's `<audio>` element (each player has its own). */
function watchAudio() {
  audio = document.querySelector('audio');
  const names = ['loadstart', 'loadedmetadata', 'canplay', 'playing', 'pause', 'waiting', 'stalled', 'seeking',
    'seeked', 'ended', 'emptied', 'ratechange', 'error', 'timeupdate', 'durationchange'];
  for (const name of names) audio.addEventListener(name, () => log.media.push({ t: now(), name, ...media() }));
}
/** What the web app does on opening another document: a new player first, then the old one unloaded and dropped,
 * then the new one loaded. */
function reopen(start) {
  const next = new Harness(provide, onEvent);
  player.unload();
  player.free();
  player = next;
  watchAudio();
  player.load(manifest.segments.length, start);
}
/** The page's Media Session as the lock screen sees it. */
function sessionView() {
  const ms = navigator.mediaSession;
  return { metadata: !!(ms && ms.metadata), handlers: Object.keys(handlers).filter((k) => handlers[k]).sort() };
}

// --- API ------------------------------------------------------------------------------------------------------------
const commands = ['load', 'reopen', 'play', 'pause', 'toggle', 'seek_by', 'jump_to', 'next', 'previous', 'set_rate', 'set_sleep',
  'set_skip', 'unload'];
const api = {
  log,
  config,
  get manifest() { return manifest; },
  configure(cfg) {
    if (cfg.delay_ms !== undefined) config.delay_ms = cfg.delay_ms;
    if (cfg.delays) config.delays = Object.fromEntries(Object.entries(cfg.delays).map(([k, v]) => [Number(k), v]));
    if (cfg.fail) config.fail = Object.fromEntries(Object.entries(cfg.fail).map(([k, v]) => [Number(k), v]));
    if (cfg.fatal) config.fatal = Object.fromEntries(Object.entries(cfg.fatal).map(([k, v]) => [Number(k), v]));
    return config;
  },
  /** Runs one player command and records the state just before it. */
  cmd(name, ...args) {
    if (!commands.includes(name)) throw new Error(`unknown command ${name}`);
    const entry = { t: now(), name, args, snap: player.snapshot(), ...media() };
    log.commands.push(entry);
    if (name === 'load') player.load(manifest.segments.length, args[0] || 0);
    else if (name === 'reopen') reopen(args[0] || 0);
    else player[name](...args);
    entry.t_after = now();
    return entry;
  },
  /** Calls a captured Media Session action handler as the lock screen would. */
  action(name, details = {}) {
    const entry = { t: now(), name: `action:${name}`, args: [details], snap: player.snapshot(), ...media() };
    log.commands.push(entry);
    if (!handlers[name]) throw new Error(`no handler for ${name}`);
    handlers[name]({ action: name, ...details });
    return entry;
  },
  state() { return { t: now(), snap: player ? player.snapshot() : null, session: sessionView(), ...media() }; },
  sampling(on, every_ms = 50) {
    if (sampler) clearInterval(sampler);
    sampler = on ? setInterval(sample, every_ms) : null;
  },
  /** Returns every log entry recorded since the last drain. */
  drain() {
    const out = {};
    // Appends and provider calls still in flight stay for the next drain, so their completion is not lost.
    const open = { appends: (e) => e.t1 === undefined && !String(e.ok || '').startsWith('throw'),
      provider: (e) => e.served === undefined };
    for (const key of Object.keys(log)) {
      const keep = open[key] ? log[key].filter(open[key]) : [];
      out[key] = open[key] ? log[key].filter((e) => !open[key](e)) : log[key];
      log[key] = keep;
    }
    return out;
  },
  handlers() { return Object.keys(handlers); },
};
for (const name of commands) api[name] = (...args) => api.cmd(name, ...args);
window.__harness = api;

window.addEventListener('error', (e) => log.errors.push({ t: now(), message: String(e.message) }));
window.addEventListener('unhandledrejection', (e) => log.errors.push({ t: now(), message: String(e.reason) }));

api.ready = (async () => {
  manifest = await (await fetch('../fixtures/manifest.json')).json();
  await init();
  player = new Harness(provide, onEvent);
  watchAudio();
  await Promise.all(manifest.segments.map((s) => clip(s.segment)));
  if (NOMSE) {
    build = 1;
    log.builds.push({ t: now(), build });
    let start = 0;
    for (const s of manifest.segments) {
      segmentOfSize.set(clips.get(s.segment).byteLength, s.segment);
      startOf[s.segment] = start;
      const end = start + s.duration_ms / 1000;
      log.appends.push({ build, segment: s.segment, t0: now(), t1: now(), start, end, ok: 'ok', mode: 'blob' });
      start = end;
    }
  }
  document.getElementById('status').textContent = `ready: ${manifest.segments.length} segments`;
  if (q.get('autoload')) api.load(Number(q.get('start') || 0));
  return true;
})();
