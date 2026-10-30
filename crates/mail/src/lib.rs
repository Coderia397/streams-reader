//! Auth emails, the part with no network (decision D11): Supabase's Send Email hook calls the Worker, which checks
//! the [`webhook`] signature, turns the payload into messages ([`hook`]), writes each in the user's language
//! ([`render`] with wording from `ftl/*.ftl` via [`bundle`]) and sends it through Brevo ([`brevo`]). Pure and
//! wasm-safe, so everything is tested natively.

pub mod brevo;
pub mod bundle;
pub mod ftl;
pub mod hook;
pub mod render;
pub mod webhook;

pub use render::{Brand, Email, Kind, Rendered};
