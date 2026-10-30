//! Keeping the current sentence in view. The reader follows playback until the user scrolls by hand (touch or wheel;
//! plain `scroll` events also come from our own scrolling and from scroll anchoring, so they are not used), then shows
//! a "Back to current" pill.

use leptos::prelude::*;

use crate::util::dom;
use crate::util::timer::now_ms;

/// Fraction of the viewport (from the top) the current sentence may sit in before the view scrolls.
const COMFORT_TOP: f64 = 0.12;
const COMFORT_BOTTOM: f64 = 0.62;

#[derive(Clone, Copy)]
pub struct Follow {
    pub on: RwSignal<bool>,
    last_auto_ms: StoredValue<i64>,
}

/// Whether an element spanning `top..bottom` (viewport px) sits comfortably in a viewport of `height`.
pub fn comfortable(top: f64, bottom: f64, height: f64) -> bool {
    top >= height * COMFORT_TOP && bottom <= height * COMFORT_BOTTOM
}

impl Follow {
    pub fn new() -> Follow {
        Follow { on: RwSignal::new(true), last_auto_ms: StoredValue::new(0) }
    }

    pub fn resume(&self) {
        self.on.set(true);
    }

    /// Called on touch/wheel: the user took over.
    pub fn user_moved(&self) {
        if now_ms() - self.last_auto_ms.get_value() > 400 && self.on.get_untracked() {
            self.on.set(false);
        }
    }

    /// Scrolls segment `idx` to the centre when it is outside the comfortable band (or always when `force`).
    pub fn scroll_to(&self, idx: u32, force: bool) {
        let Some(el) = dom::segment_element(idx) else { return };
        let rect = el.get_bounding_client_rect();
        if !force && comfortable(rect.top(), rect.bottom(), dom::viewport_height()) {
            return;
        }
        self.last_auto_ms.set_value(now_ms());
        dom::scroll_into_center(&el, !dom::prefers_reduced_motion() && !force);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn comfort_band() {
        assert!(comfortable(200.0, 260.0, 800.0));
        assert!(!comfortable(50.0, 100.0, 800.0));
        assert!(!comfortable(450.0, 520.0, 800.0));
    }
}
