//! Install prompt and service worker (`interop/pwa.js`).

use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::JsFuture;

#[wasm_bindgen(module = "/interop/pwa.js")]
extern "C" {
    #[wasm_bindgen(js_name = canInstall)]
    pub fn can_install() -> bool;
    #[wasm_bindgen(js_name = isStandalone)]
    pub fn is_standalone() -> bool;
    #[wasm_bindgen(js_name = promptInstall)]
    fn prompt_install() -> js_sys::Promise;
    #[wasm_bindgen(js_name = onInstallChange)]
    fn on_install_change(cb: &JsValue);
    #[wasm_bindgen(js_name = registerServiceWorker)]
    fn register_service_worker(on_update_ready: &JsValue);
    #[wasm_bindgen(js_name = applyUpdate)]
    fn apply_update_js() -> js_sys::Promise;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InstallOutcome {
    Accepted,
    Dismissed,
    Unavailable,
}

pub async fn install() -> InstallOutcome {
    match JsFuture::from(prompt_install()).await.ok().and_then(|v| v.as_string()).as_deref() {
        Some("accepted") => InstallOutcome::Accepted,
        Some("dismissed") => InstallOutcome::Dismissed,
        _ => InstallOutcome::Unavailable,
    }
}

/// `cb` runs whenever installability changes (app-lifetime listener).
pub fn watch_install(cb: impl Fn() + 'static) {
    let closure = Closure::<dyn Fn()>::new(cb).into_js_value();
    on_install_change(&closure);
}

/// Registers `/sw.js`; `on_update_ready` runs when a new version waits to activate (app-lifetime listener).
pub fn register(on_update_ready: impl Fn() + 'static) {
    let closure = Closure::<dyn Fn()>::new(on_update_ready).into_js_value();
    register_service_worker(&closure);
}

pub async fn apply_update() {
    let _ = JsFuture::from(apply_update_js()).await;
}
