//! Rust bindings for the ES modules in `crates/web/interop/` (wasm-bindgen copies them into the build as snippets,
//! so they load from the app's own origin under the strict CSP).

pub mod pdf;
pub mod pwa;
pub mod recorder;

use wasm_bindgen::JsValue;

/// Reads `obj[name]`.
pub(crate) fn prop(obj: &JsValue, name: &str) -> JsValue {
    js_sys::Reflect::get(obj, &JsValue::from_str(name)).unwrap_or(JsValue::UNDEFINED)
}
