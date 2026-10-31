//! Keeping the spoken line in view. While following, the view scrolls so the current word's line sits 30–45 % from
//! the top. A manual scroll (touch or wheel; plain `scroll` events also come from our own scrolling, so they are not
//! used) hands control to the reader, and a "Back to voice" pill appears.

use leptos::prelude::*;

use crate::util::dom;
use crate::util::timer::now_ms;

/// The band (fractions of the viewport height, from the top) the current line is kept in.
const BAND_TOP: f64 = 0.30;
const BAND_BOTTOM: f64 = 0.45;
/// Where a scroll puts the line.
const TARGET: f64 = 0.32;

#[derive(Clone, Copy)]
pub struct Follow {
    pub on: RwSignal<bool>,
    last_auto_ms: StoredValue<i64>,
}

/// How far to scroll (px, positive = down) so a line whose top is at `top` sits in the band; `None` when it is
/// already there. Lines far below (more than a screen) are handled the same way: the jump is instant then.
pub fn correction(top: f64, height: f64) -> Option<f64> {
    if top >= height * BAND_TOP && top <= height * BAND_BOTTOM { None } else { Some(top - height * TARGET) }
}

impl Follow {
    pub fn new() -> Follow {
        Follow { on: RwSignal::new(true), last_auto_ms: StoredValue::new(0) }
    }

    pub fn resume(&self) {
        self.on.set(true);
    }

    /// Called on touch/wheel: the reader took over.
    pub fn user_moved(&self) {
        if now_ms() - self.last_auto_ms.get_value() > 400 && self.on.get_untracked() {
            self.on.set(false);
        }
    }

    /// Brings the current word (or, before timings arrive, segment `idx`) into the band.
    pub fn keep_in_band(&self, idx: u32, force: bool) {
        let el = dom::query(".reading .word").or_else(|| dom::segment_element(idx).map(Into::into));
        let Some(el) = el else { return };
        let top = el.get_bounding_client_rect().top();
        let height = dom::viewport_height();
        let Some(dy) = correction(top, height) else { return };
        if !force && dy.abs() < 2.0 {
            return;
        }
        // Runs from timers, possibly after the reader closed.
        if self.last_auto_ms.try_set_value(now_ms()).is_some() {
            return;
        }
        let smooth = !force && !dom::prefers_reduced_motion() && dy.abs() < height * 1.5;
        dom::scroll_by(dy, smooth);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn band() {
        assert_eq!(correction(300.0, 800.0), None);
        assert_eq!(correction(360.0, 800.0), None);
        assert_eq!(correction(400.0, 800.0), Some(400.0 - 256.0));
        assert_eq!(correction(100.0, 800.0), Some(100.0 - 256.0));
    }
}
