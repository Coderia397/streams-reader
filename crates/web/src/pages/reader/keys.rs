//! Desktop keyboard shortcuts in the reader. Ignored while typing in a field or with a modifier held.

use leptos::ev;
use leptos::prelude::*;

use crate::config::SPEED_STEP;
use crate::state::AppState;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    Toggle,
    Back,
    Forward,
    Slower,
    Faster,
    PrevParagraph,
    NextParagraph,
    Follow,
    Search,
}

/// The action for a key (`KeyboardEvent.key`).
pub fn action(key: &str) -> Option<Action> {
    Some(match key {
        " " | "k" | "K" => Action::Toggle,
        "j" | "J" | "ArrowLeft" => Action::Back,
        "l" | "L" | "ArrowRight" => Action::Forward,
        "[" => Action::Slower,
        "]" => Action::Faster,
        "ArrowUp" => Action::PrevParagraph,
        "ArrowDown" => Action::NextParagraph,
        "f" | "F" => Action::Follow,
        "/" => Action::Search,
        _ => return None,
    })
}

/// Shortcut list for the help sheet: (keys, message key).
pub const HELP: [(&str, &str); 6] = [
    ("Space · K", "keys-toggle"),
    ("J · L", "keys-skip"),
    ("[ · ]", "keys-speed"),
    ("↑ · ↓", "keys-paragraph"),
    ("F", "keys-follow"),
    ("/", "keys-search"),
];

fn typing(e: &web_sys::KeyboardEvent) -> bool {
    use wasm_bindgen::JsCast;
    e.target().and_then(|t| t.dyn_into::<web_sys::Element>().ok()).is_some_and(|el| {
        matches!(el.tag_name().as_str(), "INPUT" | "TEXTAREA" | "SELECT")
            || el.closest("dialog, [role=dialog]").ok().flatten().is_some()
    })
}

/// Installs the listener for the reader's lifetime. `on_follow` and `on_search` come from the page.
pub fn install(app: AppState, on_follow: impl Fn() + 'static, on_search: impl Fn() + 'static) {
    let handle = window_event_listener(ev::keydown, move |e| {
        if e.ctrl_key() || e.meta_key() || e.alt_key() || typing(&e) {
            return;
        }
        let Some(a) = action(&e.key()) else { return };
        e.prevent_default();
        let player = app.player;
        let (back, fwd) =
            (app.settings.skip_back_s.get_untracked() as i32, app.settings.skip_forward_s.get_untracked() as i32);
        match a {
            Action::Toggle => player.toggle(),
            // The timeline runs left to right in every language (see `_player.scss`), so the arrows do too.
            Action::Back => player.skip(-back),
            Action::Forward => player.skip(fwd),
            Action::Slower | Action::Faster => {
                let step = if a == Action::Faster { 2.0 * SPEED_STEP } else { -2.0 * SPEED_STEP };
                player.set_rate(player.rate.get_untracked() + step);
                app.settings.speed.set(player.rate.get_untracked());
            }
            Action::PrevParagraph => player.previous(),
            Action::NextParagraph => player.next(),
            Action::Follow => on_follow(),
            Action::Search => on_search(),
        }
    });
    on_cleanup(move || handle.remove());
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keys_map_to_actions() {
        assert_eq!(action(" "), Some(Action::Toggle));
        assert_eq!(action("J"), Some(Action::Back));
        assert_eq!(action("]"), Some(Action::Faster));
        assert_eq!(action("ArrowDown"), Some(Action::NextParagraph));
        assert_eq!(action("x"), None);
    }

    #[test]
    fn help_lines_are_translated() {
        for (_, key) in HELP {
            assert_ne!(crate::i18n::tr(key, &[]), key);
        }
    }
}
