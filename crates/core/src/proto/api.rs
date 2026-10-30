//! HTTP API between the apps and the Worker. All routes live under `/api`, take and return JSON unless noted, and
//! answer errors with [`super::ApiError`].
//!
//! Accounts are Supabase's (decision D10). Every route needs `Authorization: Bearer <Supabase access token>` and the
//! `x-nolabs-api` version header, except `GET /api/audio/{key}?t=<audio token>` (media elements cannot send headers;
//! the token comes from `POST /api/audio-token`). Library metadata, progress, bookmarks and settings are not Worker
//! routes: the apps read and write them in Supabase directly (PostgREST + Realtime, rows in [`DocumentRecord`] and
//! friends), and the Worker only handles what needs secrets or R2.
//!
//! | Method | Path | Body → Response |
//! |---|---|---|
//! | GET | /api/session | → [`SessionInfo`] (401 when the token is missing, expired or forged) |
//! | POST | /api/audio-token | → [`AudioToken`] (valid 6 h, for `?t=` on audio URLs) |
//! | PUT | /api/documents/{id}/body | raw JSON bytes of [`DocumentBody`] (streamed to R2 unparsed) → 204; the row must exist in Supabase with `ready = false` and `body_bytes` = Content-Length; the Worker sets `ready` |
//! | GET | /api/documents/{id}/body | → [`DocumentBody`] bytes (streamed from R2) |
//! | DELETE | /api/documents/{id} | → 204 (Supabase row, its progress and bookmarks, and the R2 body) |
//! | GET | /api/voices | → [`VoiceCatalog`] |
//! | POST | /api/voices | [`CreateVoice`] → [`VoiceInfo`] |
//! | PUT | /api/voices/{id}/reference | raw WAV bytes (streamed to R2) → 204 |
//! | POST | /api/voices/{id}/prepare | → [`VoiceInfo`] (preparation queued) |
//! | DELETE | /api/voices/{id} | → 204 (voice, reference and its rendered audio) |
//! | POST | /api/render | [`RenderRequest`] → [`RenderResponse`] |
//! | GET | /api/render/status?keys=k1,k2 | → [`RenderResponse`] |
//! | GET | /api/audio/{key}[?t=…] | → `audio/webm` bytes from R2, Range supported |
//! | GET | /api/fetch?url=… | → raw page bytes, `X-Final-Url` header (SSRF-guarded) |
//! | GET | /api/status | → [`SystemStatus`] |
//! | POST | /hooks/auth-email | Supabase Send Email hook (Standard Webhooks signature), not for apps |

use serde::{Deserialize, Serialize};

use super::ApiError;

/// 2: Bearer tokens from Supabase replaced the passphrase cookie; library routes moved to Supabase.
pub const API_VERSION: u32 = 2;
/// Header the apps send on every API call; the Worker answers 409 `upgrade_required` on mismatch.
pub const API_VERSION_HEADER: &str = "x-nolabs-api";
/// Query parameter carrying an [`AudioToken`] on audio URLs.
pub const AUDIO_TOKEN_PARAM: &str = "t";

// ---------------------------------------------------------------------------------------------------------------- session

/// Who the Worker thinks the caller is, from the verified access token.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SessionInfo {
    /// Supabase user id (`auth.uid()`).
    pub user_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub email: Option<String>,
    /// When the access token expires; supabase-js refreshes it before then.
    pub expires_at_ms: i64,
}

/// A short-lived credential for `<audio src>` and MediaSource fetches, which cannot send `Authorization`. Bound to
/// the user; works for every audio key (renders are shared and content-addressed). Mint a new one before it expires.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AudioToken {
    pub token: String,
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

/// Largest document body accepted (normalised text as JSON; a long novel is about 2 MB). Also a CHECK in Supabase.
pub const MAX_BODY_BYTES: u64 = 32 * 1024 * 1024;

// -------------------------------------------------------------------------------------------- Supabase rows (PostgREST)

/// A row of `public.documents` as PostgREST reads and writes it (`supabase/migrations/0001_accounts.sql`). The app
/// creates it with a new [`crate::ids`] id and `ready = false`, then PUTs the body to the Worker. `user_id`,
/// `created_at` and `updated_at` are filled by the database; leave them out when inserting.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DocumentRecord {
    pub id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub user_id: Option<String>,
    pub title: String,
    #[serde(default)]
    pub author: Option<String>,
    pub source: SourceKind,
    #[serde(default)]
    pub source_url: Option<String>,
    pub language: String,
    pub char_count: u64,
    pub word_count: u64,
    pub block_count: u32,
    /// Exact byte length of the body the app will PUT; the Worker streams it to R2 with this length.
    pub body_bytes: u64,
    #[serde(default)]
    pub ready: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub created_at: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub updated_at: Option<String>,
}

impl DocumentRecord {
    /// The insert body for a new document.
    pub fn new(id: String, meta: DocumentMeta, body_bytes: u64) -> Self {
        DocumentRecord {
            id,
            user_id: None,
            title: meta.title,
            author: meta.author,
            source: meta.source,
            source_url: meta.source_url,
            language: meta.language,
            char_count: meta.char_count,
            word_count: meta.word_count,
            block_count: meta.block_count,
            body_bytes,
            ready: false,
            created_at: None,
            updated_at: None,
        }
    }
}

/// A row of `public.progress`, upserted with `Prefer: resolution=merge-duplicates`. `updated_at` is the device's
/// clock when the position was taken; the database ignores a write older than the stored one, so a late write from
/// another device never moves progress back. Subscribe to it over Realtime to follow other devices.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProgressRecord {
    pub document_id: String,
    /// Char offset into [`crate::text::reading_text`] of the document.
    pub char_offset: u64,
    /// RFC 3339.
    pub updated_at: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub device: Option<String>,
}

/// A row of `public.profiles`: the account's settings. Only these fields are writable by the user.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ProfileRecord {
    #[serde(default)]
    pub display_name: Option<String>,
    /// BCP 47; also the language of auth emails (keep `user_metadata.locale` in step with `updateUser`).
    pub locale: String,
    #[serde(default)]
    pub default_voice: Option<String>,
    pub default_speed: f32,
    /// `system`, `light` or `dark`.
    pub theme: String,
    /// Read-only for the user.
    #[serde(default, skip_serializing)]
    pub is_admin: bool,
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
    /// The whole deployment's budget (every user).
    pub gpu: GpuBudgetView,
    /// The caller's own share of it.
    pub my_gpu: UserGpuView,
    pub queued: u32,
}

/// The caller's daily GPU share (seconds as charged) and what is left of it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UserGpuView {
    pub used_s: u32,
    pub share_s: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub resets_at_ms: Option<i64>,
}
