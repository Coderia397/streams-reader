//! Application logic of the nolabs backend, independent of Cloudflare: auth (Supabase tokens), document bodies, voices,
//! the auth email hook, rendering through the
//! speech Space, the queue consumer and the cron. Every side effect goes through the small ports in [`ports`], which
//! the Worker crate implements over D1, R2, Queues and `fetch`, and which the tests implement over SQLite and fakes.

pub mod accounts;
pub mod app;
pub mod auth;
pub mod config;
pub mod error;
pub mod gradio;
pub mod ports;
pub mod repo;
pub mod routes;
pub mod security_headers;
pub mod services;
pub mod space;
pub mod static_assets;
pub mod static_routes;

#[cfg(test)]
pub(crate) mod testing;

pub use app::App;
pub use config::Config;
