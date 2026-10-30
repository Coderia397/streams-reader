//! Media element events, the sleep countdown and the animation-frame snapshot loop.
//!
//! Periodic snapshots ride `requestAnimationFrame`, which browsers suspend in hidden documents: with the screen off
//! nothing is sent except state changes and `SegmentChanged`, which `timeupdate` keeps delivering in the background.

use std::rc::{Rc, Weak};

use wasm_bindgen::JsCast;
use wasm_bindgen::closure::Closure;

use super::backend::Backend;
use super::{Listeners, Outbox, Shared, State, js};
use crate::sleep::SleepStep;
use crate::status::Input;
use crate::{PlayerEvent, PlayerState};

type Handler = fn(&Rc<Shared>);

/// One listener per media element event the player reacts to.
pub(super) fn listeners(weak: &Weak<Shared>) -> Listeners {
    let table: [(&'static str, Handler); 8] = [
        ("timeupdate", Shared::on_timeupdate),
        ("playing", Shared::on_playing),
        ("pause", Shared::on_pause),
        ("waiting", Shared::on_waiting),
        ("ended", Shared::on_ended),
        ("error", Shared::on_error),
        ("seeking", Shared::on_seeking),
        ("seeked", Shared::on_seeked),
    ];
    table
        .into_iter()
        .map(|(name, handler)| {
            let weak = weak.clone();
            let callback = Closure::<dyn FnMut()>::new(move || {
                if let Some(shared) = weak.upgrade() {
                    handler(&shared);
                }
            });
            (name, callback)
        })
        .collect()
}

pub(super) fn frame_callback(weak: &Weak<Shared>) -> Closure<dyn FnMut(f64)> {
    let weak = weak.clone();
    Closure::<dyn FnMut(f64)>::new(move |now: f64| {
        if let Some(shared) = weak.upgrade() {
            shared.on_frame(now);
        }
    })
}

impl Shared {
    fn on_timeupdate(self: &Rc<Self>) {
        self.update(|st, out| {
            if st.target.is_some() || st.phase == PlayerState::Idle {
                return;
            }
            let t = self.media_time(st);
            if let Some(pos) = st.timeline.locate(t) {
                self.report_segment(st, out, pos.segment);
            }
            if st.phase == PlayerState::Playing {
                self.account_sleep(st, out);
            }
            if let Some(prev) = st.last_media_s
                && let Some(stop_s) = st.sleep.section_stop(st.timeline.entries(), prev, t)
            {
                self.stop_at_section_end(st, out, stop_s);
                return;
            }
            st.last_media_s = Some(t);
        });
    }

    /// End-of-section sleep: pause in the section's closing silence and park the playhead where the next section
    /// starts, so resuming begins with it.
    fn stop_at_section_end(self: &Rc<Self>, st: &mut State, out: &mut Outbox, stop_s: f64) {
        st.want_play = false;
        st.sleep_clock_ms = None;
        let _ = self.audio.pause();
        if stop_s < st.timeline.end_s() {
            self.set_time(st, stop_s);
        }
        st.last_media_s = None;
        out.push(PlayerEvent::SleepFired);
    }

    fn on_playing(self: &Rc<Self>) {
        self.update(|st, out| {
            // A late event from the source a rebuild just replaced.
            if st.target.is_some() || st.phase == PlayerState::Idle {
                return;
            }
            self.audio.set_volume(st.sleep.volume());
            st.sleep_clock_ms = Some(js_sys::Date::now());
            self.apply(st, out, Input::Playing);
            self.update_position(st);
            self.schedule_frame(st);
        });
    }

    fn on_pause(self: &Rc<Self>) {
        self.update(|st, out| {
            // The pause that precedes `ended` is handled there.
            if self.audio.ended() {
                return;
            }
            self.account_sleep(st, out);
            st.sleep_clock_ms = None;
            if st.target.is_none() {
                // Paused by the system (call, headset unplugged, another app taking audio focus): stay paused.
                st.want_play = false;
            }
            self.apply(st, out, Input::Paused);
            self.update_position(st);
        });
    }

    fn on_waiting(self: &Rc<Self>) {
        self.update(|st, out| {
            if st.target.is_some() {
                return;
            }
            self.account_sleep(st, out);
            st.sleep_clock_ms = None;
            self.starve(st, out);
        });
    }

    /// The playhead ran out of audio: buffering, or an error when the fetch that should have filled it failed.
    pub(super) fn starve(&self, st: &mut State, out: &mut Outbox) {
        match st.fetch_failure.clone() {
            Some(message) => self.apply(st, out, Input::Failed(message)),
            None => self.apply(st, out, Input::Waiting),
        }
    }

    fn on_ended(self: &Rc<Self>) {
        self.update(|st, out| {
            if st.target.is_some() || st.phase == PlayerState::Idle {
                return;
            }
            self.account_sleep(st, out);
            st.sleep_clock_ms = None;
            if let Backend::Blob(queue) = &mut st.backend {
                let next = queue.current + 1;
                if let Some(Some(url)) = queue.urls.get(next) {
                    let promise = js::switch_source(&self.audio, url, 0.0, st.want_play);
                    queue.current = next;
                    self.watch_play(st.generation, promise);
                    if let Some(segment) = st.timeline.entries().get(next).map(|e| e.segment) {
                        self.report_segment(st, out, segment);
                    }
                    return;
                }
                if !st.finished {
                    queue.stalled = true;
                    self.starve(st, out);
                    return;
                }
            }
            if st.finished {
                st.want_play = false;
                self.apply(st, out, Input::Ended);
                out.push(PlayerEvent::Ended);
            }
        });
    }

    fn on_error(self: &Rc<Self>) {
        self.update(|st, out| {
            if matches!(st.phase, PlayerState::Idle | PlayerState::Error(_)) {
                return;
            }
            let message = js::media_error_message(&self.audio);
            if message.is_empty() {
                return;
            }
            out.push(PlayerEvent::Error(message.clone()));
            self.apply(st, out, Input::Failed(message));
        });
    }

    fn on_seeking(self: &Rc<Self>) {
        // A seek is not playback: the next `timeupdate` must not count as crossing a section end.
        self.st.borrow_mut().last_media_s = None;
    }

    fn on_seeked(self: &Rc<Self>) {
        self.update(|st, out| {
            if st.target.is_some() || st.phase == PlayerState::Idle {
                return;
            }
            st.last_media_s = Some(self.media_time(st));
            self.update_position(st);
            out.push(PlayerEvent::Snapshot(self.snapshot_of(st)));
        });
    }

    fn on_frame(self: &Rc<Self>, now: f64) {
        self.update(|st, out| {
            st.frame = None;
            if !matches!(st.phase, PlayerState::Playing | PlayerState::Buffering) {
                return;
            }
            if st.throttle.ready(now) {
                out.push(PlayerEvent::Snapshot(self.snapshot_of(st)));
            }
            self.schedule_frame(st);
        });
    }

    fn schedule_frame(&self, st: &mut State) {
        if st.frame.is_some() {
            return;
        }
        if let Some(window) = web_sys::window() {
            st.frame = window.request_animation_frame(self.hooks.frame.as_ref().unchecked_ref()).ok();
        }
    }

    /// Charges the playing time since the last call to the sleep countdown; fades or pauses as it says.
    pub(super) fn account_sleep(&self, st: &mut State, out: &mut Outbox) {
        let Some(previous) = st.sleep_clock_ms else {
            return;
        };
        let now = js_sys::Date::now();
        st.sleep_clock_ms = Some(now);
        match st.sleep.advance(now - previous) {
            SleepStep::Idle => {}
            SleepStep::Volume(volume) => self.audio.set_volume(volume),
            SleepStep::Fire => {
                // The volume stays at zero until the next play, so the pause is silent.
                st.want_play = false;
                st.sleep_clock_ms = None;
                let _ = self.audio.pause();
                out.push(PlayerEvent::SleepFired);
            }
        }
    }
}
