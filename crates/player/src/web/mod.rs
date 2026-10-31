//! Browser driver: owns the hidden `<audio>` element and wires DOM events to the pure modules.
//!
//! Ownership: [`Driver`] holds the only strong reference to [`Shared`]. Event listeners, the animation frame callback,
//! Media Session handlers and the fetch loops hold `Weak` references, so dropping the player tears everything down
//! (see `Drop for Shared`). State lives in one `RefCell`; events are collected while it is borrowed and delivered after
//! the borrow ends, so `on_event` may call back into the player.

mod backend;
mod control;
mod events;
mod fetch;
mod js;
mod session;

use std::cell::RefCell;
use std::collections::BTreeMap;
use std::rc::Rc;

use nolabs_core::proto::api::WordTiming;

use wasm_bindgen::JsCast;
use wasm_bindgen::closure::Closure;
use web_sys::HtmlAudioElement;

use crate::buffer::ahead_of;
use crate::seek::{self, Lengths, Offset, Then};
use crate::sleep::SleepTimer;
use crate::status::{self, ClockProbe, Input, PositionMark, SNAPSHOT_INTERVAL_MS, Throttle};
use crate::timeline::{Timeline, secs_to_ms, word_at};
use crate::{MediaInfo, PlayerError, PlayerEvent, PlayerState, Provider, Sleep, Snapshot};
use backend::{Backend, MSE_MIME};

type Outbox = Vec<PlayerEvent>;
/// (segment, offset in ms, word) as the listener sees it.
type View = (u32, u32, Option<usize>);
/// Media element event listeners, by event name.
type Listeners = Vec<(&'static str, Closure<dyn FnMut()>)>;
/// Media Session action handlers, by action name; the argument is the number the action carries.
type Actions = Vec<(&'static str, Closure<dyn FnMut(f64)>)>;

pub(crate) struct Driver {
    shared: Rc<Shared>,
}

struct Shared {
    provider: Provider,
    on_event: Rc<dyn Fn(PlayerEvent)>,
    audio: HtmlAudioElement,
    /// MediaSource path; false means one blob URL per segment.
    use_mse: bool,
    /// This player's id for the page's one Media Session (see `session::claim`).
    session_id: u64,
    st: RefCell<State>,
    hooks: Hooks,
}

/// JS callbacks that must live as long as the player.
struct Hooks {
    listeners: Listeners,
    frame: Closure<dyn FnMut(f64)>,
    actions: Actions,
}

struct State {
    phase: PlayerState,
    /// The listener asked to hear audio (play pressed and not paused since).
    want_play: bool,
    segment_count: u32,
    /// Bumped by every (re)build and unload; async work from an older generation drops its result.
    generation: u64,
    backend: Backend,
    timeline: Timeline,
    /// Where the current build starts; Some until its first segment is appended and the playhead placed.
    target: Option<(u32, Offset)>,
    /// Commands pressed while `target` pointed into audio of unknown length, applied in order once it is exact.
    then: Vec<Then>,
    /// Segment the current build started fetching from.
    build_from: u32,
    /// Lengths of the segments any build of this document appended, so targets resolve across builds.
    lengths: Lengths,
    /// Word timings of the same segments, so a loading target in one of them shows its word.
    words: BTreeMap<u32, Vec<WordTiming>>,
    /// What snapshots show while a build loads a target whose position is not known yet: the last position the
    /// listener saw (see [`Shared::view`]).
    held: Option<View>,
    /// A command wants the fetch loop to stop waiting (a retry backoff or a full buffer) and fetch now.
    wake_fetch: bool,
    /// Last position given to the Media Session; None while it has none.
    position_mark: Option<PositionMark>,
    /// Last segment announced with `SegmentChanged`.
    reported_segment: Option<u32>,
    /// Every segment of the document is appended.
    finished: bool,
    rate: f64,
    sleep: SleepTimer,
    /// Wall clock (ms) of the last sleep countdown accounting; Some only while playing.
    sleep_clock_ms: Option<f64>,
    /// Media time at the previous `timeupdate`, for end-of-section crossings; None right after a seek.
    last_media_s: Option<f64>,
    throttle: Throttle,
    /// Pending `requestAnimationFrame` handle.
    frame: Option<i32>,
    /// (segment, word) of the last snapshot sent from the frame loop.
    shown_word: Option<(u32, Option<usize>)>,
    /// Last fetch error, kept until a later fetch succeeds.
    fetch_failure: Option<String>,
    /// The current build's fetch loop stopped on a final error.
    fetch_stopped: bool,
    /// Media Session skip lengths when the action carries no offset.
    skip_back_ms: i32,
    skip_forward_ms: i32,
    /// Last Media Session `playbackState` this player decided on (published only while it holds the session).
    session_playback: &'static str,
    /// What the lock screen shows for the loaded document, kept to publish again when this player takes the session.
    info: Option<MediaInfo>,
    /// (generation, probe) while a probe waits for the element's clock to start (see `Shared::watch_clock`).
    clock_watch: Option<(u64, ClockProbe)>,
}

impl State {
    fn new() -> State {
        State {
            phase: PlayerState::Idle,
            want_play: false,
            segment_count: 0,
            generation: 0,
            backend: Backend::Idle,
            timeline: Timeline::default(),
            target: None,
            then: Vec::new(),
            build_from: 0,
            lengths: Lengths::default(),
            words: BTreeMap::new(),
            held: None,
            wake_fetch: false,
            position_mark: None,
            reported_segment: None,
            finished: false,
            rate: 1.0,
            sleep: SleepTimer::default(),
            sleep_clock_ms: None,
            last_media_s: None,
            throttle: Throttle::new(SNAPSHOT_INTERVAL_MS),
            frame: None,
            shown_word: None,
            fetch_failure: None,
            fetch_stopped: false,
            skip_back_ms: seek::SKIP_MS,
            skip_forward_ms: seek::SKIP_MS,
            session_playback: "none",
            info: None,
            clock_watch: None,
        }
    }
}

impl Driver {
    pub(crate) fn new(provider: Provider, on_event: Rc<dyn Fn(PlayerEvent)>) -> Result<Driver, PlayerError> {
        let audio = js::create_hidden_audio().map_err(|err| {
            PlayerError::Unsupported(format!("no document to attach audio to: {}", js::error_message(&err)))
        })?;
        let use_mse = js::mse_supported(MSE_MIME);
        if !use_mse && audio.can_play_type(MSE_MIME).is_empty() {
            audio.remove();
            return Err(PlayerError::Unsupported("WebM/Opus audio".into()));
        }
        let shared = Rc::new_cyclic(|weak| Shared {
            provider,
            on_event,
            audio,
            use_mse,
            session_id: session::join(),
            st: RefCell::new(State::new()),
            hooks: Hooks {
                listeners: events::listeners(weak),
                frame: events::frame_callback(weak),
                actions: session::actions(weak),
            },
        });
        for (name, callback) in &shared.hooks.listeners {
            // Adding a listener to an element cannot fail for a valid callback.
            let _ = shared.audio.add_event_listener_with_callback(name, callback.as_ref().unchecked_ref());
        }
        // The Media Session is taken on load or play: the app creates the next document's player while the old one
        // still plays.
        Ok(Driver { shared })
    }

    pub(crate) fn load(&self, info: MediaInfo, segment_count: u32, start_segment: u32) {
        self.shared.load(info, segment_count, start_segment);
    }
    pub(crate) fn play(&self) {
        self.shared.play();
    }
    pub(crate) fn pause(&self) {
        self.shared.pause();
    }
    pub(crate) fn toggle(&self) {
        self.shared.toggle();
    }
    pub(crate) fn seek_by(&self, delta_ms: i32) {
        self.shared.seek_by(delta_ms);
    }
    pub(crate) fn jump_to(&self, segment: u32) {
        self.shared.jump_to(segment);
    }
    pub(crate) fn next(&self) {
        self.shared.next();
    }
    pub(crate) fn previous(&self) {
        self.shared.previous();
    }
    pub(crate) fn set_rate(&self, rate: f64) {
        self.shared.set_rate(rate);
    }
    pub(crate) fn set_sleep(&self, sleep: Sleep) {
        self.shared.set_sleep(sleep);
    }
    pub(crate) fn set_skip(&self, back_ms: u32, forward_ms: u32) {
        self.shared.set_skip(back_ms, forward_ms);
    }
    pub(crate) fn snapshot(&self) -> Snapshot {
        self.shared.snapshot_of(&self.shared.st.borrow())
    }
    pub(crate) fn unload(&self) {
        self.shared.unload();
    }
}

impl Drop for Shared {
    fn drop(&mut self) {
        for (name, callback) in &self.hooks.listeners {
            let _ = self.audio.remove_event_listener_with_callback(name, callback.as_ref().unchecked_ref());
        }
        // Only the holder clears the session: the app drops the old player after creating the next one.
        if session::leave(self.session_id) {
            session::uninstall(&self.hooks.actions);
            js::session_clear();
        }
        let st = self.st.get_mut();
        if let (Some(handle), Some(window)) = (st.frame.take(), web_sys::window()) {
            let _ = window.cancel_animation_frame(handle);
        }
        st.backend.teardown();
        js::detach_media(&self.audio);
        self.audio.remove();
    }
}

impl Shared {
    /// Runs `f` on the state, then delivers the events it produced once the borrow is released.
    fn update<R>(&self, f: impl FnOnce(&mut State, &mut Outbox) -> R) -> R {
        let mut out = Outbox::new();
        let result = f(&mut self.st.borrow_mut(), &mut out);
        for event in out {
            (self.on_event)(event);
        }
        result
    }

    fn is_current(&self, generation: u64) -> bool {
        self.st.borrow().generation == generation
    }

    /// Playhead on the build's media timeline.
    fn media_time(&self, st: &State) -> f64 {
        st.backend.current_time(&self.audio, &st.timeline)
    }

    fn snapshot_of(&self, st: &State) -> Snapshot {
        let placed = st.target.is_none();
        let t = self.media_time(st);
        let (segment, segment_pos_ms, word) =
            self.view(st).unwrap_or_else(|| (st.reported_segment.unwrap_or(0), 0, None));
        let buffered_ahead_ms = if placed { secs_to_ms(ahead_of(&st.backend.ranges(&st.timeline), t)) } else { 0 };
        Snapshot {
            state: st.phase.clone(),
            segment,
            segment_pos_ms,
            word,
            buffered_ahead_ms,
            rate: st.rate,
            sleep_left_ms: st.sleep.left_ms(),
        }
    }

    /// (segment, offset, word) the listener should see for media time `t` of this build.
    fn shown(&self, st: &State, t: f64) -> Option<View> {
        st.timeline.shown(t, st.segment_count)
    }

    /// Where the listener is: the playhead once placed. While a build loads, its target once the known lengths say
    /// exactly where that is (the segment [`Shared::announce_target`] announces), and before that the last position
    /// shown, so the read-outs and the highlight neither jump to the start of the old segment nor go blank.
    fn view(&self, st: &State) -> Option<View> {
        let Some((segment, offset)) = st.target else {
            return self.shown(st, self.media_time(st));
        };
        match seek::exact_offset(&st.lengths, segment, offset) {
            Some(offset_ms) => {
                let word = st.words.get(&segment).and_then(|words| word_at(words, offset_ms));
                Some((segment, offset_ms, word))
            }
            None => st.held,
        }
    }

    /// Keeps what the listener sees now for the loading snapshots that follow, before a command moves the target.
    fn hold_view(&self, st: &mut State) {
        if let Some(view) = self.view(st) {
            st.held = Some(view);
        }
    }

    /// Feeds the state machine; a change emits a snapshot and updates the Media Session.
    fn apply(&self, st: &mut State, out: &mut Outbox, input: Input) {
        let next = status::transition(&st.phase, input, st.want_play);
        if next == st.phase {
            // The phase may stay (a pause while a build loads) while what the listener asked for changed.
            self.publish_activity(st, out);
            return;
        }
        st.phase = next;
        st.session_playback = status::session_state(&st.phase, st.want_play);
        self.publish_playback(st);
        out.push(PlayerEvent::Snapshot(self.snapshot_of(st)));
    }

    fn holds_session(&self) -> bool {
        session::holds(self.session_id)
    }

    /// Takes the page's Media Session for this player. When it changes hands, installs this player's handlers and
    /// publishes its metadata, playback state and position over what the previous holder left; returns whether it did.
    fn take_session(&self, st: &mut State) -> bool {
        if !session::claim(self.session_id) {
            return false;
        }
        session::install(&self.hooks.actions);
        self.publish_metadata(st);
        self.publish_playback(st);
        self.update_position(st);
        true
    }

    fn publish_metadata(&self, st: &State) {
        if !self.holds_session() {
            return;
        }
        match &st.info {
            Some(info) => js::session_set_metadata(&info.title, &info.artist, &info.album, info.artwork_url.clone()),
            None => js::session_clear(),
        }
    }

    fn publish_playback(&self, st: &State) {
        if self.holds_session() {
            js::session_set_playback_state(st.session_playback);
        }
    }

    /// Tells the Media Session and the app when play or pause changed what the listener is about to hear without
    /// changing the phase.
    fn publish_activity(&self, st: &mut State, out: &mut Outbox) {
        let playback = status::session_state(&st.phase, st.want_play);
        if playback != st.session_playback {
            st.session_playback = playback;
            self.publish_playback(st);
            out.push(PlayerEvent::Snapshot(self.snapshot_of(st)));
        }
    }

    /// While a build loads: announces the segment its target lands in, once the known lengths say which one it is.
    fn announce_target(&self, st: &mut State, out: &mut Outbox) {
        if let Some((segment, offset)) = st.target
            && seek::exact_offset(&st.lengths, segment, offset).is_some()
        {
            self.report_segment(st, out, segment);
        }
    }

    /// Pauses the element, and keeps a source switch still loading on the fallback path from starting it.
    fn pause_audio(&self) {
        js::set_switch_autoplay(&self.audio, false);
        let _ = self.audio.pause();
    }

    /// Drops the Media Session position (a build is loading).
    fn clear_position(&self, st: &mut State) {
        st.position_mark = None;
        if self.holds_session() {
            js::session_clear_position();
        }
    }

    /// Announces `segment` if it differs from the last one announced.
    fn report_segment(&self, st: &mut State, out: &mut Outbox, segment: u32) {
        if st.reported_segment != Some(segment) {
            st.reported_segment = Some(segment);
            out.push(PlayerEvent::SegmentChanged(segment));
            self.update_position(st);
        }
    }

    /// Media Session position state for the segment under the playhead. Not while a build loads, nor between a
    /// placement and the element's clock starting (the session says "playing" then, so the lock screen would run
    /// ahead of silence): `playing` and the next `timeupdate`s publish it.
    fn update_position(&self, st: &mut State) {
        if st.target.is_some()
            || st.phase == PlayerState::Loading
            || (st.phase == PlayerState::Buffering && st.position_mark.is_none())
        {
            return;
        }
        let t = self.media_time(st);
        if let Some(entry) = st.timeline.locate(t).and_then(|pos| st.timeline.entries().get(pos.index)) {
            let (duration, position) = entry.window(t);
            if self.holds_session() {
                js::session_set_position(duration, position, st.rate);
            }
            st.position_mark = Some(PositionMark { wall_ms: js_sys::Date::now(), media_s: t, rate: st.rate });
        }
    }

    /// Publishes the position again when the lock screen's extrapolation has run away from the playhead: Chrome fires
    /// `playing` before the clock moves, a stall stops the clock, and a seek lands a little off its target.
    fn refresh_position(&self, st: &mut State, t: f64) {
        let running = st.session_playback == "playing";
        let stale = match st.position_mark {
            Some(mark) => mark.drifted(js_sys::Date::now(), running, t),
            None => st.phase == PlayerState::Playing,
        };
        if stale {
            self.update_position(st);
        }
    }
}
