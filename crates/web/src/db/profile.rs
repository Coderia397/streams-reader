//! `public.profiles`: the account's settings (language, default voice and speed, theme), shared by every device.

use nolabs_core::proto::api::ProfileRecord;
use serde::Serialize;

use super::{get, send, to_json};
use crate::api::ClientError;

pub async fn load() -> Result<Option<ProfileRecord>, ClientError> {
    let rows: Vec<ProfileRecord> =
        get("profiles?select=display_name,locale,default_voice,default_speed,theme,is_admin").await?;
    Ok(rows.into_iter().next())
}

/// The fields a settings change writes; `None` fields are left as they are.
#[derive(Debug, Default, Serialize)]
pub struct ProfilePatch {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub locale: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub default_voice: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub default_speed: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub theme: Option<String>,
}

/// Updates the caller's row (RLS would refuse any other).
pub async fn save(user_id: &str, patch: &ProfilePatch) -> Result<(), ClientError> {
    let path = format!("profiles?id=eq.{}", crate::api::client::encode_component(user_id));
    send("PATCH", &path, Some(&to_json(patch)?), Some("return=minimal")).await.map(|_| ())
}

/// The profile's speed column allows 0.5–3.0; the app offers up to 3.5 locally.
pub const PROFILE_SPEED_MAX: f32 = 3.0;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn patches_carry_only_what_changed() {
        let p = ProfilePatch { locale: Some("ar".into()), ..Default::default() };
        assert_eq!(serde_json::to_string(&p).unwrap_or_default(), r#"{"locale":"ar"}"#);
    }
}
