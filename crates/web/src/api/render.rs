//! `/api/render` and `/api/audio/{key}`.

use nolabs_core::proto::api::{RenderRequest, RenderResponse};

use super::ClientError;
use super::client::{encode_component, encode_segment, get_json, send_json};

pub async fn render(req: &RenderRequest) -> Result<RenderResponse, ClientError> {
    send_json("POST", "/api/render", req).await
}

pub async fn status(keys: &[String]) -> Result<RenderResponse, ClientError> {
    let joined = keys.iter().map(|k| encode_component(k)).collect::<Vec<_>>().join(",");
    get_json(&format!("/api/render/status?keys={joined}")).await
}

/// Same-origin URL of a rendered clip; also the Cache API key.
pub fn audio_url(key: &str) -> String {
    format!("/api/audio/{}", encode_segment(key))
}
