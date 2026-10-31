-- nolabs D1 schema, version 2: accounts moved to Supabase (decision D10, notes/14).
-- Document metadata and progress now live in Supabase (public.documents, public.progress); the passphrase login is
-- gone, and with it the failure counter and the session epoch. D1 keeps the render pipeline (renders, jobs,
-- gpu_ledger), the private half of user voices, and a short-lived cache of ownership checks.
--
-- Data note: rows of the single-user era are not carried over. Their documents' R2 bodies stay at docs/<id>/body.json
-- (the new layout is docs/<user_id>/<id>/body.json) until removed by hand; voices without an owner become unusable.

DROP TABLE documents;
DROP TABLE auth_failures;
DELETE FROM config WHERE key = 'session_epoch';

-- The user's voices: the owner, and what the render pipeline needs that the user's Supabase row does not hold
-- (R2 keys, the kept transcript span). `synced_state` is the state last written to the Supabase row, so the Worker
-- knows which rows to update the next time the owner's token is at hand (the queue consumer has no token).
ALTER TABLE voices ADD COLUMN user_id TEXT NOT NULL DEFAULT '';
ALTER TABLE voices ADD COLUMN synced_state TEXT;
CREATE INDEX voices_by_user ON voices (user_id, created_at_ms);

-- Who asked for each job (GPU budget is charged per user) and that user's daily share when the job was admitted,
-- so the consumer and the cron can re-check the share without a user token. Empty for jobs from before accounts.
ALTER TABLE jobs ADD COLUMN user_id TEXT NOT NULL DEFAULT '';
ALTER TABLE jobs ADD COLUMN gpu_share_s INTEGER NOT NULL DEFAULT 0;
CREATE INDEX jobs_by_user ON jobs (user_id, state);

ALTER TABLE gpu_ledger ADD COLUMN user_id TEXT NOT NULL DEFAULT '';
CREATE INDEX gpu_ledger_by_user ON gpu_ledger (user_id, at_ms);

-- Positive ownership answers from Supabase (RLS decides them), kept a few minutes so every audio-heavy reading
-- session does not query PostgREST per request. kind: 'doc' (owned and ready) or 'voice'.
CREATE TABLE owner_cache (
    kind            TEXT NOT NULL CHECK (kind IN ('doc', 'voice')),
    object_id       TEXT NOT NULL,
    user_id         TEXT NOT NULL,
    expires_at_ms   INTEGER NOT NULL,
    PRIMARY KEY (kind, object_id, user_id)
);
CREATE INDEX owner_cache_by_expiry ON owner_cache (expires_at_ms);
