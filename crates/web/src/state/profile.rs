//! The account's settings row (`public.profiles`) kept in step with this device: after sign-in the row's default
//! voice, speed and theme are applied; later changes here are written back. The interface language is pushed the
//! other way (this device's language wins) to both the row and the user's metadata, so auth emails follow it.

use leptos::prelude::*;
use leptos::task::spawn_local;
use nolabs_core::proto::api::ProfileRecord;
use nolabs_core::voices::VoiceRef;

use super::AppState;
use super::session::Auth;
use super::settings::{Theme, clamp_speed};
use crate::auth::{self, http};
use crate::db::profile::{self, PROFILE_SPEED_MAX, ProfilePatch};
use crate::i18n;

/// What this module last read from or wrote to the row; a change from it is written back.
#[derive(Debug, Clone, PartialEq)]
struct Synced {
    voice: String,
    speed: f64,
    theme: &'static str,
}

/// The row's `theme` (`system`, `light`, `dark`) for an app theme. Ink and Night are both dark.
pub fn theme_to_row(t: Theme) -> &'static str {
    match t {
        Theme::Ink | Theme::Night => "dark",
        Theme::Paper => "light",
        Theme::System => "system",
    }
}

/// The app theme for the row's value, keeping this device's dark variant (Ink or Night).
pub fn theme_from_row(row: &str, local: Theme) -> Theme {
    match row {
        "light" => Theme::Paper,
        "dark" if matches!(local, Theme::Ink | Theme::Night) => local,
        "dark" => Theme::Ink,
        _ => Theme::System,
    }
}

/// The row allows 0.5–3.0; a faster local speed stays local when the row holds the maximum.
pub fn speed_from_row(row: f32, local: f64) -> f64 {
    let row = f64::from(row);
    if row >= f64::from(PROFILE_SPEED_MAX) && local > row { local } else { clamp_speed(row) }
}

/// Writes the interface language to the row and the user's metadata.
pub async fn push_locale(code: &str) {
    if let (Some(token), Some(user_id)) = (auth::access_token(), user_id_now()) {
        let _ = http::update_locale(&token, code).await;
        let _ = profile::save(&user_id, &ProfilePatch { locale: Some(code.to_owned()), ..Default::default() }).await;
    }
}

thread_local! {
    static USER_ID: std::cell::RefCell<Option<String>> = const { std::cell::RefCell::new(None) };
}

fn user_id_now() -> Option<String> {
    USER_ID.with(|u| u.borrow().clone())
}

async fn pull(app: AppState, synced: StoredValue<Option<Synced>>) {
    let Ok(Some(row)) = profile::load().await else { return };
    apply(app, &row, synced);
    let here = i18n::current().code();
    if row.locale != here {
        push_locale(here).await;
    }
}

fn apply(app: AppState, row: &ProfileRecord, synced: StoredValue<Option<Synced>>) {
    let s = app.settings;
    let voice =
        row.default_voice.clone().filter(|v| VoiceRef::parse(v).is_some()).unwrap_or_else(|| s.voice.get_untracked());
    let speed = speed_from_row(row.default_speed, s.speed.get_untracked());
    let theme = theme_from_row(&row.theme, s.theme.get_untracked());
    synced.set_value(Some(Synced { voice: voice.clone(), speed, theme: theme_to_row(theme) }));
    s.voice.set(voice);
    s.speed.set(speed);
    s.theme.set(theme);
}

/// Starts the sync; call once from the app state.
pub fn start(app: AppState) {
    let synced = StoredValue::new(None::<Synced>);
    Effect::new(move |was_in: Option<bool>| {
        let signed_in = app.session.auth.get() == Auth::SignedIn;
        let id = app.session.user.with(|u| u.as_ref().map(|u| u.id.clone()));
        USER_ID.with(|u| *u.borrow_mut() = id);
        if signed_in && was_in != Some(true) {
            spawn_local(pull(app, synced));
        }
        if !signed_in {
            synced.set_value(None);
        }
        signed_in
    });
    Effect::new(move |_| {
        let now = Synced {
            voice: app.settings.voice.get(),
            speed: app.settings.speed.get(),
            theme: theme_to_row(app.settings.theme.get()),
        };
        let Some(last) = synced.get_value() else { return };
        let Some(user_id) = user_id_now() else { return };
        if last == now {
            return;
        }
        synced.set_value(Some(now.clone()));
        let patch = ProfilePatch {
            locale: None,
            default_voice: Some(now.voice),
            default_speed: Some(now.speed.min(f64::from(PROFILE_SPEED_MAX)) as f32),
            theme: Some(now.theme.to_owned()),
        };
        spawn_local(async move {
            let _ = profile::save(&user_id, &patch).await;
        });
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn themes_map_both_ways() {
        assert_eq!(theme_to_row(Theme::Night), "dark");
        assert_eq!(theme_from_row("dark", Theme::Night), Theme::Night);
        assert_eq!(theme_from_row("dark", Theme::Paper), Theme::Ink);
        assert_eq!(theme_from_row("light", Theme::Ink), Theme::Paper);
        assert_eq!(theme_from_row("system", Theme::Ink), Theme::System);
    }

    #[test]
    fn fast_local_speeds_survive_the_row_limit() {
        assert_eq!(speed_from_row(3.0, 3.5), 3.5);
        assert_eq!(speed_from_row(1.25, 3.5), 1.25);
        assert_eq!(speed_from_row(1.0, 1.5), 1.0);
    }
}
