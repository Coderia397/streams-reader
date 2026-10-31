//! Shared state managed by Tauri (`app.state::<AppState>()`).

use std::sync::Mutex;

use streams_native_bridge::{OpenRequest, UpdateChannel};
use tauri::{AppHandle, Manager};

use crate::engine::Engine;
use crate::paths::Paths;
use crate::secrets::Secrets;

/// Production API origin (Worker `nolabs-api`). Same constant as `nolabs_core::brand::SITE_URL` once that lands.
pub const SITE_URL: &str = "https://listen.streams.watch";

pub struct AppState {
    pub paths: Paths,
    pub http: reqwest::Client,
    pub secrets: Secrets,
    pub engine: Engine,
    /// Files / links / text received before the UI asked (`open_take_pending`).
    pub pending_open: Mutex<Vec<OpenRequest>>,
    pub channel: Mutex<UpdateChannel>,
}

pub fn init(app: &AppHandle) -> Result<(), Box<dyn std::error::Error>> {
    let paths = Paths::resolve(app)?;
    paths.create_all()?;
    let http = reqwest::Client::builder().user_agent(concat!("StreamsReader/", env!("CARGO_PKG_VERSION"))).build()?;
    let state = AppState {
        engine: Engine::start(app)?,
        secrets: Secrets::new(app),
        paths,
        http,
        pending_open: Mutex::new(Vec::new()),
        channel: Mutex::new(UpdateChannel::Stable),
    };
    app.manage(state);
    Ok(())
}
