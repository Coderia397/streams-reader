use nolabs_core::job;
use nolabs_core::proto::api::{Engine, RenderState};
use nolabs_core::proto::{ApiError, ErrorCode};
use serde::{Deserialize, Serialize};

use super::row::{self, Row};
use super::{Repo, enum_text, parse_enum, sql};
use crate::ports::{PortError, SqlValue, Stmt};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum JobKind {
    /// A batch of segments for one engine and voice (`render_cpu` or `render_gpu`).
    Render,
    /// Trim a user recording into a reference clip (`prepare_reference`, CPU).
    Prepare,
}

/// A job row. Its `state` uses the render vocabulary (`ready` means done) so [`nolabs_core::job::apply`] drives it.
#[derive(Debug, Clone, PartialEq)]
pub struct JobRow {
    pub id: String,
    pub kind: JobKind,
    pub engine: Engine,
    pub voice: String,
    pub state: RenderState,
    pub request_json: String,
    pub chars: u32,
    pub requested_s: u32,
    pub attempts: u32,
    pub lease_until_ms: Option<i64>,
    pub retry_at_ms: Option<i64>,
    pub error_code: Option<ErrorCode>,
    pub error_message: Option<String>,
    pub created_at_ms: i64,
    pub updated_at_ms: i64,
}

impl JobRow {
    fn from_row(r: &Row) -> Result<Self, PortError> {
        Ok(JobRow {
            id: row::str(r, "id")?,
            kind: parse_enum(&row::str(r, "kind")?)?,
            engine: parse_enum(&row::str(r, "engine")?)?,
            voice: row::str(r, "voice")?,
            state: parse_enum(&row::str(r, "state")?)?,
            request_json: row::str(r, "request_json")?,
            chars: row::u32(r, "chars")?,
            requested_s: row::u32(r, "requested_s")?,
            attempts: row::u32(r, "attempts")?,
            lease_until_ms: row::opt_i64(r, "lease_until_ms")?,
            retry_at_ms: row::opt_i64(r, "retry_at_ms")?,
            error_code: row::opt_str(r, "error_code")?.map(|c| parse_enum(&c)).transpose()?,
            error_message: row::opt_str(r, "error_message")?,
            created_at_ms: row::i64(r, "created_at_ms")?,
            updated_at_ms: row::i64(r, "updated_at_ms")?,
        })
    }

    /// The state-machine view of this job. Preparation jobs run on the Space CPU, so they follow the Kokoro (CPU)
    /// retry policy; they are stored with engine `kokoro` for that reason.
    pub fn machine(&self) -> job::Row {
        job::Row {
            state: self.state,
            engine: self.engine,
            attempts: self.attempts,
            lease_until_ms: self.lease_until_ms,
            retry_at_ms: self.retry_at_ms,
        }
    }
}

/// A job to insert.
#[derive(Debug, Clone, PartialEq)]
pub struct NewJob {
    pub id: String,
    pub kind: JobKind,
    pub engine: Engine,
    pub voice: String,
    pub state: RenderState,
    pub request_json: String,
    pub chars: u32,
    pub requested_s: u32,
    pub retry_at_ms: Option<i64>,
    pub error: Option<ApiError>,
}

/// A compare-and-set transition: applies only while the job is still in `from`.
#[derive(Debug, Clone, PartialEq)]
pub struct JobUpdate<'a> {
    pub id: &'a str,
    pub from: RenderState,
    pub to: &'a job::Row,
    pub requested_s: u32,
    pub error: Option<&'a ApiError>,
}

impl Repo<'_> {
    pub fn insert_job_stmt(job: &NewJob, now_ms: i64) -> Stmt {
        let (code, message): (SqlValue, SqlValue) = match &job.error {
            Some(e) => (enum_text(&e.code).into(), e.message.as_str().into()),
            None => (SqlValue::Null, SqlValue::Null),
        };
        Stmt::new(
            sql::JOB_INSERT,
            vec![
                job.id.as_str().into(),
                enum_text(&job.kind).into(),
                enum_text(&job.engine).into(),
                job.voice.as_str().into(),
                enum_text(&job.state).into(),
                job.request_json.as_str().into(),
                job.chars.into(),
                job.requested_s.into(),
                job.retry_at_ms.into(),
                code,
                message,
                now_ms.into(),
                now_ms.into(),
            ],
        )
    }

    pub fn transition_job_stmt(update: &JobUpdate<'_>, now_ms: i64) -> Stmt {
        let (code, message): (SqlValue, SqlValue) = match update.error {
            Some(e) => (enum_text(&e.code).into(), e.message.as_str().into()),
            None => (SqlValue::Null, SqlValue::Null),
        };
        Stmt::new(
            sql::JOB_TRANSITION,
            vec![
                enum_text(&update.to.state).into(),
                update.to.attempts.into(),
                update.to.lease_until_ms.into(),
                update.to.retry_at_ms.into(),
                update.requested_s.into(),
                code,
                message,
                now_ms.into(),
                update.id.into(),
                enum_text(&update.from).into(),
            ],
        )
    }

    /// Apply a transition on its own; false when the job was no longer in `from` (someone else moved it).
    pub async fn transition_job(&self, update: &JobUpdate<'_>, now_ms: i64) -> Result<bool, PortError> {
        let stmt = Self::transition_job_stmt(update, now_ms);
        Ok(self.db.run(&stmt).await? > 0)
    }

    pub async fn get_job(&self, id: &str) -> Result<Option<JobRow>, PortError> {
        self.all(sql::JOB_GET, vec![id.into()]).await?.first().map(JobRow::from_row).transpose()
    }

    async fn jobs(&self, sql: &str, params: Vec<SqlValue>) -> Result<Vec<JobRow>, PortError> {
        self.all(sql, params).await?.iter().map(JobRow::from_row).collect()
    }

    pub async fn due_deferred_jobs(&self, now_ms: i64) -> Result<Vec<JobRow>, PortError> {
        self.jobs(sql::JOB_DUE_DEFERRED, vec![now_ms.into()]).await
    }

    pub async fn expired_lease_jobs(&self, now_ms: i64) -> Result<Vec<JobRow>, PortError> {
        self.jobs(sql::JOB_EXPIRED_LEASES, vec![now_ms.into()]).await
    }

    pub async fn stale_queued_jobs(&self, untouched_since_ms: i64, now_ms: i64) -> Result<Vec<JobRow>, PortError> {
        self.jobs(sql::JOB_STALE_QUEUED, vec![untouched_since_ms.into(), now_ms.into()]).await
    }

    pub async fn recent_failed_gpu_jobs(&self, since_ms: i64) -> Result<Vec<JobRow>, PortError> {
        self.jobs(sql::JOB_RECENT_FAILED_GPU, vec![since_ms.into()]).await
    }

    /// `(job id, requested seconds)` of every admitted Ming job that has not reached the GPU yet.
    pub async fn pending_gpu_requests(&self) -> Result<Vec<(String, u32)>, PortError> {
        self.all(sql::JOB_PENDING_GPU, vec![])
            .await?
            .iter()
            .map(|r| Ok((row::str(r, "id")?, row::u32(r, "requested_s")?)))
            .collect()
    }

    pub async fn touch_job(&self, id: &str, now_ms: i64) -> Result<(), PortError> {
        self.run(sql::JOB_TOUCH, vec![now_ms.into(), id.into()]).await?;
        Ok(())
    }

    pub async fn prune_finished_jobs(&self, before_ms: i64) -> Result<u64, PortError> {
        self.run(sql::JOB_PRUNE_FINISHED, vec![before_ms.into()]).await
    }
}
