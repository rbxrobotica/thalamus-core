-- 0001_audit_schema.sql
-- Phase 2 (execution master plan §2): durable audit store on Jaguar.
-- Applied by thalamus_migrator, the exclusive migration owner. The runner
-- (thalamus-migrate) wraps each migration in a transaction; do not add
-- BEGIN/COMMIT here.

-- === Session / run hierarchy ===

CREATE TABLE IF NOT EXISTS sessions (
    session_id uuid PRIMARY KEY,
    tenant text NOT NULL,
    product text NOT NULL,
    workflow text NOT NULL,
    principal text,
    delegation_token_id text,
    status text NOT NULL DEFAULT 'open',
    retention_class text NOT NULL DEFAULT 'standard',
    created_at timestamptz NOT NULL DEFAULT now(),
    updated_at timestamptz NOT NULL DEFAULT now()
);
CREATE INDEX IF NOT EXISTS sessions_tenant_product_idx ON sessions (tenant, product);
CREATE INDEX IF NOT EXISTS sessions_created_at_idx ON sessions (created_at);
CREATE INDEX IF NOT EXISTS sessions_retention_idx ON sessions (retention_class);

CREATE TABLE IF NOT EXISTS runs (
    run_id uuid PRIMARY KEY,
    session_id uuid REFERENCES sessions (session_id),
    status text NOT NULL DEFAULT 'started',
    model_alias text,
    backend_id text,
    started_at timestamptz NOT NULL DEFAULT now(),
    finished_at timestamptz,
    metadata jsonb NOT NULL DEFAULT '{}'::jsonb
);
CREATE INDEX IF NOT EXISTS runs_session_idx ON runs (session_id);
CREATE INDEX IF NOT EXISTS runs_started_at_idx ON runs (started_at);

CREATE TABLE IF NOT EXISTS tool_invocations (
    invocation_id uuid PRIMARY KEY,
    run_id uuid REFERENCES runs (run_id),
    tool text NOT NULL,
    status text NOT NULL,
    requested_at timestamptz NOT NULL DEFAULT now(),
    completed_at timestamptz,
    metadata jsonb NOT NULL DEFAULT '{}'::jsonb
);
CREATE INDEX IF NOT EXISTS tool_invocations_run_idx ON tool_invocations (run_id);

-- === Append-only audit log with per-stream hash chain ===

CREATE TABLE IF NOT EXISTS audit_streams (
    stream_id text PRIMARY KEY,
    last_seq bigint NOT NULL DEFAULT 0,
    last_hash text NOT NULL DEFAULT '',
    updated_at timestamptz NOT NULL DEFAULT now()
);

CREATE TABLE IF NOT EXISTS audit_events (
    event_id uuid PRIMARY KEY DEFAULT gen_random_uuid(),
    stream_id text NOT NULL,
    seq bigint NOT NULL,
    event_type text NOT NULL,
    audit_id text,
    trace_id text,
    session_id uuid,
    run_id uuid,
    payload jsonb NOT NULL,
    previous_hash text NOT NULL DEFAULT '',
    event_hash text NOT NULL,
    idempotency_key text NOT NULL,
    retention_class text NOT NULL DEFAULT 'standard',
    occurred_at timestamptz,
    recorded_at timestamptz NOT NULL DEFAULT now(),
    UNIQUE (stream_id, seq),
    UNIQUE (idempotency_key)
);
CREATE INDEX IF NOT EXISTS audit_events_audit_id_idx ON audit_events (audit_id);
CREATE INDEX IF NOT EXISTS audit_events_event_type_idx ON audit_events (event_type);
CREATE INDEX IF NOT EXISTS audit_events_recorded_at_idx ON audit_events (recorded_at);
CREATE INDEX IF NOT EXISTS audit_events_session_idx ON audit_events (session_id);
CREATE INDEX IF NOT EXISTS audit_events_retention_idx ON audit_events (retention_class);

CREATE OR REPLACE FUNCTION thalamus_forbid_mutation() RETURNS trigger
LANGUAGE plpgsql AS $fn$
BEGIN
    RAISE EXCEPTION 'table % is append-only; corrections are new events', TG_TABLE_NAME;
END
$fn$;

DROP TRIGGER IF EXISTS audit_events_append_only ON audit_events;
CREATE TRIGGER audit_events_append_only
    BEFORE UPDATE OR DELETE ON audit_events
    FOR EACH ROW EXECUTE FUNCTION thalamus_forbid_mutation();

-- === Governance records ===

CREATE TABLE IF NOT EXISTS approvals (
    approval_id uuid PRIMARY KEY DEFAULT gen_random_uuid(),
    session_id uuid,
    run_id uuid,
    subject text NOT NULL,
    approver text NOT NULL,
    decision text NOT NULL,
    reason text,
    decided_at timestamptz NOT NULL DEFAULT now(),
    metadata jsonb NOT NULL DEFAULT '{}'::jsonb
);
CREATE INDEX IF NOT EXISTS approvals_session_idx ON approvals (session_id);
CREATE INDEX IF NOT EXISTS approvals_decided_at_idx ON approvals (decided_at);

CREATE TABLE IF NOT EXISTS evidence_refs (
    evidence_id uuid PRIMARY KEY DEFAULT gen_random_uuid(),
    run_id uuid,
    kind text NOT NULL,
    uri text NOT NULL,
    content_hash text NOT NULL,
    created_at timestamptz NOT NULL DEFAULT now()
);
CREATE INDEX IF NOT EXISTS evidence_refs_run_idx ON evidence_refs (run_id);

CREATE TABLE IF NOT EXISTS payload_refs (
    payload_id uuid PRIMARY KEY DEFAULT gen_random_uuid(),
    audit_event_id uuid REFERENCES audit_events (event_id),
    storage_ref text NOT NULL,
    payload_hash text NOT NULL,
    retention_class text NOT NULL DEFAULT 'short',
    expires_at timestamptz,
    created_at timestamptz NOT NULL DEFAULT now()
);
CREATE INDEX IF NOT EXISTS payload_refs_event_idx ON payload_refs (audit_event_id);
CREATE INDEX IF NOT EXISTS payload_refs_expires_idx ON payload_refs (expires_at);

CREATE TABLE IF NOT EXISTS monitoring_decisions (
    decision_id uuid PRIMARY KEY DEFAULT gen_random_uuid(),
    session_id uuid,
    rule text NOT NULL,
    decision text NOT NULL,
    reviewer text,
    created_at timestamptz NOT NULL DEFAULT now(),
    metadata jsonb NOT NULL DEFAULT '{}'::jsonb
);
CREATE INDEX IF NOT EXISTS monitoring_decisions_session_idx ON monitoring_decisions (session_id);

CREATE TABLE IF NOT EXISTS repository_exceptions (
    exception_id uuid PRIMARY KEY DEFAULT gen_random_uuid(),
    repository text NOT NULL,
    granted_to text NOT NULL,
    approver text NOT NULL,
    reason text NOT NULL,
    created_at timestamptz NOT NULL DEFAULT now(),
    expires_at timestamptz,
    revoked_at timestamptz
);
CREATE INDEX IF NOT EXISTS repository_exceptions_repo_idx ON repository_exceptions (repository);

CREATE TABLE IF NOT EXISTS budgets (
    budget_id uuid PRIMARY KEY DEFAULT gen_random_uuid(),
    scope_type text NOT NULL,
    scope_ref text NOT NULL,
    period text NOT NULL,
    max_tokens bigint,
    max_cost_usd numeric(12, 4),
    consumed_tokens bigint NOT NULL DEFAULT 0,
    consumed_cost_usd numeric(12, 4) NOT NULL DEFAULT 0,
    updated_at timestamptz NOT NULL DEFAULT now(),
    UNIQUE (scope_type, scope_ref, period)
);

CREATE TABLE IF NOT EXISTS capability_leases (
    lease_id uuid PRIMARY KEY DEFAULT gen_random_uuid(),
    session_id uuid,
    capability text NOT NULL,
    scope jsonb NOT NULL DEFAULT '{}'::jsonb,
    granted_at timestamptz NOT NULL DEFAULT now(),
    expires_at timestamptz NOT NULL,
    revoked_at timestamptz
);
CREATE INDEX IF NOT EXISTS capability_leases_session_idx ON capability_leases (session_id);
CREATE INDEX IF NOT EXISTS capability_leases_expires_idx ON capability_leases (expires_at);

-- Pre-call correlation record: envelope + policy snapshot keyed by audit_id,
-- so /v1/post-call survives server restarts. Retention TTL for the embedded
-- prompt payload is a Gate E pending decision (tracked in the master plan).
CREATE TABLE IF NOT EXISTS route_envelopes (
    audit_id text PRIMARY KEY,
    session_id uuid,
    run_id uuid,
    envelope jsonb NOT NULL,
    policy jsonb NOT NULL,
    policy_ref text,
    model_alias text,
    created_at timestamptz NOT NULL DEFAULT now()
);
CREATE INDEX IF NOT EXISTS route_envelopes_created_at_idx ON route_envelopes (created_at);

-- === Least privilege: thalamus_app never mutates audit history ===
-- (default privileges grant the app SELECT/INSERT/UPDATE; revoke mutation
-- on append-only tables; the guard is skipped where the role does not exist,
-- e.g. throwaway test databases)

DO $do$
BEGIN
    IF EXISTS (SELECT FROM pg_catalog.pg_roles WHERE rolname = 'thalamus_app') THEN
        EXECUTE 'REVOKE UPDATE, DELETE ON TABLE audit_events, approvals, evidence_refs, payload_refs, monitoring_decisions FROM thalamus_app';
    END IF;
END
$do$;
