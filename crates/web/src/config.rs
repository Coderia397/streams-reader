//! App-wide constants. Product naming, paths and the account service live here, so each is a one-line change.

/// Display name of the product. User-visible text takes it from the `app-name` message; this copy is for places
/// outside the translations (Media Session, document title).
pub const APP_NAME: &str = "Streams Reader";

/// The app is served under this path; the landing site owns `/`. API calls stay absolute (`/api/…`).
pub const BASE: &str = "/app";

/// An app asset by its path under `assets/` (`asset("brand/icons/icon-512.png")` → `/app/assets/brand/icons/icon-512.png`).
pub fn asset(path: &str) -> String {
    format!("{BASE}/assets/{path}")
}

/// Supabase project (accounts). The publishable key is public by design: it only identifies the project, and row
/// level security guards the data. A local build can point at a mock with `STREAMS_READER_AUTH_URL`.
pub const SUPABASE_URL: &str = match option_env!("STREAMS_READER_AUTH_URL") {
    Some(url) => url,
    None => "https://atxbmgfuxjtcwmoaldqy.supabase.co",
};
pub const SUPABASE_KEY: &str = "sb_publishable_94EdeUv6Ybzk0W4SWPgg8g_Zabm8KDV";

/// Source code, shown in Settings → About.
pub const SOURCE_URL: &str = "https://github.com/Coderia397/streams-reader";
/// Legal pages on the landing site, same origin (the canonical host moved to reader.streams.watch, D34).
pub const PRIVACY_URL: &str = "/privacy";
pub const TERMS_URL: &str = "/terms";

/// Average narration speed at 1x, in chars per second. Kokoro reads about 150 words a minute; used only for estimates
/// (minutes left, scrubber), never for timing playback.
pub const CHARS_PER_SECOND: f64 = 15.0;

/// Items per `POST /api/render` call while playing.
pub const RENDER_WINDOW: u32 = 12;
/// Segments rendered in the background right after an import, so the first press of play starts quickly.
pub const PRERENDER_SEGMENTS: usize = 20;

/// Cache API cache holding fetched audio clips, keyed by `/api/audio/{key}`.
pub const AUDIO_CACHE: &str = "nolabs-audio-v1";

/// Local progress is written this often while playing.
pub const PROGRESS_LOCAL_MS: u32 = 5_000;
/// Progress is synced to the Worker this often while playing (and on pause / when hidden).
pub const PROGRESS_SYNC_MS: u32 = 30_000;

/// Sentence used for voice previews; long enough to hear the voice, short enough to render in a few seconds.
pub const PREVIEW_TEXT: &str = "The rain had stopped by the time she reached the station, and for a moment the whole city seemed to hold its breath.";

pub const SPEED_MIN: f64 = 0.5;
pub const SPEED_MAX: f64 = 3.5;
pub const SPEED_STEP: f64 = 0.05;
pub const SPEED_CHIPS: [f64; 6] = [0.8, 1.0, 1.2, 1.5, 2.0, 2.5];

/// Skip lengths offered for the back and forward buttons, in seconds (each has a matching Remix icon).
pub const SKIP_CHOICES_S: [u32; 4] = [5, 10, 15, 30];
pub const SKIP_BACK_DEFAULT_S: u32 = 15;
pub const SKIP_FORWARD_DEFAULT_S: u32 = 30;

/// Sleep timer choices in minutes.
pub const SLEEP_CHOICES_MIN: [u32; 7] = [5, 10, 15, 30, 45, 60, 90];

/// Reading text sizes in px (default 19).
pub const TEXT_SIZE_MIN: u32 = 16;
pub const TEXT_SIZE_MAX: u32 = 28;
pub const TEXT_SIZE_DEFAULT: u32 = 19;

/// Voices remembered under "Recent".
pub const RECENT_VOICES: usize = 4;

/// While playing, the reader's dock folds to a slim strip after this long without a touch.
pub const DOCK_FOLD_MS: u64 = 4_000;
/// How long the "Play from here" pill stays after a tap on a sentence.
pub const PLAY_HERE_MS: u64 = 4_000;
/// A library row still not ready this long after its last update is shown as an unfinished upload (Retry, Delete)
/// instead of "Preparing…".
pub const UPLOAD_STALE_MS: i64 = 120_000;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn manifest_and_page_use_the_app_name_and_base() {
        let manifest: serde_json::Value =
            serde_json::from_str(include_str!("../manifest.webmanifest")).unwrap_or_default();
        assert_eq!(manifest["name"], APP_NAME);
        assert_eq!(manifest["short_name"], "Reader");
        assert_eq!(manifest["start_url"], "/app/");
        assert_eq!(manifest["scope"], "/app/");
        assert_eq!(manifest["share_target"]["method"], "GET");
        assert_eq!(manifest["share_target"]["action"], "/app/");
        assert!(include_str!("../index.html").contains(&format!("<title>{APP_NAME}</title>")));
        assert!(include_str!("../Trunk.toml").contains("public_url = \"/app/\""));
        for icon in manifest["icons"].as_array().into_iter().flatten() {
            let src = icon["src"].as_str().unwrap_or_default();
            let file = src.rsplit('/').next().unwrap_or_default();
            assert!(src.starts_with("/app/assets/brand/icons/"), "{src}");
            assert!(
                include_str!("../index.html").contains(&format!("brand/dist/web/{file}\"")),
                "{file} is not copied"
            );
        }
    }

    #[test]
    fn paths() {
        assert_eq!(asset("brand/icons/icon-512.png"), "/app/assets/brand/icons/icon-512.png");
    }

    #[test]
    fn speed_chips_sit_on_the_slider_grid() {
        for chip in SPEED_CHIPS {
            assert!((SPEED_MIN..=SPEED_MAX).contains(&chip));
            let steps = (chip - SPEED_MIN) / SPEED_STEP;
            assert!((steps - steps.round()).abs() < 1e-9);
        }
    }

    #[test]
    fn skip_defaults_are_offered() {
        assert!(SKIP_CHOICES_S.contains(&SKIP_BACK_DEFAULT_S));
        assert!(SKIP_CHOICES_S.contains(&SKIP_FORWARD_DEFAULT_S));
    }
}
