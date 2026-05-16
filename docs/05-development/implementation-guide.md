# Thalamus Implementation Guide: Nomenclature, Glossary, Version Control

**Version**: 0.2.0 | **Last Updated**: 2026-05-16
**Status**: Binding for all implementation work post-pivot.
**Canonical references (read first, in order)**:
1. `docs/adr/ADR-0001-thalamus-as-semantic-control-layer.md`
2. `BOUNDARIES.md`
3. `docs/02-architecture/target-architecture.md`
4. `docs/02-architecture/pre-call-and-post-call-responsibilities.md`

This guide does not reopen the pivot, propose new architecture, or add ports
beyond ADR-0001. It defines how the implementation is named, versioned, and
sliced so humans and the implementing agent stay consistent.

## 1. Glossary (canonical pointer)

The domain glossary is `docs/00-getting-started/glossary.md`. It is the single
source of truth for domain terms (control plane, data plane, pre-call,
post-call, policy, envelope, routing decision, risk classification, audit
event, port, etc.). Do not redefine those terms here or anywhere else.

Version-control-specific terms (scoped to this guide only):

| Term | Meaning |
|------|---------|
| Slice (`TH-S<n>`) | A vertically scoped, independently reviewable unit of implementation work. `TH-S1` is the first. Sub-units: `TH-S<n>.<k>`. |
| Phase | A named lifecycle stage from `GOVERNANCE.md`: Architecture, Implementation, Integration, Evolution. Never written as bare "Phase N". |
| ADR | Architecture Decision Record under `docs/adr/ADR-NNNN-<kebab>.md`. ADR-0001 is the pivot. |
| IG | Implementation Guide. This file is the root IG; per-slice guides are `docs/05-development/IG-TH-S<n>-<slug>.md`. |
| Port | A trait defined in `thalamus-core` (see Transition Contract). The only allowed extension seam. |

## 2. Component and crate nomenclature (from ADR-0001, fixed)

| Crate / package | Import / dir | Language | Do not rename |
|-----------------|--------------|----------|---------------|
| `thalamus-core` | `thalamus_core` | Rust | yes |
| `thalamus-server` | `thalamus_server` | Rust | yes |
| `thalamus-agentgateway-adapter` | `thalamus_agentgateway_adapter` | Rust | yes |
| `thalamus-eval` | `thalamus_eval` | Rust + SDK | yes |
| `thalamus-sdk-python` | `thalamus` (PyPI: `thalamus-sdk`) | Python | yes |
| `thalamus-sdk-ts` | `@rbx/thalamus-sdk` | TypeScript | yes |
| `thalamus-console` | n/a | TypeScript | yes |

Workspace layout for the Rust side (slice-1 scope is `thalamus-core` only):

```
thalamus-core/                 (this repo; Cargo workspace root added in TH-S1)
  crates/
    thalamus-core/             TH-S1: domain, policy model, ports, flow (pure)
    thalamus-server/           TH-S2 (not now)
    thalamus-agentgateway-adapter/  TH-S3+ (not now)
    thalamus-eval/             later (not now)
  docs/                        (existing; unchanged by code work)
```

If a Cargo workspace does not exist yet, TH-S1 creates it with exactly one
member crate `crates/thalamus-core`. Do not scaffold the other crates.

## 3. Rust module and type nomenclature (binding)

`thalamus-core` module boundaries (from the Transition Contract; do not invent
others in TH-S1):

```
crates/thalamus-core/src/
  lib.rs            re-exports public API
  domain/           CallRequest, Envelope, PolicyDecision, PostCallResult,
                    AuditEvent, RiskLevel
  policy/           Policy, Budget, ContextGrant, RedactionRule, PolicyEngine (trait)
  ports/            BackendPort, ContextPort, PolicyPort, AuditPort,
                    EvalPort, ObservabilityPort
  flow/             pre_call(), post_call() orchestration over ports (pure)
  audit/            AuditEvent schema (types only)
```

Dependency direction: `domain`, `policy`, `flow`, `audit` may depend on
`ports`. `ports` depends only on `domain`/`policy` types. Nothing depends on an
adapter. `thalamus-core` `Cargo.toml` has no gateway/provider/HTTP-client
dependency.

Prefer these names: `ControlPlane`, `PreCall`, `PostCall`, `PolicyDecision`,
`RoutingDecision`, `Envelope`, `RiskLevel`, `AuditEvent`, `BackendPort`,
`PolicyEngine`, `ContextGrant`.

Forbidden names (compile-time review fails if present): `SignalRouter`,
`SignalBus`, `MessageBroker`, `Relay`, `CognitiveChamber`, `Nervous*`,
`analytical_signals`, `Gateway` as a domain type, `LlmProxy`,
`AgentgatewayClient` anywhere in `thalamus-core`.

Enum spellings are fixed: `PolicyDecision = Allow | Deny | AllowWithReview`;
`RiskLevel = Low | Medium | High | Prohibited`; `PostCallResult.status =
Valid | Invalid | NeedsHumanReview`.

## 4. Version control conventions (binding)

### 4.1 Branch naming

| Purpose | Pattern | Example |
|---------|---------|---------|
| Cross-repo pivot alignment (done) | `thalamus-<topic>` | `thalamus-semantic-control-layer-pivot` |
| Implementation slice | `thalamus-th-s<n>-<slug>` | `thalamus-th-s1-core-crate` |
| Fix on a slice | `thalamus-th-s<n>-fix-<slug>` | `thalamus-th-s1-fix-port-trait` |

Never commit implementation work directly to `main`. One branch per slice. One
PR per branch. The implementing agent never pushes without explicit
per-operation operator authorization in the operator's current message.

### 4.2 Commit messages (Conventional Commits)

```
<type>(<scope>): <imperative subject>

<body: what and why, boundary note if relevant>

Refs ADR-0001 [, Refs TH-S<n>]
Co-Authored-By: <agent> <email>   # only when agent-authored
```

- `type`: `feat`, `fix`, `docs`, `refactor`, `test`, `chore`.
- `scope`: `core`, `server`, `policy`, `adapter`, `sdk-py`, `sdk-ts`, `eval`,
  `console`, `docs`. TH-S1 uses `core`.
- Every implementation commit references `ADR-0001` and its slice id.
- The agent-authored co-author trailer is mandatory for agent commits.

### 4.3 Versioning and tags

- SemVer. Per `GOVERNANCE.md`: `0.1.x` Foundation (superseded), `0.2.x`
  Architecture (docs, current), `0.x -> 1.0.0` first production-ready control
  plane (`thalamus-core` + `thalamus-server` + one `BackendPort` adapter +
  SDKs).
- Each published crate carries its own SemVer in its `Cargo.toml`.
- Milestone git tags at the workspace level: `vX.Y.Z` (annotated). TH-S1 does
  not tag; tagging is an operator action at a milestone.
- Pre-1.0: no API stability guarantee. Breaking changes to the domain contract
  or any port require a new ADR and are governed.

### 4.4 ADR and IG numbering

- ADRs: `docs/adr/ADR-NNNN-<kebab-title>.md`, monotonically increasing,
  zero-padded to 4. ADR-0001 is taken. New domain-contract or port changes get
  a new ADR; do not edit ADR-0001 except to add "Superseded by" notes.
- Per-slice IGs: `docs/05-development/IG-TH-S<n>-<slug>.md`. This root IG is not
  renumbered.

## 5. Slice plan (build order, from target-architecture.md)

| Slice | Scope | Gate to next |
|-------|-------|--------------|
| **TH-S1** | `thalamus-core` crate: domain types, port traits, policy model types, pure `pre_call`/`post_call` flow, in-memory fakes, unit tests. No I/O, no adapters. | Crate compiles standalone; deny path never calls `BackendPort`; post-call always runs on allow path; forbidden-name grep clean. |
| TH-S2 | `thalamus-server`: HTTP/gRPC `/v1/decide`, `/v1/pre-call`, `/v1/post-call`, `/v1/call`, `/v1/audit`. | Endpoints serve `thalamus-core` flow; no gateway types. |
| TH-S3 | `BackendPort` adapter over existing LiteLLM data plane. | Split path (`/v1/decide` + `/v1/post-call`) usable end to end. |
| TH-S4 | `thalamus-agentgateway-adapter` (`BackendPort` over Agentgateway). | Backend swap by config, no caller change. |
| TH-S5 | `thalamus-sdk-python`, `thalamus-sdk-ts`. | Robson/Strategos can call governed path. |
| TH-S6 | `thalamus-eval` + Langfuse `EvalPort`. | Post-call evaluation submission works. |
| TH-S7 | `thalamus-console`. | Policies/traces/audit visible. |

Only `TH-S1` is in scope for the next implementation session. Do not start
TH-S2+ or introduce an `EventBusPort` or any deferred lateral integration
(OpenMetadata is explicitly Phase 3, lateral, behind a future port; see
`docs/02-architecture/observability-and-evaluation.md`).

## 6. Open items the implementer must not guess

- Policy language/representation: open. TH-S1 uses a minimal typed `Policy`
  struct plus a trait-based `PolicyEngine`; no DSL, no external taxonomy.
- Audit store schema/retention: open (must respect Postgres-external
  constraint). TH-S1 only defines `AuditEvent` types and an `AuditPort` trait
  with an in-memory fake.
- `PolicyEngine` in-process vs separate service: deferred. TH-S1 keeps it a
  trait, default impl is a simple in-process struct.

If a decision is required and not recorded in an ADR, stop and record the
ambiguity; do not invent the answer.

## 7. References

- `docs/adr/ADR-0001-thalamus-as-semantic-control-layer.md`
- `BOUNDARIES.md`, `GOVERNANCE.md`, `CONTRIBUTING.md`
- `docs/02-architecture/target-architecture.md`
- `docs/02-architecture/pre-call-and-post-call-responsibilities.md`
- `docs/05-development/glm-execution-prompt.md` (TH-S1 execution prompt)

---

*Decide and validate. Never transport. Protect the control boundary.*
*Last updated: 2026-05-16*
