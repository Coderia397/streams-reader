//! `public.documents` and `public.progress`: the library list, new documents, and listening positions.

use nolabs_core::proto::api::{DocumentRecord, ProgressRecord};

use super::{get, request_init, send, to_json, url};
use crate::api::ClientError;
use crate::api::client::encode_component;
use crate::model::{DocumentSummary, Progress, join};
use crate::util::dom;

/// Device label stored with progress rows, so another device can tell where a position came from.
const DEVICE: &str = "web";

/// Every document with its progress, newest first.
pub async fn list() -> Result<Vec<DocumentSummary>, ClientError> {
    let docs: Vec<DocumentRecord> = get("documents?select=*&order=updated_at.desc").await?;
    let progress: Vec<ProgressRecord> = get("progress?select=document_id,char_offset,updated_at,device").await?;
    Ok(join(docs, &progress))
}

/// One document and its progress.
pub async fn get_one(id: &str) -> Result<DocumentSummary, ClientError> {
    let id = encode_component(id);
    let docs: Vec<DocumentRecord> = get(&format!("documents?select=*&id=eq.{id}")).await?;
    let doc =
        docs.into_iter().next().ok_or_else(|| ClientError::Api(nolabs_core::proto::ApiError::not_found("document")))?;
    let progress: Vec<ProgressRecord> =
        get(&format!("progress?select=document_id,char_offset,updated_at,device&document_id=eq.{id}")).await?;
    Ok(join(vec![doc], &progress).remove(0))
}

/// Inserts the row for a new document (`ready = false`); the body follows with a PUT to the Worker.
pub async fn insert(record: &DocumentRecord) -> Result<DocumentSummary, ClientError> {
    let text = send("POST", "documents", Some(&to_json(record)?), Some("return=representation")).await?;
    let rows: Vec<DocumentRecord> = serde_json::from_str(&text).map_err(|e| ClientError::Decode(e.to_string()))?;
    let row = rows.into_iter().next().ok_or_else(|| ClientError::Decode("empty insert answer".into()))?;
    Ok(DocumentSummary::from_record(row, None))
}

fn progress_body(id: &str, p: Progress) -> Result<String, ClientError> {
    to_json(&p.to_record(id, Some(DEVICE.to_owned())))
}

const UPSERT: &str = "resolution=merge-duplicates,return=minimal";

/// Upserts the position; the database keeps whichever `updated_at` is newer, so a late write never moves it back.
pub async fn put_progress(id: &str, p: Progress) -> Result<(), ClientError> {
    send("POST", "progress", Some(&progress_body(id, p)?), Some(UPSERT)).await.map(|_| ())
}

/// The same upsert for a page being hidden: a `keepalive` fetch, which (unlike `sendBeacon`) can carry the bearer
/// token and still completes after the page is frozen.
pub fn put_progress_keepalive(id: &str, p: Progress) -> bool {
    let Some(window) = dom::window() else { return false };
    let Ok(body) = progress_body(id, p) else { return false };
    let Ok(init) = request_init("POST", Some(&body), Some(UPSERT)) else { return false };
    let _ = js_sys::Reflect::set(&init, &"keepalive".into(), &true.into());
    let _ = window.fetch_with_str_and_init(&url("progress"), &init);
    true
}
