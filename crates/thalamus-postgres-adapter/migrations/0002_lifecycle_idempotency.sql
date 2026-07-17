-- 0002_lifecycle_idempotency.sql
-- Phase 3 slice 1 (master plan §3): retry-safe session/run creation.
-- Client-supplied idempotency keys make POST /rbx/v1/sessions and
-- POST /rbx/v1/sessions/{id}/runs safe to retry: a replayed request returns
-- the already-created row instead of a duplicate.

ALTER TABLE sessions ADD COLUMN IF NOT EXISTS idempotency_key text;
CREATE UNIQUE INDEX IF NOT EXISTS sessions_idempotency_key_idx
    ON sessions (idempotency_key) WHERE idempotency_key IS NOT NULL;

ALTER TABLE runs ADD COLUMN IF NOT EXISTS idempotency_key text;
CREATE UNIQUE INDEX IF NOT EXISTS runs_idempotency_key_idx
    ON runs (idempotency_key) WHERE idempotency_key IS NOT NULL;

CREATE INDEX IF NOT EXISTS runs_session_status_idx ON runs (session_id, status);
