//! `/api/library` and `/api/documents`: list, create + upload body, read, delete, progress.

use nolabs_core::proto::api::{CreateDocument, DocumentBody, DocumentSummary, Library, Progress};

use super::ClientError;
use super::client::{Body, encode_segment, get_json, read_json, send, send_json, send_no_content};
use crate::util::dom;

fn doc_path(id: &str, rest: &str) -> String {
    format!("/api/documents/{}{rest}", encode_segment(id))
}

pub async fn library() -> Result<Library, ClientError> {
    get_json("/api/library").await
}

pub async fn create(req: &CreateDocument) -> Result<DocumentSummary, ClientError> {
    send_json("POST", "/api/documents", req).await
}

/// Uploads the exact bytes announced in [`CreateDocument::body_bytes`].
pub async fn put_body(id: &str, body_json: &[u8]) -> Result<(), ClientError> {
    let data = js_sys::Uint8Array::from(body_json);
    send_no_content("PUT", &doc_path(id, "/body"), Body::Bytes { data, content_type: "application/json" }).await
}

pub async fn get(id: &str) -> Result<DocumentSummary, ClientError> {
    get_json(&doc_path(id, "")).await
}

pub async fn get_body(id: &str) -> Result<DocumentBody, ClientError> {
    let resp = send("GET", &doc_path(id, "/body"), Body::Empty).await?;
    read_json(&resp).await
}

pub async fn delete(id: &str) -> Result<(), ClientError> {
    send_no_content("DELETE", &doc_path(id, ""), Body::Empty).await
}

pub async fn put_progress(id: &str, progress: Progress) -> Result<(), ClientError> {
    send_no_content("PUT", &doc_path(id, "/progress"), Body::json(&progress)?).await
}

/// Fire-and-forget progress for a page that is being hidden. `sendBeacon` cannot set headers, so the API version
/// travels as `?api=1` (see the web crate's report: the Worker accepts it on this route).
pub fn beacon_progress(id: &str, progress: Progress) -> bool {
    let Some(window) = dom::window() else { return false };
    let Ok(json) = serde_json::to_string(&progress) else { return false };
    let parts = js_sys::Array::of1(&wasm_bindgen::JsValue::from_str(&json));
    let opts = web_sys::BlobPropertyBag::new();
    opts.set_type("application/json");
    let Ok(blob) = web_sys::Blob::new_with_str_sequence_and_options(&parts, &opts) else { return false };
    let url = format!("{}?api=1", doc_path(id, "/progress"));
    window.navigator().send_beacon_with_opt_blob(&url, Some(&blob)).unwrap_or(false)
}
