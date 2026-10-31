// Service worker. tools/postbuild.py fills in the version and the precache list below at build time.
//
// - install: caches the hashed app shell (index, JS, wasm, CSS, fonts, icons); waits until the page asks to update.
// Scope: /app/ (the landing site at / is not controlled by this worker).
// - navigations under /app/: network first (4 s), falling back to the cached shell, so the app opens offline.
// - /api/*: never touched; rendered audio is cached by the app itself (Cache API "nolabs-audio-v1").
// - /app/assets/vendor/*: cache first, filled on first use (pdf.js is large and only needed for PDF imports).
// - /app/import/*: the import worker (hashed names), cache first into this version's shell cache on first use, so
//   it is not downloaded at install and goes away with the shell it belongs to.

const VERSION = "__VERSION__";
const SHELL = "nolabs-shell-" + VERSION;
const RUNTIME = "nolabs-runtime-v1";
const PRECACHE = __PRECACHE__;
const NAV_TIMEOUT_MS = 4000;

self.addEventListener("install", (event) => {
  event.waitUntil(caches.open(SHELL).then((cache) => cache.addAll(PRECACHE)));
});

self.addEventListener("message", (event) => {
  if (event.data && event.data.type === "SKIP_WAITING") self.skipWaiting();
});

self.addEventListener("activate", (event) => {
  event.waitUntil(
    (async () => {
      const keys = await caches.keys();
      await Promise.all(keys.filter((k) => k.startsWith("nolabs-shell-") && k !== SHELL).map((k) => caches.delete(k)));
      await self.clients.claim();
    })(),
  );
});

async function cachedShell() {
  const cache = await caches.open(SHELL);
  return (await cache.match("/app/")) || (await cache.match("/app/index.html"));
}

async function networkFirst(request) {
  try {
    const response = await Promise.race([
      fetch(request),
      new Promise((_, reject) => setTimeout(() => reject(new Error("timeout")), NAV_TIMEOUT_MS)),
    ]);
    return response;
  } catch (_) {
    const shell = await cachedShell();
    return shell || Response.error();
  }
}

async function cacheFirst(request, cacheName) {
  const cache = await caches.open(cacheName);
  const hit = await cache.match(request);
  if (hit) return hit;
  const response = await fetch(request);
  if (response.ok) cache.put(request, response.clone());
  return response;
}

self.addEventListener("fetch", (event) => {
  const request = event.request;
  if (request.method !== "GET") return;
  const url = new URL(request.url);
  if (url.origin !== self.location.origin) return;
  if (url.pathname.startsWith("/api/") || !url.pathname.startsWith("/app")) return;
  if (request.mode === "navigate") {
    event.respondWith(networkFirst(request));
    return;
  }
  if (url.pathname.startsWith("/app/assets/vendor/")) {
    event.respondWith(cacheFirst(request, RUNTIME));
    return;
  }
  if (url.pathname.startsWith("/app/import/")) {
    event.respondWith(cacheFirst(request, SHELL));
    return;
  }
  event.respondWith(caches.match(request).then((hit) => hit || fetch(request)));
});
