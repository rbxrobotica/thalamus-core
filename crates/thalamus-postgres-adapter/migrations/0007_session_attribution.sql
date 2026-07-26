-- 0007_session_attribution.sql
-- "Who spent these tokens" was already answerable: principal comes from the
-- verified credential. "On what" stopped at product + workflow, which for a
-- coding agent is too coarse to audit: every session in a product looks the
-- same. The repository and branch the agent works in are the missing piece,
-- and the Bridge launcher already resolves the canonical remote to check it
-- against the allowlist before discarding it.
--
-- ATTESTATION BOUNDARY, deliberate and load-bearing: principal and
-- delegation_token_id are derived server-side from the verified credential
-- and a caller cannot influence them. repository and branch are DECLARED by
-- the client. They are attribution, not proof. An auditor reading these
-- columns must know the difference, which is why they are documented here
-- rather than only in the API. The allowlist gate still constrains what a
-- launcher will run against; it does not make the declared value evidence.
--
-- Both columns are nullable: sessions created before this migration, and any
-- caller that is not a repository-bound agent, legitimately have none.

ALTER TABLE sessions ADD COLUMN IF NOT EXISTS repository text;
ALTER TABLE sessions ADD COLUMN IF NOT EXISTS branch text;

COMMENT ON COLUMN sessions.repository IS
    'Client-declared canonical repo (host/org/repo). Attribution, not attested.';
COMMENT ON COLUMN sessions.branch IS
    'Client-declared branch. Attribution, not attested.';

CREATE INDEX IF NOT EXISTS sessions_repository_idx ON sessions (repository);

-- Rebuilt rather than CREATE OR REPLACE so the new columns sit next to the
-- other attribution fields instead of being appended after the timings.
-- Atomic inside the migration's transaction, and the grant is reapplied in
-- the same step, so no window exists where the view is missing or unreadable.
DROP VIEW IF EXISTS run_ledger;

CREATE VIEW run_ledger AS
SELECT
    r.run_id,
    r.session_id,
    s.principal,
    s.tenant,
    s.product,
    s.workflow,
    s.repository,
    s.branch,
    s.governance_mode,
    r.model_alias,
    r.backend_id,
    r.status,
    r.execution_state,
    COALESCE(
        (r.metadata -> 'usage' ->> 'prompt_tokens')::bigint,
        (r.metadata -> 'partial_usage' ->> 'prompt_tokens')::bigint
    ) AS prompt_tokens,
    COALESCE(
        (r.metadata -> 'usage' ->> 'completion_tokens')::bigint,
        (r.metadata -> 'partial_usage' ->> 'completion_tokens')::bigint
    ) AS completion_tokens,
    COALESCE(
        (r.metadata -> 'usage' ->> 'total_tokens')::bigint,
        (r.metadata -> 'partial_usage' ->> 'total_tokens')::bigint
    ) AS total_tokens,
    (r.metadata ->> 'latency_ms')::bigint AS latency_ms,
    (r.metadata ->> 'cost_micros')::bigint AS cost_micros,
    r.metadata ->> 'cost_basis' AS cost_basis,
    r.metadata ->> 'cost_currency' AS cost_currency,
    r.metadata ->> 'audit_id' AS audit_id,
    r.metadata ->> 'backend_error' AS backend_error,
    r.metadata ->> 'post_call_status' AS post_call_status,
    r.started_at,
    r.finished_at,
    EXTRACT(EPOCH FROM (r.finished_at - r.started_at)) * 1000 AS wall_ms
FROM runs r
JOIN sessions s USING (session_id);

COMMENT ON VIEW run_ledger IS
    'Per-run audit projection: principal, session, repo/branch, model, tokens, latency and cost. principal is attested by the credential; repository and branch are client-declared.';

DO $do$
BEGIN
    IF EXISTS (SELECT FROM pg_catalog.pg_roles WHERE rolname = 'thalamus_app') THEN
        EXECUTE 'GRANT SELECT ON run_ledger TO thalamus_app';
    END IF;
END
$do$;
