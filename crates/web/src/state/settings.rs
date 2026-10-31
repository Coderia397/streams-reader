//! User preferences, saved to localStorage on every change. `assets/theme-boot.js` reads `theme` before the first
//! paint, so that field keeps its name and values; every other field defaults when missing, so older saves load.

use leptos::prelude::*;
use nolabs_core::voices::{DEFAULT_VOICE, VoiceRef};
use serde::{Deserialize, Serialize};

use crate::brand;
use crate::config::{
    RECENT_VOICES, SKIP_BACK_DEFAULT_S, SKIP_CHOICES_S, SKIP_FORWARD_DEFAULT_S, SPEED_MAX, SPEED_MIN,
    TEXT_SIZE_DEFAULT, TEXT_SIZE_MAX, TEXT_SIZE_MIN,
};
use crate::storage::prefs;
use crate::util::dom;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum Theme {
    #[default]
    Ink,
    Paper,
    /// True black for OLED screens at night.
    Night,
    System,
}

impl Theme {
    pub const ALL: [Theme; 4] = [Theme::Ink, Theme::Paper, Theme::Night, Theme::System];

    /// The token theme in effect (`ink`, `paper` or `night`); System follows the device.
    pub fn resolved(self) -> &'static str {
        match self {
            Theme::Ink => "ink",
            Theme::Paper => "paper",
            Theme::Night => "night",
            Theme::System => {
                if dom::media_matches("(prefers-color-scheme: light)") {
                    "paper"
                } else {
                    "ink"
                }
            }
        }
    }

    pub fn label(self) -> String {
        match self {
            Theme::Ink => t!("theme-ink"),
            Theme::Paper => t!("theme-paper"),
            Theme::Night => t!("theme-night"),
            Theme::System => t!("theme-system"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
struct Stored {
    voice: String,
    speed: f64,
    theme: Theme,
    skip_back_s: u32,
    skip_forward_s: u32,
    text_size: u32,
    favourites: Vec<String>,
    recent: Vec<String>,
}

impl Default for Stored {
    fn default() -> Self {
        Stored {
            voice: DEFAULT_VOICE.to_owned(),
            speed: 1.0,
            theme: Theme::Ink,
            skip_back_s: SKIP_BACK_DEFAULT_S,
            skip_forward_s: SKIP_FORWARD_DEFAULT_S,
            text_size: TEXT_SIZE_DEFAULT,
            favourites: Vec::new(),
            recent: Vec::new(),
        }
    }
}

/// Keeps a stored speed inside the supported range and on the 0.05 grid.
pub fn clamp_speed(speed: f64) -> f64 {
    if !speed.is_finite() {
        return 1.0;
    }
    ((speed.clamp(SPEED_MIN, SPEED_MAX) * 20.0).round()) / 20.0
}

fn valid_skip(s: u32, default: u32) -> u32 {
    if SKIP_CHOICES_S.contains(&s) { s } else { default }
}

/// Moves `id` to the front of a recent list, keeping at most [`RECENT_VOICES`].
pub fn push_recent(list: &mut Vec<String>, id: &str) {
    list.retain(|v| v != id);
    list.insert(0, id.to_owned());
    list.truncate(RECENT_VOICES);
}

#[derive(Clone, Copy)]
pub struct Settings {
    pub voice: RwSignal<String>,
    pub speed: RwSignal<f64>,
    pub theme: RwSignal<Theme>,
    pub skip_back_s: RwSignal<u32>,
    pub skip_forward_s: RwSignal<u32>,
    pub text_size: RwSignal<u32>,
    pub favourites: RwSignal<Vec<String>>,
    pub recent: RwSignal<Vec<String>>,
}

impl Settings {
    pub fn load() -> Settings {
        let st: Stored = prefs::get(prefs::SETTINGS).unwrap_or_default();
        let voice = if VoiceRef::parse(&st.voice).is_some() { st.voice } else { DEFAULT_VOICE.to_owned() };
        let s = Settings {
            voice: RwSignal::new(voice),
            speed: RwSignal::new(clamp_speed(st.speed)),
            theme: RwSignal::new(st.theme),
            skip_back_s: RwSignal::new(valid_skip(st.skip_back_s, SKIP_BACK_DEFAULT_S)),
            skip_forward_s: RwSignal::new(valid_skip(st.skip_forward_s, SKIP_FORWARD_DEFAULT_S)),
            text_size: RwSignal::new(st.text_size.clamp(TEXT_SIZE_MIN, TEXT_SIZE_MAX)),
            favourites: RwSignal::new(st.favourites),
            recent: RwSignal::new(st.recent),
        };
        Effect::new(move |_| {
            let stored = Stored {
                voice: s.voice.get(),
                speed: s.speed.get(),
                theme: s.theme.get(),
                skip_back_s: s.skip_back_s.get(),
                skip_forward_s: s.skip_forward_s.get(),
                text_size: s.text_size.get(),
                favourites: s.favourites.get(),
                recent: s.recent.get(),
            };
            prefs::set(prefs::SETTINGS, &stored);
            apply_theme(stored.theme);
        });
        // Follow the device when the theme is System.
        if let Some(mq) = dom::window().and_then(|w| w.match_media("(prefers-color-scheme: light)").ok().flatten()) {
            let cb = wasm_bindgen::closure::Closure::<dyn Fn()>::new(move || {
                if s.theme.get_untracked() == Theme::System {
                    apply_theme(Theme::System);
                }
            })
            .into_js_value();
            let _ = mq.add_event_listener_with_callback("change", wasm_bindgen::JsCast::unchecked_ref(&cb));
        }
        s
    }

    pub fn is_favourite(&self, id: &str) -> bool {
        self.favourites.with(|f| f.iter().any(|v| v == id))
    }

    pub fn toggle_favourite(&self, id: &str) {
        self.favourites.update(|f| {
            if f.iter().any(|v| v == id) {
                f.retain(|v| v != id);
            } else {
                f.push(id.to_owned());
            }
        });
    }

    /// Makes `id` the default voice and remembers it under Recent.
    pub fn use_voice(&self, id: &str) {
        self.voice.set(id.to_owned());
        self.recent.update(|r| push_recent(r, id));
    }
}

fn apply_theme(theme: Theme) {
    let resolved = theme.resolved();
    if let Some(root) = dom::html_element() {
        let _ = root.set_attribute("data-theme", resolved);
    }
    if let Some(meta) = dom::document().and_then(|d| d.query_selector("meta[name=theme-color]").ok().flatten()) {
        let _ = meta.set_attribute("content", &brand::theme_color(resolved, "bg"));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn speed_is_clamped_to_the_grid() {
        assert_eq!(clamp_speed(1.0), 1.0);
        assert_eq!(clamp_speed(1.27), 1.25);
        assert_eq!(clamp_speed(9.0), 3.5);
        assert_eq!(clamp_speed(0.1), 0.5);
        assert_eq!(clamp_speed(f64::NAN), 1.0);
    }

    #[test]
    fn older_saves_still_load() {
        let old: Stored =
            serde_json::from_str(r#"{"voice":"kokoro:af_heart","speed":1.25,"theme":"paper"}"#).unwrap_or_default();
        assert_eq!(old.theme, Theme::Paper);
        assert_eq!(old.speed, 1.25);
        assert_eq!(old.skip_forward_s, SKIP_FORWARD_DEFAULT_S);
        assert_eq!(old.text_size, TEXT_SIZE_DEFAULT);
        let json = serde_json::to_string(&Stored::default()).unwrap_or_default();
        assert!(json.starts_with(r#"{"voice":"kokoro:af_heart","speed":1.0,"theme":"ink","#), "{json}");
    }

    #[test]
    fn skips_outside_the_choices_fall_back() {
        assert_eq!(valid_skip(10, 15), 10);
        assert_eq!(valid_skip(7, 15), 15);
    }

    #[test]
    fn recent_keeps_the_latest_few() {
        let mut r = Vec::new();
        for id in ["a", "b", "c", "a", "d", "e"] {
            push_recent(&mut r, id);
        }
        assert_eq!(r, vec!["e", "d", "a", "c"]);
    }
}
