//! In-memory fakes for the non-database ports, each cheaply cloneable so a test keeps a handle for inspection.

use std::cell::{Cell, RefCell};
use std::collections::{BTreeMap, HashMap};
use std::rc::Rc;

use async_trait::async_trait;

use crate::ports::{
    Blob, ByteStream, Clock, Http, HttpError, HttpRequest, HttpResponse, JobMessage, Method, PortError, Queue, Rng,
    StreamResponse,
};

// ----------------------------------------------------------------------------------------------------------------- blob

#[derive(Clone, Default)]
pub struct FakeBlob {
    pub objects: Rc<RefCell<BTreeMap<String, Vec<u8>>>>,
}

impl FakeBlob {
    pub fn insert(&self, key: &str, body: impl Into<Vec<u8>>) {
        self.objects.borrow_mut().insert(key.to_owned(), body.into());
    }

    pub fn keys(&self) -> Vec<String> {
        self.objects.borrow().keys().cloned().collect()
    }
}

#[async_trait(?Send)]
impl Blob for FakeBlob {
    async fn get(&self, key: &str) -> Result<Option<Vec<u8>>, PortError> {
        Ok(self.objects.borrow().get(key).cloned())
    }

    async fn put(&self, key: &str, body: Vec<u8>, _content_type: &str) -> Result<(), PortError> {
        self.insert(key, body);
        Ok(())
    }

    async fn head(&self, key: &str) -> Result<Option<u64>, PortError> {
        Ok(self.objects.borrow().get(key).map(|b| b.len() as u64))
    }

    async fn delete_many(&self, keys: &[String]) -> Result<(), PortError> {
        let mut objects = self.objects.borrow_mut();
        for k in keys {
            objects.remove(k);
        }
        Ok(())
    }

    async fn list_prefix(&self, prefix: &str) -> Result<Vec<String>, PortError> {
        Ok(self.objects.borrow().keys().filter(|k| k.starts_with(prefix)).cloned().collect())
    }
}

// ---------------------------------------------------------------------------------------------------------------- queue

#[derive(Clone, Default)]
pub struct FakeQueue {
    pub sent: Rc<RefCell<Vec<(JobMessage, u32)>>>,
}

impl FakeQueue {
    /// Every message sent so far, removed from the fake.
    pub fn take(&self) -> Vec<(JobMessage, u32)> {
        std::mem::take(&mut self.sent.borrow_mut())
    }
}

#[async_trait(?Send)]
impl Queue for FakeQueue {
    async fn send(&self, message: &JobMessage, delay_s: u32) -> Result<(), PortError> {
        self.sent.borrow_mut().push((message.clone(), delay_s));
        Ok(())
    }
}

// --------------------------------------------------------------------------------------------------------- clock / rng

#[derive(Clone)]
pub struct FakeClock(pub Rc<Cell<i64>>);

impl Clock for FakeClock {
    fn now_ms(&self) -> i64 {
        self.0.get()
    }
}

/// Deterministic, distinct bytes per call.
#[derive(Clone, Default)]
pub struct FakeRng(Rc<Cell<u64>>);

impl Rng for FakeRng {
    fn fill(&self, buf: &mut [u8]) -> Result<(), PortError> {
        let n = self.0.get() + 1;
        self.0.set(n);
        for (i, b) in buf.iter_mut().enumerate() {
            *b = n.to_le_bytes().get(i).copied().unwrap_or(0);
        }
        Ok(())
    }
}

// ----------------------------------------------------------------------------------------------------------------- http

type Handler = Box<dyn FnMut(&HttpRequest) -> Result<HttpResponse, HttpError>>;

/// Answers requests with a test-supplied handler and records them. Streamed bodies are cut into 5-byte chunks so the
/// SSE parser always sees split lines.
#[derive(Clone)]
pub struct FakeHttp {
    handler: Rc<RefCell<Handler>>,
    pub requests: Rc<RefCell<Vec<HttpRequest>>>,
}

impl Default for FakeHttp {
    fn default() -> Self {
        FakeHttp {
            handler: Rc::new(RefCell::new(Box::new(|_| Err(HttpError::Network("no handler".into()))))),
            requests: Rc::default(),
        }
    }
}

impl FakeHttp {
    pub fn set(&self, handler: impl FnMut(&HttpRequest) -> Result<HttpResponse, HttpError> + 'static) {
        *self.handler.borrow_mut() = Box::new(handler);
    }

    pub fn urls(&self) -> Vec<String> {
        self.requests.borrow().iter().map(|r| format!("{} {}", r.method.as_str(), r.url)).collect()
    }

    fn answer(&self, req: &HttpRequest) -> Result<HttpResponse, HttpError> {
        self.requests.borrow_mut().push(req.clone());
        (self.handler.borrow_mut())(req)
    }
}

pub fn response(status: u16, headers: &[(&str, &str)], body: impl Into<Vec<u8>>) -> HttpResponse {
    HttpResponse {
        status,
        headers: headers.iter().map(|(k, v)| (k.to_ascii_lowercase(), (*v).to_owned())).collect(),
        body: body.into(),
    }
}

struct Chunks(Vec<Vec<u8>>);

#[async_trait(?Send)]
impl ByteStream for Chunks {
    async fn next_chunk(&mut self) -> Result<Option<Vec<u8>>, HttpError> {
        Ok(if self.0.is_empty() { None } else { Some(self.0.remove(0)) })
    }
}

#[async_trait(?Send)]
impl Http for FakeHttp {
    async fn send(&self, req: HttpRequest) -> Result<HttpResponse, HttpError> {
        let res = self.answer(&req)?;
        if res.body.len() as u64 > req.max_body_bytes {
            return Err(HttpError::TooLarge);
        }
        Ok(res)
    }

    async fn open_stream(&self, req: HttpRequest) -> Result<StreamResponse, HttpError> {
        let res = self.answer(&req)?;
        let chunks = res.body.chunks(5).map(<[u8]>::to_vec).collect();
        Ok(StreamResponse { status: res.status, headers: res.headers, body: Box::new(Chunks(chunks)) })
    }
}

// --------------------------------------------------------------------------------------------------------------- gradio

/// What a fake Space endpoint does with one call.
pub enum Reply {
    Ok(String),
    /// An SSE `error` event with this message.
    Error(String),
    /// The submit POST fails with this status and body.
    Http(u16, String),
}

/// Install a fake Gradio Space on `http`: `endpoint(api, input_json)` decides each call's outcome. Calls to other
/// hosts get `other(req)`.
pub fn fake_space(
    http: &FakeHttp,
    mut endpoint: impl FnMut(&str, &str) -> Reply + 'static,
    mut other: impl FnMut(&HttpRequest) -> Result<HttpResponse, HttpError> + 'static,
) {
    let pending: Rc<RefCell<HashMap<String, Reply>>> = Rc::default();
    let counter = Rc::new(Cell::new(0u32));
    http.set(move |req| {
        let Some(path) = req.url.strip_prefix("https://space.test/gradio_api/call/") else {
            return other(req);
        };
        match req.method {
            Method::Post => {
                let body: serde_json::Value = serde_json::from_slice(req.body.as_deref().unwrap_or_default()).unwrap();
                let input = body["data"][0].as_str().unwrap().to_owned();
                match endpoint(path, &input) {
                    Reply::Http(status, text) => Ok(response(status, &[], text)),
                    reply => {
                        counter.set(counter.get() + 1);
                        let id = format!("ev{}", counter.get());
                        pending.borrow_mut().insert(id.clone(), reply);
                        Ok(response(200, &[], format!("{{\"event_id\":\"{id}\"}}")))
                    }
                }
            }
            Method::Get => {
                let id = path.rsplit('/').next().unwrap_or_default();
                let sse = match pending.borrow_mut().remove(id) {
                    Some(Reply::Ok(out)) => format!(
                        "event: heartbeat\ndata: null\n\nevent: complete\ndata: {}\n\n",
                        serde_json::to_string(&vec![out]).unwrap()
                    ),
                    Some(Reply::Error(msg)) => {
                        format!("event: error\ndata: {}\n\n", serde_json::to_string(&msg).unwrap())
                    }
                    _ => return Ok(response(404, &[], "unknown event")),
                };
                Ok(response(200, &[("content-type", "text/event-stream")], sse))
            }
        }
    });
}
