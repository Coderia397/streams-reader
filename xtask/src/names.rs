//! Resource names and the ownership guard. The Cloudflare account also holds the user's production Workers, bucket
//! and zone (note 04), so every mutating request names its target and [`guard`] refuses anything this project does not
//! own. The guard panics instead of returning an error: reaching it with a foreign name is a bug in this tool, and no
//! caller may catch and continue past it.

pub const WORKER: &str = "nolabs-api";
pub const D1_DATABASE: &str = "nolabs";
pub const QUEUE: &str = "nolabs-render";
pub const BUCKET: &str = "nolabs-media";
pub const DEPLOY_TOKEN: &str = "nolabs-deploy";

pub const SPACE_REPO: &str = "ibrahimar397/nolabs-engine";
pub const SPACE_URL: &str = "https://ibrahimar397-nolabs-engine.hf.space";
pub const KOKORO_REPO: &str = "hexgrad/Kokoro-82M";
pub const MING_REPO: &str = "inclusionAI/Ming-3.0-tts-0.5B";

pub const COMPATIBILITY_DATE: &str = "2026-10-01";
pub const CRON: &str = "*/30 * * * *";
/// Lifecycle: rendered Kokoro audio is cheap to re-render, so it expires; Ming audio and voices are kept.
pub const KOKORO_AUDIO_PREFIX: &str = "audio/kokoro/";
pub const KOKORO_AUDIO_MAX_AGE_DAYS: u64 = 90;

/// True for `nolabs` and `nolabs-<something>` (case-sensitive).
pub fn is_owned(name: &str) -> bool {
    name == "nolabs" || name.strip_prefix("nolabs-").is_some_and(|rest| !rest.is_empty())
}

/// Panics unless `name` is a resource this project owns. Call before every mutating request.
#[expect(clippy::panic, reason = "a foreign target is a bug; nothing may catch it and carry on (see module docs)")]
pub fn guard(name: &str) {
    if !is_owned(name) {
        panic!("refusing to modify {name:?}: only resources named \"nolabs\" or \"nolabs-*\" may be changed");
    }
}

/// The repository name of a Hugging Face `owner/name` id, which is what the guard checks for Spaces.
pub fn repo_name(repo_id: &str) -> &str {
    repo_id.rsplit_once('/').map_or(repo_id, |(_, name)| name)
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use super::*;

    #[test]
    fn owned_names() {
        for name in ["nolabs", "nolabs-api", "nolabs-render", "nolabs-media", "nolabs-deploy", "nolabs-engine"] {
            assert!(is_owned(name), "{name}");
        }
    }

    #[test]
    fn foreign_names() {
        for name in [
            "",
            "nolabs-",
            "Nolabs-api",
            "NOLABS",
            "nolabsx",
            "nolab-api",
            "streams-api",
            "pstream-data",
            "x-nolabs-api",
            " nolabs",
        ] {
            assert!(!is_owned(name), "{name:?}");
        }
    }

    #[test]
    fn guard_accepts_owned_names() {
        guard("nolabs");
        guard(WORKER);
        guard(QUEUE);
        guard(BUCKET);
        guard(D1_DATABASE);
        guard(DEPLOY_TOKEN);
        guard(repo_name(SPACE_REPO));
    }

    #[test]
    #[should_panic(expected = "refusing to modify \"pstream-data\"")]
    fn guard_panics_on_foreign_bucket() {
        guard("pstream-data");
    }

    #[test]
    #[should_panic(expected = "refusing to modify")]
    fn guard_panics_on_bare_prefix() {
        guard("nolabs-");
    }

    #[test]
    fn repo_name_takes_the_part_after_the_owner() {
        assert_eq!(repo_name("ibrahimar397/nolabs-engine"), "nolabs-engine");
        assert_eq!(repo_name("ibrahimar397/streams-engine"), "streams-engine");
        assert_eq!(repo_name("bare"), "bare");
    }
}
