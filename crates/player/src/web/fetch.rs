//! The fetch loop of one build: asks the provider for segments in order and appends them until three minutes are
//! buffered ahead, then idles until playback consumes some. Every await is followed by a generation check, so a loop
//! whose build was replaced stops at its next step and drops what it fetched.

use std::rc::{Rc, Weak};

use wasm_bindgen::JsCast;
use wasm_bindgen_futures::JsFuture;

use super::backend::{BLOB_MIME, Backend, MSE_MIME};
use super::js::{self, MseSession};
use super::{Outbox, Shared, State};
use crate::buffer::{self, Eviction, FetchStep, KEEP_BEHIND_S};
use crate::status::Input;
use crate::{PlayerError, PlayerEvent, PlayerState, Track};

/// How often a loop with a full buffer checks again.
const WAIT_POLL_MS: u32 = 1_000;
/// Wait before retrying an append that hit the quota with nothing old enough to evict.
const QUOTA_RETRY_MS: u32 = 2_000;

/// Opens a MediaSource on the element, then runs the fetch loop on it.
pub(super) async fn open_and_run(weak: Weak<Shared>, generation: u64, first: u32) {
    let Some(promise) = weak.upgrade().map(|shared| MseSession::open(&shared.audio, MSE_MIME)) else {
        return;
    };
    let opened = JsFuture::from(promise).await;
    let Some(shared) = weak.upgrade() else {
        return;
    };
    let session = match opened {
        Ok(value) => value.unchecked_into::<MseSession>(),
        Err(err) => {
            shared.fail(generation, format!("could not open the audio stream: {}", js::error_message(&err)));
            return;
        }
    };
    let current = shared.update(|st, _| {
        let current = st.generation == generation;
        if current {
            st.backend = Backend::Mse(session.clone());
        }
        current
    });
    drop(shared);
    if current {
        run(weak, generation, first).await;
    } else {
        session.close();
    }
}

pub(super) async fn run(weak: Weak<Shared>, generation: u64, first: u32) {
    let mut next = first;
    let mut attempt = 0u32;
    loop {
        let Some(shared) = weak.upgrade() else {
            return;
        };
        let Some(step) = shared.fetch_step(generation, next) else {
            return;
        };
        match step {
            FetchStep::Finish => {
                shared.finish(generation);
                return;
            }
            FetchStep::Wait => {
                drop(shared);
                js::delay(WAIT_POLL_MS).await;
            }
            FetchStep::Fetch(segment) => {
                let request = (shared.provider)(segment);
                drop(shared);
                let result = request.await;
                let Some(shared) = weak.upgrade() else {
                    return;
                };
                if !shared.is_current(generation) {
                    return;
                }
                match result {
                    Ok(track) => {
                        drop(shared);
                        attempt = 0;
                        if !append(&weak, generation, segment, track).await {
                            return;
                        }
                        next = segment.saturating_add(1);
                    }
                    Err(err) => {
                        // Load errors are transient (render still running, network gap); the rest are final.
                        let retry = matches!(err, PlayerError::Load(_));
                        shared.fetch_failed(generation, format!("segment {segment}: {err}"), attempt == 0);
                        drop(shared);
                        if !retry {
                            return;
                        }
                        let wait = buffer::retry_delay_ms(attempt);
                        attempt = attempt.saturating_add(1);
                        js::delay(wait).await;
                    }
                }
            }
        }
    }
}

/// Appends one track. False when the loop should stop (build replaced or a fatal error).
async fn append(weak: &Weak<Shared>, generation: u64, segment: u32, track: Track) -> bool {
    loop {
        let Some(shared) = weak.upgrade() else {
            return false;
        };
        if !shared.is_current(generation) {
            return false;
        }
        let session = shared.st.borrow().backend.session().cloned();
        let Some(session) = session else {
            return shared.append_blob(generation, segment, &track);
        };
        drop(shared);
        let outcome = JsFuture::from(session.append(&track.bytes)).await;
        let Some(shared) = weak.upgrade() else {
            return false;
        };
        if !shared.is_current(generation) {
            return false;
        }
        match outcome {
            Ok(end) => {
                shared.appended(generation, segment, end.as_f64(), &track);
                return true;
            }
            Err(err) if js::error_name(&err) == "QuotaExceededError" => {
                let eviction = {
                    let st = shared.st.borrow();
                    buffer::eviction(&st.backend.ranges(&st.timeline), shared.media_time(&st))
                };
                drop(shared);
                let removed = match eviction {
                    Eviction::Remove { start_s, end_s } => JsFuture::from(session.remove(start_s, end_s)).await.is_ok(),
                    Eviction::WaitForPlayback => false,
                };
                if !removed {
                    js::delay(QUOTA_RETRY_MS).await;
                }
            }
            Err(err) => {
                shared.fail(generation, format!("could not play segment {segment}: {}", js::error_message(&err)));
                return false;
            }
        }
    }
}

impl Shared {
    /// Next step for the loop of `generation`; None when that build was replaced.
    fn fetch_step(&self, generation: u64, next: u32) -> Option<FetchStep> {
        let st = self.st.borrow();
        if st.generation != generation {
            return None;
        }
        let ahead_s = if st.target.is_some() { 0.0 } else { (st.timeline.end_s() - self.media_time(&st)).max(0.0) };
        Some(buffer::next_step(ahead_s, next, st.segment_count))
    }

    /// Every segment is in: let the element reach `ended`.
    fn finish(&self, generation: u64) {
        self.update(|st, out| {
            if st.generation != generation {
                return;
            }
            st.finished = true;
            match &st.backend {
                Backend::Mse(session) => session.finish(),
                // The fallback element already ran out waiting for a segment that does not exist.
                Backend::Blob(queue) if queue.stalled => {
                    st.want_play = false;
                    self.apply(st, out, Input::Ended);
                    out.push(PlayerEvent::Ended);
                }
                _ => {}
            }
        });
    }

    /// A fatal error for this build.
    pub(super) fn fail(&self, generation: u64, message: String) {
        self.update(|st, out| {
            if st.generation != generation || matches!(st.phase, PlayerState::Error(_)) {
                return;
            }
            out.push(PlayerEvent::Error(message.clone()));
            self.apply(st, out, Input::Failed(message));
        });
    }

    /// A provider error. Reported once per run of failures; the state only turns to Error when nothing is left to
    /// play, otherwise playback continues on the buffer and `waiting` reports it if the fetch never recovers.
    fn fetch_failed(&self, generation: u64, message: String, first: bool) {
        self.update(|st, out| {
            if st.generation != generation {
                return;
            }
            if first {
                out.push(PlayerEvent::Error(message.clone()));
            }
            st.fetch_failure = Some(message);
            let stalled = matches!(&st.backend, Backend::Blob(queue) if queue.stalled);
            if st.target.is_some() || st.phase == PlayerState::Buffering || stalled {
                self.starve(st, out);
            }
        });
    }

    fn appended(self: &Rc<Self>, generation: u64, segment: u32, end_s: Option<f64>, track: &Track) {
        self.update(|st, out| {
            if st.generation != generation {
                return;
            }
            let start_s = st.timeline.end_s();
            // Sequence mode reports the exact end; the provider's duration covers a browser that does not.
            let end_s = end_s.filter(|e| e.is_finite() && *e > start_s).unwrap_or(start_s + secs(track.duration_ms));
            st.timeline.push(segment, start_s, end_s, track.words.clone(), track.ends_section);
            self.after_append(st, out, segment);
        });
    }

    fn append_blob(self: &Rc<Self>, generation: u64, segment: u32, track: &Track) -> bool {
        let url = match js::blob_url(&track.bytes, BLOB_MIME) {
            Ok(url) => url,
            Err(err) => {
                self.fail(generation, format!("could not play segment {segment}: {}", js::error_message(&err)));
                return false;
            }
        };
        self.update(|st, out| {
            let Backend::Blob(queue) = &mut st.backend else {
                js::revoke_url(&url);
                return false;
            };
            if st.generation != generation {
                js::revoke_url(&url);
                return false;
            }
            queue.urls.push(Some(url));
            let start_s = st.timeline.end_s();
            st.timeline.push(
                segment,
                start_s,
                start_s + secs(track.duration_ms),
                track.words.clone(),
                track.ends_section,
            );
            self.after_append(st, out, segment);
            true
        })
    }

    /// Bookkeeping after a segment joins the timeline: place the playhead of a new build, recover from a failed
    /// fetch, resume a stalled fallback element, release old blobs.
    fn after_append(self: &Rc<Self>, st: &mut State, out: &mut Outbox, segment: u32) {
        let recovered = st.fetch_failure.take().is_some();
        let Some(index) = st.timeline.entries().len().checked_sub(1) else {
            return;
        };
        if let Some((target, offset)) = st.target
            && target == segment
        {
            let time_s = st.timeline.entries().get(index).map_or(0.0, |entry| offset.resolve(entry));
            st.target = None;
            self.set_time(st, time_s);
            st.last_media_s = None;
            self.apply(st, out, Input::Ready);
            if st.want_play && self.use_mse {
                // The fallback path starts playback as part of loading the blob in set_time.
                self.start_playback(st);
            }
            self.update_position(st);
            return;
        }
        if recovered && matches!(st.phase, PlayerState::Error(_)) {
            self.apply(st, out, Input::Ready);
        }
        let t = self.media_time(st);
        if let Backend::Blob(queue) = &mut st.backend {
            if queue.stalled && index == queue.current + 1 {
                if let Some(Some(url)) = queue.urls.get(index) {
                    let promise = js::switch_source(&self.audio, url, 0.0, st.want_play);
                    queue.current = index;
                    queue.stalled = false;
                    self.watch_play(st.generation, promise);
                }
                self.report_segment(st, out, segment);
                return;
            }
            queue.revoke_before(&st.timeline, t - KEEP_BEHIND_S);
        }
    }
}

fn secs(ms: u32) -> f64 {
    f64::from(ms) / 1000.0
}
