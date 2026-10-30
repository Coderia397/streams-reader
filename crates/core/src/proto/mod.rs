//! Wire contracts. `api` is the HTTP API between the web app and the Worker; `engine` is contract v1 between the Worker
//! and the speech Space (mirrored in `space/nolabs_engine/contracts.py`); `error` is the one error body every API route
//! returns. Field names are the JSON names; all text offsets are Unicode scalar (char) indices into the exact text sent.

pub mod api;
pub mod engine;
pub mod error;

pub use error::{ApiError, ErrorCode};
