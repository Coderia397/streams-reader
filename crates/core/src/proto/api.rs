//! HTTP API between the web app and the Worker. All routes live under `/api`, take and return JSON unless noted, and
//! answer errors with [`super::ApiError`].
//!
//! | Method | Path | Body → Response |
//! |---|---|---|
//! | POST | /api/session | [`LoginRequest`] → 204 + `__Host-nolabs_s` cookie |
//! | DELETE | /api/session | → 204, cookie cleared |
//! | GET | /api/session | → [`SessionInfo`] (401 when logged out) |
//! | GET | /api/library | → [`Library`] |
//! | POST | /api/documents | [`CreateDocument`] → [`DocumentSummary`] |
//! | PUT | /api/documents/{id}/body | raw JSON bytes of [`DocumentBody`] (streamed to R2 unparsed) → 204 |
//! | GET | /api/documents/{id} | → [`DocumentSummary`] |
//! | GET | /api/documents/{id}/body | → [`DocumentBody`] bytes (streamed from R2) |
//! | DELETE | /api/documents/{id} | → 204 |
//! | PUT | /api/documents/{id}/progress | [`Progress`] → 204 (also accepted as POST for `sendBeacon`) |
//! | GET | /api/voices | → [`VoiceCatalog`] |
//! | POST | /api/voices | [`CreateVoice`] → [`VoiceInfo`] |
//! | PUT | /api/voices/{id}/reference | raw WAV bytes (streamed to R2) → 204 |
//! | POST | /api/voices/{id}/prepare | → [`VoiceInfo`] (preparation queued) |
//! | DELETE | /api/voices/{id} | → 204 (voice, reference and its rendered audio) |
//! | POST | /api/render | [`RenderRequest`] → [`RenderResponse`] |
//! | GET | /api/render/status?keys=k1,k2 | → [`RenderResponse`] |
//! | GET | /api/audio/{key} | → `audio/webm` bytes from R2, Range supported |
//! | GET | /api/fetch?url=… | → raw page bytes, `X-Final-Url` header (SSRF-guarded) |
//! | GET | /api/status | → [`SystemStatus`] |

use serde::{Deserialize, Serialize};

use super::ApiError;

pub const API_VERSION: u32 = 1;
/// Header the web app sends on every API call; the Worker answers 409 `upgrade_required` on mismatch.
pub const API_VERSION_HEADER: &str = "x-nolabs-api";
pub const SESSION_COOKIE: &str = "__Host-nolabs_s";

// ---------------------------------------------------------------------------------------------------------------- session

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LoginRequest {
    pub passphrase: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SessionInfo {
    pub expires_at_ms: i64,
}

// -------------------------------------------------------------------------------------------------------------- documents

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SourceKind {
    Paste,
    Url,
    Epub,
    Text,
    Markdown,
    Html,
    Pdf,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DocumentMeta {
    pub title: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub author: Option<String>,
    pub source: SourceKind,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_url: Option<String>,
    /// BCP 47 tag, `en` for now.
    pub language: String,
    pub char_count: u64,
    pub word_count: u64,
    pub block_count: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CreateDocument {
    pub meta: DocumentMeta,
    /// Exact byte length of the body the app will PUT next; the Worker streams it to R2 with this length.
    pub body_bytes: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Progress {
    /// Char offset into [`crate::text::reading_text`] of the document.
    pub char_offset: u64,
    pub updated_at_ms: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DocumentSummary {
    pub id: String,
    pub meta: DocumentMeta,
    pub created_at_ms: i64,
    pub updated_at_ms: i64,
    /// False until the body upload has finished.
    pub ready: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub progress: Option<Progress>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Library {
    pub documents: Vec<DocumentSummary>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BlockKind {
    Heading,
    Paragraph,
    Quote,
    ListItem,
}

/// One block of normalised, plain text. No markup survives import.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Block {
    pub kind: BlockKind,
    pub text: String,
}

pub const DOCUMENT_BODY_VERSION: u32 = 1;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DocumentBody {
    pub v: u32,
    pub blocks: Vec<Block>,
}

// ----------------------------------------------------------------------------------------------------------------- voices

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Engine {
    Kokoro,
    Ming,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Tier {
    /// Kokoro on the Space CPU: unlimited.
    Standard,
    /// Ming on ZeroGPU: limited daily budget.
    Expressive,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum VoiceKind {
    Preset,
    Clone,
    Designed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum VoiceState {
    Ready,
    /// Waiting for the reference upload.
    AwaitingReference,
    /// Reference uploaded, preparation queued or running.
    Preparing,
    Failed,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VoiceInfo {
    /// `kokoro:<id>`, `ming:<id>` or `user:<id>`; see [`crate::voices::VoiceRef`].
    pub id: String,
    pub engine: Engine,
    pub tier: Tier,
    pub kind: VoiceKind,
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub gender: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub accent: Option<String>,
    pub description: String,
    /// Shown in the main list; the rest sit under "More voices".
    pub featured: bool,
    pub state: VoiceState,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub preview_key: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VoiceCatalog {
    pub voices: Vec<VoiceInfo>,
    pub default_voice: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CreateVoice {
    pub name: String,
    /// Exact words spoken in the reference recording (the guided script).
    pub transcript: String,
    pub reference_bytes: u64,
}

// ----------------------------------------------------------------------------------------------------------------- render

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum Boundary {
    /// Next segment continues the same paragraph.
    #[default]
    Sentence,
    /// Next segment starts a new paragraph.
    Paragraph,
    /// Next segment starts a new section (heading or scene break).
    Section,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum Emotion {
    #[default]
    Neutral,
    Happy,
    Sad,
    Angry,
    Fearful,
    Disgusted,
    Surprised,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum Priority {
    #[default]
    Interactive,
    Background,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RenderItem {
    pub text: String,
    #[serde(default)]
    pub boundary: Boundary,
    #[serde(default)]
    pub emotion: Emotion,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RenderRequest {
    pub voice: String,
    pub items: Vec<RenderItem>,
    #[serde(default)]
    pub priority: Priority,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RenderState {
    Ready,
    Queued,
    Rendering,
    /// Waiting for the GPU budget to free up; see `retry_after_s`.
    Deferred,
    Failed,
}

/// Word timing as `[start_ms, end_ms, char_start, char_end]`, char offsets into the segment text.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct WordTiming(pub u32, pub u32, pub u32, pub u32);

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SegmentAudio {
    pub duration_ms: u32,
    pub bytes: u64,
    #[serde(default)]
    pub words: Vec<WordTiming>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RenderItemStatus {
    pub key: String,
    pub state: RenderState,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub audio: Option<SegmentAudio>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<ApiError>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RenderResponse {
    pub items: Vec<RenderItemStatus>,
}

// ----------------------------------------------------------------------------------------------------------------- status

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SpaceStatus {
    /// Hugging Face runtime stage: RUNNING, SLEEPING, BUILDING, RUNTIME_ERROR, …
    pub stage: String,
    pub healthy: bool,
    pub checked_at_ms: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GpuBudgetView {
    pub used_s: u32,
    pub cap_s: u32,
    pub runs_used: u32,
    pub runs_cap: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub resets_at_ms: Option<i64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SystemStatus {
    pub version: String,
    pub space: SpaceStatus,
    pub gpu: GpuBudgetView,
    pub queued: u32,
}
