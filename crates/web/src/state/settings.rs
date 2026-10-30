//! User preferences: default voice, default speed, theme. Saved to localStorage on every change; the theme is also
//! read by `assets/theme-boot.js` before the first paint, so the stored shape must stay `{voice, speed, theme}`.

use leptos::prelude::*;
use nolabs_core::voices::{DEFAULT_VOICE, VoiceRef};
use serde::{Deserialize, Serialize};

use crate::config::{SPEED_MAX, SPEED_MIN};
use crate::storage::prefs;
use crate::util::dom;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum Theme {
    #[default]
    Ink,
    Paper,
    System,
}

impl Theme {
    pub fn attr(self) -> &'static str {
        match self {
            Theme::Ink => "ink",
            Theme::Paper => "paper",
            Theme::System => "system",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Theme::Ink => "Ink",
            Theme::Paper => "Paper",
            Theme::System => "System",
        }
    }

    /// Browser chrome colour (status bar) for this theme.
    fn chrome_colour(self) -> &'static str {
        let paper = match self {
            Theme::Ink => false,
            Theme::Paper => true,
            Theme::System => dom::media_matches("(prefers-color-scheme: light)"),
        };
        if paper { "#F6F2EA" } else { "#0B0B0F" }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
struct Stored {
    voice: String,
    speed: f64,
    theme: Theme,
}

impl Default for Stored {
    fn default() -> Self {
        Stored { voice: DEFAULT_VOICE.to_owned(), speed: 1.0, theme: Theme::Ink }
    }
}

/// Keeps a stored speed inside the supported range and on the 0.05 grid.
pub fn clamp_speed(speed: f64) -> f64 {
    if !speed.is_finite() {
        return 1.0;
    }
    ((speed.clamp(SPEED_MIN, SPEED_MAX) * 20.0).round()) / 20.0
}

#[derive(Clone, Copy)]
pub struct Settings {
    pub voice: RwSignal<String>,
    pub speed: RwSignal<f64>,
    pub theme: RwSignal<Theme>,
}

impl Settings {
    pub fn load() -> Settings {
        let stored: Stored = prefs::get(prefs::SETTINGS).unwrap_or_default();
        let voice = if VoiceRef::parse(&stored.voice).is_some() { stored.voice } else { DEFAULT_VOICE.to_owned() };
        let s = Settings {
            voice: RwSignal::new(voice),
            speed: RwSignal::new(clamp_speed(stored.speed)),
            theme: RwSignal::new(stored.theme),
        };
        Effect::new(move |_| {
            let stored = Stored { voice: s.voice.get(), speed: s.speed.get(), theme: s.theme.get() };
            prefs::set(prefs::SETTINGS, &stored);
            apply_theme(stored.theme);
        });
        s
    }
}

fn apply_theme(theme: Theme) {
    if let Some(root) = dom::html_element() {
        let _ = root.set_attribute("data-theme", theme.attr());
    }
    if let Some(meta) = dom::document().and_then(|d| d.query_selector("meta[name=theme-color]").ok().flatten()) {
        let _ = meta.set_attribute("content", theme.chrome_colour());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn speed_is_clamped_to_the_grid() {
        assert_eq!(clamp_speed(1.0), 1.0);
        assert_eq!(clamp_speed(1.27), 1.25);
        assert_eq!(clamp_speed(9.0), 3.0);
        assert_eq!(clamp_speed(0.1), 0.5);
        assert_eq!(clamp_speed(f64::NAN), 1.0);
    }

    #[test]
    fn stored_shape_is_stable() {
        let json = serde_json::to_string(&Stored::default()).unwrap_or_default();
        assert_eq!(json, r#"{"voice":"kokoro:af_heart","speed":1.0,"theme":"ink"}"#);
    }
}
