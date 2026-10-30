//! Typed calls to the speech Space (contract v1, [`nolabs_core::proto::engine`]) and to the Hugging Face runtime API
//! that reports and restarts it.

use nolabs_core::engine_error::EngineError;
use nolabs_core::proto::api::Engine;
use nolabs_core::proto::engine::{EngineHealth, EngineResponse, PrepareReferenceRequest, PrepareReferenceResponse};
use serde::de::DeserializeOwned;

use crate::gradio::GradioClient;
use crate::ports::{Http, HttpRequest, Method};

const HUB_API: &str = "https://huggingface.co/api/spaces";

pub struct SpaceClient<'a> {
    http: &'a dyn Http,
    gradio: GradioClient<'a>,
    token: &'a str,
}

fn decode<T: DeserializeOwned>(api: &str, json: &str) -> Result<T, EngineError> {
    serde_json::from_str(json).map_err(|e| EngineError::Internal(format!("{api} returned an unexpected body: {e}")))
}

impl<'a> SpaceClient<'a> {
    pub fn new(http: &'a dyn Http, base_url: &'a str, token: &'a str) -> Self {
        SpaceClient { http, gradio: GradioClient::new(http, base_url, token), token }
    }

    /// Run a stored [`nolabs_core::proto::engine::EngineRequest`] (sent verbatim) on the engine's endpoint.
    pub async fn render(&self, engine: Engine, request_json: &str) -> Result<EngineResponse, EngineError> {
        let api = match engine {
            Engine::Kokoro => "render_cpu",
            Engine::Ming => "render_gpu",
        };
        decode(api, &self.gradio.call(api, request_json).await?)
    }

    pub async fn prepare_reference(
        &self,
        req: &PrepareReferenceRequest,
    ) -> Result<PrepareReferenceResponse, EngineError> {
        let json = serde_json::to_string(req).map_err(|e| EngineError::Internal(e.to_string()))?;
        decode("prepare_reference", &self.gradio.call("prepare_reference", &json).await?)
    }

    pub async fn health(&self) -> Result<EngineHealth, EngineError> {
        decode("health", &self.gradio.call("health", "{}").await?)
    }

    /// The Space's runtime stage (`RUNNING`, `SLEEPING`, `BUILDING`, `PAUSED`, `RUNTIME_ERROR`, …).
    pub async fn runtime_stage(&self, repo: &str) -> Result<String, EngineError> {
        let req = HttpRequest::new(Method::Get, format!("{HUB_API}/{repo}/runtime"))
            .header("authorization", format!("Bearer {}", self.token))
            .timeout_ms(15_000);
        let res = self.http.send(req).await.map_err(|e| EngineError::Internal(e.to_string()))?;
        if !(200..300).contains(&res.status) {
            return Err(EngineError::classify(Some(res.status), &String::from_utf8_lossy(&res.body)));
        }
        let value: serde_json::Value = serde_json::from_slice(&res.body)
            .map_err(|e| EngineError::Internal(format!("runtime API returned an unexpected body: {e}")))?;
        value
            .get("stage")
            .and_then(|s| s.as_str())
            .map(str::to_owned)
            .ok_or_else(|| EngineError::Internal("runtime API response has no stage".into()))
    }

    pub async fn restart(&self, repo: &str) -> Result<(), EngineError> {
        let req = HttpRequest::new(Method::Post, format!("{HUB_API}/{repo}/restart"))
            .header("authorization", format!("Bearer {}", self.token))
            .timeout_ms(15_000);
        let res = self.http.send(req).await.map_err(|e| EngineError::Internal(e.to_string()))?;
        if (200..300).contains(&res.status) {
            Ok(())
        } else {
            Err(EngineError::classify(Some(res.status), &String::from_utf8_lossy(&res.body)))
        }
    }
}

/// Healthy when every engine reports `ready` and the Space can reach R2.
pub fn is_healthy(health: &EngineHealth) -> bool {
    health.storage_ok && !health.engines.is_empty() && health.engines.values().all(|s| s == "ready")
}
