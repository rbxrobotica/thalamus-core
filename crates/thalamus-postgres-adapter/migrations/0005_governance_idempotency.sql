-- 0005_governance_idempotency.sql
-- Additive idempotency support for POST /rbx/v1/tool-decisions and
-- POST /rbx/v1/approvals. A client-supplied idempotency_key, scoped by
-- (tenant, source_system), makes both endpoints retry-safe: a replayed
-- request with an identical payload returns the original row instead of a
-- duplicate. tenant is always derived server-side from the owning session
-- (never taken from the request body); source_system is always the
-- caller's verified client_app_id. Both columns stay NULL, and the unique
-- indexes stay inert, for any caller that never sends idempotency_key —
-- fully backward-compatible with the existing unscoped contract.

ALTER TABLE tool_invocations ADD COLUMN IF NOT EXISTS tenant text;
ALTER TABLE tool_invocations ADD COLUMN IF NOT EXISTS source_system text;
ALTER TABLE tool_invocations ADD COLUMN IF NOT EXISTS idempotency_key text;
ALTER TABLE tool_invocations ADD COLUMN IF NOT EXISTS request_fingerprint text;

CREATE UNIQUE INDEX IF NOT EXISTS tool_invocations_idempotency_idx
    ON tool_invocations (tenant, source_system, idempotency_key)
    WHERE idempotency_key IS NOT NULL;

ALTER TABLE approvals ADD COLUMN IF NOT EXISTS tenant text;
ALTER TABLE approvals ADD COLUMN IF NOT EXISTS source_system text;
ALTER TABLE approvals ADD COLUMN IF NOT EXISTS idempotency_key text;
ALTER TABLE approvals ADD COLUMN IF NOT EXISTS request_fingerprint text;

CREATE UNIQUE INDEX IF NOT EXISTS approvals_idempotency_idx
    ON approvals (tenant, source_system, idempotency_key)
    WHERE idempotency_key IS NOT NULL;
