# Observability Migration

This note tracks the first conservative increment of the ADR-0010 / ADR-0300
Thalamus slim-down for observability.

## Current State

Thalamus still keeps `crates/thalamus-langfuse-adapter` as the default Langfuse
implementation. Runtime behavior is preserved: when the `langfuse` feature is
enabled, evaluation records are still forwarded to the Langfuse ingestion API
with the same metadata-only default content policy.

The server now routes that export through a thin `TraceExporter` seam in
`thalamus-server`. The default implementation is `LangfuseTraceExporter`, which
wraps the existing adapter instead of replacing it.

## New Owner

Reusable Langfuse ingestion logic now also lives in `rbx-observability` as a
Rust library module. That repo is the service owner for RBX-wide traces,
metrics, and alerting per governance ADR-0300.

## Planned Cutover

The next increment should keep the `TraceExporter` trait and replace
`LangfuseTraceExporter` with an HTTP exporter that sends trace/eval submissions
to `rbx-observability` once its real ingestion endpoint is live.

After that service path is verified, a later increment can remove
`thalamus-langfuse-adapter` from Thalamus.

## References

- `rbx-governance/docs/adr/ADR-0010-thalamus-slim-down-and-ecosystem-reorganization.md`
- `rbx-governance/docs/adr/ADR-0300-rbx-observability.md`
