//! Audio URL tokens: `<audio src>` and MediaSource segment fetches cannot carry `Authorization`, so the app mints a
//! token with its access token (`POST /api/audio-token`) and appends `?t=<token>` to audio URLs. The token is
//! `a1.<user_id>.<expiry unix s>.<base64url HMAC-SHA256>` under `AUDIO_URL_KEY`: bound to the user (logged, and a
//! leaked URL names whose it was), valid [`TTL_S`], and not bound to one key because renders are shared and
//! content-addressed. It cannot be used for anything but audio.

use base64::Engine as _;
use base64::engine::general_purpose::URL_SAFE_NO_PAD as B64;
use hmac::{Hmac, Mac};
use sha2::Sha256;

/// Six hours: a long listening session on one token; the app mints a new one before this runs out.
pub const TTL_S: i64 = 6 * 60 * 60;
const PREFIX: &str = "a1";
/// Domain separation from any other MAC made with the same key.
const MAC_TAG: &[u8] = b"nolabs-audio-url-v1|";

fn mac(key: &[u8], user_id: &str, exp_s: i64) -> Option<Hmac<Sha256>> {
    let mut m = Hmac::<Sha256>::new_from_slice(key).ok()?;
    m.update(MAC_TAG);
    m.update(user_id.as_bytes());
    m.update(b"|");
    m.update(exp_s.to_string().as_bytes());
    Some(m)
}

/// A token for `user_id` valid until `now_s + TTL_S`; None only if the key is unusable (never for HMAC).
pub fn mint(key: &[u8], user_id: &str, now_s: i64) -> Option<(String, i64)> {
    if user_id.is_empty() || user_id.contains('.') {
        return None;
    }
    let exp_s = now_s + TTL_S;
    let tag = mac(key, user_id, exp_s)?.finalize().into_bytes();
    Some((format!("{PREFIX}.{user_id}.{exp_s}.{}", B64.encode(tag)), exp_s))
}

/// The user id of a valid, unexpired token.
pub fn verify(key: &[u8], token: &str, now_s: i64) -> Option<String> {
    let mut parts = token.split('.');
    let (Some(PREFIX), Some(user_id), Some(exp), Some(tag), None) =
        (parts.next(), parts.next(), parts.next(), parts.next(), parts.next())
    else {
        return None;
    };
    let exp_s: i64 = exp.parse().ok()?;
    if exp_s <= now_s || user_id.is_empty() {
        return None;
    }
    mac(key, user_id, exp_s)?.verify_slice(&B64.decode(tag).ok()?).ok()?;
    Some(user_id.to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    const KEY: &[u8] = &[3; 32];
    const NOW: i64 = 1_793_448_000;
    const USER: &str = "088b18e9-086a-4c8e-94b3-97c8fce220e2";

    #[test]
    fn minted_tokens_verify_until_they_expire() {
        let (t, exp) = mint(KEY, USER, NOW).unwrap();
        assert_eq!(exp, NOW + TTL_S);
        assert!(t.starts_with(&format!("a1.{USER}.{exp}.")));
        assert_eq!(verify(KEY, &t, NOW).as_deref(), Some(USER));
        assert_eq!(verify(KEY, &t, exp - 1).as_deref(), Some(USER));
        assert_eq!(verify(KEY, &t, exp), None);
    }

    #[test]
    fn forged_or_altered_tokens_fail() {
        let (t, exp) = mint(KEY, USER, NOW).unwrap();
        assert_eq!(verify(&[4; 32], &t, NOW), None, "another key");
        let other_user = t.replace(USER, "11111111-1111-1111-1111-111111111111");
        assert_eq!(verify(KEY, &other_user, NOW), None, "user swapped");
        let longer = t.replace(&exp.to_string(), &(exp + 3600).to_string());
        assert_eq!(verify(KEY, &longer, NOW), None, "expiry extended");
        for bad in ["", "a1", "a1.u.1.x", "a2.u.99999999999.AAAA", &format!("{t}.x")] {
            assert_eq!(verify(KEY, bad, NOW), None, "{bad}");
        }
        assert!(mint(KEY, "", NOW).is_none() && mint(KEY, "a.b", NOW).is_none());
    }
}
