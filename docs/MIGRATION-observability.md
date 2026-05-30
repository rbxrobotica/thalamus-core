# Observability Migration

This note tracks the conservative increments of the ADR-0010 / ADR-0300
Thalamus slim-down for observability.

## Current State

Thalamus still keeps `crates/thalamus-langfuse-adapter` as the default Langfuse
implementation. Runtime behavior is preserved: when the `langfuse` feature is
enabled, evaluation records are still forwarded to the Langfuse ingestion API
with the same metadata-only default content policy.

The server now routes that export through a thin `TraceExporter` seam in
`thalamus-server`. The default implementation is `LangfuseTraceExporter`, which
wraps the existing adapter instead of replacing it.

An HTTP exporter is also available behind configuration. When
`RBX_OBSERVABILITY_URL` is set, Thalamus sends eval trace spans to
`{RBX_OBSERVABILITY_URL}/v1/traces` using `Authorization: Bearer
<RBX_OBSERVABILITY_TOKEN>` when `RBX_OBSERVABILITY_TOKEN` is configured. When
`RBX_OBSERVABILITY_URL` is unset, runtime behavior is unchanged.

## New Owner

Reusable Langfuse ingestion logic now also lives in `rbx-observability` as a
Rust library module. That repo is the service owner for RBX-wide traces,
metrics, and alerting per governance ADR-0300.

## Planned Cutover

The final increment should remove Thalamus' in-process Langfuse adapter only
after `rbx-observability` is deployed and the `/v1/traces` service path is
validated. That removal is intentionally out of scope for the HTTP-exporter
increment.

## References

- `rbx-governance/docs/adr/ADR-0010-thalamus-slim-down-and-ecosystem-reorganization.md`
- `rbx-governance/docs/adr/ADR-0300-rbx-observability.md`
