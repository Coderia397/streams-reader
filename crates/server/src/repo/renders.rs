use nolabs_core::proto::api::{Engine, RenderItemStatus, RenderState, SegmentAudio, WordTiming};
use nolabs_core::proto::engine::EngineItemResult;
use nolabs_core::proto::{ApiError, ErrorCode};

use super::row::{self, Row};
use super::{Repo, enum_text, parse_enum, sql};
use crate::ports::{PortError, SqlValue, Stmt};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RenderRow {
    pub key: String,
    pub engine: Engine,
    pub voice: String,
    pub state: RenderState,
    pub job_id: Option<String>,
    pub object_key: String,
    pub chars: u32,
    pub duration_ms: Option<u32>,
    pub bytes: Option<u64>,
    pub words: Vec<[u32; 4]>,
    pub error_code: Option<ErrorCode>,
    pub error_message: Option<String>,
    pub retry_at_ms: Option<i64>,
    pub updated_at_ms: i64,
}

impl RenderRow {
    fn from_row(r: &Row) -> Result<Self, PortError> {
        let words = match row::opt_str(r, "words_json")? {
            Some(text) => serde_json::from_str(&text).map_err(|e| PortError::new(format!("words_json: {e}")))?,
            None => Vec::new(),
        };
        Ok(RenderRow {
            key: row::str(r, "key")?,
            engine: parse_enum(&row::str(r, "engine")?)?,
            voice: row::str(r, "voice")?,
            state: parse_enum(&row::str(r, "state")?)?,
            job_id: row::opt_str(r, "job_id")?,
            object_key: row::str(r, "object_key")?,
            chars: row::u32(r, "chars")?,
            duration_ms: row::opt_u32(r, "duration_ms")?,
            bytes: row::opt_u64(r, "bytes")?,
            words,
            error_code: row::opt_str(r, "error_code")?.map(|c| parse_enum(&c)).transpose()?,
            error_message: row::opt_str(r, "error_message")?,
            retry_at_ms: row::opt_i64(r, "retry_at_ms")?,
            updated_at_ms: row::i64(r, "updated_at_ms")?,
        })
    }

    /// The API view of this row at `now_ms`.
    pub fn status(&self, now_ms: i64) -> RenderItemStatus {
        let mut status = RenderItemStatus { key: self.key.clone(), state: self.state, audio: None, error: None };
        match self.state {
            RenderState::Ready => {
                status.audio = Some(SegmentAudio {
                    duration_ms: self.duration_ms.unwrap_or(0),
                    bytes: self.bytes.unwrap_or(0),
                    words: self.words.iter().map(|w| WordTiming(w[0], w[1], w[2], w[3])).collect(),
                });
            }
            RenderState::Deferred | RenderState::Failed => {
                let code = self.error_code.unwrap_or(if self.state == RenderState::Deferred {
                    ErrorCode::GpuBudget
                } else {
                    ErrorCode::EngineFailed
                });
                let message = self.error_message.clone().unwrap_or_else(|| "render failed".to_owned());
                let mut error = ApiError::new(code, message);
                if let Some(at) = self.retry_at_ms.filter(|at| *at > now_ms) {
                    error = error.with_retry_after(seconds_until(at, now_ms));
                }
                status.error = Some(error);
            }
            RenderState::Queued | RenderState::Rendering => {}
        }
        status
    }
}

/// Whole seconds from `now_ms` until `at_ms`, rounded up.
pub fn seconds_until(at_ms: i64, now_ms: i64) -> u64 {
    u64::try_from((at_ms - now_ms).max(0)).unwrap_or(0).div_ceil(1000)
}

/// A cache miss to claim.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewClaim {
    pub key: String,
    pub engine: Engine,
    pub voice: String,
    pub object_key: String,
    pub chars: u32,
}

fn error_params(error: Option<&ApiError>) -> (SqlValue, SqlValue) {
    match error {
        Some(e) => (enum_text(&e.code).into(), e.message.as_str().into()),
        None => (SqlValue::Null, SqlValue::Null),
    }
}

impl Repo<'_> {
    /// Rows for these keys, in no particular order; missing keys are simply absent.
    pub async fn select_renders(&self, keys: &[String]) -> Result<Vec<RenderRow>, PortError> {
        let mut out = Vec::with_capacity(keys.len());
        for chunk in keys.chunks(sql::MAX_IN_PARAMS) {
            let params = chunk.iter().map(|k| k.as_str().into()).collect();
            for r in self.all(sql::render_select_keys(chunk.len()), params).await? {
                out.push(RenderRow::from_row(&r)?);
            }
        }
        Ok(out)
    }

    pub async fn get_render(&self, key: &str) -> Result<Option<RenderRow>, PortError> {
        self.all(sql::render_get(), vec![key.into()]).await?.first().map(RenderRow::from_row).transpose()
    }

    /// Claim misses atomically; returns the keys this call claimed (new rows and reclaimed failures).
    pub async fn claim_renders(&self, claims: &[NewClaim], now_ms: i64) -> Result<Vec<String>, PortError> {
        let stmts: Vec<Stmt> = claims
            .iter()
            .map(|c| {
                Stmt::new(
                    sql::RENDER_CLAIM,
                    vec![
                        c.key.as_str().into(),
                        enum_text(&c.engine).into(),
                        c.voice.as_str().into(),
                        c.object_key.as_str().into(),
                        c.chars.into(),
                        now_ms.into(),
                        now_ms.into(),
                    ],
                )
            })
            .collect();
        let results = self.batch(&stmts).await?;
        let mut claimed = Vec::new();
        for result in results {
            for r in &result.rows {
                claimed.push(row::str(r, "key")?);
            }
        }
        Ok(claimed)
    }

    pub fn assign_render_stmt(
        key: &str,
        job_id: &str,
        state: RenderState,
        retry_at_ms: Option<i64>,
        error: Option<&ApiError>,
        now_ms: i64,
    ) -> Stmt {
        let (code, message) = error_params(error);
        Stmt::new(
            sql::RENDER_ASSIGN_JOB,
            vec![job_id.into(), enum_text(&state).into(), retry_at_ms.into(), code, message, now_ms.into(), key.into()],
        )
    }

    pub fn renders_for_job_stmt(
        job_id: &str,
        state: RenderState,
        retry_at_ms: Option<i64>,
        error: Option<&ApiError>,
        now_ms: i64,
    ) -> Stmt {
        let (code, message) = error_params(error);
        Stmt::new(
            sql::RENDER_SET_STATE_FOR_JOB,
            vec![enum_text(&state).into(), retry_at_ms.into(), code, message, now_ms.into(), job_id.into()],
        )
    }

    pub fn render_ready_stmt(job_id: &str, item: &EngineItemResult, now_ms: i64) -> Stmt {
        let words = serde_json::to_string(&item.words).unwrap_or_else(|_| "[]".to_owned());
        Stmt::new(
            sql::RENDER_SET_READY,
            vec![
                item.object_key.as_str().into(),
                item.duration_ms.into(),
                item.bytes.into(),
                words.into(),
                now_ms.into(),
                item.key.as_str().into(),
                job_id.into(),
            ],
        )
    }

    pub fn render_failed_stmt(
        job_id: &str,
        key: &str,
        error: &ApiError,
        retry_at_ms: Option<i64>,
        now_ms: i64,
    ) -> Stmt {
        let (code, message) = error_params(Some(error));
        Stmt::new(
            sql::RENDER_SET_FAILED,
            vec![code, message, retry_at_ms.into(), now_ms.into(), key.into(), job_id.into()],
        )
    }

    /// Segments waiting for or in a render.
    pub async fn count_active_renders(&self) -> Result<u32, PortError> {
        match self.all(sql::RENDER_COUNT_ACTIVE, vec![]).await?.first() {
            Some(r) => row::u32(r, "n"),
            None => Ok(0),
        }
    }

    /// Release rows claimed before `older_than_ms` that never got a job; returns how many.
    pub async fn release_orphan_renders(&self, now_ms: i64, older_than_ms: i64) -> Result<u64, PortError> {
        self.run(sql::RENDER_RELEASE_ORPHANS, vec![now_ms.into(), older_than_ms.into()]).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row(state: RenderState) -> RenderRow {
        RenderRow {
            key: "k".into(),
            engine: Engine::Ming,
            voice: "ming:US_Emma".into(),
            state,
            job_id: Some("j".into()),
            object_key: "audio/ming/k.webm".into(),
            chars: 10,
            duration_ms: Some(1500),
            bytes: Some(900),
            words: vec![[0, 300, 0, 5]],
            error_code: None,
            error_message: None,
            retry_at_ms: Some(10_500),
            updated_at_ms: 0,
        }
    }

    #[test]
    fn ready_rows_carry_audio_and_words() {
        let s = row(RenderState::Ready).status(0);
        let audio = s.audio.unwrap();
        assert_eq!((audio.duration_ms, audio.bytes), (1500, 900));
        assert_eq!(audio.words, vec![WordTiming(0, 300, 0, 5)]);
        assert!(s.error.is_none());
    }

    #[test]
    fn deferred_rows_report_budget_and_retry_after() {
        let s = row(RenderState::Deferred).status(1_000);
        let e = s.error.unwrap();
        assert_eq!(e.code, ErrorCode::GpuBudget);
        assert_eq!(e.retry_after_s, Some(10));
        assert!(s.audio.is_none());
    }

    #[test]
    fn queued_rows_have_neither() {
        let s = row(RenderState::Queued).status(0);
        assert!(s.audio.is_none() && s.error.is_none());
    }

    #[test]
    fn seconds_round_up() {
        assert_eq!(seconds_until(1001, 0), 2);
        assert_eq!(seconds_until(0, 5), 0);
    }
}
