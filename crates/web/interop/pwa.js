// Install prompt and service worker registration.
// beforeinstallprompt is captured as soon as this module loads (it is imported by the app's JS glue before the wasm
// starts), so the welcome screen can offer an Install button later.

let deferred = null;
const listeners = new Set();
let updateRequested = false;

const notify = () => listeners.forEach((cb) => cb());

window.addEventListener("beforeinstallprompt", (e) => {
  e.preventDefault();
  deferred = e;
  notify();
});
window.addEventListener("appinstalled", () => {
  deferred = null;
  notify();
});

export function canInstall() {
  return deferred !== null;
}

export function isStandalone() {
  return window.matchMedia("(display-mode: standalone)").matches || navigator.standalone === true;
}

// Resolves to "accepted", "dismissed" or "unavailable".
export async function promptInstall() {
  if (!deferred) return "unavailable";
  const e = deferred;
  deferred = null;
  await e.prompt();
  const choice = await e.userChoice;
  notify();
  return choice.outcome;
}

export function onInstallChange(cb) {
  listeners.add(cb);
}

// onUpdateReady() runs when a new version has installed and waits; applyUpdate() activates it and reloads.
export function registerServiceWorker(onUpdateReady) {
  if (!("serviceWorker" in navigator)) return;
  navigator.serviceWorker.addEventListener("controllerchange", () => {
    if (updateRequested) window.location.reload();
  });
  navigator.serviceWorker
    .register("/app/sw.js", { scope: "/app/" })
    .then((reg) => {
      const announce = (w) => {
        if (w && navigator.serviceWorker.controller) onUpdateReady();
      };
      if (reg.waiting) announce(reg.waiting);
      reg.addEventListener("updatefound", () => {
        const w = reg.installing;
        if (!w) return;
        w.addEventListener("statechange", () => {
          if (w.state === "installed") announce(w);
        });
      });
      setInterval(() => reg.update().catch(() => {}), 60 * 60 * 1000);
    })
    .catch(() => {});
}

export async function applyUpdate() {
  const reg = await navigator.serviceWorker.getRegistration();
  if (reg && reg.waiting) {
    updateRequested = true;
    reg.waiting.postMessage({ type: "SKIP_WAITING" });
  } else {
    window.location.reload();
  }
}
