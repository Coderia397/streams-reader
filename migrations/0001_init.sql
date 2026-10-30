-- nolabs D1 schema, version 1. Forward-only: later changes go in 0002_*.sql and never edit this file.
-- Times are Unix milliseconds (INTEGER). Booleans are 0/1. JSON columns hold the serde form of the named core type.
-- Every query lives in crates/server/src/repo/sql.rs; the server tests run this file on SQLite before each test.

-- Imported documents. The body (DocumentBody JSON) lives in R2 at docs/<id>/body.json.
CREATE TABLE documents (
    id              TEXT PRIMARY KEY NOT NULL,
    meta_json       TEXT NOT NULL,                       -- proto::api::DocumentMeta
    body_bytes      INTEGER NOT NULL CHECK (body_bytes > 0),
    ready           INTEGER NOT NULL DEFAULT 0 CHECK (ready IN (0, 1)),
    progress_offset INTEGER,                             -- proto::api::Progress.char_offset
    progress_at_ms  INTEGER,                             -- proto::api::Progress.updated_at_ms
    created_at_ms   INTEGER NOT NULL,
    updated_at_ms   INTEGER NOT NULL
);
CREATE INDEX documents_by_updated ON documents (updated_at_ms DESC);

-- The user's own voices (Ming reference voices). Raw recording at voices/<id>/raw.wav, prepared clip at ref_key.
CREATE TABLE voices (
    id              TEXT PRIMARY KEY NOT NULL,
    name            TEXT NOT NULL,
    kind            TEXT NOT NULL DEFAULT 'clone' CHECK (kind IN ('clone', 'designed')),
    state           TEXT NOT NULL CHECK (state IN ('awaiting_reference', 'preparing', 'ready', 'failed')),
    transcript      TEXT NOT NULL,                       -- the whole guided script as recorded
    reference_bytes INTEGER NOT NULL CHECK (reference_bytes > 0),
    raw_uploaded    INTEGER NOT NULL DEFAULT 0 CHECK (raw_uploaded IN (0, 1)),
    ref_key         TEXT,                                -- set by preparation
    ref_transcript  TEXT,                                -- transcript of the kept span only
    ref_sha256      TEXT,                                -- part of the voice fingerprint in cache keys
    ref_duration_ms INTEGER,
    error           TEXT,
    created_at_ms   INTEGER NOT NULL,
    updated_at_ms   INTEGER NOT NULL
);

-- One row per rendered (or rendering) segment, keyed by the content-addressed cache key.
-- retry_at_ms: for 'deferred' and 'queued' the earliest retry; for 'failed' when a new request may claim it again.
CREATE TABLE renders (
    key             TEXT PRIMARY KEY NOT NULL,
    engine          TEXT NOT NULL CHECK (engine IN ('kokoro', 'ming')),
    voice           TEXT NOT NULL,                       -- voice id as in the catalogue (kokoro:…, ming:…, user:…)
    state           TEXT NOT NULL CHECK (state IN ('queued', 'rendering', 'ready', 'deferred', 'failed')),
    job_id          TEXT,
    object_key      TEXT NOT NULL,
    chars           INTEGER NOT NULL,
    duration_ms     INTEGER,
    bytes           INTEGER,
    words_json      TEXT,                                -- [[start_ms, end_ms, char_start, char_end], …]
    error_code      TEXT,
    error_message   TEXT,
    retry_at_ms     INTEGER,
    created_at_ms   INTEGER NOT NULL,
    updated_at_ms   INTEGER NOT NULL
);
CREATE INDEX renders_by_job ON renders (job_id);
CREATE INDEX renders_by_voice ON renders (voice);
CREATE INDEX renders_by_state ON renders (state, updated_at_ms);

-- Units of work for the queue consumer: a render batch for one engine and voice, or a reference preparation.
-- request_json is the exact JSON sent to the Space (EngineRequest or PrepareReferenceRequest).
CREATE TABLE jobs (
    id              TEXT PRIMARY KEY NOT NULL,
    kind            TEXT NOT NULL CHECK (kind IN ('render', 'prepare')),
    engine          TEXT NOT NULL CHECK (engine IN ('kokoro', 'ming')),
    voice           TEXT NOT NULL,
    state           TEXT NOT NULL CHECK (state IN ('queued', 'rendering', 'ready', 'deferred', 'failed')),
    request_json    TEXT NOT NULL,
    chars           INTEGER NOT NULL DEFAULT 0,
    requested_s     INTEGER NOT NULL DEFAULT 0,          -- GPU seconds requested (Ming only)
    attempts        INTEGER NOT NULL DEFAULT 0,
    lease_until_ms  INTEGER,
    retry_at_ms     INTEGER,
    error_code      TEXT,
    error_message   TEXT,
    created_at_ms   INTEGER NOT NULL,
    updated_at_ms   INTEGER NOT NULL
);
CREATE INDEX jobs_by_state ON jobs (state, retry_at_ms);
CREATE INDEX jobs_by_voice ON jobs (voice);

-- Every ZeroGPU call, written before the call (used_s = 0) and completed after it.
CREATE TABLE gpu_ledger (
    job_id          TEXT NOT NULL,
    attempt         INTEGER NOT NULL,
    at_ms           INTEGER NOT NULL,
    requested_s     INTEGER NOT NULL,
    used_s          INTEGER NOT NULL DEFAULT 0,
    PRIMARY KEY (job_id, attempt)
);
CREATE INDEX gpu_ledger_by_time ON gpu_ledger (at_ms);

-- Failed passphrase attempts, for the 20-per-hour lockout. Pruned by the cron after a day.
CREATE TABLE auth_failures (
    id              INTEGER PRIMARY KEY,
    at_ms           INTEGER NOT NULL
);
CREATE INDEX auth_failures_by_time ON auth_failures (at_ms);

-- Small key/value settings: session_epoch (bump to revoke every session), space_status (SpaceStatus JSON).
CREATE TABLE config (
    key             TEXT PRIMARY KEY NOT NULL,
    value           TEXT NOT NULL,
    updated_at_ms   INTEGER NOT NULL
);
INSERT INTO config (key, value, updated_at_ms) VALUES ('session_epoch', '1', 0);
