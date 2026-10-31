//! Document bodies on the Worker (R2): upload after the Supabase row exists, download, delete. The row itself
//! (title, counts, `ready`) is read and written in Supabase (`crate::db::library`).

use nolabs_core::proto::api::DocumentBody;

use super::ClientError;
use super::client::{Body, encode_segment, read_json, send, send_no_content};

fn doc_path(id: &str, rest: &str) -> String {
    format!("/api/documents/{}{rest}", encode_segment(id))
}

/// Uploads exactly the bytes announced in the row's `body_bytes`; the Worker then sets `ready`.
pub async fn put_body(id: &str, body_json: &[u8]) -> Result<(), ClientError> {
    let data = js_sys::Uint8Array::from(body_json);
    send_no_content("PUT", &doc_path(id, "/body"), Body::Bytes { data, content_type: "application/json" }).await
}

pub async fn get_body(id: &str) -> Result<DocumentBody, ClientError> {
    let resp = send("GET", &doc_path(id, "/body"), Body::Empty).await?;
    read_json(&resp).await
}

/// Deletes the row, its progress and bookmarks, and the body.
pub async fn delete(id: &str) -> Result<(), ClientError> {
    send_no_content("DELETE", &doc_path(id, ""), Body::Empty).await
}
