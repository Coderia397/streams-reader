//! Renders through the normal path outside the player: voice previews (one fixed sentence) and the background
//! pre-render right after an import.

use nolabs_core::proto::api::{Block, Boundary, Priority, RenderItem, RenderRequest, RenderState};
use nolabs_core::text::{SegmentProfile, segment};
use nolabs_core::voices::VoiceRef;

use super::session::render_items;
use crate::api::{ClientError, render as render_api};
use crate::config::{PRERENDER_SEGMENTS, PREVIEW_TEXT};
use crate::storage::audio_cache;
use crate::util::backoff::Backoff;
use crate::util::timer::sleep_ms;

/// Gives up on a preview after this many polls (about 3 minutes with backoff).
const MAX_PREVIEW_POLLS: u32 = 14;

/// Renders [`PREVIEW_TEXT`] (or plays the voice's stored preview) and returns the clip bytes.
pub async fn preview(voice: &str, preview_key: Option<&str>) -> Result<js_sys::Uint8Array, ClientError> {
    if let Some(key) = preview_key {
        return audio_cache::fetch(key).await;
    }
    let req = RenderRequest {
        voice: voice.to_owned(),
        items: vec![RenderItem {
            text: PREVIEW_TEXT.to_owned(),
            boundary: Boundary::Sentence,
            emotion: Default::default(),
        }],
        priority: Priority::Interactive,
    };
    let mut status = render_api::render(&req).await?.items.into_iter().next();
    let mut backoff = Backoff::default();
    for _ in 0..MAX_PREVIEW_POLLS {
        let Some(item) = status.take() else { break };
        match item.state {
            RenderState::Ready => return audio_cache::fetch(&item.key).await,
            RenderState::Failed | RenderState::Deferred => {
                return Err(ClientError::Api(
                    item.error.unwrap_or_else(|| nolabs_core::proto::ApiError::internal("preview failed")),
                ));
            }
            RenderState::Queued | RenderState::Rendering => {
                sleep_ms(backoff.next_ms()).await;
                status = render_api::status(std::slice::from_ref(&item.key)).await?.items.into_iter().next();
            }
        }
    }
    Err(ClientError::Api(nolabs_core::proto::ApiError::new(
        nolabs_core::proto::ErrorCode::EngineUnavailable,
        "preview timed out",
    )))
}

/// Queues the first segments of a new document at background priority so the first play starts at once.
pub async fn prerender(voice: &str, blocks: &[Block]) {
    let Some(voice_ref) = VoiceRef::parse(voice) else { return };
    let segments = segment(blocks, SegmentProfile::for_engine(voice_ref.engine()));
    let items: Vec<RenderItem> = render_items(&segments).into_iter().take(PRERENDER_SEGMENTS).collect();
    if items.is_empty() {
        return;
    }
    let req = RenderRequest { voice: voice.to_owned(), items, priority: Priority::Background };
    let _ = render_api::render(&req).await;
}
