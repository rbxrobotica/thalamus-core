# Thalamus Glossary

**Version**: 0.2.0 | **Last Updated**: 2026-05-16

This supersedes the 0.1.0 signal-mediation glossary. See
[ADR-0001](../adr/ADR-0001-thalamus-as-semantic-control-layer.md).

## Core concepts

### Thalamus
The semantic control layer for AI traffic. Applies business rules, policy,
context, validation, routing decisions, observability, evaluation, and
auditability before and after calls to language models, tools, MCP servers, A2A
agents, or other AI execution backends. Not the model, not a proxy, not a
gateway, not Agentgateway.

### Control plane
The layer that decides and validates. Thalamus is the control plane for AI
traffic.

### Data plane
The layer that moves bytes: connectivity, proxy, transport routing, rate
limits, transport-level enforcement. Agentgateway, LiteLLM, Envoy, Kong, direct
provider calls. Replaceable. Reached only through `BackendPort`.

### AI execution backend
What ultimately runs the work: an LLM provider, a tool, an MCP server, or an
A2A agent.

### Pre-call mediation
The phase before a backend call: identity, intent, policy, model/tool
selection, budget, envelope build, context authorization, redaction, routing
decision, `trace_id`/`audit_id` creation.

### Post-call validation
The phase after a backend response: validation, schema check, risk
classification, hallucination signals, citation/source checks, business rules,
redaction, audit, evaluation submission, persistence.

### Policy
Data that governs an AI-mediated call: which tenant/product/workflow may use
which model/tool, with which budget, with which authorized context, with which
risk thresholds, with which output rules, with which evaluation. Evaluated by
the policy engine. Not branches in product code.

### Envelope
The approved, policy-built request handed to `BackendPort`: prompt/payload plus
authorized context plus routing metadata plus `trace_id`/`audit_id`.

### Routing decision
The control-plane choice of which backend, model, or tool a call goes to. An
opaque handle, never a gateway type, at the domain layer.

### Risk classification
Post-call assignment of `Low | Medium | High | Prohibited` to a response,
gating what the caller may do with it.

### Audit event
A durable governance record of a pre-call decision or post-call outcome, joined
by `audit_id`. Not sampled. Distinct from observability telemetry.

### trace_id
OpenTelemetry trace identifier propagated across Thalamus, the data plane, the
provider/tool, and back.

### audit_id
Stable identifier joining the pre-call and post-call audit events for one
logical call.

## Components

### thalamus-core
Rust crate: domain types, policy model, envelopes, decisions, risk levels,
audit schemas, context authorization types, validation primitives, port traits.
No I/O, no adapters, no gateway types.

### thalamus-server
Rust service exposing the control-plane API (`decide`, `pre-call`, `call`,
`post-call`, `evaluate`, `audit`).

### thalamus-sdk-python / thalamus-sdk-ts
Thin SDKs. Python for Robson, jobs, Python agents. TypeScript for Strategos,
Eden, admin tools. They carry no policy.

### thalamus-agentgateway-adapter
`BackendPort` implementation over Agentgateway. The only place that knows
Agentgateway types.

### thalamus-eval
Evaluation layer: schema checks, scoring, hallucination signals, citation
checks, future TruthMetal integration. Uses Langfuse.

### thalamus-console
TypeScript admin UI: policies, traces, audit, costs, model/tool permissions,
evaluation outcomes.

### Port
A trait defined in `thalamus-core` that keeps the data plane replaceable and
the domain pure: `BackendPort`, `ContextPort`, `PolicyPort`, `AuditPort`,
`EvalPort`, `ObservabilityPort`.

## Observability and evaluation

### OpenTelemetry
The vendor-neutral observability backbone: traces, spans, metrics, logs
correlation, trace propagation.

### Langfuse
The LLM observability and evaluation layer: prompts/versions, generations,
traces, scoring, datasets, eval runs, model comparison, cost/token analysis.
Not replaced by Prometheus/Grafana.

### Prometheus
Metrics scraping and alerting: SLO/SLA, rate/error/duration/saturation, policy
decision counts, validation failure counts, provider failure rates. Present in
`rbx-infra` (kube-prometheus-stack).

### Grafana
Dashboards and operational views. Present in `rbx-infra`
(`grafana.rbxsystems.ch`).

## Boundary terms

### Gateway-agnostic at the product layer
Domain and product code never reference a gateway/provider type. The phrase in
full: "Thalamus is gateway-agnostic at the product layer, but
Agentgateway-native at the RBX infrastructure adapter layer."

### Control-boundary framework
The five questions that decide whether a capability belongs in Thalamus:
Control, Data plane, Gateway-coupling, Ownership, Policy. Replaces the 0.1.0
Five-Question Framework.

### Invariant
`thalamus-core` depends on no adapter and no gateway type. Adapters depend on
`thalamus-core`, never the reverse.

## Sibling products

### Strategos
Strategic brain and strategic memory. Consumes Thalamus; receives
operational/audit/evaluation events. Not the LLM gateway.

### TruthMetal
Future ground-truth and evidence layer. Supplies datasets, assertions, citation
checks to `thalamus-eval`. Not Thalamus.

### Robson
Trading system. Consumes Thalamus for AI-mediated analysis. Keeps hard
trading/risk invariants deterministic and separate from LLM suggestions.

### Eden
IDP/CLI. Uses Thalamus as the governed AI execution baseline for agentic
workflows.

## Historical

### Signal mediation layer (deprecated)
The 0.1.0 definition: a business-agnostic biological-thalamus signal router.
Superseded by ADR-0001. Retained only as history.

### Five-Question Framework (deprecated)
The 0.1.0 boundary check (Signal, Decision, Domain, State, Reusability).
Replaced by the control-boundary framework.

---

*Last updated: 2026-05-16*
