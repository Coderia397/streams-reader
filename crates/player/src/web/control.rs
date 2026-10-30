//! Commands: load, transport, seeks, rate, sleep and unload, plus the build and playhead plumbing they share.

use std::rc::Rc;

use wasm_bindgen_futures::spawn_local;

use super::backend::{Backend, BlobQueue};
use super::{Outbox, Shared, State, fetch, js};
use crate::seek::{self, Offset, SeekPlan};
use crate::status::{self, Input};
use crate::{MediaInfo, PlayerEvent, PlayerState, Sleep};

impl Shared {
    pub(super) fn load(self: &Rc<Self>, info: MediaInfo, segment_count: u32, start_segment: u32) {
        self.update(|st, out| {
            self.reset(st, out);
            js::session_set_metadata(&info.title, &info.artist, &info.album, info.artwork_url.clone());
            st.segment_count = segment_count;
            match segment_count.checked_sub(1) {
                Some(last) => self.rebuild(st, out, start_segment.min(last), Offset::START),
                None => {
                    self.apply(st, out, Input::Ended);
                    out.push(PlayerEvent::Ended);
                }
            }
        });
    }

    pub(super) fn play(self: &Rc<Self>) {
        self.update(|st, out| {
            if st.phase == PlayerState::Idle {
                return;
            }
            st.want_play = true;
            if st.phase == PlayerState::Ended {
                // Play after the end starts the document again.
                let plan = seek::plan_jump(&st.timeline, &st.backend.ranges(&st.timeline), 0, st.segment_count);
                self.go(st, out, plan);
            } else if st.target.is_none() {
                self.start_playback(st);
            }
            // While a build is loading, it starts playback once its first segment is in.
        });
    }

    pub(super) fn pause(self: &Rc<Self>) {
        self.update(|st, out| {
            st.want_play = false;
            self.account_sleep(st, out);
            st.sleep_clock_ms = None;
            let _ = self.audio.pause();
            // The element only fires `pause` when it was playing; a stalled or loading player changes state here.
            self.apply(st, out, Input::Paused);
        });
    }

    pub(super) fn toggle(self: &Rc<Self>) {
        let active = {
            let st = self.st.borrow();
            status::is_active(&st.phase, st.want_play)
        };
        if active { self.pause() } else { self.play() }
    }

    pub(super) fn seek_by(self: &Rc<Self>, delta_ms: i32) {
        self.update(|st, out| {
            if let Some((segment, offset)) = st.target {
                st.target = Some((segment, offset.shifted(delta_ms)));
                return;
            }
            let t = self.media_time(st);
            let plan =
                seek::plan_seek_by(&st.timeline, &st.backend.ranges(&st.timeline), t, delta_ms, st.segment_count);
            self.go(st, out, plan);
        });
    }

    /// Seek to `position_s` inside the current segment (the Media Session scrubber shows one segment).
    pub(super) fn seek_in_segment(self: &Rc<Self>, position_s: f64) {
        if !position_s.is_finite() {
            return;
        }
        self.update(|st, out| {
            if st.target.is_some() {
                return;
            }
            let t = self.media_time(st);
            let Some(start_s) =
                st.timeline.locate(t).and_then(|p| st.timeline.entries().get(p.index)).map(|e| e.start_s)
            else {
                return;
            };
            let target_s = start_s + position_s.max(0.0);
            let plan = seek::plan_seek_to(&st.timeline, &st.backend.ranges(&st.timeline), target_s, st.segment_count);
            self.go(st, out, plan);
        });
    }

    pub(super) fn jump_to(self: &Rc<Self>, segment: u32) {
        self.update(|st, out| self.jump(st, out, segment));
    }

    pub(super) fn next(self: &Rc<Self>) {
        self.update(|st, out| {
            let (segment, _) = self.position(st);
            if let Some(target) = seek::next_target(segment, st.segment_count) {
                self.jump(st, out, target);
            }
        });
    }

    pub(super) fn previous(self: &Rc<Self>) {
        self.update(|st, out| {
            let (segment, offset_ms) = self.position(st);
            self.jump(st, out, seek::previous_target(segment, offset_ms));
        });
    }

    pub(super) fn set_rate(&self, rate: f64) {
        self.update(|st, out| {
            st.rate = status::clamp_rate(rate);
            // The default rate survives a source change on the fallback path.
            self.audio.set_default_playback_rate(st.rate);
            self.audio.set_playback_rate(st.rate);
            self.update_position(st);
            out.push(PlayerEvent::Snapshot(self.snapshot_of(st)));
        });
    }

    pub(super) fn set_sleep(&self, sleep: Sleep) {
        self.update(|st, out| {
            st.sleep.set(sleep);
            self.audio.set_volume(st.sleep.volume());
            st.sleep_clock_ms = (st.phase == PlayerState::Playing).then(js_sys::Date::now);
            out.push(PlayerEvent::Snapshot(self.snapshot_of(st)));
        });
    }

    pub(super) fn unload(&self) {
        self.update(|st, out| self.reset(st, out));
    }

    /// (segment, offset) of the playhead, or the pending target of a build.
    fn position(&self, st: &State) -> (u32, u32) {
        if let Some((segment, _)) = st.target {
            return (segment, 0);
        }
        match st.timeline.locate(self.media_time(st)) {
            Some(pos) => (pos.segment, pos.offset_ms),
            None => (st.reported_segment.unwrap_or(0), 0),
        }
    }

    fn jump(self: &Rc<Self>, st: &mut State, out: &mut Outbox, segment: u32) {
        let plan = seek::plan_jump(&st.timeline, &st.backend.ranges(&st.timeline), segment, st.segment_count);
        self.go(st, out, plan);
    }

    fn go(self: &Rc<Self>, st: &mut State, out: &mut Outbox, plan: SeekPlan) {
        match plan {
            SeekPlan::Ignore => {}
            SeekPlan::Rebuild { segment, offset } => self.rebuild(st, out, segment, offset),
            SeekPlan::InBuffer { time_s } => {
                if !self.set_time(st, time_s) {
                    // Fallback path: that segment's blob was already released.
                    if let Some(pos) = st.timeline.locate(time_s) {
                        self.rebuild(st, out, pos.segment, Offset::FromStart(pos.offset_ms));
                    }
                    return;
                }
                st.last_media_s = None;
                if st.phase == PlayerState::Ended {
                    self.apply(st, out, Input::Ready);
                    if st.want_play {
                        self.start_playback(st);
                    }
                }
                if let Some(pos) = st.timeline.locate(time_s) {
                    self.report_segment(st, out, pos.segment);
                }
                self.update_position(st);
                out.push(PlayerEvent::Snapshot(self.snapshot_of(st)));
            }
        }
    }

    /// Starts a new build at `segment`: drops the current audio, then opens a new MediaSource (or blob queue) and
    /// starts its fetch loop. The playhead lands on `offset` once that segment is appended.
    pub(super) fn rebuild(self: &Rc<Self>, st: &mut State, out: &mut Outbox, segment: u32, offset: Offset) {
        st.generation += 1;
        let generation = st.generation;
        self.account_sleep(st, out);
        st.sleep_clock_ms = None;
        st.backend.teardown();
        let _ = self.audio.pause();
        st.timeline.clear();
        st.target = Some((segment, offset));
        st.finished = false;
        st.fetch_failure = None;
        st.last_media_s = None;
        self.report_segment(st, out, segment);
        self.apply(st, out, Input::Build);
        let weak = Rc::downgrade(self);
        if self.use_mse {
            st.backend = Backend::Opening;
            spawn_local(fetch::open_and_run(weak, generation, segment));
        } else {
            st.backend = Backend::Blob(BlobQueue::default());
            spawn_local(fetch::run(weak, generation, segment));
        }
    }

    /// Moves the playhead to `time_s` on the build's timeline. False when the fallback path no longer holds that
    /// segment.
    pub(super) fn set_time(self: &Rc<Self>, st: &mut State, time_s: f64) -> bool {
        let generation = st.generation;
        let want_play = st.want_play;
        let Backend::Blob(queue) = &mut st.backend else {
            self.audio.set_current_time(time_s);
            return true;
        };
        let Some(pos) = st.timeline.locate(time_s) else {
            return false;
        };
        let Some(entry) = st.timeline.entries().get(pos.index) else {
            return false;
        };
        let offset_s = (time_s - entry.start_s).max(0.0);
        if pos.index == queue.current && queue.loaded && !queue.stalled {
            self.audio.set_current_time(offset_s);
            return true;
        }
        let Some(Some(url)) = queue.urls.get(pos.index) else {
            return false;
        };
        let promise = js::switch_source(&self.audio, url, offset_s, want_play);
        queue.current = pos.index;
        queue.loaded = true;
        queue.stalled = false;
        self.watch_play(generation, promise);
        true
    }

    pub(super) fn start_playback(self: &Rc<Self>, st: &mut State) {
        self.audio.set_volume(st.sleep.volume());
        if let Backend::Blob(queue) = &st.backend
            && queue.stalled
        {
            // The element finished its last blob; the next one starts playing when it arrives.
            return;
        }
        let promise = js::play_media(&self.audio);
        self.watch_play(st.generation, promise);
    }

    /// Handles a `play()` the browser refused.
    pub(super) fn watch_play(self: &Rc<Self>, generation: u64, promise: js_sys::Promise) {
        let weak = Rc::downgrade(self);
        spawn_local(async move {
            let Err(name) = js::play_outcome(promise).await else {
                return;
            };
            if let Some(shared) = weak.upgrade() {
                shared.play_refused(generation, &name);
            }
        });
    }

    fn play_refused(&self, generation: u64, name: &str) {
        // AbortError: a pause or a new source interrupted this play(); that is not a failure.
        if name == "AbortError" {
            return;
        }
        self.update(|st, out| {
            if st.generation != generation {
                return;
            }
            st.want_play = false;
            self.apply(st, out, Input::Paused);
            // NotAllowedError: the browser wants a user gesture first; the listener just presses play again.
            if name != "NotAllowedError" {
                out.push(PlayerEvent::Error(format!("playback failed: {name}")));
            }
        });
    }

    /// Back to Idle: drops the document's audio and the notification.
    fn reset(&self, st: &mut State, out: &mut Outbox) {
        st.generation += 1;
        st.backend.teardown();
        js::detach_media(&self.audio);
        st.timeline.clear();
        st.target = None;
        st.reported_segment = None;
        st.finished = false;
        st.fetch_failure = None;
        st.want_play = false;
        st.segment_count = 0;
        st.sleep_clock_ms = None;
        st.last_media_s = None;
        self.apply(st, out, Input::Unload);
        js::session_clear();
    }
}
