//! Pure logic and wire contracts for nolabs-reader. No I/O lives here: everything is deterministic and natively testable,
//! and the same code runs in the Cloudflare Worker (wasm32), the web app (wasm32) and the native test suite.

pub mod budget;
pub mod cache_key;
pub mod engine_error;
pub mod ids;
pub mod job;
pub mod proto;
pub mod text;
pub mod voices;
