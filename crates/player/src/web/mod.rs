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
use std::rc::Rc;

use wasm_bindgen::JsCast;
use wasm_bindgen::closure::Closure;
use web_sys::HtmlAudioElement;

use crate::buffer::ahead_of;
use crate::seek::Offset;
use crate::sleep::SleepTimer;
use crate::status::{self, Input, SNAPSHOT_INTERVAL_MS, Throttle};
use crate::timeline::{Timeline, secs_to_ms};
use crate::{MediaInfo, PlayerError, PlayerEvent, PlayerState, Provider, Sleep, Snapshot};
use backend::{Backend, MSE_MIME};

type Outbox = Vec<PlayerEvent>;
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
    /// Last fetch error, kept until a later fetch succeeds.
    fetch_failure: Option<String>,
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
            reported_segment: None,
            finished: false,
            rate: 1.0,
            sleep: SleepTimer::default(),
            sleep_clock_ms: None,
            last_media_s: None,
            throttle: Throttle::new(SNAPSHOT_INTERVAL_MS),
            frame: None,
            fetch_failure: None,
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
        session::install(&shared.hooks.actions);
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
        session::uninstall(&self.hooks.actions);
        js::session_clear();
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
        let (segment, segment_pos_ms, word) = match st.timeline.locate(t).filter(|_| placed) {
            Some(pos) => (pos.segment, pos.offset_ms, pos.word),
            None => (st.target.map(|(s, _)| s).or(st.reported_segment).unwrap_or(0), 0, None),
        };
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

    /// Feeds the state machine; a change emits a snapshot and updates the Media Session.
    fn apply(&self, st: &mut State, out: &mut Outbox, input: Input) {
        let next = status::transition(&st.phase, input, st.want_play);
        if next == st.phase {
            return;
        }
        st.phase = next;
        js::session_set_playback_state(match st.phase {
            PlayerState::Idle => "none",
            _ if status::is_active(&st.phase, st.want_play) => "playing",
            _ => "paused",
        });
        out.push(PlayerEvent::Snapshot(self.snapshot_of(st)));
    }

    /// Announces `segment` if it differs from the last one announced.
    fn report_segment(&self, st: &mut State, out: &mut Outbox, segment: u32) {
        if st.reported_segment != Some(segment) {
            st.reported_segment = Some(segment);
            out.push(PlayerEvent::SegmentChanged(segment));
            self.update_position(st);
        }
    }

    /// Media Session position state for the segment under the playhead.
    fn update_position(&self, st: &State) {
        if st.target.is_some() {
            return;
        }
        let t = self.media_time(st);
        if let Some(entry) = st.timeline.locate(t).and_then(|pos| st.timeline.entries().get(pos.index)) {
            let (duration, position) = entry.window(t);
            js::session_set_position(duration, position, st.rate);
        }
    }
}
