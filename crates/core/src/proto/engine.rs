//! Contract v1 between the Worker (queue consumer) and the speech Space. Mirrored by
//! `space/nolabs_engine/contracts.py`; shared fixtures live in `contracts/fixtures/` and are checked by tests on both
//! sides. Every Space endpoint takes one JSON string and returns one JSON string.
//!
//! Endpoints: `render_cpu` (Kokoro), `render_gpu` (Ming), `prepare_reference` (CPU: trim a recording to the best
//! 10–18 s span), `health` (CPU, unqueued).

use serde::{Deserialize, Serialize};

use super::api::{Boundary, Emotion, Engine};

pub const CONTRACT_VERSION: u32 = 1;

/// How Ming should render one item. Kokoro items always use `Kokoro`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Mode {
    Kokoro,
    /// Ming built-in voice, neutral (`IP` task).
    Ip,
    /// Ming cloned/designed voice from a reference + transcript, optional inline tags (`TTS` task).
    Tts,
    /// Ming reference voice with one emotion label (`EMOTION` task); preset voices use an anchor clip.
    Emotion,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum EngineVoice {
    /// Built-in voice of the engine (Kokoro voice id or Ming IP id).
    Preset { id: String },
    /// Reference clip in R2 (`voices/<id>/ref.wav`) with its exact transcript.
    Reference { id: String, ref_key: String, transcript: String },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EngineItem {
    pub key: String,
    pub text: String,
    pub mode: Mode,
    #[serde(default)]
    pub emotion: Emotion,
    #[serde(default)]
    pub boundary: Boundary,
    /// Where the Space must write the WebM/Opus file in R2; the sidecar JSON goes to `<object_key>.json`.
    pub object_key: String,
}

/// Silence appended after a segment, by what follows it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct PauseProfile {
    pub sentence_ms: u32,
    pub paragraph_ms: u32,
    pub section_ms: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct AudioProfile {
    /// Identifies these settings inside the cache key.
    pub id: u32,
    pub bitrate_kbps: u32,
    pub sample_rate: u32,
    pub target_lufs: f32,
    /// Largest per-segment deviation from the batch gain, in dB.
    pub max_correction_db: f32,
    pub peak_ceiling_dbfs: f32,
    pub pauses: PauseProfile,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EngineRequest {
    pub v: u32,
    pub job_id: String,
    pub engine: Engine,
    pub voice: EngineVoice,
    pub items: Vec<EngineItem>,
    pub audio: AudioProfile,
    /// The Space writes the full [`EngineResponse`] here when done, so a lost connection never repeats a GPU run.
    pub marker_key: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EngineItemError {
    pub code: String,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EngineItemResult {
    pub key: String,
    pub ok: bool,
    #[serde(default)]
    pub object_key: String,
    #[serde(default)]
    pub duration_ms: u32,
    #[serde(default)]
    pub bytes: u64,
    /// `[start_ms, end_ms, char_start, char_end]` per word; empty when the engine gives no timings.
    #[serde(default)]
    pub words: Vec<[u32; 4]>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<EngineItemError>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EngineResponse {
    pub v: u32,
    pub job_id: String,
    pub engine: Engine,
    pub items: Vec<EngineItemResult>,
    pub elapsed_ms: u64,
    #[serde(default)]
    pub gpu_ms: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PrepareReferenceRequest {
    pub v: u32,
    pub raw_key: String,
    pub ref_key: String,
    /// Transcript of the whole raw recording (the guided script, line breaks between lines).
    pub transcript: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PrepareReferenceResponse {
    pub v: u32,
    pub ref_key: String,
    /// Transcript of the kept span only.
    pub transcript: String,
    pub duration_ms: u32,
    /// sha256 of the stored reference WAV; part of the voice fingerprint in cache keys.
    pub sha256: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EngineHealth {
    pub v: u32,
    pub engines: std::collections::BTreeMap<String, String>,
    pub storage_ok: bool,
    #[serde(default)]
    pub model_revs: std::collections::BTreeMap<String, String>,
}
