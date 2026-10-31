//! Reader state shared by the text and the overlays: follow mode and the tapped sentence. A single tap on a
//! sentence picks it and shows "Play from here" above it for a few seconds; a double tap plays at once.

use std::time::Duration;

use leptos::prelude::*;
use wasm_bindgen::JsCast;

use super::follow::Follow;
use crate::config::PLAY_HERE_MS;
use crate::state::player::PlayerBridge;
use crate::util::dom;
use crate::util::timer::now_ms;

const DOUBLE_TAP_MS: i64 = 350;

/// A sentence picked by a tap, with its box (viewport px) to place the pill.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Picked {
    pub segment: u32,
    pub top: f64,
    pub bottom: f64,
}

#[derive(Clone, Copy)]
pub struct ReaderCtx {
    pub follow: Follow,
    pub picked: RwSignal<Option<Picked>>,
    player: PlayerBridge,
    last_tap: StoredValue<(u32, i64)>,
}

fn has_selection() -> bool {
    dom::window().and_then(|w| w.get_selection().ok().flatten()).is_some_and(|s| !s.is_collapsed())
}

impl ReaderCtx {
    pub fn new(player: PlayerBridge) -> ReaderCtx {
        ReaderCtx {
            follow: Follow::new(),
            picked: RwSignal::new(None),
            player,
            last_tap: StoredValue::new((u32::MAX, 0)),
        }
    }

    pub fn tap(&self, idx: u32, e: &leptos::ev::MouseEvent) {
        if has_selection() {
            return;
        }
        let now = now_ms();
        let (last_idx, last_ms) = self.last_tap.get_value();
        self.last_tap.set_value((idx, now));
        if last_idx == idx && now - last_ms < DOUBLE_TAP_MS {
            self.play_from(idx);
            return;
        }
        let Some(el) = e.current_target().and_then(|t| t.dyn_into::<web_sys::Element>().ok()) else { return };
        let rect = el.get_bounding_client_rect();
        let picked = Picked { segment: idx, top: rect.top(), bottom: rect.bottom() };
        self.picked.set(Some(picked));
        let me = *self;
        set_timeout(
            move || {
                if me.picked.try_get_untracked() == Some(Some(picked)) {
                    let _ = me.picked.try_set(None);
                }
            },
            Duration::from_millis(PLAY_HERE_MS),
        );
    }

    pub fn play_from(&self, idx: u32) {
        self.picked.set(None);
        self.follow.resume();
        self.player.jump_to(idx);
    }
}

/// Where the pill goes: just above the sentence, or below it when that would hide under the top bar.
pub fn pill_top(p: &Picked, bar_height: f64) -> f64 {
    let above = p.top - 48.0;
    if above < bar_height + 8.0 { p.bottom + 8.0 } else { above }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pill_avoids_the_top_bar() {
        assert_eq!(pill_top(&Picked { segment: 0, top: 300.0, bottom: 340.0 }, 56.0), 252.0);
        assert_eq!(pill_top(&Picked { segment: 0, top: 80.0, bottom: 120.0 }, 56.0), 128.0);
    }
}
