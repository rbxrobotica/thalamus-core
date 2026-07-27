# Target Architecture

**Version**: 0.2.0 | **Last Updated**: 2026-05-16

Normative reference: [ADR-0001](../adr/ADR-0001-thalamus-as-semantic-control-layer.md).
Read [BOUNDARIES.md](../../BOUNDARIES.md) and [ARCHITECTURE.md](../../ARCHITECTURE.md)
first.

## Components

```
                        +---------------------------+
                        |   thalamus-console (TS)   |
                        |   policies, traces,       |
                        |   audit, costs, evals     |
                        +-------------+-------------+
                                      | reads/writes via API
+----------------+   +----------------v----------------+   +----------------+
| thalamus-sdk-  |   |        thalamus-server          |   | thalamus-sdk-  |
| python         +-->|        (Rust service)           |<--+ ts             |
| (Robson, jobs, |   |  decide / pre_call / post_call  |   | (Strategos,    |
|  py agents)    |   |  evaluate / audit               |   |  Eden, admin)  |
+----------------+   +----------------+----------------+   +----------------+
                                      | depends on
                        +-------------v-------------+
                        |  thalamus-core (Rust)     |
                        |  domain types, policy     |
                        |  model, envelopes,        |
                        |  decisions, risk levels,  |
                        |  audit schemas, context   |
                        |  auth types, port traits  |
                        +-------------+-------------+
                                      | traits implemented by
        +-----------------+-----------+-----------+------------------+
        v                 v                       v                  v
  PolicyEngine     thalamus-          thalamus-eval          Audit / Context
  (Rust,           agentgateway-      (schema, scoring,      adapters
   in-process)     adapter (Rust)     hallucination,         (Postgres external,
                   BackendPort over   citations, future      RBX context sources)
                   Agentgateway       TruthMetal) + Langfuse
```

## Component contracts

### thalamus-core (Rust crate)

Owns the domain. No I/O, no adapters, no gateway types.

Provides:
- Domain types: `CallRequest`, `PolicyDecision`, `Envelope`, `PostCallResult`,
  `AuditEvent`, `RiskLevel`, `Policy`, `Budget`, `ContextGrant`,
  `RedactionRule`.
- Port traits: `BackendPort`, `ContextPort`, `PolicyPort`, `AuditPort`,
  `EvalPort`, `ObservabilityPort`.
- Validation primitives: schema check, risk classification interface,
  hallucination-signal interface, citation-check interface.

Invariant: depends only on the standard library and minimal, audited crates
(serialization, time, ids). Never depends on an adapter crate or a gateway SDK.

### thalamus-server (Rust service)

Exposes the control-plane API over HTTP and gRPC.

| Endpoint | Phase | Purpose |
|----------|-------|---------|
| `POST /v1/decide` | pre-call | Resolve policy, return `PolicyDecision` without executing |
| `POST /v1/pre-call` | pre-call | Decide and produce an approved `Envelope` |
| `POST /v1/call` | both | Pre-call, delegate to `BackendPort`, post-call, return `PostCallResult` |
| `POST /v1/embeddings` | both | Authenticated policy/redaction/audit, delegate to `EmbeddingPort`, validate vectors |
| `POST /v1/post-call` | post-call | Validate an externally executed response |
| `POST /v1/evaluate` | post-call | Submit a response/dataset to evaluation |
| `GET  /v1/audit/{audit_id}` | n/a | Retrieve audit events for a call |

`/v1/call` is the primary path: callers that let Thalamus own the round trip.
`/v1/decide` + `/v1/post-call` is the split path: callers that execute the
backend themselves (for example through an existing data plane) but still want
governance.

`/v1/embeddings` is mounted only when `THALAMUS_RBX_API` enables the credential
middleware. The verified credential must target audience `thalamus` and carry
the least-privilege `thalamus:embeddings` scope. Its additive v1 contract is:

```json
{
  "tenant": "RBX",
  "product": "rbx-rag-public-assistant",
  "workflow": "index",
  "model_alias": "embedding.default",
  "input": ["one or up to 128 strings"]
}
```

A successful response contains `model_alias`, `vectors`, `trace_id`, and
`audit_id`. Provider model names and metadata never cross this HTTP boundary.
Policy/auth/refusal errors use the governed typed error envelope and include
trace/audit IDs once the authenticated handler has accepted the request.

### thalamus-sdk-python and thalamus-sdk-ts

Thin clients. They:
- build `CallRequest` from caller intent,
- call `thalamus-server`,
- surface typed `PolicyDecision` / `PostCallResult`,
- propagate `trace_id` and `audit_id`.

They do NOT embed policy, model lists, budgets, or redaction rules. Those live
in policy, server-side.

- `thalamus-sdk-python`: Robson, Python agents, batch jobs, backend services,
  evaluation workflows.
- `thalamus-sdk-ts`: Strategos, Eden, admin tools, web apps, TypeScript
  services.

### thalamus-agentgateway-adapter (Rust)

Implements `BackendPort` over Agentgateway. May also:
- translate `PolicyDecision` into Agentgateway-compatible routes, headers,
  metadata, and policy hooks,
- inject tenant/workflow/risk headers,
- propagate `trace_id` and `audit_id`,
- route LLM, MCP, A2A, and tool traffic,
- consume traffic telemetry and expose route/provider/tool metadata back to
  Thalamus audit and evaluation,
- support rate-limit and budget enforcement at the transport edge.

Go is permitted for this adapter only if an Agentgateway ecosystem integration
makes Go clearly better. Default is Rust.

A parallel `BackendPort` adapter targets the current experimental LiteLLM data
plane (`rbx-infra/apps/prod/llm-gateway`) so migration does not require a
product change.

### thalamus-eval

Evaluation layer. Implements `EvalPort`. Provides schema checks, output
validation, quality scoring, hallucination signals, citation/source checks, and
a forward integration point for TruthMetal. Uses Langfuse for LLM trace and
evaluation persistence (datasets, scoring, runs, model comparison).

### thalamus-console (TypeScript)

Admin UI built on `thalamus-sdk-ts`. Surfaces policies, traces, audit events,
costs, model/tool permissions, and evaluation outcomes. Read-mostly; policy
edits go through governed change control.

## Ports (the stable seams)

| Port | Input | Output | Default impl |
|------|-------|--------|--------------|
| `PolicyPort` | `CallRequest` | `Policy` + resolution context | `PolicyEngine` (Rust, in-process) |
| `ContextPort` | `ContextGrant` | authorized context only | RBX context sources per policy |
| `BackendPort` | approved `Envelope` | raw backend response | `thalamus-agentgateway-adapter` / LiteLLM adapter |
| `EmbeddingPort` | governed alias + redacted inputs + trace/audit IDs | embedding vectors | LiteLLM adapter |
| `AuditPort` | `AuditEvent` | durable ack | Postgres (external VPS) |
| `EvalPort` | response + policy | eval submission ref | `thalamus-eval` + Langfuse |
| `ObservabilityPort` | spans/metrics | exporter ack | OpenTelemetry OTLP |

The set of ports is the contract that keeps the data plane replaceable and the
domain pure.

## Language decision (governed)

| Layer | Language | Reason |
|-------|----------|--------|
| Core | Rust | Infrastructure-critical; explicit invariants; aligns with Robson |
| Server | Rust | Same process model and type guarantees as core |
| Policy engine | Rust | Policy correctness is a safety property |
| Gateway adapters | Rust first, Go only if clearly better | Adapter may need ecosystem fit |
| SDKs | Python, TypeScript | Match Robson (Py) and Strategos/Eden (TS) |
| Admin UI | TypeScript | Web app |
| Zig | Not for v0/v1 | Premature for this maturity |

Recorded in [ADR-0001](../adr/ADR-0001-thalamus-as-semantic-control-layer.md).

## Build order

1. `thalamus-core`: domain types, port traits, policy model, audit schemas.
2. `PolicyEngine`: policy resolution and pure evaluation.
3. `thalamus-server`: `/v1/decide`, `/v1/pre-call`, `/v1/post-call`,
   `/v1/call`, `/v1/audit`.
4. `thalamus-agentgateway-adapter` and a LiteLLM `BackendPort` adapter.
5. `thalamus-sdk-python`, `thalamus-sdk-ts`.
6. `thalamus-eval` + Langfuse.
7. `thalamus-console`.

## Open items

- Exact policy language/representation (data format and evaluation semantics)
  is not yet decided. Tracked in `docs/99-reference/design-decisions.md`.
- Audit store schema and retention are not yet decided; must respect the
  Postgres-external constraint.
- Whether `PolicyEngine` stays in-process or becomes a separable service is
  deferred until load characteristics are known.

## References

- [ADR-0001](../adr/ADR-0001-thalamus-as-semantic-control-layer.md)
- [pre-call-and-post-call-responsibilities.md](pre-call-and-post-call-responsibilities.md)
- [agentgateway-and-data-plane.md](agentgateway-and-data-plane.md)
- [observability-and-evaluation.md](observability-and-evaluation.md)
- [../03-integration/cross-product-positioning.md](../03-integration/cross-product-positioning.md)
