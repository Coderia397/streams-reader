//! Every SQL statement the backend runs, in one place. D1 is SQLite (3.4x): `RETURNING`, upserts and `max(a, b)` are
//! available. D1 allows at most 100 bound parameters per statement, so per-item writes are separate statements in one
//! batch and `IN (…)` lists are chunked by the caller to [`MAX_IN_PARAMS`].

pub const MAX_IN_PARAMS: usize = 90;

// ---------------------------------------------------------------------------------------------------------- documents

pub const DOC_INSERT: &str = "INSERT INTO documents (id, meta_json, body_bytes, ready, created_at_ms, updated_at_ms) \
     VALUES (?, ?, ?, 0, ?, ?)";
pub const DOC_GET: &str = "SELECT * FROM documents WHERE id = ?";
/// Most recently touched (edited or read) first.
pub const DOC_LIST: &str =
    "SELECT * FROM documents ORDER BY max(updated_at_ms, coalesce(progress_at_ms, 0)) DESC, id LIMIT 2000";
pub const DOC_SET_READY: &str = "UPDATE documents SET ready = 1, updated_at_ms = ? WHERE id = ?";
pub const DOC_DELETE: &str = "DELETE FROM documents WHERE id = ?";
/// Last writer by client timestamp wins, so a late beacon from another device never moves progress backwards in time.
pub const DOC_SET_PROGRESS: &str = "UPDATE documents SET progress_offset = ?, progress_at_ms = ? \
     WHERE id = ? AND (progress_at_ms IS NULL OR progress_at_ms <= ?)";

// ------------------------------------------------------------------------------------------------------------- voices

pub const VOICE_INSERT: &str = "INSERT INTO voices (id, name, kind, state, transcript, reference_bytes, raw_uploaded, \
     created_at_ms, updated_at_ms) VALUES (?, ?, 'clone', 'awaiting_reference', ?, ?, 0, ?, ?)";
pub const VOICE_GET: &str = "SELECT * FROM voices WHERE id = ?";
pub const VOICE_LIST: &str = "SELECT * FROM voices ORDER BY created_at_ms, id";
pub const VOICE_SET_RAW_UPLOADED: &str = "UPDATE voices SET raw_uploaded = 1, state = 'awaiting_reference', \
     error = NULL, updated_at_ms = ? WHERE id = ? AND state <> 'preparing'";
pub const VOICE_SET_PREPARING: &str = "UPDATE voices SET state = 'preparing', error = NULL, updated_at_ms = ? \
     WHERE id = ? AND raw_uploaded = 1 AND state <> 'preparing'";
pub const VOICE_SET_READY: &str = "UPDATE voices SET state = 'ready', ref_key = ?, ref_transcript = ?, ref_sha256 = ?, \
     ref_duration_ms = ?, error = NULL, updated_at_ms = ? WHERE id = ?";
pub const VOICE_SET_FAILED: &str = "UPDATE voices SET state = 'failed', error = ?, updated_at_ms = ? WHERE id = ?";
pub const VOICE_DELETE: &str = "DELETE FROM voices WHERE id = ?";

// ------------------------------------------------------------------------------------------------------------ renders

const RENDER_COLUMNS: &str = "key, engine, voice, state, job_id, object_key, chars, duration_ms, bytes, words_json, \
     error_code, error_message, retry_at_ms, updated_at_ms";

/// `SELECT … FROM renders WHERE key IN (?, …)` with `n` placeholders (`n` ≤ [`MAX_IN_PARAMS`]).
pub fn render_select_keys(n: usize) -> String {
    format!("SELECT {RENDER_COLUMNS} FROM renders WHERE key IN ({})", placeholders(n))
}

pub fn render_get() -> String {
    format!("SELECT {RENDER_COLUMNS} FROM renders WHERE key = ?")
}

/// Claim one cache miss: a new key, or a failed one whose retry time has passed. Returns the key when claimed and no
/// row otherwise, so concurrent requests never both enqueue the same segment.
pub const RENDER_CLAIM: &str = "INSERT INTO renders (key, engine, voice, state, job_id, object_key, chars, \
     created_at_ms, updated_at_ms) VALUES (?, ?, ?, 'queued', NULL, ?, ?, ?, ?) \
     ON CONFLICT (key) DO UPDATE SET state = 'queued', job_id = NULL, voice = excluded.voice, \
     object_key = excluded.object_key, chars = excluded.chars, duration_ms = NULL, bytes = NULL, words_json = NULL, \
     error_code = NULL, error_message = NULL, retry_at_ms = NULL, updated_at_ms = excluded.updated_at_ms \
     WHERE renders.state = 'failed' AND (renders.retry_at_ms IS NULL OR renders.retry_at_ms <= excluded.updated_at_ms) \
     RETURNING key";
/// Attach a freshly claimed row to its job (state `queued`, or `deferred` with an error and retry time).
pub const RENDER_ASSIGN_JOB: &str = "UPDATE renders SET job_id = ?, state = ?, retry_at_ms = ?, error_code = ?, \
     error_message = ?, updated_at_ms = ? WHERE key = ? AND state = 'queued' AND job_id IS NULL";
/// Mirror a job transition onto its unfinished items.
pub const RENDER_SET_STATE_FOR_JOB: &str = "UPDATE renders SET state = ?, retry_at_ms = ?, error_code = ?, \
     error_message = ?, updated_at_ms = ? WHERE job_id = ? AND state IN ('queued', 'rendering', 'deferred')";
pub const RENDER_SET_READY: &str = "UPDATE renders SET state = 'ready', object_key = ?, duration_ms = ?, bytes = ?, \
     words_json = ?, error_code = NULL, error_message = NULL, retry_at_ms = NULL, updated_at_ms = ? \
     WHERE key = ? AND job_id = ?";
pub const RENDER_SET_FAILED: &str = "UPDATE renders SET state = 'failed', error_code = ?, error_message = ?, \
     retry_at_ms = ?, updated_at_ms = ? WHERE key = ? AND job_id = ? AND state <> 'ready'";
pub const RENDER_COUNT_ACTIVE: &str =
    "SELECT count(*) AS n FROM renders WHERE state IN ('queued', 'rendering', 'deferred')";
pub const RENDER_OBJECTS_BY_VOICE: &str = "SELECT object_key FROM renders WHERE voice = ?";
pub const RENDER_DELETE_BY_VOICE: &str = "DELETE FROM renders WHERE voice = ?";
/// Rows claimed by a request that died before creating their job: make them claimable again at once.
pub const RENDER_RELEASE_ORPHANS: &str = "UPDATE renders SET state = 'failed', error_code = 'internal', \
     error_message = 'lost before queueing', retry_at_ms = NULL, updated_at_ms = ? \
     WHERE state = 'queued' AND job_id IS NULL AND updated_at_ms < ?";

// --------------------------------------------------------------------------------------------------------------- jobs

pub const JOB_INSERT: &str = "INSERT INTO jobs (id, kind, engine, voice, state, request_json, chars, requested_s, \
     attempts, retry_at_ms, error_code, error_message, created_at_ms, updated_at_ms) \
     VALUES (?, ?, ?, ?, ?, ?, ?, ?, 0, ?, ?, ?, ?, ?)";
pub const JOB_GET: &str = "SELECT * FROM jobs WHERE id = ?";
/// Compare-and-set on the state, so two consumers (a duplicate message) can never both start one job.
pub const JOB_TRANSITION: &str = "UPDATE jobs SET state = ?, attempts = ?, lease_until_ms = ?, retry_at_ms = ?, \
     requested_s = ?, error_code = ?, error_message = ?, updated_at_ms = ? WHERE id = ? AND state = ?";
pub const JOB_TOUCH: &str = "UPDATE jobs SET updated_at_ms = ? WHERE id = ?";
pub const JOB_DUE_DEFERRED: &str =
    "SELECT * FROM jobs WHERE state = 'deferred' AND retry_at_ms <= ? ORDER BY created_at_ms LIMIT 50";
pub const JOB_EXPIRED_LEASES: &str =
    "SELECT * FROM jobs WHERE state = 'rendering' AND lease_until_ms < ? ORDER BY created_at_ms LIMIT 50";
/// Queued jobs not touched for a while whose message may have been lost; the consumer ignores duplicates.
pub const JOB_STALE_QUEUED: &str = "SELECT * FROM jobs WHERE state = 'queued' AND updated_at_ms < ? \
     AND (retry_at_ms IS NULL OR retry_at_ms <= ?) ORDER BY created_at_ms LIMIT 50";
/// Ming jobs that failed recently: the Space may still have finished them and written the marker.
pub const JOB_RECENT_FAILED_GPU: &str = "SELECT * FROM jobs WHERE kind = 'render' AND engine = 'ming' \
     AND state = 'failed' AND updated_at_ms > ? ORDER BY created_at_ms LIMIT 20";
/// GPU work admitted but not started yet (not in the ledger).
pub const JOB_PENDING_GPU: &str =
    "SELECT id, requested_s FROM jobs WHERE kind = 'render' AND engine = 'ming' AND state = 'queued'";
pub const JOB_DELETE_BY_VOICE: &str = "DELETE FROM jobs WHERE voice = ?";
pub const JOB_PRUNE_FINISHED: &str = "DELETE FROM jobs WHERE state IN ('ready', 'failed') AND updated_at_ms < ?";

// ------------------------------------------------------------------------------------------------------------- ledger

pub const LEDGER_SINCE: &str = "SELECT at_ms, requested_s, used_s FROM gpu_ledger WHERE at_ms > ? ORDER BY at_ms";
pub const LEDGER_INSERT: &str = "INSERT INTO gpu_ledger (job_id, attempt, at_ms, requested_s, used_s) \
     VALUES (?, ?, ?, ?, 0) ON CONFLICT (job_id, attempt) DO NOTHING";
pub const LEDGER_SET_USED: &str = "UPDATE gpu_ledger SET used_s = ? WHERE job_id = ? AND attempt = ?";
pub const LEDGER_DELETE: &str = "DELETE FROM gpu_ledger WHERE job_id = ? AND attempt = ?";
pub const LEDGER_PRUNE: &str = "DELETE FROM gpu_ledger WHERE at_ms < ?";

// --------------------------------------------------------------------------------------------------------------- auth

pub const AUTH_FAILURES_SINCE: &str = "SELECT count(*) AS n, min(at_ms) AS first_ms FROM auth_failures WHERE at_ms > ?";
pub const AUTH_FAILURE_INSERT: &str = "INSERT INTO auth_failures (at_ms) VALUES (?)";
pub const AUTH_FAILURES_PRUNE: &str = "DELETE FROM auth_failures WHERE at_ms < ?";
pub const AUTH_FAILURES_CLEAR: &str = "DELETE FROM auth_failures";

// ------------------------------------------------------------------------------------------------------------- config

pub const CONFIG_GET: &str = "SELECT value FROM config WHERE key = ?";
pub const CONFIG_PUT: &str = "INSERT INTO config (key, value, updated_at_ms) VALUES (?, ?, ?) \
     ON CONFLICT (key) DO UPDATE SET value = excluded.value, updated_at_ms = excluded.updated_at_ms";
pub const CONFIG_BUMP_EPOCH: &str = "INSERT INTO config (key, value, updated_at_ms) VALUES ('session_epoch', '2', ?) \
     ON CONFLICT (key) DO UPDATE SET value = CAST(CAST(value AS INTEGER) + 1 AS TEXT), \
     updated_at_ms = excluded.updated_at_ms RETURNING value";

fn placeholders(n: usize) -> String {
    vec!["?"; n.max(1)].join(", ")
}

#[cfg(test)]
mod tests {
    #[test]
    fn placeholder_lists_have_one_marker_per_param() {
        assert!(super::render_select_keys(3).ends_with("IN (?, ?, ?)"));
        assert!(super::render_select_keys(0).ends_with("IN (?)"));
    }
}
