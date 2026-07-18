-- 0003_governed_calls.sql
-- SLICE-T1 (master plan §7 / Gate D): run-bound governed calls.
-- sessions.governance_mode records the mode a session was created under
-- (ADR-0403: external agents = governed_llm_access, never workspace claims).
-- runs.execution_state backs the 1:1 run <-> call invariant: pending ->
-- executing (atomic claim) -> executed; a second call on a run is refused.
-- route_envelopes.session_id / run_id (added in 0001) start being populated
-- by the correlated store path; no schema change needed there.

ALTER TABLE sessions
    ADD COLUMN IF NOT EXISTS governance_mode text NOT NULL DEFAULT 'governed_llm_access';

ALTER TABLE runs
    ADD COLUMN IF NOT EXISTS execution_state text NOT NULL DEFAULT 'pending';
CREATE INDEX IF NOT EXISTS runs_execution_state_idx ON runs (execution_state);
CREATE INDEX IF NOT EXISTS route_envelopes_run_idx ON route_envelopes (run_id);
CREATE INDEX IF NOT EXISTS route_envelopes_session_idx ON route_envelopes (session_id);
