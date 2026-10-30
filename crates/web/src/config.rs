//! App-wide constants. Product naming lives in one place so the final name is a one-line change.

/// Display name of the product (working name; the user picks the final one later).
pub const APP_NAME: &str = "nolabs";

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
pub const SPEED_MAX: f64 = 3.0;
pub const SPEED_STEP: f64 = 0.05;
pub const SPEED_CHIPS: [f64; 4] = [1.0, 1.25, 1.5, 2.0];

/// Seek step for the ±15 s buttons.
pub const SEEK_STEP_MS: i32 = 15_000;

/// Sleep timer choices in minutes.
pub const SLEEP_CHOICES_MIN: [u32; 4] = [15, 30, 45, 60];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn manifest_and_page_title_use_the_app_name() {
        let manifest: serde_json::Value =
            serde_json::from_str(include_str!("../manifest.webmanifest")).unwrap_or_default();
        assert_eq!(manifest["name"], APP_NAME);
        assert_eq!(manifest["short_name"], APP_NAME);
        assert_eq!(manifest["share_target"]["method"], "GET");
        assert!(include_str!("../index.html").contains(&format!("<title>{APP_NAME}</title>")));
    }

    #[test]
    fn speed_chips_sit_on_the_slider_grid() {
        for chip in SPEED_CHIPS {
            assert!((SPEED_MIN..=SPEED_MAX).contains(&chip));
            let steps = (chip - SPEED_MIN) / SPEED_STEP;
            assert!((steps - steps.round()).abs() < 1e-9);
        }
    }
}
