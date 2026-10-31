//! The import Web Worker. trunk builds it beside the app (`data-type="worker"` in crates/web/index.html) and
//! tools/postbuild.py gives it hashed file names. It installs its message handler, says it is ready, and answers
//! each job (see the crate docs for the message shape).

use js_sys::{Object, Reflect, Uint8Array};
use nolabs_web_import::field;
use nolabs_web_import::job::run_json;
use wasm_bindgen::JsCast;
use wasm_bindgen::prelude::*;
use web_sys::{DedicatedWorkerGlobalScope, MessageEvent};

fn get(v: &JsValue, key: &str) -> JsValue {
    Reflect::get(v, &JsValue::from_str(key)).unwrap_or(JsValue::UNDEFINED)
}

fn post(scope: &DedicatedWorkerGlobalScope, pairs: &[(&str, JsValue)]) {
    let msg = Object::new();
    for (k, v) in pairs {
        let _ = Reflect::set(&msg, &JsValue::from_str(k), v);
    }
    let _ = scope.post_message(&msg);
}

fn main() {
    console_error_panic_hook::set_once();
    let scope: DedicatedWorkerGlobalScope = js_sys::global().unchecked_into();
    let reply_scope = scope.clone();
    let on_message = Closure::<dyn FnMut(MessageEvent)>::new(move |e: MessageEvent| {
        let data = e.data();
        let id = get(&data, field::ID);
        let job = get(&data, field::JOB).as_string().unwrap_or_default();
        let bytes = get(&data, field::BYTES).dyn_into::<Uint8Array>().map(|b| b.to_vec()).unwrap_or_default();
        let reply = run_json(&job, &bytes);
        post(&reply_scope, &[(field::ID, id), (field::REPLY, JsValue::from_str(&reply))]);
    });
    scope.set_onmessage(Some(on_message.as_ref().unchecked_ref()));
    // The handler lives as long as the worker.
    on_message.forget();
    post(&scope, &[(field::READY, JsValue::TRUE)]);
}
