//! Player events → signals. Snapshots arrive several times a second, so each signal is written only when its value
//! changed; otherwise every highlight and progress bar would re-render on every tick.

use std::rc::Rc;

use leptos::prelude::*;
use leptos::task::spawn_local;
use nolabs_player::{PlayerEvent, PlayerState, Snapshot};

use super::{PlayerBridge, SleepChoice};
use crate::state::AppState;

fn set_if_changed<T: PartialEq + Send + Sync + 'static>(signal: RwSignal<T>, value: T) {
    if signal.with_untracked(|v| *v != value) {
        signal.set(value);
    }
}

/// The player reports `segment 3: could not load audio: <reason>`; the reason is already a friendly, translated line
/// from the render session, so only that part is shown. Anything else becomes a generic line.
pub fn listener_message(raw: &str) -> String {
    match raw.rsplit_once("could not load audio: ") {
        Some((_, reason)) if !reason.trim().is_empty() => reason.trim().to_owned(),
        _ => t!("player-error"),
    }
}

impl PlayerBridge {
    pub(super) fn event_handler(self, app: AppState) -> Rc<dyn Fn(PlayerEvent)> {
        Rc::new(move |event| self.on_event(event, app))
    }

    fn on_event(self, event: PlayerEvent, app: AppState) {
        match event {
            PlayerEvent::Snapshot(s) => self.apply_snapshot(s),
            PlayerEvent::SegmentChanged(segment) => self.enter_segment(segment),
            PlayerEvent::Ended => {
                set_if_changed(self.state, PlayerState::Ended);
                spawn_local(async move { self.mark_finished(app).await });
            }
            PlayerEvent::SleepFired => {
                self.sleep.set(SleepChoice::Off);
                self.sleep_left_ms.set(None);
                app.toasts.info(t!("sleep-ended"));
                spawn_local(async move { self.sync(app).await });
            }
            PlayerEvent::Error(msg) => {
                set_if_changed(self.wants_play, false);
                let msg = listener_message(&msg);
                self.error.set(Some(msg.clone()));
                app.toasts.error(msg);
            }
        }
    }

    /// Follows the segment the player reports and keeps [`PlayerBridge::words`] for it. Timings are reloaded when
    /// they belong to another segment or came back empty (the segment was still rendering), so the highlight never
    /// runs on the previous segment's words.
    fn enter_segment(self, segment: u32) {
        if self.segment.get_untracked() != segment {
            self.segment.set(segment);
            set_if_changed(self.word, None);
        }
        let loaded = self.words_for.get_value();
        if !needs_words(loaded, segment, self.words.with_untracked(Vec::is_empty)) {
            return;
        }
        let words = self.session().map(|s| s.words(segment)).unwrap_or_default();
        self.words_for.set_value((!words.is_empty()).then_some(segment));
        set_if_changed(self.words, words);
    }

    fn apply_snapshot(self, s: Snapshot) {
        self.enter_segment(s.segment);
        match s.state {
            PlayerState::Playing => {
                set_if_changed(self.error, None);
                // Started from outside the app (headset button, lock screen).
                set_if_changed(self.wants_play, true);
            }
            // Only a change into a stopped state cancels the intent: a periodic snapshot still saying "paused" can
            // arrive just after a tap on play.
            PlayerState::Paused | PlayerState::Ended | PlayerState::Error(_) | PlayerState::Idle => {
                if self.state.with_untracked(|prev| *prev != s.state) {
                    set_if_changed(self.wants_play, false);
                }
            }
            PlayerState::Loading | PlayerState::Buffering => {}
        }
        set_if_changed(self.state, s.state);
        set_if_changed(self.word, s.word);
        set_if_changed(self.buffered_ms, s.buffered_ahead_ms);
        set_if_changed(self.rate, s.rate);
        // The countdown shows whole seconds; finer updates would only cost renders.
        set_if_changed(self.sleep_left_ms, s.sleep_left_ms.map(|ms| ms / 1000 * 1000));
    }
}

/// Whether the word timings must be (re)loaded for `segment`.
pub(super) fn needs_words(loaded_for: Option<u32>, segment: u32, list_empty: bool) -> bool {
    loaded_for != Some(segment) || list_empty
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn words_reload_for_a_new_segment_or_an_empty_list() {
        assert!(needs_words(None, 0, true));
        assert!(needs_words(Some(3), 4, false));
        assert!(needs_words(Some(4), 4, true));
        assert!(!needs_words(Some(4), 4, false));
    }

    #[test]
    fn player_errors_keep_only_the_friendly_reason() {
        assert_eq!(
            listener_message("segment 2: could not load audio: The engine is waking up."),
            "The engine is waking up."
        );
        assert_eq!(listener_message("decode failed"), "Playback stopped unexpectedly. Press play to try again.");
    }
}
