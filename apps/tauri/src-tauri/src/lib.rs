//! Streams Reader native app. Architecture: `notes/18-native-apps-architecture.md`.
//!
//! One process. The Rust core owns everything that must keep working when the webview is hidden, throttled or
//! destroyed: playback (desktop engine or the Android Media3 service), the feeder that renders ahead and downloads
//! audio, the session (refresh token in the OS keystore), downloads and updates. The webview runs the same Leptos UI
//! as `listen.streams.watch/app` and talks to the core only through the commands in [`commands`] (contract:
//! `streams-native-bridge`).

#[deny(unsafe_code)]
mod commands;
#[deny(unsafe_code)]
mod engine;
#[deny(unsafe_code)]
mod feed;
#[cfg(desktop)]
#[deny(unsafe_code)]
mod media_controls;
#[deny(unsafe_code)]
mod open;
#[deny(unsafe_code)]
mod paths;
#[deny(unsafe_code)]
mod secrets;
#[deny(unsafe_code)]
mod state;

use streams_native_bridge::event;
use tauri::{Emitter, Manager};
use tauri_plugin_deep_link::DeepLinkExt;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // reqwest and the updater use rustls without a bundled provider; install ring once.
    let _ = rustls::crypto::ring::default_provider().install_default();

    let mut builder = tauri::Builder::default();

    // Desktop: a second launch (file association, `nolabs://` link on Windows/Linux) hands its argv to the running
    // instance. Must be the first plugin registered.
    #[cfg(desktop)]
    {
        builder = builder.plugin(tauri_plugin_single_instance::init(|app, argv, _cwd| {
            open::from_argv(app, &argv);
            if let Some(w) = app.get_webview_window("main") {
                let _ = w.unminimize();
                let _ = w.set_focus();
            }
        }));
        builder = builder.plugin(tauri_plugin_updater::Builder::new().build());
    }

    builder = builder
        .plugin(tauri_plugin_deep_link::init())
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_streams_media::init());

    let app = builder
        .setup(|app| {
            state::init(app.handle())?;
            // nolabs://auth/... callbacks (Supabase PKCE, magic links) go straight to the UI, which owns the PKCE
            // verifier and finishes the exchange.
            let handle = app.handle().clone();
            app.deep_link().on_open_url(move |e| {
                for url in e.urls() {
                    let _ = handle.emit(event::AUTH_CALLBACK, serde_json::json!({ "url": url.as_str() }));
                }
            });
            // Files passed on the first launch (desktop argv).
            #[cfg(desktop)]
            open::from_argv(app.handle(), &std::env::args().collect::<Vec<_>>());
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::app::app_info,
            commands::player::player_load,
            commands::player::player_control,
            commands::player::player_snapshot,
            commands::downloads::download_start,
            commands::downloads::download_cancel,
            commands::downloads::download_remove,
            commands::downloads::download_list,
            commands::downloads::storage_usage,
            commands::downloads::cache_clear,
            commands::auth::auth_store,
            commands::auth::auth_session,
            commands::auth::auth_refresh,
            commands::auth::auth_sign_out,
            commands::auth::auth_open_browser,
            commands::updater::update_check,
            commands::updater::update_install,
            commands::files::open_take_pending,
            commands::files::file_read,
        ])
        .build(tauri::generate_context!());

    let app = match app {
        Ok(app) => app,
        Err(e) => {
            eprintln!("Streams Reader failed to start: {e}");
            std::process::exit(1);
        }
    };

    app.run(|_handle, _event| {
        // macOS delivers "open with" files and nolabs:// URLs as an Apple Event, not argv.
        #[cfg(target_os = "macos")]
        if let tauri::RunEvent::Opened { urls } = &_event {
            open::from_urls(_handle, urls);
        }
    });
}
