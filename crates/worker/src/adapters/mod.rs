//! workers-rs implementations of the `nolabs_server::ports` traits, and the [`App`] built from the Worker's bindings.

mod d1;
mod http;
mod queue;
mod r2;
mod sys;

use nolabs_core::proto::ApiError;
use nolabs_server::{App, Config};
use worker::Env;

pub use d1::D1Db;
pub use http::FetchHttp;
pub use queue::RenderQueue;
pub use r2::R2Blob;
pub use sys::{CryptoRng, WorkerClock};

/// Binding names (notes/09-build-spec.md).
pub const DB: &str = "DB";
pub const MEDIA: &str = "MEDIA";
pub const RENDER_QUEUE: &str = "RENDER_QUEUE";

fn binding(name: &str, e: worker::Error) -> ApiError {
    ApiError::internal(format!("binding {name} is not configured: {e}"))
}

/// Read vars and secrets; a missing one is a deployment error reported by name only.
pub fn config(env: &Env) -> Result<Config, ApiError> {
    Config::from_lookup(|name| {
        env.var(name).map(|v| v.to_string()).ok().or_else(|| env.secret(name).map(|s| s.to_string()).ok())
    })
    .map_err(|e| ApiError::internal(format!("server misconfigured: {e}")))
}

pub fn app(env: &Env) -> Result<App, ApiError> {
    Ok(App {
        db: Box::new(D1Db(env.d1(DB).map_err(|e| binding(DB, e))?)),
        blob: Box::new(R2Blob(env.bucket(MEDIA).map_err(|e| binding(MEDIA, e))?)),
        http: Box::new(FetchHttp),
        queue: Box::new(RenderQueue(env.queue(RENDER_QUEUE).map_err(|e| binding(RENDER_QUEUE, e))?)),
        clock: Box::new(WorkerClock),
        rng: Box::new(CryptoRng),
        config: config(env)?,
    })
}

pub fn media(env: &Env) -> Result<worker::Bucket, ApiError> {
    env.bucket(MEDIA).map_err(|e| binding(MEDIA, e))
}
