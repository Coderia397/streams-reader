//! Minimal IndexedDB access: one database, string keys, JSON string values. IndexedDB requests are event based, so
//! each one is wrapped in a `Promise` and awaited. The connection is opened once and reused.

use std::cell::RefCell;

use serde::Serialize;
use serde::de::DeserializeOwned;
use wasm_bindgen::prelude::Closure;
use wasm_bindgen::{JsCast, JsValue};
use wasm_bindgen_futures::JsFuture;
use web_sys::{IdbDatabase, IdbRequest, IdbTransactionMode};

use crate::api::ClientError;
use crate::api::client::js_error_text;
use crate::util::dom;

const DB_NAME: &str = "nolabs";
const DB_VERSION: u32 = 1;

/// `doc_id` → [`super::progress::LocalProgress`].
pub const PROGRESS: &str = "progress";
/// `doc_id` → [`nolabs_core::proto::api::DocumentBody`], so opened documents read offline.
pub const BODIES: &str = "bodies";
/// `doc_id|voice` → ready render slots, so cached audio replays offline.
pub const RENDERS: &str = "renders";
const STORES: [&str; 3] = [PROGRESS, BODIES, RENDERS];

thread_local! {
    static DB: RefCell<Option<IdbDatabase>> = const { RefCell::new(None) };
}

fn browser(e: JsValue) -> ClientError {
    ClientError::Browser(js_error_text(e))
}

/// Resolves with `request.result` on success.
async fn await_request(req: &IdbRequest) -> Result<JsValue, ClientError> {
    let promise = js_sys::Promise::new(&mut |resolve, reject| {
        let ok_req = req.clone();
        let on_ok = Closure::once_into_js(move |_ev: web_sys::Event| {
            let value = ok_req.result().unwrap_or(JsValue::UNDEFINED);
            let _ = resolve.call1(&JsValue::NULL, &value);
        });
        let err_req = req.clone();
        let on_err = Closure::once_into_js(move |_ev: web_sys::Event| {
            let err = err_req.error().ok().flatten().map(JsValue::from).unwrap_or(JsValue::NULL);
            let _ = reject.call1(&JsValue::NULL, &err);
        });
        req.set_onsuccess(Some(on_ok.unchecked_ref()));
        req.set_onerror(Some(on_err.unchecked_ref()));
    });
    JsFuture::from(promise).await.map_err(browser)
}

async fn open() -> Result<IdbDatabase, ClientError> {
    if let Some(db) = DB.with(|d| d.borrow().clone()) {
        return Ok(db);
    }
    let window = dom::window().ok_or_else(|| ClientError::Browser("no window".into()))?;
    let factory =
        window.indexed_db().map_err(browser)?.ok_or_else(|| ClientError::Browser("IndexedDB is off".into()))?;
    let req = factory.open_with_u32(DB_NAME, DB_VERSION).map_err(browser)?;
    let upgrade_req = req.clone();
    let on_upgrade = Closure::once_into_js(move |_ev: web_sys::Event| {
        let Ok(result) = upgrade_req.result() else { return };
        let Ok(db) = result.dyn_into::<IdbDatabase>() else { return };
        let names = db.object_store_names();
        for store in STORES {
            if !names.contains(store) {
                let _ = db.create_object_store(store);
            }
        }
    });
    req.set_onupgradeneeded(Some(on_upgrade.unchecked_ref()));
    let db: IdbDatabase = await_request(&req).await?.dyn_into().map_err(browser)?;
    DB.with(|d| *d.borrow_mut() = Some(db.clone()));
    Ok(db)
}

async fn store(name: &str, mode: IdbTransactionMode) -> Result<web_sys::IdbObjectStore, ClientError> {
    let db = open().await?;
    let tx = db.transaction_with_str_and_mode(name, mode).map_err(browser)?;
    tx.object_store(name).map_err(browser)
}

pub async fn get<T: DeserializeOwned>(store_name: &str, key: &str) -> Result<Option<T>, ClientError> {
    let s = store(store_name, IdbTransactionMode::Readonly).await?;
    let value = await_request(&s.get(&JsValue::from_str(key)).map_err(browser)?).await?;
    match value.as_string() {
        Some(text) => serde_json::from_str(&text).map(Some).map_err(|e| ClientError::Decode(e.to_string())),
        None => Ok(None),
    }
}

pub async fn put<T: Serialize>(store_name: &str, key: &str, value: &T) -> Result<(), ClientError> {
    let text = serde_json::to_string(value).map_err(|e| ClientError::Decode(e.to_string()))?;
    let s = store(store_name, IdbTransactionMode::Readwrite).await?;
    await_request(&s.put_with_key(&JsValue::from_str(&text), &JsValue::from_str(key)).map_err(browser)?).await?;
    Ok(())
}

pub async fn delete(store_name: &str, key: &str) -> Result<(), ClientError> {
    let s = store(store_name, IdbTransactionMode::Readwrite).await?;
    await_request(&s.delete(&JsValue::from_str(key)).map_err(browser)?).await?;
    Ok(())
}

/// Every value in a store; entries that no longer parse are skipped.
pub async fn all<T: DeserializeOwned>(store_name: &str) -> Result<Vec<T>, ClientError> {
    let s = store(store_name, IdbTransactionMode::Readonly).await?;
    let value = await_request(&s.get_all().map_err(browser)?).await?;
    let array: js_sys::Array = value.dyn_into().map_err(browser)?;
    Ok(array.iter().filter_map(|v| v.as_string()).filter_map(|t| serde_json::from_str(&t).ok()).collect())
}
