//! `/api/render` and `/api/render/status` (audio URLs are in [`super::audio`]).

use nolabs_core::proto::api::{RenderRequest, RenderResponse};

use super::ClientError;
use super::client::{encode_component, get_json, send_json};

pub async fn render(req: &RenderRequest) -> Result<RenderResponse, ClientError> {
    send_json("POST", "/api/render", req).await
}

pub async fn status(keys: &[String]) -> Result<RenderResponse, ClientError> {
    let joined = keys.iter().map(|k| encode_component(k)).collect::<Vec<_>>().join(",");
    get_json(&format!("/api/render/status?keys={joined}")).await
}
