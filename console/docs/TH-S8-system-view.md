# TH-S8 System View

The console System view is a read-only topology snapshot for the lean Thalamus
runtime described by ADR-0010.

It shows:

- Lean core: whether the console has a configured Thalamus server connection.
- Observability: `HTTP exporter -> rbx-observability (ADR-0300)` when an RBX
  observability URL or token is configured, otherwise the Langfuse default path.
- Service discovery: AI backends are discovered through `rbx-maestro`
  (ADR-0400); routing decisions stay in Thalamus.
- Memory: `rbx-memory` is shown as an external dependency when configured,
  preserving the ADR-0010 slim-down boundary.

The view does not make heavy live calls during render, check, or build. It reads
the same session-backed console configuration used by Settings and degrades with
the existing `Unavailable` component when the Thalamus base URL is absent.

References:

- ADR-0010: Thalamus Slim-Down and Ecosystem Reorganization
- ADR-0300: rbx-observability as General RBX Observability
- ADR-0400: rbx-maestro Expanded to General Service Discovery
