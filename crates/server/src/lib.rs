//! Application logic of the nolabs backend, independent of Cloudflare: auth, documents, voices, rendering through the
//! speech Space, the queue consumer and the cron. Every side effect goes through the small ports in [`ports`], which
//! the Worker crate implements over D1, R2, Queues and `fetch`, and which the tests implement over SQLite and fakes.

pub mod app;
pub mod auth;
pub mod config;
pub mod error;
pub mod gradio;
pub mod ports;
pub mod repo;
pub mod routes;
pub mod services;
pub mod space;
pub mod static_assets;

#[cfg(test)]
pub(crate) mod testing;

pub use app::App;
pub use config::Config;
