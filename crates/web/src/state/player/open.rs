//! Opening a document in the player: summary and body (local first for the body), segmentation for the voice's
//! engine, resume position, render session, then a fresh `Player`.

use std::rc::Rc;
use std::sync::Arc;

use leptos::prelude::*;
use nolabs_core::proto::api::Engine;
use nolabs_core::text::{SegmentProfile, segment, sentences};
use nolabs_core::voices::{DEFAULT_VOICE, VoiceRef};
use nolabs_player::{MediaInfo, Player, PlayerState};

use super::{Active, OpenDoc, PlayerBridge};
use crate::api::documents;
use crate::config::{APP_NAME, asset};
use crate::db;
use crate::provider::RenderSession;
use crate::reading::doc::DocModel;
use crate::reading::resume::resume_offset;
use crate::state::AppState;
use crate::storage::{bodies, progress};

/// Positions this close to the end count as finished; opening again starts from the top.
const FINISHED_MARGIN_CHARS: u32 = 40;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OpenRequest {
    pub doc_id: String,
    /// Voice to use instead of the default (voice switch from the sheet).
    pub voice: Option<String>,
    /// Start exactly here instead of the saved position (voice switch keeps the place).
    pub at_offset: Option<u32>,
    pub autoplay: bool,
}

impl OpenRequest {
    pub fn doc(doc_id: impl Into<String>, autoplay: bool) -> OpenRequest {
        OpenRequest { doc_id: doc_id.into(), voice: None, at_offset: None, autoplay }
    }
}

fn valid_voice(voice: String) -> (String, Engine) {
    match VoiceRef::parse(&voice) {
        Some(v) => (voice, v.engine()),
        None => (DEFAULT_VOICE.to_owned(), Engine::Kokoro),
    }
}

impl PlayerBridge {
    /// Opens (or re-opens with another voice) and optionally starts playing. Errors become toasts.
    pub async fn open(self, req: OpenRequest, app: AppState) {
        let (voice, engine) = valid_voice(req.voice.clone().unwrap_or_else(|| app.settings.voice.get_untracked()));
        let same =
            self.doc.with_untracked(|d| d.as_ref().is_some_and(|d| d.summary.id == req.doc_id && d.voice == voice));
        if same && req.at_offset.is_none() {
            if req.autoplay {
                self.play();
            }
            return;
        }
        if self.doc.with_untracked(Option::is_some) {
            self.sync(app).await;
        }
        self.opening.set(Some(req.doc_id.clone()));
        let result = self.load(req, voice, engine, app).await;
        self.opening.set(None);
        if let Err(msg) = result {
            app.toasts.error(msg);
        }
    }

    async fn load(self, req: OpenRequest, voice: String, engine: Engine, app: AppState) -> Result<(), String> {
        let id = req.doc_id;
        let summary = match db::library::get_one(&id).await {
            Ok(s) => s,
            Err(e) => app.library.get(&id).ok_or_else(|| e.friendly())?,
        };
        let body = match bodies::load(&id).await {
            Some(b) => b,
            None => {
                let b = documents::get_body(&id).await.map_err(|e| e.friendly())?;
                bodies::save(&id, &b).await;
                b
            }
        };
        let segments = segment(&body.blocks, SegmentProfile::for_engine(engine));
        if segments.is_empty() {
            return Err(t!("reader-no-text"));
        }
        let model = DocModel::new(body.blocks, segments);
        let offset = match req.at_offset {
            Some(o) => o,
            None => {
                let local = progress::load(&id).await;
                progress::newest(local.as_ref(), summary.progress).map_or(0, |p| {
                    let saved = u32::try_from(p.char_offset).unwrap_or(u32::MAX);
                    resume_offset(&model.blocks, &model.block_offsets, saved, sentences)
                })
            }
        };
        let offset = if offset + FINISHED_MARGIN_CHARS >= model.total_chars { 0 } else { offset };
        let start = model.segment_for_offset(offset);
        let count = model.segment_count();

        let session = RenderSession::open(&id, &voice, &model.segments).await;
        let player = Player::new(session.provider(), self.event_handler(app)).map_err(|e| e.to_string())?;
        let info = MediaInfo {
            title: summary.meta.title.clone(),
            artist: summary.meta.author.clone().unwrap_or_else(|| APP_NAME.to_owned()),
            album: APP_NAME.to_owned(),
            artwork_url: Some(asset("brand/icons/icon-512.png")),
        };
        let words = session.words(start);
        self.replace_active(Some(Rc::new(Active { player, session })));
        self.doc.set(Some(Arc::new(OpenDoc { summary, voice, engine, model })));
        self.segment.set(start);
        self.word.set(None);
        self.words_for.set_value((!words.is_empty()).then_some(start));
        self.words.set(words);
        self.state.set(PlayerState::Idle);
        self.error.set(None);
        let rate = app.settings.speed.get_untracked();
        self.rate.set(rate);
        let sleep = self.sleep.get_untracked();
        self.with_player(|p| {
            p.load(info, count, start);
            p.set_rate(rate);
            p.set_sleep(sleep.to_player());
            if req.autoplay {
                p.play();
            }
        });
        // After `load`, whose first snapshot (idle) would otherwise clear it.
        self.wants_play.set(req.autoplay);
        Ok(())
    }

    /// Re-opens the current document with another voice at the current position.
    pub async fn switch_voice(self, voice: String, app: AppState) {
        let Some(doc_id) = self.doc.with_untracked(|d| d.as_ref().map(|d| d.summary.id.clone())) else { return };
        let autoplay = self.wants_play.get_untracked();
        let at = self.current_offset();
        self.open(OpenRequest { doc_id, voice: Some(voice), at_offset: at, autoplay }, app).await;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unknown_voices_fall_back_to_the_default() {
        assert_eq!(valid_voice("ming:US_Emma".into()), ("ming:US_Emma".into(), Engine::Ming));
        assert_eq!(valid_voice("nope".into()), (DEFAULT_VOICE.into(), Engine::Kokoro));
    }
}
