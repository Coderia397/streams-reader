//! [`Http`] over the Workers `fetch`. Redirects are never followed (`redirect: "manual"`), nothing but the given
//! headers is sent, and every call is raced against a timer that aborts the request.

use std::future::Future;
use std::time::Duration;

use async_trait::async_trait;
use futures_util::StreamExt;
use futures_util::future::{Either, select};
use nolabs_server::ports::{
    ByteStream, Http, HttpError, HttpRequest, HttpResponse, Method as PortMethod, StreamResponse,
};
use worker::js_sys::Uint8Array;
use worker::{AbortController, Delay, Fetch, Headers, Method, Request, RequestInit, RequestRedirect, Response};

pub struct FetchHttp;

fn network(e: worker::Error) -> HttpError {
    HttpError::Network(e.to_string())
}

/// Run `fut`, aborting the request through `controller` when `ms` passes first.
async fn deadline<T>(
    ms: u32,
    controller: AbortController,
    fut: impl Future<Output = Result<T, HttpError>>,
) -> Result<T, HttpError> {
    let fut = std::pin::pin!(fut);
    let timer = Delay::from(Duration::from_millis(u64::from(ms)));
    match select(fut, timer).await {
        Either::Left((result, _)) => result,
        Either::Right(_) => {
            controller.abort();
            Err(HttpError::Timeout)
        }
    }
}

fn build(req: &HttpRequest) -> Result<Request, HttpError> {
    let headers = Headers::new();
    for (name, value) in &req.headers {
        headers.set(name, value).map_err(network)?;
    }
    let mut init = RequestInit::new();
    init.with_method(match req.method {
        PortMethod::Get => Method::Get,
        PortMethod::Post => Method::Post,
        PortMethod::Patch => Method::Patch,
        PortMethod::Delete => Method::Delete,
    })
    .with_redirect(RequestRedirect::Manual)
    .with_headers(headers);
    if let Some(body) = &req.body {
        init.with_body(Some(Uint8Array::from(body.as_slice()).into()));
    }
    Request::new_with_init(&req.url, &init).map_err(network)
}

fn header_list(resp: &Response) -> Vec<(String, String)> {
    resp.headers().entries().map(|(k, v)| (k.to_ascii_lowercase(), v)).collect()
}

/// Read the whole body, failing as soon as it grows past `max` bytes.
async fn read_capped(resp: &mut Response, max: u64) -> Result<Vec<u8>, HttpError> {
    let declared = resp.headers().get("content-length").ok().flatten().and_then(|v| v.parse::<u64>().ok());
    if declared.is_some_and(|n| n > max) {
        return Err(HttpError::TooLarge);
    }
    // Redirects and 204s have no body stream.
    let Ok(mut stream) = resp.stream() else {
        return Ok(Vec::new());
    };
    let mut body = Vec::new();
    while let Some(chunk) = stream.next().await {
        let chunk = chunk.map_err(network)?;
        if (body.len() + chunk.len()) as u64 > max {
            return Err(HttpError::TooLarge);
        }
        body.extend_from_slice(&chunk);
    }
    Ok(body)
}

struct WorkerStream(Option<worker::ByteStream>);

#[async_trait(?Send)]
impl ByteStream for WorkerStream {
    async fn next_chunk(&mut self) -> Result<Option<Vec<u8>>, HttpError> {
        let Some(stream) = self.0.as_mut() else {
            return Ok(None);
        };
        match stream.next().await {
            Some(Ok(chunk)) => Ok(Some(chunk)),
            Some(Err(e)) => Err(network(e)),
            None => Ok(None),
        }
    }
}

#[async_trait(?Send)]
impl Http for FetchHttp {
    async fn send(&self, req: HttpRequest) -> Result<HttpResponse, HttpError> {
        let request = build(&req)?;
        let controller = AbortController::default();
        let signal = controller.signal();
        let max = req.max_body_bytes;
        deadline(req.timeout_ms, controller, async move {
            let mut resp = Fetch::Request(request).send_with_signal(&signal).await.map_err(network)?;
            let headers = header_list(&resp);
            let body = read_capped(&mut resp, max).await?;
            Ok(HttpResponse { status: resp.status_code(), headers, body })
        })
        .await
    }

    async fn open_stream(&self, req: HttpRequest) -> Result<StreamResponse, HttpError> {
        let request = build(&req)?;
        let controller = AbortController::default();
        let signal = controller.signal();
        let mut resp = deadline(req.timeout_ms, controller, async move {
            Fetch::Request(request).send_with_signal(&signal).await.map_err(network)
        })
        .await?;
        let headers = header_list(&resp);
        let body = WorkerStream(resp.stream().ok());
        Ok(StreamResponse { status: resp.status_code(), headers, body: Box::new(body) })
    }
}
