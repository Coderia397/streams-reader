//! Bridge between the UI and `nolabs_player::Player`. One player exists for the app's lifetime of a document; the
//! mini-player and the reader both drive it through this handle. Player events become signals here.

mod derived;
mod events;
mod open;
mod progress;

use std::rc::Rc;
use std::sync::Arc;

use leptos::prelude::*;
use nolabs_core::proto::api::{DocumentSummary, Engine, WordTiming};
use nolabs_player::{Player, PlayerState, Sleep};

use crate::config::SEEK_STEP_MS;
use crate::provider::RenderSession;
use crate::reading::doc::DocModel;
use crate::state::settings::clamp_speed;

pub use open::OpenRequest;

/// The document loaded in the player, prepared for the current voice.
#[derive(Debug, Clone, PartialEq)]
pub struct OpenDoc {
    pub summary: DocumentSummary,
    pub voice: String,
    pub engine: Engine,
    pub model: DocModel,
}

/// What the sleep sheet shows as chosen.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SleepChoice {
    Off,
    Minutes(u32),
    EndOfChapter,
}

impl SleepChoice {
    fn to_player(self) -> Sleep {
        match self {
            SleepChoice::Off => Sleep::Off,
            SleepChoice::Minutes(m) => Sleep::After { ms: m.saturating_mul(60_000) },
            SleepChoice::EndOfChapter => Sleep::EndOfSection,
        }
    }
}

pub(crate) struct Active {
    pub player: Player,
    pub session: Rc<RenderSession>,
}

#[derive(Clone, Copy)]
pub struct PlayerBridge {
    active: StoredValue<Option<Rc<Active>>, LocalStorage>,
    pub doc: RwSignal<Option<Arc<OpenDoc>>>,
    /// Id of the document being opened (spinner on its card).
    pub opening: RwSignal<Option<String>>,
    pub state: RwSignal<PlayerState>,
    pub segment: RwSignal<u32>,
    pub word: RwSignal<Option<usize>>,
    pub words: RwSignal<Vec<WordTiming>>,
    pub rate: RwSignal<f64>,
    pub sleep: RwSignal<SleepChoice>,
    pub sleep_left_ms: RwSignal<Option<u32>>,
    pub buffered_ms: RwSignal<u32>,
    /// Last playback error, shown in the dock until playback resumes.
    pub error: RwSignal<Option<String>>,
    /// The listener asked for playback (play, tap, autoplay) and has not paused since. The player reports `Loading`
    /// both while preloading and while starting, so the play/pause icon follows this instead.
    pub wants_play: RwSignal<bool>,
    last_sync_ms: StoredValue<i64>,
}

impl PlayerBridge {
    pub fn new() -> PlayerBridge {
        PlayerBridge {
            active: StoredValue::new_local(None),
            doc: RwSignal::new(None),
            opening: RwSignal::new(None),
            state: RwSignal::new(PlayerState::Idle),
            segment: RwSignal::new(0),
            word: RwSignal::new(None),
            words: RwSignal::new(Vec::new()),
            rate: RwSignal::new(1.0),
            sleep: RwSignal::new(SleepChoice::Off),
            sleep_left_ms: RwSignal::new(None),
            buffered_ms: RwSignal::new(0),
            error: RwSignal::new(None),
            wants_play: RwSignal::new(false),
            last_sync_ms: StoredValue::new(0),
        }
    }

    fn with_player(&self, f: impl FnOnce(&Player)) {
        if let Some(active) = self.active.get_value() {
            f(&active.player);
        }
    }

    pub fn has_doc(&self) -> bool {
        self.doc.with(Option::is_some)
    }

    pub fn play(&self) {
        self.error.set(None);
        self.wants_play.set(true);
        self.with_player(Player::play);
    }

    pub fn pause(&self) {
        self.wants_play.set(false);
        self.with_player(Player::pause);
    }

    /// Play/pause by what the listener sees, so a tap during loading cancels the start instead of being lost.
    pub fn toggle(&self) {
        if self.wants_play.get_untracked() { self.pause() } else { self.play() }
    }

    pub fn back(&self) {
        self.with_player(|p| p.seek_by(-SEEK_STEP_MS));
    }

    pub fn forward(&self) {
        self.with_player(|p| p.seek_by(SEEK_STEP_MS));
    }

    pub fn next(&self) {
        self.with_player(Player::next);
    }

    pub fn previous(&self) {
        self.with_player(Player::previous);
    }

    /// Jumps to a segment and plays from it (tap on a sentence, chapter, scrubber).
    pub fn jump_to(&self, segment: u32) {
        self.error.set(None);
        self.wants_play.set(true);
        self.segment.set(segment);
        self.word.set(None);
        self.with_player(|p| {
            p.jump_to(segment);
            p.play();
        });
    }

    pub fn set_rate(&self, rate: f64) {
        let rate = clamp_speed(rate);
        self.rate.set(rate);
        self.with_player(|p| p.set_rate(rate));
    }

    pub fn set_sleep(&self, choice: SleepChoice) {
        self.sleep.set(choice);
        if choice == SleepChoice::Off {
            self.sleep_left_ms.set(None);
        }
        self.with_player(|p| p.set_sleep(choice.to_player()));
    }

    /// Char offset of the current listening position.
    pub fn current_offset(&self) -> Option<u32> {
        let segment = self.segment.get_untracked();
        let word = self.word.get_untracked();
        let words = self.words.read_untracked();
        self.doc
            .with_untracked(|d| d.as_ref().map(|d| d.model.position_offset(segment, word.and_then(|w| words.get(w)))))
    }

    /// Stops and forgets the current document.
    pub fn close(&self) {
        if let Some(active) = self.active.get_value() {
            active.session.close();
            active.player.unload();
        }
        self.active.set_value(None);
        self.doc.set(None);
        self.wants_play.set(false);
        self.state.set(PlayerState::Idle);
        self.sleep.set(SleepChoice::Off);
        self.sleep_left_ms.set(None);
    }

    pub(crate) fn replace_active(&self, next: Option<Rc<Active>>) {
        if let Some(old) = self.active.get_value() {
            old.session.close();
            old.player.unload();
        }
        self.active.set_value(next);
    }

    pub(crate) fn session(&self) -> Option<Rc<RenderSession>> {
        self.active.get_value().map(|a| Rc::clone(&a.session))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sleep_choices_map_to_player_modes() {
        assert_eq!(SleepChoice::Minutes(15).to_player(), Sleep::After { ms: 900_000 });
        assert_eq!(SleepChoice::EndOfChapter.to_player(), Sleep::EndOfSection);
        assert_eq!(SleepChoice::Off.to_player(), Sleep::Off);
    }
}
