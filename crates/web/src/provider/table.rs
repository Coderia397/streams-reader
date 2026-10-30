//! What the app knows about each segment's audio for one (document, voice): nothing yet, queued under a key, ready,
//! or failed. Pure bookkeeping over the render API's answers.

use std::collections::HashMap;
use std::ops::Range;

use nolabs_core::proto::ApiError;
use nolabs_core::proto::api::{RenderItemStatus, RenderResponse, RenderState, SegmentAudio, WordTiming};
use serde::{Deserialize, Serialize};

use crate::config::RENDER_WINDOW;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Slot {
    Unknown,
    Pending { key: String, state: RenderState, retry_after_s: Option<u64> },
    Ready { key: String, audio: SegmentAudio },
    Failed { error: Option<ApiError> },
}

/// A ready slot as stored in IndexedDB.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReadyEntry {
    pub segment: u32,
    pub key: String,
    pub audio: SegmentAudio,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RenderTable {
    slots: Vec<Slot>,
}

/// Segments covered by render window `w`.
pub fn window_range(w: u32, segment_count: u32) -> Range<u32> {
    let start = w.saturating_mul(RENDER_WINDOW).min(segment_count);
    start..(start + RENDER_WINDOW).min(segment_count)
}

pub fn window_of(segment: u32) -> u32 {
    segment / RENDER_WINDOW
}

fn slot_from(status: &RenderItemStatus) -> Slot {
    match status.state {
        RenderState::Ready => match &status.audio {
            Some(audio) => Slot::Ready { key: status.key.clone(), audio: audio.clone() },
            None => Slot::Failed { error: Some(ApiError::internal("ready without audio")) },
        },
        RenderState::Failed => Slot::Failed { error: status.error.clone() },
        state => Slot::Pending {
            key: status.key.clone(),
            state,
            retry_after_s: status.error.as_ref().and_then(|e| e.retry_after_s),
        },
    }
}

impl RenderTable {
    pub fn new(segment_count: u32) -> RenderTable {
        RenderTable { slots: vec![Slot::Unknown; segment_count as usize] }
    }

    pub fn slot(&self, segment: u32) -> Slot {
        self.slots.get(segment as usize).cloned().unwrap_or(Slot::Unknown)
    }

    pub fn is_ready(&self, segment: u32) -> bool {
        matches!(self.slots.get(segment as usize), Some(Slot::Ready { .. }))
    }

    /// True when some slot in the range has never been requested or failed (worth asking again).
    pub fn needs_request(&self, range: Range<u32>) -> bool {
        range.filter_map(|i| self.slots.get(i as usize)).any(|s| matches!(s, Slot::Unknown))
    }

    /// Applies a `POST /api/render` answer for items `start..start + n` (same order as requested). Returns whether
    /// any slot became ready.
    pub fn apply_render(&mut self, start: u32, resp: &RenderResponse) -> bool {
        let mut ready = false;
        for (i, status) in resp.items.iter().enumerate() {
            if let Some(slot) = self.slots.get_mut(start as usize + i) {
                let next = slot_from(status);
                ready |= matches!(next, Slot::Ready { .. }) && !matches!(slot, Slot::Ready { .. });
                *slot = next;
            }
        }
        ready
    }

    /// Applies a status answer by key to every pending slot with that key (identical text shares one key).
    pub fn apply_status(&mut self, resp: &RenderResponse) -> bool {
        let by_key: HashMap<&str, &RenderItemStatus> = resp.items.iter().map(|s| (s.key.as_str(), s)).collect();
        let mut ready = false;
        for slot in &mut self.slots {
            if let Slot::Pending { key, .. } = slot
                && let Some(status) = by_key.get(key.as_str())
            {
                let next = slot_from(status);
                ready |= matches!(next, Slot::Ready { .. });
                *slot = next;
            }
        }
        ready
    }

    /// Distinct keys still queued or rendering in the range.
    pub fn pending_keys(&self, range: Range<u32>) -> Vec<String> {
        let mut keys: Vec<String> = Vec::new();
        for i in range {
            if let Some(Slot::Pending { key, .. }) = self.slots.get(i as usize)
                && !keys.contains(key)
            {
                keys.push(key.clone());
            }
        }
        keys
    }

    /// Forgets a slot (e.g. its clip vanished from storage) so the next request renders it again.
    pub fn reset(&mut self, segment: u32) {
        if let Some(slot) = self.slots.get_mut(segment as usize) {
            *slot = Slot::Unknown;
        }
    }

    pub fn words(&self, segment: u32) -> Vec<WordTiming> {
        match self.slots.get(segment as usize) {
            Some(Slot::Ready { audio, .. }) => audio.words.clone(),
            _ => Vec::new(),
        }
    }

    pub fn ready_entries(&self) -> Vec<ReadyEntry> {
        self.slots
            .iter()
            .enumerate()
            .filter_map(|(i, s)| match s {
                Slot::Ready { key, audio } => {
                    Some(ReadyEntry { segment: i as u32, key: key.clone(), audio: audio.clone() })
                }
                _ => None,
            })
            .collect()
    }

    pub fn restore(&mut self, entries: Vec<ReadyEntry>) {
        for e in entries {
            if let Some(slot) = self.slots.get_mut(e.segment as usize) {
                *slot = Slot::Ready { key: e.key, audio: e.audio };
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use nolabs_core::proto::ErrorCode;

    use super::*;

    fn status(key: &str, state: RenderState) -> RenderItemStatus {
        let audio = (state == RenderState::Ready).then(|| SegmentAudio { duration_ms: 1000, bytes: 10, words: vec![] });
        RenderItemStatus { key: key.into(), state, audio, error: None }
    }

    fn resp(items: Vec<RenderItemStatus>) -> RenderResponse {
        RenderResponse { items }
    }

    #[test]
    fn windows_cover_the_document() {
        assert_eq!(window_range(0, 30), 0..12);
        assert_eq!(window_range(2, 30), 24..30);
        assert_eq!(window_range(5, 30), 30..30);
        assert_eq!(window_of(11), 0);
        assert_eq!(window_of(12), 1);
    }

    #[test]
    fn render_answers_fill_slots_in_order() {
        let mut t = RenderTable::new(4);
        assert!(t.needs_request(0..4));
        let became_ready =
            t.apply_render(1, &resp(vec![status("a", RenderState::Ready), status("b", RenderState::Queued)]));
        assert!(became_ready);
        assert!(t.is_ready(1));
        assert!(matches!(t.slot(2), Slot::Pending { ref key, state: RenderState::Queued, .. } if key == "b"));
        assert!(t.needs_request(0..4));
        assert!(!t.needs_request(1..3));
        assert_eq!(t.pending_keys(0..4), vec!["b".to_owned()]);
    }

    #[test]
    fn status_answers_update_every_slot_sharing_a_key() {
        let mut t = RenderTable::new(3);
        t.apply_render(0, &resp(vec![status("k", RenderState::Queued), status("k", RenderState::Queued)]));
        assert_eq!(t.pending_keys(0..3), vec!["k".to_owned()]);
        assert!(t.apply_status(&resp(vec![status("k", RenderState::Ready)])));
        assert!(t.is_ready(0) && t.is_ready(1));
        assert!(!t.apply_status(&resp(vec![status("k", RenderState::Ready)])));
    }

    #[test]
    fn deferred_keeps_the_retry_hint_and_failures_keep_the_error() {
        let mut t = RenderTable::new(2);
        let mut deferred = status("d", RenderState::Deferred);
        deferred.error = Some(ApiError::new(ErrorCode::GpuBudget, "spent").with_retry_after(600));
        let mut failed = status("f", RenderState::Failed);
        failed.error = Some(ApiError::new(ErrorCode::EngineFailed, "boom"));
        t.apply_render(0, &resp(vec![deferred, failed]));
        assert!(matches!(t.slot(0), Slot::Pending { state: RenderState::Deferred, retry_after_s: Some(600), .. }));
        assert!(matches!(t.slot(1), Slot::Failed { error: Some(ref e) } if e.code == ErrorCode::EngineFailed));
    }

    #[test]
    fn ready_without_audio_is_a_failure() {
        let mut t = RenderTable::new(1);
        let mut s = status("x", RenderState::Ready);
        s.audio = None;
        t.apply_render(0, &resp(vec![s]));
        assert!(matches!(t.slot(0), Slot::Failed { .. }));
    }

    #[test]
    fn persist_round_trip_and_reset() {
        let mut t = RenderTable::new(3);
        let mut s = status("a", RenderState::Ready);
        if let Some(a) = s.audio.as_mut() {
            a.words = vec![WordTiming(0, 100, 0, 3)];
        }
        t.apply_render(2, &resp(vec![s]));
        let entries = t.ready_entries();
        assert_eq!(entries.len(), 1);
        let mut u = RenderTable::new(3);
        u.restore(entries);
        assert_eq!(u.words(2), vec![WordTiming(0, 100, 0, 3)]);
        u.reset(2);
        assert_eq!(u.slot(2), Slot::Unknown);
        assert_eq!(u.slot(99), Slot::Unknown);
    }
}
