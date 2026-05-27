# Purpose of Thalamus

**Version**: 0.2.0 | **Last Updated**: 2026-05-16

This document supersedes the 0.1.0 "signal mediation" purpose. See
[ADR-0001](docs/adr/ADR-0001-thalamus-as-semantic-control-layer.md).

## The core question

Every RBX product now calls AI execution backends: language models, tools, MCP
servers, A2A agents. Robson asks for risk classification and operational
recommendations. Strategos asks for strategic analysis. Agents call tools and
each other. Jobs call models in batch.

Without a control layer, every product re-implements, inconsistently:

- which model is allowed for which workflow and tenant,
- which budget and latency limit applies,
- which context may be attached to a prompt,
- what counts as a valid response,
- what must be audited,
- what must be evaluated,
- what may become an executable action.

This is not a transport problem. A gateway moves bytes and applies rate limits.
This is a control problem: deciding, recording, and validating AI-mediated work
against business policy.

Thalamus exists to solve that problem once, as a governed control plane, and to
make the data plane replaceable.

## What Thalamus provides

```
   product / agent / job
            |
            |  "I want to call an AI backend for workflow W
            |   with this input and (maybe) this context"
            v
   +-------------------------------+
   |  Thalamus pre-call decision   |
   |  identity, intent, policy,    |
   |  model/tool selection,        |
   |  budget, context auth,        |
   |  redaction, route, trace/audit|
   +---------------+---------------+
                   | allowed envelope + routing decision
                   v
        data plane backend  -->  AI backend (LLM/tool/MCP/A2A)
                   |
                   v
   +-------------------------------+
   |  Thalamus post-call validation|
   |  schema, risk, hallucination, |
   |  citations, business rules,   |
   |  redaction, audit, evaluation,|
   |  persistence                  |
   +---------------+---------------+
                   | validated, classified, audited result
                   v
   product / agent / job  (and: Strategos events, TruthMetal evidence)
```

## Why a separate control layer

### Problem 1: No control point

Without Thalamus, model/tool/agent calls happen wherever code happens to live.
There is no single place to enforce policy, budget, or context authorization.
With Thalamus, there is exactly one.

### Problem 2: Backend lock-in

Calling a provider SDK or a specific gateway directly couples products to that
backend. With Thalamus, the backend (Agentgateway, LiteLLM, OpenRouter, Envoy,
Kong, direct provider calls) is reached through an adapter and is replaceable.
Domain logic never depends on backend types.

### Problem 3: Audit and evaluation as afterthoughts

Without a control layer, audit and evaluation are added per product, late, and
inconsistently. With Thalamus, every call produces a `trace_id` and `audit_id`,
audit events are first-class, and responses can be routed to automatic
evaluation by policy.

### Problem 4: Unsafe execution paths

Without policy gates, an LLM suggestion can flow into execution. With Thalamus,
execution-affecting responses are classified by risk and gated by policy,
deterministic validation, or human review before they can become actions.

## What Thalamus is responsible for

Pre-call: identify tenant/product/user/workflow, classify intent, select policy,
select permitted model/tool/gateway/backend, enforce budget and limits, build
the envelope, retrieve only authorized context, redact or block sensitive data,
make the routing decision, create `trace_id` and `audit_id`.

Post-call: validate the response, check it against a schema, classify
operational risk, detect likely hallucination signals, check citations or
sources where required, apply business rules, redact sensitive information,
register audit events, send to automatic evaluation, persist events.

See
[docs/02-architecture/pre-call-and-post-call-responsibilities.md](docs/02-architecture/pre-call-and-post-call-responsibilities.md).

## What Thalamus is not responsible for

- Running the model. Providers and AI backends do that.
- Moving bytes, rate limiting at the transport level, MCP/A2A/LLM proxying.
  The data plane does that (Agentgateway is the recommended RBX backend; it is
  not Thalamus).
- Strategic memory. Strategos owns decision history and rationale.
- Ground truth. TruthMetal will own datasets, assertions, and factual checks.
- Hard trading/risk invariants. Robson owns those, independent of any LLM
  suggestion.

## The boundary phrase

> Thalamus is gateway-agnostic at the product layer, but Agentgateway-native at
> the RBX infrastructure adapter layer.

Product and domain code must never import a gateway type. The adapter layer may
be Agentgateway-specific.

## Long-term vision

Thalamus becomes the standard control plane for AI traffic across RBX. Every
AI-mediated call from Robson, Strategos, Eden, agents, and jobs is governed by a
policy, traced, audited, and (by policy) evaluated. The data plane underneath is
an operational choice, not an architectural commitment.

## Success criteria

1. Every AI-mediated RBX call passes a Thalamus pre-call decision and post-call
   validation, or is explicitly exempted by policy.
2. No product or domain module depends directly on a gateway or provider type.
3. Backend swap (for example LiteLLM to Agentgateway) requires an adapter
   change, not a product change.
4. Every call has a `trace_id` and `audit_id`; audit events are queryable.
5. Policy, risk classification, and evaluation outcomes are inspectable in
   `thalamus-console`.

## Related documents

- [BOUNDARIES.md](BOUNDARIES.md) - Control boundary
- [ARCHITECTURE.md](ARCHITECTURE.md) - Control plane architecture
- [docs/adr/ADR-0001-thalamus-as-semantic-control-layer.md](docs/adr/ADR-0001-thalamus-as-semantic-control-layer.md)
- [docs/03-integration/cross-product-positioning.md](docs/03-integration/cross-product-positioning.md)

---

*Last updated: 2026-05-16*
