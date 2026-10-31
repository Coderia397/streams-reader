//! Media Session actions: lock screen, notification and headset buttons drive the same commands as the UI.

use std::cell::RefCell;
use std::rc::{Rc, Weak};

use wasm_bindgen::JsCast;
use wasm_bindgen::closure::Closure;

use super::{Actions, Shared, js};
use crate::seat::Seat;

thread_local! {
    /// The page has one Media Session; this says which player it belongs to (see [`crate::seat`]).
    static SEAT: RefCell<Seat> = RefCell::new(Seat::default());
}

/// A new player's id; it does not hold the session until [`claim`].
pub(super) fn join() -> u64 {
    SEAT.with(|seat| seat.borrow_mut().join())
}

/// Gives the session to player `id`; true when it changed hands.
pub(super) fn claim(id: u64) -> bool {
    SEAT.with(|seat| seat.borrow_mut().claim(id))
}

pub(super) fn holds(id: u64) -> bool {
    SEAT.with(|seat| seat.borrow().holds(id))
}

/// Player `id` is going away; true when it held the session.
pub(super) fn leave(id: u64) -> bool {
    SEAT.with(|seat| seat.borrow_mut().leave(id))
}

/// Receives the number the action carries (seek offset or time in seconds, NaN when absent).
type Action = fn(&Rc<Shared>, f64);

pub(super) fn actions(weak: &Weak<Shared>) -> Actions {
    let table: [(&'static str, Action); 8] = [
        ("play", |s, _| s.play()),
        ("pause", |s, _| s.pause()),
        // Android's notification has no stop button; a headset or watch "stop" pauses so the place is kept.
        ("stop", |s, _| s.pause()),
        ("seekbackward", |s, offset_s| s.media_skip(offset_s, false)),
        ("seekforward", |s, offset_s| s.media_skip(offset_s, true)),
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
