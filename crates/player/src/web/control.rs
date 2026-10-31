//! Commands: load, transport, seeks, rate, sleep and unload, plus the build and playhead plumbing they share.

use std::rc::Rc;

use wasm_bindgen_futures::spawn_local;

use super::backend::{Backend, BlobQueue};
use super::{Outbox, Shared, State, fetch, js};
use crate::buffer;
use crate::seek::{self, Offset, SeekPlan, TargetStep, Then};
use crate::status::{self, Input};
use crate::{MediaInfo, PlayerEvent, PlayerState, Sleep};

/// A playhead this close to the end of the document is at its end (the element may stop a frame short of it).
const END_SLACK_S: f64 = 0.05;

impl Shared {
    pub(super) fn load(self: &Rc<Self>, info: MediaInfo, segment_count: u32, start_segment: u32) {
        self.update(|st, out| {
            self.reset(st, out);
            st.info = Some(info);
            if !self.take_session(st) {
                self.publish_metadata(st);
            }
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
            // A player played from the page owns the lock screen, even if a later player took it meanwhile.
            self.take_session(st);
            st.want_play = true;
            if st.fetch_failure.is_some() {
                // The listener is waiting on a segment that failed: ask for it again now, not after the backoff.
                st.wake_fetch = true;
            }
            // Paused on the very end (a skip past it while paused): the browser would restart an ended stream from
            // the start of this build, so it is the end of the document like Ended.
            let at_end = st.target.is_none()
                && st.finished
                && seek::is_document_end(&st.timeline, self.media_time(st) + END_SLACK_S, st.segment_count);
            if st.phase == PlayerState::Ended || at_end {
                // Play after the end starts the document again.
                let plan = seek::plan_jump(&st.timeline, &st.backend.ranges(&st.timeline), 0, st.segment_count);
                self.go(st, out, plan);
            } else if st.target.is_none() {
                self.start_playback(st);
                if buffer::is_starved(&st.backend.ranges(&st.timeline), self.media_time(st)) {
                    // Nothing buffered at the playhead (the next segment is still rendering): the element waits for
                    // it, and so does the listener. It may never fire `waiting` (a fallback element that finished
                    // its last blob).
                    self.apply(st, out, Input::Waiting);
                }
            } else {
                // While a build is loading, it starts playback once its first segment is in; the lock screen shows
                // the intent now.
                self.publish_activity(st, out);
            }
        });
    }

    pub(super) fn pause(self: &Rc<Self>) {
        self.update(|st, out| {
            st.want_play = false;
            self.account_sleep(st, out);
            st.sleep_clock_ms = None;
            self.pause_audio();
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
                // A skip while a build loads moves its target; it may now be in a segment already appended, or one
                // before the build, which restarts it there. Behind queued commands it waits its turn.
                self.hold_view(st);
                if st.then.is_empty() {
                    st.target = Some((segment, offset.shifted(delta_ms)));
                } else {
                    st.then.push(Then::Shift(delta_ms));
                }
                self.advance_target(st, out);
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
                // The segment the build lands in may not be known yet: apply once it is.
                st.then.push(Then::Within((position_s.max(0.0) * 1000.0).round().min(1e15) as i64));
                self.advance_target(st, out);
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
            if st.target.is_some() {
                self.queue(st, out, Then::Next);
                return;
            }
            let (segment, _) = self.position(st);
            if let Some(target) = seek::next_target(segment, st.segment_count) {
                self.jump(st, out, target);
            }
        });
    }

    pub(super) fn previous(self: &Rc<Self>) {
        self.update(|st, out| {
            if st.target.is_some() {
                self.queue(st, out, Then::Previous);
                return;
            }
            let (segment, offset_ms) = self.position(st);
            self.jump(st, out, seek::previous_target(segment, offset_ms));
        });
    }

    /// A command that depends on where a loading build's target really is: applied as soon as that is known.
    fn queue(self: &Rc<Self>, st: &mut State, out: &mut Outbox, then: Then) {
        st.then.push(then);
        self.advance_target(st, out);
    }

    /// Media Session seekbackward/seekforward: the action's own offset, or the listener's skip setting.
    pub(super) fn media_skip(self: &Rc<Self>, offset_s: f64, forward: bool) {
        let delta_ms = {
            let st = self.st.borrow();
            if forward {
                seek::skip_ms(offset_s, st.skip_forward_ms)
            } else {
                seek::skip_ms(offset_s, st.skip_back_ms).saturating_neg()
            }
        };
        self.seek_by(delta_ms);
    }

    pub(super) fn set_skip(&self, back_ms: u32, forward_ms: u32) {
        let mut st = self.st.borrow_mut();
        st.skip_back_ms = seek::skip_setting(back_ms);
        st.skip_forward_ms = seek::skip_setting(forward_ms);
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

    /// (segment, offset) of the placed playhead, as the listener sees it.
    fn position(&self, st: &State) -> (u32, u32) {
        match self.shown(st, self.media_time(st)) {
            Some((segment, offset_ms, _)) => (segment, offset_ms),
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
            SeekPlan::Rebuild { segment, offset } => {
                st.then.clear();
                // Lengths measured by earlier builds may put the target further on: start where it really is.
                let (segment, offset) = seek::resolve(&st.lengths, segment, offset, st.segment_count);
                if self.can_hold(st, segment) {
                    self.hold(st, out, segment, offset);
                } else {
                    self.rebuild(st, out, segment, offset);
                }
            }
            SeekPlan::InBuffer { time_s } => {
                st.then.clear();
                if st.target.is_some() {
                    // The build is still loading a later segment, but this target is already appended: it lands
                    // now, and the build's old target is dropped.
                    self.place(st, out, time_s);
                    return;
                }
                if st.phase == PlayerState::Ended && seek::is_document_end(&st.timeline, time_s, st.segment_count) {
                    // A skip forward after the end: still the end.
                    return;
                }
                if !self.set_time(st, time_s) {
                    // Fallback path: that segment's blob was already released.
                    if let Some(pos) = st.timeline.locate(time_s) {
                        self.rebuild(st, out, pos.segment, Offset::FromStart(i64::from(pos.offset_ms)));
                    }
                    return;
                }
                st.last_media_s = None;
                // Leaving the end, or an error whose playhead ran dry, for audio that can play.
                let resume = match &st.phase {
                    PlayerState::Ended => true,
                    PlayerState::Error(_) => !buffer::is_starved(&st.backend.ranges(&st.timeline), time_s),
                    _ => false,
                };
                if resume {
                    self.apply(st, out, Input::Ready);
                    if st.want_play {
                        self.start_playback(st);
                    }
                }
                if let Some((segment, _, _)) = self.shown(st, time_s) {
                    self.report_segment(st, out, segment);
                }
                self.update_position(st);
                out.push(PlayerEvent::Snapshot(self.snapshot_of(st)));
            }
        }
    }

    /// True when a new target in `segment` can be reached by the current build: that segment is the one its fetch
    /// loop is getting next. Not after a failure (a fresh build retries at once and restarts the backoff), nor once
    /// the loop stopped on a final error while buffered audio still plays (nothing would append it).
    fn can_hold(&self, st: &State, segment: u32) -> bool {
        !matches!(st.backend, Backend::Idle)
            && !st.fetch_stopped
            && !matches!(st.phase, PlayerState::Error(_) | PlayerState::Idle)
            && seek::is_next_fetch(&st.timeline, st.build_from, segment)
    }

    /// Keeps the current build and its fetch in flight, and lands on `(segment, offset)` when that segment is
    /// appended: like [`Shared::rebuild`] for the listener (silence, Loading), without asking the provider again or
    /// dropping the audio already buffered.
    fn hold(self: &Rc<Self>, st: &mut State, out: &mut Outbox, segment: u32, offset: Offset) {
        self.hold_view(st);
        self.account_sleep(st, out);
        st.sleep_clock_ms = None;
        st.target = Some((segment, offset));
        self.pause_audio();
        st.last_media_s = None;
        // A loop idling on a full buffer fetches now.
        st.wake_fetch = true;
        self.clear_position(st);
        // Announced before the Loading snapshot, which names the same segment.
        self.announce_target(st, out);
        self.apply(st, out, Input::Build);
        self.advance_target(st, out);
    }

    /// Starts a new build at `segment`: drops the current audio, then opens a new MediaSource (or blob queue) and
    /// starts its fetch loop. The playhead lands on `offset` once that segment is appended.
    pub(super) fn rebuild(self: &Rc<Self>, st: &mut State, out: &mut Outbox, segment: u32, offset: Offset) {
        // Before the timeline of the build that is replaced goes.
        self.hold_view(st);
        st.generation += 1;
        let generation = st.generation;
        self.account_sleep(st, out);
        st.sleep_clock_ms = None;
        st.backend.teardown();
        js::cancel_switch(&self.audio);
        self.pause_audio();
        st.timeline.clear();
        st.target = Some((segment, offset));
        st.build_from = segment;
        st.finished = false;
        st.fetch_failure = None;
        st.fetch_stopped = false;
        st.wake_fetch = false;
        st.last_media_s = None;
        // Until the build lands, the lock screen must not keep advancing the old segment's progress.
        self.clear_position(st);
        // Only a target whose segment is certain is announced; one carried across lengths not known yet is announced
        // when it lands. Announced before the Loading snapshot, which names the same segment.
        self.announce_target(st, out);
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

    /// Moves a pending build target as far as the appended segments allow: places the playhead once its segment is
    /// in, carries it into a neighbour when it points past an edge, restarts the build where it cannot reach the
    /// target soon, and applies queued commands once the target is exact.
    pub(super) fn advance_target(self: &Rc<Self>, st: &mut State, out: &mut Outbox) {
        self.hold_view(st);
        // Each retarget moves one segment and each queued command is applied once, which bounds the loop.
        let rounds = (st.then.len() + 1).saturating_mul(st.segment_count as usize + 2);
        for _ in 0..rounds {
            let Some((segment, offset)) = st.target else {
                return;
            };
            let step = seek::target_step(&st.timeline, &st.lengths, st.build_from, segment, offset, st.segment_count);
            if !st.then.is_empty() {
                let exact = match step {
                    TargetStep::Place(time_s) => {
                        self.shown(st, time_s).map(|(segment, offset_ms, _)| (segment, offset_ms))
                    }
                    // The start of a segment is exact before its length is known, and any offset inside a segment
                    // an earlier build measured.
                    _ => seek::exact_offset(&st.lengths, segment, offset).map(|offset_ms| (segment, offset_ms)),
                };
                if let Some((segment, offset_ms)) = exact {
                    let then = st.then.remove(0);
                    st.target = Some(then.apply(segment, offset_ms, st.segment_count));
                    continue;
                }
            }
            match step {
                TargetStep::Wait => {
                    self.announce_target(st, out);
                    return;
                }
                TargetStep::Retarget(next, offset) => st.target = Some((next, offset)),
                TargetStep::Rebuild(next, offset) => {
                    self.rebuild(st, out, next, offset);
                    return;
                }
                TargetStep::Place(time_s) => {
                    self.place(st, out, time_s);
                    return;
                }
            }
        }
    }

    /// Ends a build's loading: puts the playhead on `time_s` and starts playback if the listener wants it.
    fn place(self: &Rc<Self>, st: &mut State, out: &mut Outbox, time_s: f64) {
        st.target = None;
        st.then.clear();
        self.set_time(st, time_s);
        st.last_media_s = None;
        if let Some((segment, _, _)) = self.shown(st, time_s) {
            self.report_segment(st, out, segment);
        }
        self.apply(st, out, Input::Ready);
        if st.want_play {
            if !self.use_mse && js::pending_offset(&self.audio).is_finite() {
                // Fallback path: the blob switch still loading starts playback once it is ready (a hold paused it).
                js::set_switch_autoplay(&self.audio, true);
            } else {
                self.start_playback(st);
            }
        }
        self.update_position(st);
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
            // A switch to this blob still loading seeks to the new offset when it is ready; setting currentTime now
            // would be overwritten by it.
            if !js::retarget_switch(&self.audio, offset_s) {
                self.audio.set_current_time(offset_s);
            }
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
        st.lengths.clear();
        st.words.clear();
        st.held = None;
        st.target = None;
        st.then.clear();
        st.reported_segment = None;
        st.wake_fetch = false;
        st.position_mark = None;
        st.finished = false;
        st.fetch_failure = None;
        st.fetch_stopped = false;
        st.want_play = false;
        st.segment_count = 0;
        st.sleep_clock_ms = None;
        st.last_media_s = None;
        self.apply(st, out, Input::Unload);
        st.info = None;
        if self.holds_session() {
            js::session_clear();
        }
    }
}
