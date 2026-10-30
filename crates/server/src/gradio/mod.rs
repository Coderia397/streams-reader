//! A minimal client for Gradio's HTTP API (Gradio 5/6): `POST /gradio_api/call/<api>` with `{"data": [...]}`
//! returns an `event_id`; `GET /gradio_api/call/<api>/<event_id>` streams server-sent events `generating`,
//! `heartbeat`, then `complete` (data: the outputs array) or `error` (data: the message as a JSON string, or null).
//! Every Space endpoint takes one JSON string and returns one JSON string.

pub mod sse;

use nolabs_core::engine_error::EngineError;
use serde::Deserialize;

use crate::ports::{Http, HttpError, HttpRequest, Method};
use sse::SseParser;

const SUBMIT_TIMEOUT_MS: u32 = 30_000;
/// Time until the stream's headers arrive; the stream itself runs as long as the render does.
const STREAM_OPEN_TIMEOUT_MS: u32 = 60_000;

pub struct GradioClient<'a> {
    http: &'a dyn Http,
    base_url: &'a str,
    token: &'a str,
}

#[derive(Deserialize)]
struct Submitted {
    event_id: String,
}

fn transport(e: &HttpError) -> EngineError {
    EngineError::classify(None, &format!("connection to the speech engine failed: {e}"))
}

impl<'a> GradioClient<'a> {
    pub fn new(http: &'a dyn Http, base_url: &'a str, token: &'a str) -> Self {
        GradioClient { http, base_url, token }
    }

    /// Call `api` with one JSON string argument and return its one JSON string output.
    pub async fn call(&self, api: &str, input: &str) -> Result<String, EngineError> {
        let event_id = self.submit(api, input).await?;
        self.await_result(api, &event_id).await
    }

    async fn submit(&self, api: &str, input: &str) -> Result<String, EngineError> {
        let body = serde_json::to_vec(&serde_json::json!({ "data": [input] }))
            .map_err(|e| EngineError::Internal(format!("encode request: {e}")))?;
        let req = HttpRequest::new(Method::Post, format!("{}/gradio_api/call/{api}", self.base_url))
            .header("authorization", format!("Bearer {}", self.token))
            .json_body(body)
            .timeout_ms(SUBMIT_TIMEOUT_MS)
            .max_body_bytes(256 * 1024);
        let res = self.http.send(req).await.map_err(|e| transport(&e))?;
        let text = String::from_utf8_lossy(&res.body);
        if !(200..300).contains(&res.status) {
            return Err(EngineError::classify(Some(res.status), &text));
        }
        match serde_json::from_slice::<Submitted>(&res.body) {
            Ok(s) if !s.event_id.is_empty() => Ok(s.event_id),
            // A 200 with HTML (the Space's loading page) instead of JSON.
            _ => Err(EngineError::classify(Some(res.status), &text)),
        }
    }

    async fn await_result(&self, api: &str, event_id: &str) -> Result<String, EngineError> {
        let req = HttpRequest::new(Method::Get, format!("{}/gradio_api/call/{api}/{event_id}", self.base_url))
            .header("authorization", format!("Bearer {}", self.token))
            .header("accept", "text/event-stream")
            .timeout_ms(STREAM_OPEN_TIMEOUT_MS);
        let mut res = self.http.open_stream(req).await.map_err(|e| transport(&e))?;
        if res.status != 200 {
            return Err(EngineError::classify(Some(res.status), ""));
        }
        let mut parser = SseParser::new();
        loop {
            let chunk = res.body.next_chunk().await.map_err(|e| transport(&e))?;
            let events = match &chunk {
                Some(bytes) => parser.push(bytes).map_err(|e| EngineError::Internal(e.to_string()))?,
                None => parser.finish().into_iter().collect(),
            };
            for event in events {
                match event.event.as_str() {
                    "complete" => return first_output(&event.data),
                    "error" => return Err(EngineError::classify(None, &error_message(&event.data))),
                    _ => {}
                }
            }
            if chunk.is_none() {
                return Err(EngineError::Internal("speech engine closed the stream without a result".into()));
            }
        }
    }
}

/// `complete` data is the outputs array; our endpoints have exactly one string output.
fn first_output(data: &str) -> Result<String, EngineError> {
    match serde_json::from_str::<Vec<serde_json::Value>>(data) {
        Ok(outputs) => match outputs.into_iter().next() {
            Some(serde_json::Value::String(s)) => Ok(s),
            _ => Err(EngineError::Internal("speech engine returned no string output".into())),
        },
        Err(e) => Err(EngineError::Internal(format!("malformed result from the speech engine: {e}"))),
    }
}

/// `error` data is a JSON string (with `show_error=True`) or `null`; fall back to the raw text.
fn error_message(data: &str) -> String {
    match serde_json::from_str::<serde_json::Value>(data) {
        Ok(serde_json::Value::String(s)) => s,
        Ok(serde_json::Value::Null) => "speech engine error (no message)".to_owned(),
        _ => data.to_owned(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn outputs_and_error_messages() {
        assert_eq!(first_output(r#"["{\"a\":1}"]"#).unwrap(), r#"{"a":1}"#);
        assert!(first_output("[]").is_err());
        assert!(first_output("[1]").is_err());
        assert!(first_output("nope").is_err());
        assert_eq!(error_message(r#""bad request: x""#), "bad request: x");
        assert_eq!(error_message("null"), "speech engine error (no message)");
        assert_eq!(error_message("plain"), "plain");
    }
}
