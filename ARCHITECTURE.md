# Thalamus Architecture

**Version**: 0.3.0 | **Last Updated**: 2026-07-12 | **Phase**: Architecture and P0 contracts

This document supersedes the 0.1.0 signal-mediation architecture. See
[ADR-0001](docs/adr/ADR-0001-thalamus-as-semantic-control-layer.md). Read
[BOUNDARIES.md](BOUNDARIES.md) first.

## Architectural vision

Thalamus is the semantic control layer for AI traffic. It sits between any RBX
caller (product, agent, job) and any AI execution backend (LLM, tool, MCP
server, A2A agent), reached through a replaceable data plane.

```
+--------------------------------------------------------------+
|                       Caller layer                           |
|        Robson, Strategos, Eden, agents, batch jobs           |
|   - holds business intent, not policy or backend knowledge   |
+----------------------------+---------------------------------+
                             |  call request (via SDK)
+----------------------------v---------------------------------+
|                   THALAMUS CONTROL PLANE                      |
|                                                              |
|   pre-call mediation  --->  routing decision                 |
|        ^                          |                          |
|        |                          v                          |
|   post-call validation  <---  BackendPort / backend adapter  |
|                                                              |
|   policy | context auth | risk | budget | audit | eval       |
+----------------------------+---------------------------------+
                             |  backend port (no gateway types leak up)
+----------------------------v---------------------------------+
|                       DATA PLANE                              |
|   Agentgateway (privileged) | LiteLLM | OpenRouter | Envoy   |
|   Kong | Azure API Mgmt | direct provider calls               |
|   - connectivity, proxy, rate limits, transport enforcement  |
+----------------------------+---------------------------------+
                             |
+----------------------------v---------------------------------+
|                       AI backends                            |
|        LLM providers | tools | MCP servers | A2A agents       |
+--------------------------------------------------------------+
```

## Core architectural principles

### 1. Control plane with inline enforcement, not provider transport owner

Thalamus decides, validates, and may mediate approved model payloads inline
through `BackendPort`. It does not own provider-specific protocol, credentials,
connection pools, gateway types, or technical retry/fallback semantics.

```
CORRECT: Thalamus selects model class M under policy P, builds the route
         envelope, records the decision, and invokes BackendPort.
WRONG:   thalamus-core imports a provider SDK or Agentgateway/LiteLLM type.
```

### 2. Gateway-agnostic at the product layer

No domain or product code references a gateway type. The only place that knows
about Agentgateway is `thalamus-agentgateway-adapter`.

```
CORRECT: domain depends on trait BackendPort
WRONG:   domain depends on agentgateway::Route
```

### 3. Two phases per call

Every governed call has a pre-call decision and a post-call validation. Neither
is optional unless policy explicitly exempts the workflow.

### 4. Policy-driven, not hardcoded

Tenant, product, workflow, model permissions, budgets, context authorization,
risk thresholds, evaluation requirements, and output rules are policy. Policy is
data evaluated by the policy engine, not branches in product code.

### 5. Auditable by construction

Every call produces `trace_id` and `audit_id`. Policy decisions, routing
decisions, validation outcomes, and risk classifications are recorded as audit
events. Audit is not a logging side effect; it is an output of the control
plane.

## Logical components and ports

```
+-----------------------------------------------------------+
|  thalamus-core (Rust crate)                               |
|  domain types, policy model, envelopes, decisions,        |
|  risk levels, audit event schemas, context auth types,    |
|  validation primitives, port traits                       |
+-------------------------+---------------------------------+
                          | implemented/served by
+-------------------------v---------------------------------+
|  thalamus-server (Rust service)                           |
|  HTTP/gRPC: decide(), pre_call(), post_call(),            |
|  evaluate(), audit(), gateway integration                 |
+--+----------------+----------------+----------------+-----+
   |                |                |                |
   v                v                v                v
PolicyEngine   ContextPort     BackendPort      AuditPort
(Rust)         (authorized     (data plane:     (audit sink)
               context only)   adapter behind   |
                               a trait)         v
                                |          EvalPort -> thalamus-eval
                                v
                       thalamus-agentgateway-adapter
                       (Agentgateway-native, optional)
```

Ports (traits in `thalamus-core`, implementations elsewhere):

| Port | Responsibility | Reference implementation |
|------|----------------|--------------------------|
| `BackendPort` | Execute an approved envelope against an AI backend | `thalamus-agentgateway-adapter`; also LiteLLM/direct adapters |
| `ContextPort` | Return only policy-authorized context for a request | RBX context sources, per policy |
| `PolicyPort` | Resolve and evaluate the applicable policy | `PolicyEngine` (Rust, in-process) |
| `AuditPort` | Persist audit events durably | audit store (Postgres external; see infra constraint) |
| `EvalPort` | Submit responses to evaluation | `thalamus-eval` + Langfuse |
| `ObservabilityPort` | Emit traces/metrics | OpenTelemetry exporter |

Invariant: `thalamus-core` defines the traits. It must not depend on any
adapter crate. Adapters depend on `thalamus-core`, never the reverse.

## The two phases

### Pre-call mediation

```
caller.call(request)
  -> identify tenant, product, user, workflow
  -> classify intent
  -> resolve applicable policy (PolicyPort)
  -> select permitted model / tool / gateway / backend
  -> enforce budget, token, latency limits
  -> build the prompt / envelope
  -> retrieve only authorized context (ContextPort)
  -> redact or block sensitive data
  -> make the routing decision
  -> create trace_id and audit_id
  -> emit audit event: PreCallDecision
  -> hand approved envelope to BackendPort
```

If policy denies the call, Thalamus returns a typed decision (denied, with
reason and policy reference) and emits an audit event. The backend is never
contacted.

### Post-call validation

```
backend response received
  -> validate the response
  -> check response against schema
  -> classify operational risk
  -> detect likely hallucination signals
  -> check citations or sources where required
  -> apply business rules
  -> redact sensitive information
  -> register audit events
  -> send data to automatic evaluation (EvalPort)
  -> persist events
  -> return validated, classified, audited result to caller
```

Full responsibility lists and worked examples are in
[docs/02-architecture/pre-call-and-post-call-responsibilities.md](docs/02-architecture/pre-call-and-post-call-responsibilities.md).

## Domain model (conceptual)

These are the conceptual types `thalamus-core` will define in Rust. Names are
indicative, not final.

```
CallRequest {
  tenant, product, user, workflow
  intent_hint
  input
  requested_context_refs
  caller_constraints { max_tokens, timeout_ms, output_format }
}

PolicyDecision {
  decision: Allow | Deny | AllowWithReview
  selected_backend            // opaque handle, not a gateway type
  permitted_model_or_tool
  budget { max_tokens, max_cost, timeout_ms }
  context_grant              // which context refs are authorized
  redaction_rules
  trace_id, audit_id
  policy_ref
}

Envelope {
  trace_id, audit_id
  tenant, product, workflow
  prompt_or_payload
  authorized_context
  routing { backend, model_or_tool, risk_tier }
}

PostCallResult {
  status: Valid | Invalid | NeedsHumanReview
  schema_check
  risk_class: Low | Medium | High | Prohibited
  hallucination_signals
  citation_check
  business_rule_outcomes
  redacted_output
  audit_event_ids
  eval_submission_ref
  executable_by_agent: bool
  strategos_event: Option<...>
}

AuditEvent {
  audit_id, trace_id
  tenant, product, workflow
  phase: PreCall | PostCall
  decision_or_outcome
  timestamp
}
```

`RiskLevel`, `Policy`, `Budget`, `ContextGrant`, and `RedactionRule` are
first-class types with explicit invariants. Strong typing is a deliberate
reliability choice (see language decision in ADR-0001).

## Deployment shape

- `thalamus-server` runs as a service in the `thalamus` namespace
  (`rbx-infra/core/namespaces/thalamus.yml`). It is a control-plane service:
  request/decision/validation traffic, not high-throughput byte streaming.
- The data plane (Agentgateway or the current experimental LiteLLM in
  `llm-gateway`) runs separately. Thalamus reaches it through `BackendPort`.
- Audit store and any policy/eval persistence use external Postgres. PostgreSQL
  never runs inside the production k3s cluster
  (`rbx-infra/docs/infra/ARCHITECTURE.md`); use a dedicated VPS instance.
- SDKs (`thalamus-sdk-python`, `thalamus-sdk-ts`) are libraries embedded in
  callers. They speak the Thalamus API; they do not embed policy.

## Observability and evaluation

OpenTelemetry is the vendor-neutral observability backbone. Langfuse is the LLM
observability and evaluation layer. Prometheus and Grafana are the
infrastructure metrics and dashboard tools and are already present in
`rbx-infra`. They are not replacements for Langfuse. See
[docs/02-architecture/observability-and-evaluation.md](docs/02-architecture/observability-and-evaluation.md).

## Quality attributes

- **Correctness over throughput**: Thalamus is a decision service. A wrong
  allow is worse than a slow allow. The data plane carries throughput.
- **Determinism of policy**: the same request under the same policy yields the
  same decision. Policy evaluation is pure given inputs.
- **Auditability**: every decision and outcome is reconstructable from audit
  events plus `trace_id`.
- **Backend independence**: swapping the data plane is an adapter change.
- **Explicit invariants**: typed domain model; no stringly-typed policy.

## What is deliberately excluded

- Token streaming and connection pooling (data plane).
- Provider SDK calls from domain code (adapter only).
- Strategic memory (Strategos).
- Ground-truth datasets and factual oracles (TruthMetal).
- Hard trading/risk invariants (Robson domain).

## Next steps

1. `thalamus-core` Rust crate: domain types, port traits, policy model, audit
   schemas.
2. `PolicyEngine` (Rust): policy resolution and evaluation.
3. `thalamus-server` (Rust): `decide`, `pre_call`, `post_call`, `evaluate`,
   `audit` endpoints.
4. `thalamus-agentgateway-adapter` (Rust): `BackendPort` over Agentgateway;
   parallel `BackendPort` over the current LiteLLM data plane for migration.
5. `thalamus-sdk-python` and `thalamus-sdk-ts`.
6. `thalamus-eval` and Langfuse wiring.
7. `thalamus-console` (TypeScript).

## References

- [ADR-0001](docs/adr/ADR-0001-thalamus-as-semantic-control-layer.md)
- [BOUNDARIES.md](BOUNDARIES.md)
- [docs/02-architecture/target-architecture.md](docs/02-architecture/target-architecture.md)
- [docs/02-architecture/agentgateway-and-data-plane.md](docs/02-architecture/agentgateway-and-data-plane.md)
- [docs/02-architecture/observability-and-evaluation.md](docs/02-architecture/observability-and-evaluation.md)

---

*Last updated: 2026-05-16*
