//! The nolabs phone app: a Leptos client-side app served by the Worker. It imports documents in the browser, sends them
//! to the Worker, asks for audio segment by segment and plays it through `nolabs_player`.
//!
//! Layout: `api` talks HTTP, `storage` keeps local state (IndexedDB, Cache API, localStorage), `state` holds the
//! reactive app state, `provider` turns segments into playable tracks, `pages` and `components` are the UI, and
//! `reading`, `audio`, `util` hold pure logic that is unit tested natively.

mod api;
mod app;
mod audio;
mod components;
mod config;
mod import_flow;
mod interop;
mod pages;
mod provider;
mod reading;
mod state;
mod storage;
mod util;

fn main() {
    console_error_panic_hook::set_once();
    leptos::mount::mount_to_body(app::App);
}
