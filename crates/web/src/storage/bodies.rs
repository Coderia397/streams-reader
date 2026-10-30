//! Document bodies cached on the phone, so an opened book reads (and replays cached audio) offline.

use nolabs_core::proto::api::DocumentBody;

use super::idb;

pub async fn save(doc_id: &str, body: &DocumentBody) {
    let _ = idb::put(idb::BODIES, doc_id, body).await;
}

pub async fn load(doc_id: &str) -> Option<DocumentBody> {
    idb::get(idb::BODIES, doc_id).await.ok().flatten()
}

pub async fn remove(doc_id: &str) {
    let _ = idb::delete(idb::BODIES, doc_id).await;
}
