//! Streams Reader, the web app: a Leptos client-side app served under `/app`. It imports documents in the browser,
//! sends them to the Worker, asks for audio segment by segment and plays it through `nolabs_player`.
//!
//! Layout: `api` talks to the Worker, `auth` to Supabase Auth, `storage` keeps local state (IndexedDB, Cache API,
//! localStorage), `state` holds the reactive app state, `provider` turns segments into playable tracks, `pages` and
//! `components` are the UI, `i18n` the interface text, and `reading`, `audio`, `util` hold pure logic that is unit
//! tested natively.

#[macro_use]
mod i18n;

mod api;
mod app;
mod audio;
mod auth;
mod brand;
mod components;
mod config;
mod db;
mod import_flow;
mod interop;
mod model;
mod pages;
mod provider;
mod reading;
mod state;
mod storage;
mod util;

fn main() {
    console_error_panic_hook::set_once();
    i18n::init();
    if let Some(doc) = util::dom::document() {
        doc.set_title(config::APP_NAME);
    }
    leptos::mount::mount_to_body(app::App);
}
