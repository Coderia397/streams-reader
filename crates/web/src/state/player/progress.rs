//! Listening progress: IndexedDB every 5 s while playing; Supabase on pause, every 30 s and (by a keepalive fetch) when the
//! page is hidden, which on Android is often the last chance before the tab is frozen.

use std::time::Duration;

use crate::model::Progress;
use leptos::prelude::*;
use leptos::task::spawn_local;
use nolabs_player::PlayerState;
use wasm_bindgen::JsCast;
use wasm_bindgen::prelude::Closure;

use super::PlayerBridge;
use crate::config::{PROGRESS_LOCAL_MS, PROGRESS_SYNC_MS};
use crate::db::library as rows;
use crate::state::AppState;
use crate::storage::progress::{self, LocalProgress};
use crate::util::dom;
use crate::util::timer::now_ms;

impl PlayerBridge {
    /// Starts the app-lifetime timers and listeners. Call once from the app root.
    pub fn start_background(self, app: AppState) {
        set_interval(
            move || {
                if self.state.get_untracked() == PlayerState::Playing {
                    spawn_local(async move {
                        let _ = self.save_local(app).await;
                    });
                }
            },
            Duration::from_millis(u64::from(PROGRESS_LOCAL_MS)),
        );
        set_interval(
            move || {
                let due = now_ms() - self.last_sync_ms.get_value() >= i64::from(PROGRESS_SYNC_MS);
                if self.state.get_untracked() == PlayerState::Playing && due {
                    spawn_local(async move { self.sync(app).await });
                }
            },
            Duration::from_millis(5_000),
        );
        // Sync when playback pauses (user, headphones out, phone call).
        Effect::new(move |prev: Option<PlayerState>| {
            let now = self.state.get();
            if prev == Some(PlayerState::Playing) && matches!(now, PlayerState::Paused) {
                spawn_local(async move { self.sync(app).await });
            }
            now
        });
        if let Some(doc) = dom::document() {
            let on_visibility = Closure::<dyn Fn()>::new(move || {
                if dom::is_hidden() {
                    self.send_on_hide();
                    spawn_local(async move {
                        let _ = self.save_local(app).await;
                    });
                }
            })
            .into_js_value();
            let _ = doc.add_event_listener_with_callback("visibilitychange", on_visibility.unchecked_ref());
        }
    }

    fn current_progress(&self) -> Option<(String, Progress)> {
        let offset = self.current_offset()?;
        let id = self.doc.with_untracked(|d| d.as_ref().map(|d| d.summary.id.clone()))?;
        Some((id, Progress { char_offset: u64::from(offset), updated_at_ms: now_ms() }))
    }

    pub(super) async fn save_local(self, app: AppState) -> Option<(String, Progress)> {
        let (id, p) = self.current_progress()?;
        let local = LocalProgress {
            doc_id: id.clone(),
            char_offset: p.char_offset,
            updated_at_ms: p.updated_at_ms,
            synced: false,
        };
        let _ = progress::save(&local).await;
        app.library.set_progress(&id, p);
        Some((id, p))
    }

    /// Saves locally and upserts the Supabase row; offline failures are silent (the next sync catches up).
    pub async fn sync(self, app: AppState) {
        let Some((id, p)) = self.save_local(app).await else { return };
        self.last_sync_ms.set_value(now_ms());
        if rows::put_progress(&id, p).await.is_ok() {
            let local =
                LocalProgress { doc_id: id, char_offset: p.char_offset, updated_at_ms: p.updated_at_ms, synced: true };
            let _ = progress::save(&local).await;
        }
    }

    fn send_on_hide(&self) {
        if let Some((id, p)) = self.current_progress() {
            rows::put_progress_keepalive(&id, p);
            self.last_sync_ms.set_value(now_ms());
        }
    }

    /// At the end of a document, store the end position so the library shows it finished.
    pub(super) async fn mark_finished(self, app: AppState) {
        let Some((id, total)) =
            self.doc.with_untracked(|d| d.as_ref().map(|d| (d.summary.id.clone(), d.model.total_chars)))
        else {
            return;
        };
        let p = Progress { char_offset: u64::from(total), updated_at_ms: now_ms() };
        let _ = progress::save(&LocalProgress {
            doc_id: id.clone(),
            char_offset: p.char_offset,
            updated_at_ms: p.updated_at_ms,
            synced: false,
        })
        .await;
        app.library.set_progress(&id, p);
        let _ = rows::put_progress(&id, p).await;
    }
}
