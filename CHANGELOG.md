# Changelog

All notable changes to Thalamus will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Architecture phase (post-pivot)

### Added (Phase 3 slice 4 — governance endpoints + §3 security)

- `POST /rbx/v1/tool-decisions`, `POST /rbx/v1/approvals`,
  `POST /rbx/v1/evidence` behind the credential middleware, persisted on the
  Phase 2 schema (`tool_invocations`, `approvals`, `evidence_refs`) with
  lifecycle audit events. The approver always comes from the verified
  credential — an `approver` field in the body is ignored. Evidence carries
  pointer + content hash only, never the payload.
- Rate limiting on `/rbx/v1/*` per subject and client app
  (`THALAMUS_RBX_RATE_LIMIT` requests/min per key, default 120, `off`
  disables): typed `rate_limited` 429 with `Retry-After`.
- Secret redaction before operational logs (`Bearer`/JWT/`sk-`/`rbxsess_`/
  key-value tokens masked in backend-error log lines).
- `/readyz` now probes the identity verifier upstream
  (`identity_verifier` / `identity_reachable`; 503 `identity_unavailable`
  when the introspection endpoint is unreachable).

### Added (Phase 3 slice 3 — SSE streaming + mid-flight cancel)

- `BackendPort::execute_streaming(route, cancel, sink)`: content deltas
  through a sink, cancel token checked between chunks, partial usage on
  cancellation/timeout. Default impl bridges non-streaming adapters as a
  single chunk.
- LiteLLM adapter streams over the OpenAI-compatible SSE wire
  (`stream: true` + `stream_options.include_usage`); a cancelled token
  aborts the stream mid-flight with whatever usage is known; a broken
  stream after first content surfaces as timeout-class with partial usage.
- `POST /v1/call/stream`: SSE endpoint with `decision` → `chunk`* →
  `result` (post-call summary + usage) event sequence; typed `error`
  events carry `partial_usage`; Deny/AllowWithReview produce no chunks and
  never call the backend; client disconnect cancels the backend stream
  through the token; the route envelope is audited before execution.

### Added (Phase 3 slice 2 — BackendPort route envelope)

- `RouteEnvelope`, `BackendExecution`, `BackendUsage`, `BackendCallError` and
  `CancelToken` in `thalamus-core` (§3): the route envelope is the only
  authority on provider pool, region, data class, capability class, cost
  class and timeout; typed backend errors carry partial usage for
  interrupted calls.
- `BackendPort::execute(route, cancel)` with a compatibility bridge for
  legacy adapters (empty content maps to typed `backend_unavailable`).
- `AuditEvent::RouteEnvelope` emitted before every model call on `/v1/call`
  (§3 acceptance: route envelope audited for every model call).
- LiteLLM adapter implements `execute` with an internal
  `BackendExecutionPlan`: provider-pool and model-alias crossings are refused
  (`envelope_violation`) before any wire call; per-request timeout from the
  route envelope; usage (prompt/completion/total) and backend metadata
  returned; 429 maps to `backend_rate_limited`.
- `/v1/call` compatibility preserved: typed backend failures surface in a new
  additive `backend_error` field, post-call still runs, status stays 200.

### Added (Phase 3 slice 1 — session/run lifecycle API)

- Lifecycle domain types in `thalamus-core` (`SessionRecord`, `RunRecord`,
  `SessionLimits`, `BudgetLine`, statuses) and a new `AuditEvent::Lifecycle`
  variant: a session's whole lifecycle (created, runs, refusals, closed) forms
  one hash chain keyed by session id.
- `/rbx/v1` lifecycle endpoints behind the Gate A credential middleware:
  `POST /sessions`, `POST /sessions/{id}/runs`, `POST /sessions/{id}/close`,
  `GET /sessions/{id}/limits`, `POST /runs/{id}/cancel`. Principal and
  delegation token id come from the verified credential, never the body.
- Budget enforcement (§3 acceptance): run creation locks governing budget rows
  (session/product/tenant scope) in one transaction and refuses with typed
  `budget_exceeded` when exhausted; `limits` reports budget lines plus the
  initial 70% context-utilization policy (`context-utilization-70`).
- Idempotency keys for session/run creation (migration
  `0002_lifecycle_idempotency`) — replays return the original row.
- §3 security: 256 KiB body limit on the `/rbx/v1/*` surface; typed-error
  responses (`unknown_session`, `session_closed`, `budget_exceeded`,
  `store_unavailable`).
- `SessionStore` port: in-memory by default, durable Postgres store
  (Phase 2 schema) when `THALAMUS_DATABASE_URL` is wired.

### Added (Phase 2 — durable audit store on Jaguar)

- `crates/thalamus-postgres-adapter`: authoritative Postgres audit store
  (execution master plan §2). Hash-chained append-only `audit_events`
  (per-stream `seq`, `previous_hash`/`event_hash`), content-derived
  idempotency keys (retry/duplicate safe), and the full §2 schema
  (`sessions`, `runs`, `tool_invocations`, `audit_events`, `approvals`,
  `evidence_refs`, `payload_refs`, `monitoring_decisions`,
  `repository_exceptions`, `budgets`, `capability_leases`,
  `route_envelopes`) via embedded migrations owned by `thalamus_migrator`.
- `thalamus-migrate` bin: migration runner (`THALAMUS_MIGRATE_DATABASE_URL`).
- `thalamus-server` `postgres` feature: `THALAMUS_DATABASE_URL` wires the
  durable store as authoritative (disable with `THALAMUS_DURABLE_AUDIT=off`).
  Pre-call records persist in `route_envelopes`, so `/v1/post-call` and
  `/v1/audit/{id}` survive restarts. Fail-closed: startup aborts if the store
  is configured but unreachable; `/readyz` and the call routes return 503
  when authoritative audit writes are unavailable. In-memory audit is no
  longer authoritative when the durable store is wired.

## [0.2.0] - 2026-05-16

### Changed (BREAKING: definition pivot)

Thalamus is redefined as **the semantic control layer for AI traffic**. The
0.1.0 "signal mediation layer / biological thalamus" definition is superseded.
See [docs/adr/ADR-0001-thalamus-as-semantic-control-layer.md](docs/adr/ADR-0001-thalamus-as-semantic-control-layer.md).

- README.md, PURPOSE.md, ARCHITECTURE.md, BOUNDARIES.md, GOVERNANCE.md,
  CONTRIBUTING.md rewritten for the control-plane definition.
- docs/00-getting-started/glossary.md and for-agents.md rewritten.
- .claude/agent-guidelines.md rewritten.
- docs/99-reference/design-decisions.md: Foundation decisions marked Superseded
  by ADR-0001; new decisions recorded.

### Added

- docs/adr/ADR-0001-thalamus-as-semantic-control-layer.md (the pivot decision)
- docs/02-architecture/target-architecture.md (components, ports, language)
- docs/02-architecture/pre-call-and-post-call-responsibilities.md
- docs/02-architecture/observability-and-evaluation.md (OpenTelemetry, Langfuse,
  Prometheus, Grafana; rbx-infra monitoring reality)
- docs/02-architecture/agentgateway-and-data-plane.md
- docs/03-integration/cross-product-positioning.md (Strategos, TruthMetal,
  Robson, Eden)

### Governed decisions in this release

- Thalamus is the control plane for AI traffic; the data plane is replaceable.
- Gateway-agnostic at the product layer; Agentgateway-native only at the
  adapter layer. `thalamus-core` must not depend on gateway types.
- Rust-first for core, server, policy engine; Python and TypeScript SDKs;
  TypeScript admin UI; no Zig for v0/v1.
- The Five-Question Framework is replaced by the control-boundary framework.

### Removed

- The "Phase 0 = no technology, no code" constraint (technology is now decided).
- The biological-thalamus model as the normative architecture (kept only as
  historical context).

## [0.1.0] - 2026-02-02

### Added

**Critical Foundation Documents:**
- BOUNDARIES.md - Definitive boundary framework with Five-Question validation
- README.md - Project overview and navigation
- ARCHITECTURE.md - Conceptual architecture (pre-implementation)
- PURPOSE.md - Vision, rationale, and biological inspiration
- GOVERNANCE.md - Decision framework with three-tier model (Autonomous/Reviewed/Governed)
- CONTRIBUTING.md - Contribution guidelines for humans and AI agents
- CHANGELOG.md - This file

**Agent Documentation:**
- .claude/agent-guidelines.md - Operational manual for AI agents

**Getting Started Documentation:**
- docs/00-getting-started/for-agents.md - Quick start guide for AI agents
- docs/00-getting-started/glossary.md - Terminology definitions

**Reference Documentation:**
- docs/99-reference/design-decisions.md - Design decision log with initial foundation decisions

**Repository Structure:**
- Created directory structure for documentation (docs/00-getting-started/, docs/01-concept/, docs/02-architecture/, docs/03-integration/, docs/04-governance/, docs/05-development/, docs/99-reference/)

**Key Architectural Decisions:**
- Establish foundation before implementation (Phase 0 → Phase 1)
- Use biological thalamus as architectural model
- Implement Five-Question Framework for boundary validation
- AI-first, human-centered governance model with three decision tiers
- Documentation-first architecture approach
- Single repository for Thalamus core

**Core Principles Established:**
- Signal mediation, not decision-making
- Business-logic agnostic design
- Reusable across all RBX products
- Short-term context management only
- Configuration-driven behavior

### Documentation

- All critical foundation documents completed
- Agent contribution framework established
- Boundary enforcement mechanisms defined
- Decision documentation process established

### Status

- **Phase**: 0 (Foundation)
- **Version**: 0.1.0 (Pre-release)
- **Implementation**: None (intentional - documentation phase)
- **Next Phase**: Implementation (Phase 1) - Pending human approval

---

## Version History Guidelines

### Version Number Format

Thalamus follows [Semantic Versioning](https://semver.org/): `MAJOR.MINOR.PATCH`

- **MAJOR**: Breaking changes to API or architecture
- **MINOR**: New features, backward-compatible
- **PATCH**: Bug fixes, documentation, non-breaking changes

### Phase-Version Mapping

- **Phase 0 (Foundation)**: `0.x.x` (Pre-release versions)
- **Phase 1 (Implementation)**: `0.x.x` → `1.0.0` (First production-ready release)
- **Phase 2+ (Integration, Evolution)**: `1.x.x` onwards

### Changelog Categories

Use these categories for entries:

- **Added**: New features, capabilities, documentation
- **Changed**: Changes to existing functionality
- **Deprecated**: Features marked for future removal
- **Removed**: Removed features
- **Fixed**: Bug fixes
- **Security**: Security improvements or fixes
- **Documentation**: Documentation-only changes (in Foundation phase)

### Changelog Maintenance

- Update this file with every significant change
- Group changes by version and date
- Include links to design decisions for architectural changes
- Note phase transitions
- Document breaking changes clearly
- Add migration guides for major versions

---

## Future Versions

### [0.2.0] - TBD

**Planned:**
- Complete docs/01-concept/ (biological inspiration, signal theory, cognitive routing, state management)
- Complete docs/02-architecture/ (system boundaries, data flow, layering, reusability)
- Complete docs/03-integration/ (Strategos integration guide, Robson integration guide)
- Complete docs/04-governance/ (detailed governance documentation)
- Complete docs/05-development/ (development principles and philosophy)

### [1.0.0] - TBD (Phase 1 Completion)

**Planned:**
- Core signal routing implementation
- Signal normalization layer
- Context management system
- Integration interfaces
- Test framework
- Performance validation
- First production-ready release

**Breaking Changes:**
- N/A (first major version)

**Migration Guide:**
- Not applicable (initial release)

---

## Contributing to Changelog

### When to Update

Update CHANGELOG.md when:
- Adding significant features or documentation
- Making architectural changes
- Fixing important bugs
- Releasing new versions
- Making breaking changes
- Transitioning phases

### How to Update

1. Add entry under "Unreleased" section
2. Use appropriate category (Added, Changed, etc.)
3. Write clear, user-facing description
4. Link to design decisions for architectural changes
5. Note any breaking changes prominently
6. When releasing, move "Unreleased" to versioned section

### Example Entry

```markdown
## [Unreleased]

### Added
- Signal compression concept to ARCHITECTURE.md (refs: design-decisions.md#2026-02-15-signal-compression)

### Changed
- Updated routing layer description with priority queue details

### Documentation
- Enhanced examples in BOUNDARIES.md for clarity
```

---

## Links and References

- [BOUNDARIES.md](BOUNDARIES.md) - Boundary framework
- [GOVERNANCE.md](GOVERNANCE.md) - Version and release policies
- [design-decisions.md](docs/99-reference/design-decisions.md) - Architectural decisions
- [Keep a Changelog](https://keepachangelog.com/en/1.0.0/) - Changelog format
- [Semantic Versioning](https://semver.org/) - Versioning specification

---

**Note**: This changelog begins with the foundational documentation phase. Implementation changes will be tracked starting in Phase 1.

*Last updated: 2026-02-02*
