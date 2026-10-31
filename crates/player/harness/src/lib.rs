//! wasm side of the player test harness (see harness/README.md). Wraps [`nolabs_player::Player`] for a JS page: the
//! page hands in a provider function (segment -> Promise of {bytes, duration_ms, words, ends_section} or {error}) and
//! an event sink, and drives the player through the exported methods. Events and snapshots cross as plain objects.
//!
//! Built as an example of the player crate (`cargo build -p nolabs-player --example harness --target
//! wasm32-unknown-unknown`) so it needs no workspace changes; on the host it compiles to nothing.
#![cfg(target_arch = "wasm32")]

use std::rc::Rc;

use js_sys::{Array, Function, Object, Promise, Reflect, Uint8Array};
use nolabs_core::proto::api::WordTiming;
use nolabs_player::{MediaInfo, Player, PlayerError, PlayerEvent, PlayerState, Provider, Sleep, Snapshot, Track};
use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::JsFuture;

#[wasm_bindgen]
pub struct Harness {
    player: Player,
}

#[wasm_bindgen]
impl Harness {
    /// `provide(segment)` returns a Promise of a track object; `on_event(event)` receives every PlayerEvent.
    #[wasm_bindgen(constructor)]
    pub fn new(provide: Function, on_event: Function) -> Result<Harness, JsValue> {
        let provider: Provider = Rc::new(move |segment| {
            let provide = provide.clone();
            Box::pin(async move { fetch_track(&provide, segment).await })
        });
        let sink: Rc<dyn Fn(PlayerEvent)> = Rc::new(move |event| {
            let _ = on_event.call1(&JsValue::NULL, &event_value(&event));
        });
        let player = Player::new(provider, sink).map_err(|e| JsValue::from_str(&e.to_string()))?;
        Ok(Harness { player })
    }

    pub fn load(&self, segment_count: u32, start_segment: u32) {
        let info = MediaInfo {
            title: "Harness".into(),
            artist: "nolabs".into(),
            album: "player harness".into(),
            artwork_url: None,
        };
        self.player.load(info, segment_count, start_segment);
    }
    pub fn play(&self) {
        self.player.play();
    }
    pub fn pause(&self) {
        self.player.pause();
    }
    pub fn toggle(&self) {
        self.player.toggle();
    }
    pub fn seek_by(&self, delta_ms: i32) {
        self.player.seek_by(delta_ms);
    }
    pub fn jump_to(&self, segment: u32) {
        self.player.jump_to(segment);
    }
    pub fn next(&self) {
        self.player.next();
    }
    pub fn previous(&self) {
        self.player.previous();
    }
    pub fn set_rate(&self, rate: f64) {
        self.player.set_rate(rate);
    }
    /// `ms` = 0 turns the timer off; `section` selects end of section.
    pub fn set_sleep(&self, ms: u32, section: bool) {
        let sleep = if section {
            Sleep::EndOfSection
        } else if ms == 0 {
            Sleep::Off
        } else {
            Sleep::After { ms }
        };
        self.player.set_sleep(sleep);
    }
    pub fn set_skip(&self, back_ms: u32, forward_ms: u32) {
        self.player.set_skip(back_ms, forward_ms);
    }
    pub fn snapshot(&self) -> JsValue {
        snapshot_value(&self.player.snapshot())
    }
    pub fn unload(&self) {
        self.player.unload();
    }
}

async fn fetch_track(provide: &Function, segment: u32) -> Result<Track, PlayerError> {
    let promise: Promise = provide
        .call1(&JsValue::NULL, &JsValue::from(segment))
        .map_err(|e| PlayerError::Other(format!("{e:?}")))?
        .dyn_into()
        .map_err(|_| PlayerError::Other("provider did not return a promise".into()))?;
    let value = JsFuture::from(promise).await.map_err(|e| PlayerError::Load(format!("{e:?}")))?;
    let get = |key: &str| Reflect::get(&value, &JsValue::from_str(key)).unwrap_or(JsValue::UNDEFINED);
    if let Some(error) = get("error").as_string() {
        return Err(if get("fatal").is_truthy() { PlayerError::Other(error) } else { PlayerError::Load(error) });
    }
    let bytes: Uint8Array = get("bytes").dyn_into().map_err(|_| PlayerError::Other("bytes missing".into()))?;
    let duration_ms = get("duration_ms").as_f64().unwrap_or(0.0) as u32;
    let words = Array::from(&get("words"))
        .iter()
        .map(|w| {
            let w = Array::from(&w);
            let n = |i: u32| w.get(i).as_f64().unwrap_or(0.0) as u32;
            WordTiming(n(0), n(1), n(2), n(3))
        })
        .collect();
    Ok(Track { segment, bytes, duration_ms, words, ends_section: get("ends_section").is_truthy() })
}

fn object(pairs: &[(&str, JsValue)]) -> JsValue {
    let obj = Object::new();
    for (key, value) in pairs {
        let _ = Reflect::set(&obj, &JsValue::from_str(key), value);
    }
    obj.into()
}

fn state_name(state: &PlayerState) -> (&'static str, Option<String>) {
    match state {
        PlayerState::Idle => ("Idle", None),
        PlayerState::Loading => ("Loading", None),
        PlayerState::Buffering => ("Buffering", None),
        PlayerState::Playing => ("Playing", None),
        PlayerState::Paused => ("Paused", None),
        PlayerState::Ended => ("Ended", None),
        PlayerState::Error(message) => ("Error", Some(message.clone())),
    }
}

fn snapshot_value(s: &Snapshot) -> JsValue {
    let (state, error) = state_name(&s.state);
    let opt = |v: Option<u32>| v.map_or(JsValue::NULL, JsValue::from);
    object(&[
        ("state", JsValue::from_str(state)),
        ("error", error.map_or(JsValue::NULL, |e| JsValue::from_str(&e))),
        ("segment", JsValue::from(s.segment)),
        ("segment_pos_ms", JsValue::from(s.segment_pos_ms)),
        ("word", s.word.map_or(JsValue::NULL, |w| JsValue::from(w as u32))),
        ("buffered_ahead_ms", JsValue::from(s.buffered_ahead_ms)),
        ("rate", JsValue::from(s.rate)),
        ("sleep_left_ms", opt(s.sleep_left_ms)),
    ])
}

fn event_value(event: &PlayerEvent) -> JsValue {
    match event {
        PlayerEvent::Snapshot(s) => object(&[("type", "Snapshot".into()), ("snapshot", snapshot_value(s))]),
        PlayerEvent::SegmentChanged(segment) => {
            object(&[("type", "SegmentChanged".into()), ("segment", JsValue::from(*segment))])
        }
        PlayerEvent::Ended => object(&[("type", "Ended".into())]),
        PlayerEvent::SleepFired => object(&[("type", "SleepFired".into())]),
        PlayerEvent::Error(message) => object(&[("type", "Error".into()), ("message", JsValue::from_str(message))]),
    }
}
