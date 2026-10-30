//! Reactive read-outs for the UI (tracked: call them inside closures in views).

use leptos::prelude::*;
use nolabs_player::PlayerState;

use super::PlayerBridge;

impl PlayerBridge {
    /// Current char offset (tracks segment, word and document).
    pub fn offset(&self) -> Option<u32> {
        let segment = self.segment.get();
        let word = self.word.get();
        self.words.with(|words| {
            self.doc.with(|d| d.as_ref().map(|d| d.model.position_offset(segment, word.and_then(|w| words.get(w)))))
        })
    }

    /// 0.0–1.0 through the document.
    pub fn fraction(&self) -> f64 {
        let offset = self.offset();
        self.doc.with(|d| match (d, offset) {
            (Some(d), Some(o)) => d.model.fraction_at(o),
            _ => 0.0,
        })
    }

    /// Estimated seconds left at the current speed.
    pub fn seconds_left(&self) -> f64 {
        let offset = self.offset();
        let rate = self.rate.get();
        self.doc.with(|d| match (d, offset) {
            (Some(d), Some(o)) => d.model.seconds_left(o, rate),
            _ => 0.0,
        })
    }

    pub fn title(&self) -> String {
        self.doc.with(|d| d.as_ref().map(|d| d.summary.meta.title.clone()).unwrap_or_default())
    }

    pub fn doc_id(&self) -> Option<String> {
        self.doc.with(|d| d.as_ref().map(|d| d.summary.id.clone()))
    }

    /// Spinner on the play button: playback was asked for and audio is not flowing yet.
    pub fn is_waiting(&self) -> bool {
        self.wants_play.get() && matches!(self.state.get(), PlayerState::Loading | PlayerState::Buffering)
    }

    /// Show pause rather than play.
    pub fn shows_pause(&self) -> bool {
        self.wants_play.get() || self.state.get() == PlayerState::Playing
    }

    /// Title of the chapter being read.
    pub fn chapter_title(&self) -> Option<String> {
        let segment = self.segment.get();
        self.doc.with(|d| {
            let d = d.as_ref()?;
            d.model.chapter_at(segment).map(|i| d.model.chapters[i].title.clone())
        })
    }
}
