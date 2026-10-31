//! `public.bookmarks`: saved places in a document.

use serde::{Deserialize, Serialize};

use super::{get, send, to_json};
use crate::api::ClientError;
use crate::api::client::encode_component;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Bookmark {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    pub document_id: String,
    pub char_offset: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub created_at: Option<String>,
}

pub async fn list(document_id: &str) -> Result<Vec<Bookmark>, ClientError> {
    let id = encode_component(document_id);
    get(&format!("bookmarks?select=*&document_id=eq.{id}&order=char_offset.asc")).await
}

pub async fn add(document_id: &str, char_offset: u64) -> Result<Bookmark, ClientError> {
    let b = Bookmark { id: None, document_id: document_id.to_owned(), char_offset, note: None, created_at: None };
    let text = send("POST", "bookmarks", Some(&to_json(&b)?), Some("return=representation")).await?;
    let rows: Vec<Bookmark> = serde_json::from_str(&text).map_err(|e| ClientError::Decode(e.to_string()))?;
    rows.into_iter().next().ok_or_else(|| ClientError::Decode("empty insert answer".into()))
}

pub async fn remove(id: &str) -> Result<(), ClientError> {
    send("DELETE", &format!("bookmarks?id=eq.{}", encode_component(id)), None, None).await.map(|_| ())
}
