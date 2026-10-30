//! Media Session actions: lock screen, notification and headset buttons drive the same commands as the UI.

use std::rc::{Rc, Weak};

use wasm_bindgen::JsCast;
use wasm_bindgen::closure::Closure;

use super::{Actions, Shared, js};
use crate::seek::skip_ms;

/// Receives the number the action carries (seek offset or time in seconds, NaN when absent).
type Action = fn(&Rc<Shared>, f64);

pub(super) fn actions(weak: &Weak<Shared>) -> Actions {
    let table: [(&'static str, Action); 8] = [
        ("play", |s, _| s.play()),
        ("pause", |s, _| s.pause()),
        // Android's notification has no stop button; a headset or watch "stop" pauses so the place is kept.
        ("stop", |s, _| s.pause()),
        ("seekbackward", |s, offset_s| s.seek_by(-skip_ms(offset_s))),
        ("seekforward", |s, offset_s| s.seek_by(skip_ms(offset_s))),
        ("previoustrack", |s, _| s.previous()),
        ("nexttrack", |s, _| s.next()),
        ("seekto", |s, time_s| s.seek_in_segment(time_s)),
    ];
    table
        .into_iter()
        .map(|(name, action)| {
            let weak = weak.clone();
            let callback = Closure::<dyn FnMut(f64)>::new(move |value: f64| {
                if let Some(shared) = weak.upgrade() {
                    action(&shared, value);
                }
            });
            (name, callback)
        })
        .collect()
}

pub(super) fn install(actions: &Actions) {
    for (name, callback) in actions {
        // False when this browser does not know the action; the rest still work.
        let _ = js::session_set_action_handler(name, Some(callback.as_ref().unchecked_ref()));
    }
}

pub(super) fn uninstall(actions: &Actions) {
    for (name, _) in actions {
        let _ = js::session_set_action_handler(name, None);
    }
}
