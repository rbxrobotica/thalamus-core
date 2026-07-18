-- 0004_maintenance_grants.sql
-- Gate E decision D3 (retention): the daily maintenance job purges
-- route_envelopes rows older than 30 days using the app credential.
-- route_envelopes is a correlation cache with retention semantics, NOT part
-- of the append-only audit chain (audit_events stays protected: the app
-- role holds no UPDATE/DELETE there). Default privileges never granted
-- DELETE, so grant it explicitly and only here.
DO $do$
BEGIN
    IF EXISTS (SELECT FROM pg_catalog.pg_roles WHERE rolname = 'thalamus_app') THEN
        EXECUTE 'GRANT DELETE ON TABLE route_envelopes TO thalamus_app';
    END IF;
END
$do$;
