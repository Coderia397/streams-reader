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
                app.toasts.info("Sleep timer ended. Sleep well.");
                spawn_local(async move { self.sync(app).await });
            }
            PlayerEvent::Error(msg) => {
                set_if_changed(self.wants_play, false);
                self.error.set(Some(msg.clone()));
                app.toasts.error(msg);
            }
        }
    }

    fn enter_segment(self, segment: u32) {
        if self.segment.get_untracked() != segment {
            self.segment.set(segment);
            set_if_changed(self.word, None);
            let words = self.session().map(|s| s.words(segment)).unwrap_or_default();
            self.words.set(words);
        }
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
