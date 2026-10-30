use nolabs_core::proto::api::{VoiceKind, VoiceState};
use nolabs_core::proto::engine::PrepareReferenceResponse;

use super::row::{self, Row};
use super::{Repo, parse_enum, sql};
use crate::ports::{PortError, Stmt};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VoiceRow {
    pub id: String,
    pub name: String,
    pub kind: VoiceKind,
    pub state: VoiceState,
    pub transcript: String,
    pub reference_bytes: u64,
    pub raw_uploaded: bool,
    pub ref_key: Option<String>,
    pub ref_transcript: Option<String>,
    pub ref_sha256: Option<String>,
    pub ref_duration_ms: Option<u32>,
    pub error: Option<String>,
    pub created_at_ms: i64,
    pub updated_at_ms: i64,
}

impl VoiceRow {
    fn from_row(r: &Row) -> Result<Self, PortError> {
        Ok(VoiceRow {
            id: row::str(r, "id")?,
            name: row::str(r, "name")?,
            kind: parse_enum(&row::str(r, "kind")?)?,
            state: parse_enum(&row::str(r, "state")?)?,
            transcript: row::str(r, "transcript")?,
            reference_bytes: row::u64(r, "reference_bytes")?,
            raw_uploaded: row::bool(r, "raw_uploaded")?,
            ref_key: row::opt_str(r, "ref_key")?,
            ref_transcript: row::opt_str(r, "ref_transcript")?,
            ref_sha256: row::opt_str(r, "ref_sha256")?,
            ref_duration_ms: row::opt_u32(r, "ref_duration_ms")?,
            error: row::opt_str(r, "error")?,
            created_at_ms: row::i64(r, "created_at_ms")?,
            updated_at_ms: row::i64(r, "updated_at_ms")?,
        })
    }
}

impl Repo<'_> {
    pub async fn insert_voice(
        &self,
        id: &str,
        name: &str,
        transcript: &str,
        reference_bytes: u64,
        now_ms: i64,
    ) -> Result<(), PortError> {
        self.run(
            sql::VOICE_INSERT,
            vec![id.into(), name.into(), transcript.into(), reference_bytes.into(), now_ms.into(), now_ms.into()],
        )
        .await?;
        Ok(())
    }

    pub async fn get_voice(&self, id: &str) -> Result<Option<VoiceRow>, PortError> {
        self.all(sql::VOICE_GET, vec![id.into()]).await?.first().map(VoiceRow::from_row).transpose()
    }

    pub async fn list_voices(&self) -> Result<Vec<VoiceRow>, PortError> {
        self.all(sql::VOICE_LIST, vec![]).await?.iter().map(VoiceRow::from_row).collect()
    }

    /// Returns false when the voice is missing or being prepared.
    pub async fn set_voice_raw_uploaded(&self, id: &str, now_ms: i64) -> Result<bool, PortError> {
        Ok(self.run(sql::VOICE_SET_RAW_UPLOADED, vec![now_ms.into(), id.into()]).await? > 0)
    }

    pub fn set_voice_preparing_stmt(id: &str, now_ms: i64) -> Stmt {
        Stmt::new(sql::VOICE_SET_PREPARING, vec![now_ms.into(), id.into()])
    }

    pub async fn set_voice_ready(
        &self,
        id: &str,
        prepared: &PrepareReferenceResponse,
        now_ms: i64,
    ) -> Result<(), PortError> {
        self.run(
            sql::VOICE_SET_READY,
            vec![
                prepared.ref_key.as_str().into(),
                prepared.transcript.as_str().into(),
                prepared.sha256.as_str().into(),
                prepared.duration_ms.into(),
                now_ms.into(),
                id.into(),
            ],
        )
        .await?;
        Ok(())
    }

    pub async fn set_voice_failed(&self, id: &str, error: &str, now_ms: i64) -> Result<(), PortError> {
        self.run(sql::VOICE_SET_FAILED, vec![error.into(), now_ms.into(), id.into()]).await?;
        Ok(())
    }

    /// Object keys of every segment rendered with this voice (catalogue id, e.g. `user:<id>`).
    pub async fn render_objects_by_voice(&self, voice: &str) -> Result<Vec<String>, PortError> {
        self.all(sql::RENDER_OBJECTS_BY_VOICE, vec![voice.into()])
            .await?
            .iter()
            .map(|r| row::str(r, "object_key"))
            .collect()
    }

    /// Remove the voice row, its rendered segments and its jobs in one transaction.
    pub async fn delete_voice_cascade(&self, id: &str, voice_ref: &str) -> Result<bool, PortError> {
        let results = self
            .batch(&[
                Stmt::new(sql::RENDER_DELETE_BY_VOICE, vec![voice_ref.into()]),
                Stmt::new(sql::JOB_DELETE_BY_VOICE, vec![voice_ref.into()]),
                Stmt::new(sql::VOICE_DELETE, vec![id.into()]),
            ])
            .await?;
        Ok(results.last().is_some_and(|r| r.changes > 0))
    }
}
