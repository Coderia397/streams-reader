//! HTTP client for the Worker API (`nolabs_core::proto::api`), one module per resource.

pub mod client;
pub mod documents;
pub mod error;
pub mod fetch;
pub mod render;
pub mod session;
pub mod status;
pub mod voices;

pub use error::ClientError;
