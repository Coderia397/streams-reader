//! `GET /api/audio/{key}`: find a ready segment's R2 object. The Worker streams it with Range support; audio objects
//! never change once written (the key is content-addressed), so browsers may cache them for a year.

use nolabs_core::cache_key;
use nolabs_core::proto::ApiError;
use nolabs_core::proto::api::RenderState;

use crate::app::App;

pub const AUDIO_CONTENT_TYPE: &str = "audio/webm";
pub const AUDIO_CACHE_CONTROL: &str = "private, max-age=31536000, immutable";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AudioObject {
    pub object_key: String,
    /// Size recorded by the Space; None for rows written before it was known.
    pub bytes: Option<u64>,
}

impl App {
    pub async fn audio_object(&self, key: &str) -> Result<AudioObject, ApiError> {
        if !cache_key::is_key(key) {
            return Err(ApiError::not_found("no such audio"));
        }
        match self.repo().get_render(key).await? {
            Some(row) if row.state == RenderState::Ready => {
                Ok(AudioObject { object_key: row.object_key, bytes: row.bytes.filter(|b| *b > 0) })
            }
            Some(_) => Err(ApiError::not_found("this audio is not ready yet")),
            None => Err(ApiError::not_found("no such audio")),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testing::{TestEnv, run};
    use nolabs_core::proto::ErrorCode;

    #[test]
    fn only_ready_rows_resolve() {
        let env = TestEnv::new();
        let a = "a".repeat(64);
        let b = "b".repeat(64);
        env.db.exec(&format!(
            "INSERT INTO renders (key, engine, voice, state, object_key, chars, bytes, created_at_ms, updated_at_ms) \
             VALUES ('{a}', 'kokoro', 'kokoro:af_heart', 'ready', 'audio/kokoro/{a}.webm', 3, 1234, 0, 0), \
             ('{b}', 'kokoro', 'kokoro:af_heart', 'rendering', 'audio/kokoro/{b}.webm', 3, NULL, 0, 0)"
        ));
        let o = run(env.app.audio_object(&a)).unwrap();
        assert_eq!(o, AudioObject { object_key: format!("audio/kokoro/{a}.webm"), bytes: Some(1234) });
        assert_eq!(run(env.app.audio_object(&b)).unwrap_err().code, ErrorCode::NotFound);
        assert_eq!(run(env.app.audio_object(&"c".repeat(64))).unwrap_err().code, ErrorCode::NotFound);
        assert_eq!(run(env.app.audio_object("../x")).unwrap_err().code, ErrorCode::NotFound);
    }
}
