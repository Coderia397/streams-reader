//! Turning takes into a voice: create it, upload the WAV, ask the Space to prepare it, then wait until it is ready.

use leptos::prelude::*;
use nolabs_core::proto::api::{CreateVoice, VoiceState};

use super::script::transcript;
use super::takes::Takes;
use crate::api::{ClientError, voices};
use crate::util::backoff::Backoff;
use crate::util::timer::sleep_ms;

/// About ten minutes of polling with backoff before giving up on the wait (preparation continues server-side).
const MAX_POLLS: u32 = 45;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Phase {
    Recording,
    Uploading,
    Preparing,
    Ready { voice_id: String },
    Failed(String),
}

pub async fn submit(name: String, takes: Takes, phase: RwSignal<Phase>) {
    phase.set(Phase::Uploading);
    match run(name, takes, phase).await {
        Ok(voice_id) => phase.set(Phase::Ready { voice_id }),
        Err(msg) => phase.set(Phase::Failed(msg)),
    }
}

async fn run(name: String, takes: Takes, phase: RwSignal<Phase>) -> Result<String, String> {
    let wav = takes.wav().ok_or_else(|| "Record every line first.".to_owned())?;
    let req = CreateVoice { name, transcript: transcript(), reference_bytes: wav.len() as u64 };
    let friendly = |e: ClientError| e.friendly();
    let voice = voices::create(&req).await.map_err(friendly)?;
    voices::put_reference(&voice.id, &wav).await.map_err(friendly)?;
    voices::prepare(&voice.id).await.map_err(friendly)?;
    phase.set(Phase::Preparing);
    let mut backoff = Backoff::default();
    for _ in 0..MAX_POLLS {
        sleep_ms(backoff.next_ms()).await;
        let Ok(catalog) = voices::catalog().await else { continue };
        match catalog.voices.iter().find(|v| v.id == voice.id).map(|v| v.state) {
            Some(VoiceState::Ready) => return Ok(voice.id),
            Some(VoiceState::Failed) => {
                return Err(
                    "The recording couldn't be turned into a voice. Try recording again somewhere quieter.".into()
                );
            }
            Some(_) => {}
            None => return Err("The voice disappeared while it was being prepared.".into()),
        }
    }
    Err("Still preparing. It will appear under My voices when it's ready.".into())
}
