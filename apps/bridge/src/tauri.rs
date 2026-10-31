//! [`NativeBridge`] for the webview inside the Tauri app (wasm32 only).
//!
//! Calls go through Tauri's public global API, `window.__TAURI__` (enabled by `app.withGlobalTauri` in
//! `tauri.conf.json`), so the UI needs no JavaScript bundle: `__TAURI__.core.invoke(cmd, args)` returns a promise of
//! the command's JSON result (or an `ArrayBuffer` for raw responses), and `__TAURI__.event.listen(name, cb)` returns
//! a promise of an unlisten function. Payloads cross the boundary as JSON text (`JSON.parse` / `JSON.stringify`), so
//! no serde-wasm-bindgen dependency is needed and the wire format is exactly the serde one in [`crate::proto`].

use std::cell::RefCell;
use std::rc::Rc;

use js_sys::{Function, JSON, Promise, Reflect, Uint8Array};
use serde::Serialize;
use serde::de::DeserializeOwned;
use serde_json::{Value, json};
use wasm_bindgen::JsCast;
use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::{JsFuture, spawn_local};

use crate::{
    AppInfo, BRIDGE_VERSION, BridgeError, BridgeEvent, BridgeFuture, DownloadArgs, DownloadInfo, ErrorCode, LoadArgs,
    NativeBridge, NewSession, OpenRequest, PlayerCommand, SessionView, Snapshot, StorageUsage, Subscription,
    UpdateChannel, UpdateInfo, cmd, event,
};

/// True when the page runs inside a Tauri webview. Cheap; does not call the shell.
pub fn is_tauri() -> bool {
    let g = js_sys::global();
    let flag = Reflect::get(&g, &JsValue::from_str("isTauri")).ok().and_then(|v| v.as_bool()).unwrap_or(false);
    flag || Reflect::get(&g, &JsValue::from_str("__TAURI_INTERNALS__")).map(|v| v.is_object()).unwrap_or(false)
}

fn js_err(what: &str, e: &JsValue) -> BridgeError {
    // Commands reject with a serialised BridgeError; anything else (Tauri's own errors) arrives as a string.
    if let Some(text) = JSON::stringify(e).ok().and_then(|s| s.as_string())
        && let Ok(err) = serde_json::from_str::<BridgeError>(&text)
    {
        return err;
    }
    let message = e.as_string().unwrap_or_else(|| format!("{what} failed"));
    BridgeError::new(ErrorCode::Internal, message)
}

fn tauri_part(name: &str) -> Result<JsValue, BridgeError> {
    let g = js_sys::global();
    let tauri = Reflect::get(&g, &JsValue::from_str("__TAURI__"))
        .ok()
        .filter(JsValue::is_object)
        .ok_or_else(|| BridgeError::unsupported("window.__TAURI__ (set app.withGlobalTauri)"))?;
    Reflect::get(&tauri, &JsValue::from_str(name)).ok().filter(JsValue::is_object).ok_or_else(|| BridgeError::unsupported(name))
}

fn method(obj: &JsValue, name: &str) -> Result<Function, BridgeError> {
    Reflect::get(obj, &JsValue::from_str(name))
        .ok()
        .and_then(|f| f.dyn_into::<Function>().ok())
        .ok_or_else(|| BridgeError::unsupported(name))
}

/// `__TAURI__.core.invoke(cmd, args)` and the raw JS result.
async fn invoke_raw(command: &str, args: &Value) -> Result<JsValue, BridgeError> {
    let core = tauri_part("core")?;
    let invoke = method(&core, "invoke")?;
    let args_text = serde_json::to_string(args).map_err(|e| BridgeError::internal(e.to_string()))?;
    let js_args = JSON::parse(&args_text).map_err(|e| js_err(command, &e))?;
    let promise: Promise = invoke
        .call2(&core, &JsValue::from_str(command), &js_args)
        .map_err(|e| js_err(command, &e))?
        .dyn_into()
        .map_err(|e| js_err(command, &e))?;
    JsFuture::from(promise).await.map_err(|e| js_err(command, &e))
}

async fn invoke<R: DeserializeOwned>(command: &str, args: Value) -> Result<R, BridgeError> {
    let out = invoke_raw(command, &args).await?;
    let text = if out.is_undefined() || out.is_null() {
        "null".to_owned()
    } else {
        JSON::stringify(&out).ok().and_then(|s| s.as_string()).unwrap_or_else(|| "null".to_owned())
    };
    serde_json::from_str(&text).map_err(|e| BridgeError::internal(format!("{command}: bad response: {e}")))
}

fn arg<T: Serialize>(name: &str, value: &T) -> Value {
    json!({ name: value })
}

/// The bridge inside the app. Construct with [`TauriBridge::connect`].
pub struct TauriBridge {
    info: AppInfo,
}

impl TauriBridge {
    /// Ask the shell for its [`AppInfo`]. Fails outside Tauri, or when the shell speaks another contract version
    /// (the UI then shows "update the app" instead of misbehaving).
    pub async fn connect() -> Result<Self, BridgeError> {
        if !is_tauri() {
            return Err(BridgeError::unsupported("Tauri"));
        }
        let info: AppInfo = invoke(cmd::APP_INFO, json!({})).await?;
        if info.bridge_version != BRIDGE_VERSION {
            return Err(BridgeError::new(
                ErrorCode::Invalid,
                format!("app shell speaks bridge v{}, UI expects v{BRIDGE_VERSION}", info.bridge_version),
            ));
        }
        Ok(Self { info })
    }
}

impl NativeBridge for TauriBridge {
    fn info(&self) -> &AppInfo {
        &self.info
    }

    fn player_load(&self, args: LoadArgs) -> BridgeFuture<'_, ()> {
        Box::pin(async move { invoke(cmd::PLAYER_LOAD, arg("args", &args)).await })
    }
    fn player_control(&self, command: PlayerCommand) -> BridgeFuture<'_, ()> {
        Box::pin(async move { invoke(cmd::PLAYER_CONTROL, arg("command", &command)).await })
    }
    fn player_snapshot(&self) -> BridgeFuture<'_, Snapshot> {
        Box::pin(invoke(cmd::PLAYER_SNAPSHOT, json!({})))
    }

    fn download_start(&self, args: DownloadArgs) -> BridgeFuture<'_, ()> {
        Box::pin(async move { invoke(cmd::DOWNLOAD_START, arg("args", &args)).await })
    }
    fn download_cancel(&self, doc_id: String) -> BridgeFuture<'_, ()> {
        Box::pin(async move { invoke(cmd::DOWNLOAD_CANCEL, arg("docId", &doc_id)).await })
    }
    fn download_remove(&self, doc_id: String) -> BridgeFuture<'_, ()> {
        Box::pin(async move { invoke(cmd::DOWNLOAD_REMOVE, arg("docId", &doc_id)).await })
    }
    fn download_list(&self) -> BridgeFuture<'_, Vec<DownloadInfo>> {
        Box::pin(invoke(cmd::DOWNLOAD_LIST, json!({})))
    }
    fn storage_usage(&self) -> BridgeFuture<'_, StorageUsage> {
        Box::pin(invoke(cmd::STORAGE_USAGE, json!({})))
    }
    fn cache_clear(&self) -> BridgeFuture<'_, StorageUsage> {
        Box::pin(invoke(cmd::CACHE_CLEAR, json!({})))
    }

    fn auth_store(&self, session: NewSession) -> BridgeFuture<'_, SessionView> {
        Box::pin(async move { invoke(cmd::AUTH_STORE, arg("session", &session)).await })
    }
    fn auth_session(&self) -> BridgeFuture<'_, Option<SessionView>> {
        Box::pin(invoke(cmd::AUTH_SESSION, json!({})))
    }
    fn auth_refresh(&self) -> BridgeFuture<'_, SessionView> {
        Box::pin(invoke(cmd::AUTH_REFRESH, json!({})))
    }
    fn auth_sign_out(&self) -> BridgeFuture<'_, ()> {
        Box::pin(invoke(cmd::AUTH_SIGN_OUT, json!({})))
    }
    fn auth_open_browser(&self, url: String) -> BridgeFuture<'_, ()> {
        Box::pin(async move { invoke(cmd::AUTH_OPEN_BROWSER, arg("url", &url)).await })
    }

    fn update_check(&self, channel: UpdateChannel) -> BridgeFuture<'_, Option<UpdateInfo>> {
        Box::pin(async move { invoke(cmd::UPDATE_CHECK, arg("channel", &channel)).await })
    }
    fn update_install(&self) -> BridgeFuture<'_, ()> {
        Box::pin(invoke(cmd::UPDATE_INSTALL, json!({})))
    }

    fn open_take_pending(&self) -> BridgeFuture<'_, Vec<OpenRequest>> {
        Box::pin(invoke(cmd::OPEN_TAKE_PENDING, json!({})))
    }
    fn file_read(&self, path: String) -> BridgeFuture<'_, Vec<u8>> {
        Box::pin(async move {
            let out = invoke_raw(cmd::FILE_READ, &arg("path", &path)).await?;
            // A `tauri::ipc::Response` arrives as an ArrayBuffer.
            Ok(Uint8Array::new(&out).to_vec())
        })
    }

    fn subscribe(&self, handler: Box<dyn Fn(BridgeEvent)>) -> Subscription {
        let handler: Rc<dyn Fn(BridgeEvent)> = Rc::from(handler);
        // Unlisten functions arrive asynchronously; the subscription may be dropped before they do.
        let state = Rc::new(RefCell::new(ListenState::default()));
        for name in event::ALL {
            let name: &'static str = name;
            let h = handler.clone();
            let cb = Closure::<dyn Fn(JsValue)>::new(move |e: JsValue| {
                let payload = Reflect::get(&e, &JsValue::from_str("payload")).unwrap_or(JsValue::NULL);
                let text = JSON::stringify(&payload).ok().and_then(|s| s.as_string()).unwrap_or_default();
                if let Some(ev) = decode_event(name, &text) {
                    h(ev);
                }
            });
            let st = state.clone();
            let fut = listen(name, cb.as_ref().unchecked_ref::<Function>().clone());
            state.borrow_mut().closures.push(cb);
            spawn_local(async move {
                if let Ok(unlisten) = fut.await {
                    let mut s = st.borrow_mut();
                    if s.dropped {
                        let _ = unlisten.call0(&JsValue::NULL);
                    } else {
                        s.unlisten.push(unlisten);
                    }
                }
            });
        }
        Subscription::new(move || {
            let mut s = state.borrow_mut();
            s.dropped = true;
            for f in s.unlisten.drain(..) {
                let _ = f.call0(&JsValue::NULL);
            }
            // Dropping the closures after unlisten: Tauri no longer calls them.
            s.closures.clear();
        })
    }
}

#[derive(Default)]
struct ListenState {
    closures: Vec<Closure<dyn Fn(JsValue)>>,
    unlisten: Vec<Function>,
    dropped: bool,
}

async fn listen(name: &str, cb: Function) -> Result<Function, BridgeError> {
    let ev = tauri_part("event")?;
    let listen = method(&ev, "listen")?;
    let promise: Promise = listen
        .call2(&ev, &JsValue::from_str(name), &cb)
        .map_err(|e| js_err(name, &e))?
        .dyn_into()
        .map_err(|e| js_err(name, &e))?;
    let unlisten = JsFuture::from(promise).await.map_err(|e| js_err(name, &e))?;
    unlisten.dyn_into::<Function>().map_err(|e| js_err(name, &e))
}

/// Map a Tauri event (name + JSON payload) to a [`BridgeEvent`]. Unknown names or bad payloads are dropped.
fn decode_event(name: &str, payload: &str) -> Option<BridgeEvent> {
    Some(match name {
        n if n == event::PLAYER => BridgeEvent::Player { event: serde_json::from_str(payload).ok()? },
        n if n == event::DOWNLOAD => BridgeEvent::Download { info: serde_json::from_str(payload).ok()? },
        n if n == event::UPDATE => BridgeEvent::Update { event: serde_json::from_str(payload).ok()? },
        n if n == event::OPEN => BridgeEvent::Open { request: serde_json::from_str(payload).ok()? },
        n if n == event::AUTH_CALLBACK => {
            let v: Value = serde_json::from_str(payload).ok()?;
            BridgeEvent::AuthCallback { url: v.get("url")?.as_str()?.to_owned() }
        }
        _ => return None,
    })
}
