-- 0005_run_ledger.sql
-- One question, one query: what did this run cost, how long did it take, how
-- many tokens did it burn, who asked for it, in which session, on which
-- model. Those facts already exist, split between sessions, runs and the
-- run's outcome metadata; the ledger joins them so auditing does not depend
-- on knowing the shape of a jsonb blob.
--
-- A view, not a table: it derives from the rows the governed path already
-- writes, so it cannot drift from them and needs no backfill.
--
-- cost_micros is millionths of a currency unit. cost_basis says what the
-- amount means: 'metered' (billed per token), 'subscription' (seat already
-- paid, marginal cost zero) or 'unpriced' (no rate configured for the alias,
-- amount deliberately null rather than a fabricated zero).

CREATE OR REPLACE VIEW run_ledger AS
SELECT
    r.run_id,
    r.session_id,
    s.principal,
    s.tenant,
    s.product,
    s.workflow,
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
    'Per-run audit projection: principal, session, model, tokens, latency and cost.';

-- The app role reads its own ledger; it still cannot write it (a view over
-- append-only-by-policy tables), and audit_events remains untouched.
DO $do$
BEGIN
    IF EXISTS (SELECT FROM pg_catalog.pg_roles WHERE rolname = 'thalamus_app') THEN
        EXECUTE 'GRANT SELECT ON run_ledger TO thalamus_app';
    END IF;
END
$do$;
