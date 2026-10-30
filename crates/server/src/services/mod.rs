//! The services behind each API route, the queue consumer and the cron, as `impl App` blocks.

pub mod audio;
pub mod auth_email;
pub mod consumer;
pub mod cron;
pub mod documents;
pub mod fetch;
pub mod gpu;
pub mod ownership;
pub mod range;
pub mod render;
pub mod status;
pub mod voices;
