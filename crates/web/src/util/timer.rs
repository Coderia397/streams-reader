//! Async sleep on `setTimeout`, so polling loops can `await` without a timer crate.

use wasm_bindgen::JsCast;
use wasm_bindgen::prelude::Closure;

/// Resolves after `ms` milliseconds. Resolves immediately when there is no window (never in the app).
pub async fn sleep_ms(ms: u32) {
    let promise = js_sys::Promise::new(&mut |resolve, _reject| {
        let Some(window) = web_sys::window() else {
            let _ = resolve.call0(&wasm_bindgen::JsValue::NULL);
            return;
        };
        let cb = Closure::once_into_js(move || {
            let _ = resolve.call0(&wasm_bindgen::JsValue::NULL);
        });
        let _ = window.set_timeout_with_callback_and_timeout_and_arguments_0(cb.unchecked_ref(), ms as i32);
    });
    let _ = wasm_bindgen_futures::JsFuture::from(promise).await;
}

/// Milliseconds since the Unix epoch.
pub fn now_ms() -> i64 {
    js_sys::Date::now() as i64
}
