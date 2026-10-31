//! Client of the import Web Worker (crates/web-import), which holds the parsers so the startup bundle does not.
//!
//! The worker starts on first use ([`warm`] when the Add sheet opens, or the first [`parse`]), loads from the hashed
//! URL tools/postbuild.py writes into `<meta name="import-worker">`, and is stopped once no job is waiting, so its
//! memory goes back after a large book. Jobs posted before it reports ready wait in a queue.

use std::cell::RefCell;
use std::collections::HashMap;

use futures::channel::oneshot;
use js_sys::{Array, Object, Reflect, Uint8Array};
use leptos::task::spawn_local;
use nolabs_web_import::{Fault, Job, Reply, field};
use wasm_bindgen::JsCast;
use wasm_bindgen::prelude::*;
use web_sys::{ErrorEvent, MessageEvent, Worker};

use crate::util::dom;

/// The `<meta>` that carries the worker's loader URL.
const META: &str = "meta[name=import-worker]";

struct Client {
    worker: Worker,
    ready: bool,
    /// Messages (with their transfer lists) posted before the worker was ready.
    queue: Vec<(Object, Array)>,
    pending: HashMap<u32, oneshot::Sender<Reply>>,
    next_id: u32,
    _on_message: Closure<dyn FnMut(MessageEvent)>,
    _on_error: Closure<dyn FnMut(ErrorEvent)>,
}

thread_local! {
    static CLIENT: RefCell<Option<Client>> = const { RefCell::new(None) };
}

fn get(v: &JsValue, key: &str) -> JsValue {
    Reflect::get(v, &JsValue::from_str(key)).unwrap_or(JsValue::UNDEFINED)
}

fn set(o: &Object, key: &str, v: &JsValue) {
    let _ = Reflect::set(o, &JsValue::from_str(key), v);
}

fn worker_url() -> Option<String> {
    let meta = dom::document()?.query_selector(META).ok().flatten()?;
    meta.get_attribute("content").filter(|u| !u.is_empty())
}

/// Stops the worker after the current event handler has returned (its closures are owned by the client), unless a
/// job arrived in the meantime. `force` stops it even then (after an error).
fn retire(force: bool) {
    spawn_local(async move {
        let client = CLIENT.with(|c| {
            let mut c = c.borrow_mut();
            let idle = c.as_ref().is_some_and(|cl| cl.pending.is_empty());
            if force || idle { c.take() } else { None }
        });
        if let Some(client) = client {
            client.worker.terminate();
            for (_, tx) in client.pending {
                let _ = tx.send(Err(Fault::Worker("the importer stopped".into())));
            }
        }
    });
}

fn on_message(e: MessageEvent) {
    let data = e.data();
    if get(&data, field::READY).is_truthy() {
        CLIENT.with(|c| {
            if let Some(cl) = c.borrow_mut().as_mut() {
                cl.ready = true;
                for (msg, transfer) in cl.queue.drain(..) {
                    let _ = cl.worker.post_message_with_transfer(&msg, &transfer);
                }
            }
        });
        return;
    }
    let Some(id) = get(&data, field::ID).as_f64().map(|n| n as u32) else { return };
    let reply = get(&data, field::REPLY).as_string().map_or_else(
        || Err(Fault::Worker("empty reply".into())),
        |json| serde_json::from_str::<Reply>(&json).unwrap_or_else(|e| Err(Fault::Worker(e.to_string()))),
    );
    let tx = CLIENT.with(|c| c.borrow_mut().as_mut().and_then(|cl| cl.pending.remove(&id)));
    if let Some(tx) = tx {
        let _ = tx.send(reply);
    }
    retire(false);
}

fn on_error(e: ErrorEvent) {
    e.prevent_default();
    let why = e.message();
    let pending = CLIENT.with(|c| c.borrow_mut().as_mut().map(|cl| std::mem::take(&mut cl.pending)));
    for (_, tx) in pending.unwrap_or_default() {
        let _ = tx.send(Err(Fault::Worker(why.clone())));
    }
    retire(true);
}

fn spawn() -> Result<Client, Fault> {
    let url = worker_url().ok_or_else(|| Fault::Worker("no import worker in this build".into()))?;
    let worker = Worker::new(&url).map_err(|e| Fault::Worker(format!("{e:?}")))?;
    let on_msg = Closure::<dyn FnMut(MessageEvent)>::new(on_message);
    let on_err = Closure::<dyn FnMut(ErrorEvent)>::new(on_error);
    worker.set_onmessage(Some(on_msg.as_ref().unchecked_ref()));
    worker.set_onerror(Some(on_err.as_ref().unchecked_ref()));
    Ok(Client {
        worker,
        ready: false,
        queue: Vec::new(),
        pending: HashMap::new(),
        next_id: 0,
        _on_message: on_msg,
        _on_error: on_err,
    })
}

fn with_client<T>(f: impl FnOnce(&mut Client) -> T) -> Result<T, Fault> {
    CLIENT.with(|c| {
        let mut c = c.borrow_mut();
        if c.is_none() {
            *c = Some(spawn()?);
        }
        c.as_mut().map(f).ok_or_else(|| Fault::Worker("no client".into()))
    })
}

/// Starts the worker ahead of a likely import, so its download overlaps with the person choosing what to add.
pub fn warm() {
    let _ = with_client(|_| ());
}

/// Parses `job` in the worker. `bytes` (an EPUB) is transferred, not copied.
pub async fn parse(job: Job, bytes: Option<Vec<u8>>) -> Reply {
    let json = serde_json::to_string(&job).map_err(|e| Fault::Worker(e.to_string()))?;
    let (tx, rx) = oneshot::channel();
    with_client(move |cl| {
        let id = cl.next_id;
        cl.next_id = cl.next_id.wrapping_add(1);
        cl.pending.insert(id, tx);
        let msg = Object::new();
        set(&msg, field::ID, &JsValue::from(id));
        set(&msg, field::JOB, &JsValue::from_str(&json));
        let transfer = Array::new();
        if let Some(bytes) = bytes {
            let arr = Uint8Array::from(bytes.as_slice());
            transfer.push(&arr.buffer());
            set(&msg, field::BYTES, &arr);
        }
        if cl.ready {
            let _ = cl.worker.post_message_with_transfer(&msg, &transfer);
        } else {
            cl.queue.push((msg, transfer));
        }
    })?;
    rx.await.unwrap_or_else(|_| Err(Fault::Worker("the importer stopped".into())))
}
