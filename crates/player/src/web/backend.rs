//! Where the current build's audio lives: one MediaSource, or (fallback) one blob URL per segment. Both expose the
//! same media timeline; for blobs it is virtual, the element's clock offset by the start of the segment it holds.

use web_sys::HtmlMediaElement;

use super::js::{self, MseSession};
use crate::timeline::Timeline;

pub(super) const MSE_MIME: &str = r#"audio/webm; codecs="opus""#;
pub(super) const BLOB_MIME: &str = "audio/webm";

#[derive(Debug, Default)]
pub(super) enum Backend {
    /// No document.
    #[default]
    Idle,
    /// Waiting for the MediaSource to open.
    Opening,
    Mse(MseSession),
    Blob(BlobQueue),
}

#[derive(Debug, Default)]
pub(super) struct BlobQueue {
    /// Object URL per timeline entry (same index); None once revoked.
    pub urls: Vec<Option<String>>,
    /// Timeline index of the entry the element holds (or is about to).
    pub current: usize,
    /// The element has been given `current`.
    pub loaded: bool,
    /// The element finished `current` before the next entry arrived.
    pub stalled: bool,
}

impl BlobQueue {
    /// Revokes URLs of entries before `current` that ended before `cutoff_s`.
    pub fn revoke_before(&mut self, timeline: &Timeline, cutoff_s: f64) {
        let current = self.current;
        for (url, entry) in self.urls.iter_mut().zip(timeline.entries()).take(current) {
            if entry.end_s < cutoff_s
                && let Some(url) = url.take()
            {
                js::revoke_url(&url);
            }
        }
    }
}

impl Backend {
    pub fn current_time(&self, audio: &HtmlMediaElement, timeline: &Timeline) -> f64 {
        match self {
            Backend::Blob(queue) => match timeline.entries().get(queue.current) {
                // Clamped: the element's real duration can exceed the reported one by a frame or two.
                Some(entry) if queue.loaded => entry.start_s + audio.current_time().clamp(0.0, entry.duration_s()),
                Some(entry) => entry.start_s,
                None => 0.0,
            },
            Backend::Mse(_) => audio.current_time(),
            Backend::Idle | Backend::Opening => 0.0,
        }
    }

    /// Buffered ranges on the media timeline, sorted.
    pub fn ranges(&self, timeline: &Timeline) -> Vec<(f64, f64)> {
        match self {
            Backend::Mse(session) => {
                session.buffered().as_chunks::<2>().0.iter().map(|&[start, end]| (start, end)).collect()
            }
            Backend::Blob(queue) => queue
                .urls
                .iter()
                .position(Option::is_some)
                .and_then(|first| timeline.entries().get(first))
                .map(|entry| vec![(entry.start_s, timeline.end_s())])
                .unwrap_or_default(),
            Backend::Idle | Backend::Opening => Vec::new(),
        }
    }

    pub fn session(&self) -> Option<&MseSession> {
        match self {
            Backend::Mse(session) => Some(session),
            _ => None,
        }
    }

    /// Releases this build's resources and leaves [`Backend::Idle`]. The element's source is replaced by the next
    /// build or detached by the caller.
    pub fn teardown(&mut self) {
        match std::mem::take(self) {
            Backend::Mse(session) => session.close(),
            Backend::Blob(queue) => queue.urls.into_iter().flatten().for_each(|url| js::revoke_url(&url)),
            Backend::Idle | Backend::Opening => {}
        }
    }
}
