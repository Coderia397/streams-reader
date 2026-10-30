//! Listening position kept on the phone. Written every few seconds while playing (cheap, survives a killed tab) and
//! synced to the Worker less often; whichever copy is newer wins when a document opens.

use nolabs_core::proto::api::Progress;
use serde::{Deserialize, Serialize};

use super::idb;
use crate::api::ClientError;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LocalProgress {
    pub doc_id: String,
    pub char_offset: u64,
    pub updated_at_ms: i64,
    /// False until the Worker has acknowledged this position.
    pub synced: bool,
}

impl LocalProgress {
    pub fn progress(&self) -> Progress {
        Progress { char_offset: self.char_offset, updated_at_ms: self.updated_at_ms }
    }
}

pub async fn save(p: &LocalProgress) -> Result<(), ClientError> {
    idb::put(idb::PROGRESS, &p.doc_id, p).await
}

pub async fn load(doc_id: &str) -> Option<LocalProgress> {
    idb::get(idb::PROGRESS, doc_id).await.ok().flatten()
}

pub async fn all() -> Vec<LocalProgress> {
    idb::all(idb::PROGRESS).await.unwrap_or_default()
}

pub async fn remove(doc_id: &str) {
    let _ = idb::delete(idb::PROGRESS, doc_id).await;
}

/// The newer of the local and server positions (local wins a tie: it is at least as fresh).
pub fn newest(local: Option<&LocalProgress>, server: Option<Progress>) -> Option<Progress> {
    match (local.map(LocalProgress::progress), server) {
        (Some(l), Some(s)) => Some(if s.updated_at_ms > l.updated_at_ms { s } else { l }),
        (l, s) => l.or(s),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn local(offset: u64, at: i64) -> LocalProgress {
        LocalProgress { doc_id: "d".into(), char_offset: offset, updated_at_ms: at, synced: false }
    }

    #[test]
    fn newest_prefers_the_later_position() {
        let server = Progress { char_offset: 50, updated_at_ms: 200 };
        assert_eq!(newest(Some(&local(10, 100)), Some(server)), Some(server));
        assert_eq!(newest(Some(&local(10, 300)), Some(server)).map(|p| p.char_offset), Some(10));
        assert_eq!(newest(Some(&local(10, 200)), Some(server)).map(|p| p.char_offset), Some(10));
        assert_eq!(newest(None, Some(server)), Some(server));
        assert_eq!(newest(Some(&local(7, 1)), None).map(|p| p.char_offset), Some(7));
        assert_eq!(newest(None, None), None);
    }
}
