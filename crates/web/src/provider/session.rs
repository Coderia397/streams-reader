//! The `nolabs_player::Provider` for one (document, voice). The player asks for segment `i` when it needs it; the
//! session asks the Worker to render the window of [`RENDER_WINDOW`] items around it, polls queued items with
//! backoff, then returns the clip from the offline cache or the network. Ready slots are saved to IndexedDB so a
//! document that was listened to before replays offline without asking the Worker for keys.

use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::rc::Rc;

use futures::FutureExt;
use futures::future::{LocalBoxFuture, Shared};
use leptos::task::spawn_local;
use nolabs_core::proto::api::{Boundary, Priority, RenderItem, RenderRequest, RenderState, WordTiming};
use nolabs_core::text::{SEGMENTER_VERSION, Segment};
use nolabs_player::{PlayerError, Provider, Track};
use serde::{Deserialize, Serialize};

use super::table::{ReadyEntry, RenderTable, Slot, window_of, window_range};
use crate::api::{ClientError, render as render_api};
use crate::config::RENDER_WINDOW;
use crate::storage::{audio_cache, idb};
use crate::util::backoff::Backoff;
use crate::util::hash::fnv1a;
use crate::util::timer::{now_ms, sleep_ms};

/// Status polls for the same window closer together than this reuse the last answer.
const MIN_POLL_GAP_MS: i64 = 800;
/// Network failures tolerated while waiting for a queued item before giving up.
const MAX_POLL_FAILURES: u32 = 4;

type WindowFuture = Shared<LocalBoxFuture<'static, Result<(), ClientError>>>;

#[derive(Serialize, Deserialize)]
struct Persisted {
    /// Hash of the segment texts: stored slots are reused only for the same segmentation.
    fingerprint: u32,
    entries: Vec<ReadyEntry>,
}

pub struct RenderSession {
    store_key: String,
    fingerprint: u32,
    voice: String,
    items: Vec<RenderItem>,
    table: RefCell<RenderTable>,
    inflight: RefCell<HashMap<u32, WindowFuture>>,
    last_poll: RefCell<HashMap<u32, i64>>,
    alive: Cell<bool>,
}

/// The items sent to `/api/render`: segment text, boundary, neutral emotion (the director is phase 2).
pub fn render_items(segments: &[Segment]) -> Vec<RenderItem> {
    segments
        .iter()
        .map(|s| RenderItem { text: s.text.clone(), boundary: s.boundary, emotion: Default::default() })
        .collect()
}

fn fingerprint(items: &[RenderItem]) -> u32 {
    let mut joined = format!("v{SEGMENTER_VERSION}");
    for it in items {
        joined.push('\u{1f}');
        joined.push_str(&it.text);
    }
    fnv1a(&joined)
}

fn load_error(e: &ClientError) -> PlayerError {
    PlayerError::Load(e.friendly())
}

impl RenderSession {
    /// Creates the session and restores ready slots saved by an earlier session.
    pub async fn open(doc_id: &str, voice: &str, segments: &[Segment]) -> Rc<RenderSession> {
        let items = render_items(segments);
        let fingerprint = fingerprint(&items);
        let store_key = format!("{doc_id}|{voice}");
        let mut table = RenderTable::new(items.len() as u32);
        if let Ok(Some(saved)) = idb::get::<Persisted>(idb::RENDERS, &store_key).await
            && saved.fingerprint == fingerprint
        {
            table.restore(saved.entries);
        }
        Rc::new(RenderSession {
            store_key,
            fingerprint,
            voice: voice.to_owned(),
            items,
            table: RefCell::new(table),
            inflight: RefCell::new(HashMap::new()),
            last_poll: RefCell::new(HashMap::new()),
            alive: Cell::new(true),
        })
    }

    pub fn provider(self: &Rc<Self>) -> Provider {
        let session = Rc::clone(self);
        Rc::new(move |segment: u32| {
            let session = Rc::clone(&session);
            async move { session.track(segment).await }.boxed_local()
        })
    }

    /// Word timings of a ready segment (empty when unknown).
    pub fn words(&self, segment: u32) -> Vec<WordTiming> {
        self.table.borrow().words(segment)
    }

    /// Stops background work; pending provider calls end with an error the player ignores after `unload`.
    pub fn close(&self) {
        self.alive.set(false);
    }

    fn count(&self) -> u32 {
        self.items.len() as u32
    }

    async fn persist(&self) {
        let saved = Persisted { fingerprint: self.fingerprint, entries: self.table.borrow().ready_entries() };
        let _ = idb::put(idb::RENDERS, &self.store_key, &saved).await;
    }

    async fn request_window(self: Rc<Self>, w: u32) -> Result<(), ClientError> {
        let range = window_range(w, self.count());
        let items = self.items[range.start as usize..range.end as usize].to_vec();
        if items.is_empty() {
            return Ok(());
        }
        let req = RenderRequest { voice: self.voice.clone(), items, priority: Priority::Interactive };
        let resp = render_api::render(&req).await?;
        let ready = self.table.borrow_mut().apply_render(range.start, &resp);
        if ready {
            self.persist().await;
        }
        Ok(())
    }

    /// Requests window `w` unless every slot in it is already known; concurrent callers share one request.
    async fn ensure_window(self: &Rc<Self>, w: u32) -> Result<(), ClientError> {
        if !self.table.borrow().needs_request(window_range(w, self.count())) {
            return Ok(());
        }
        let existing = self.inflight.borrow().get(&w).cloned();
        let fut = match existing {
            Some(f) => f,
            None => {
                let me = Rc::clone(self);
                let f = async move {
                    let result = Rc::clone(&me).request_window(w).await;
                    me.inflight.borrow_mut().remove(&w);
                    result
                }
                .boxed_local()
                .shared();
                self.inflight.borrow_mut().insert(w, f.clone());
                f
            }
        };
        fut.await
    }

    async fn poll(&self, w: u32) -> Result<(), ClientError> {
        let now = now_ms();
        if self.last_poll.borrow().get(&w).is_some_and(|&t| now - t < MIN_POLL_GAP_MS) {
            return Ok(());
        }
        self.last_poll.borrow_mut().insert(w, now);
        let keys = self.table.borrow().pending_keys(window_range(w, self.count()));
        if keys.is_empty() {
            return Ok(());
        }
        let resp = render_api::status(&keys).await?;
        let ready = self.table.borrow_mut().apply_status(&resp);
        if ready {
            self.persist().await;
        }
        Ok(())
    }

    fn prefetch_next(self: &Rc<Self>, segment: u32) {
        let w = window_of(segment);
        let next_start = (w + 1) * RENDER_WINDOW;
        if segment % RENDER_WINDOW >= RENDER_WINDOW / 2 && next_start < self.count() {
            let me = Rc::clone(self);
            spawn_local(async move {
                let _ = me.ensure_window(w + 1).await;
            });
        }
    }

    async fn track(self: Rc<Self>, segment: u32) -> Result<Track, PlayerError> {
        if segment >= self.count() {
            return Err(PlayerError::Other("past the end".into()));
        }
        let w = window_of(segment);
        let mut backoff = Backoff::default();
        let mut poll_failures = 0u32;
        let mut refetched = false;
        loop {
            if !self.alive.get() {
                return Err(PlayerError::Other("closed".into()));
            }
            if let Err(e) = self.ensure_window(w).await
                && !self.table.borrow().is_ready(segment)
            {
                return Err(load_error(&e));
            }
            self.prefetch_next(segment);
            let slot = self.table.borrow().slot(segment);
            match slot {
                Slot::Ready { key, audio } => match audio_cache::fetch(&key).await {
                    Ok(bytes) => {
                        let ends_section = self.items[segment as usize].boundary == Boundary::Section;
                        return Ok(Track {
                            segment,
                            bytes,
                            duration_ms: audio.duration_ms,
                            words: audio.words,
                            ends_section,
                        });
                    }
                    Err(e) if e.is_not_found() && !refetched => {
                        refetched = true;
                        self.table.borrow_mut().reset(segment);
                    }
                    Err(e) => return Err(load_error(&e)),
                },
                Slot::Failed { error } => {
                    let msg = error.map_or_else(
                        || "The voice engine couldn't read this part.".to_owned(),
                        |e| ClientError::Api(e).friendly(),
                    );
                    return Err(PlayerError::Load(msg));
                }
                Slot::Pending { state: RenderState::Deferred, retry_after_s, .. } => {
                    let mut e = nolabs_core::proto::ApiError::new(nolabs_core::proto::ErrorCode::GpuBudget, "deferred");
                    e.retry_after_s = retry_after_s;
                    return Err(load_error(&ClientError::Api(e)));
                }
                Slot::Pending { retry_after_s, .. } => {
                    sleep_ms(backoff.next_ms_with_hint(retry_after_s)).await;
                    if let Err(e) = self.poll(w).await {
                        poll_failures += 1;
                        if poll_failures >= MAX_POLL_FAILURES {
                            return Err(load_error(&e));
                        }
                    }
                }
                Slot::Unknown => {
                    // The answer did not cover this item; ask again once after a pause.
                    sleep_ms(backoff.next_ms()).await;
                    poll_failures += 1;
                    if poll_failures >= MAX_POLL_FAILURES {
                        return Err(PlayerError::Load("The server didn't return audio for this part.".into()));
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn seg(text: &str, boundary: Boundary) -> Segment {
        Segment { idx: 0, block: 0, text: text.into(), boundary, char_start: 0, char_end: 0, heading: false }
    }

    #[test]
    fn items_carry_boundary_and_neutral_emotion() {
        let items = render_items(&[seg("A.", Boundary::Sentence), seg("B.", Boundary::Section)]);
        assert_eq!(items[1].boundary, Boundary::Section);
        assert_eq!(items[0].emotion, Default::default());
    }

    #[test]
    fn fingerprint_changes_with_text() {
        let a = render_items(&[seg("A.", Boundary::Sentence)]);
        let b = render_items(&[seg("B.", Boundary::Sentence)]);
        assert_ne!(fingerprint(&a), fingerprint(&b));
        assert_eq!(fingerprint(&a), fingerprint(&a.clone()));
    }
}
