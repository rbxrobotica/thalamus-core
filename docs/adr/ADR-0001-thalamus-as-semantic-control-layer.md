# ADR-0001: Thalamus is the Semantic Control Layer for AI Traffic

**Status**: Accepted

**Date**: 2026-05-16

**Deciders**: RBX Systems leadership

**Supersedes**: The Foundation-phase definition of Thalamus as a biological-inspired
"signal mediation layer" (see `docs/99-reference/design-decisions.md`,
entries dated 2026-02-02: "Biological Thalamus as Architectural Model",
"Five-Question Boundary Framework", "Single Repository for Core").

## Context

The Foundation phase (Phase 0, 2026-02-02) defined Thalamus as a business-agnostic
signal router inspired by the biological thalamus: it would normalize, prioritize,
and route opaque "signals" between a perception layer and a decision layer, without
making any decision and without domain knowledge.

That framing predates concrete RBX needs that have since become explicit:

- Robson, Strategos, Eden, and future agents call language models, tools, MCP
  servers, and A2A agents directly. There is no control point that decides
  who may call which model, under which policy, with which budget, with which
  context, and with which evaluation.
- RBX runs an experimental LiteLLM data plane in the `llm-gateway` namespace and
  evaluates Agentgateway for MCP/A2A/LLM routing. These are connectivity layers.
  They do not enforce business policy, audit, context authorization, or
  evaluation.
- Audit, traceability, risk classification, and output validation are now hard
  requirements for AI-mediated operations, not optional enrichment.
- The "opaque signal" model does not express the actual unit of work: a governed
  call to an AI execution backend, with a pre-call decision and a post-call
  validation.

The biological-signal-router framing is therefore too generic. It does not name
the control boundary it is supposed to enforce.

## Decision

Thalamus is the **semantic control layer for AI traffic**.

Thalamus applies business rules, policies, context, validation, routing
decisions, observability, evaluation, and auditability before and after calls to
language models, tools, MCP servers, A2A agents, or other AI execution backends.

Thalamus is not the language model.
Thalamus is not merely an LLM proxy.
Thalamus is not merely a gateway.
Thalamus is not based on Agentgateway.
Thalamus is gateway-agnostic at the product and domain layer.

Agentgateway is a low-level data plane backend that Thalamus may support as a
privileged backend through an adapter.

### Responsibility split

```
Thalamus (control plane / semantic layer)      Agentgateway (data plane)
-------------------------------------------    ----------------------------------
business rules                                 connectivity
policy selection and enforcement               proxy
evaluation                                     routing (transport)
context authorization                          MCP gateway
audit                                          A2A gateway
routing decisions                              LLM gateway
risk classification                            rate limits
pre-call validation                            low-level traffic observability
post-call validation                           transport-level enforcement
traceability
model and tool governance
```

Design phrase, normative:

> Thalamus is gateway-agnostic at the product layer, but Agentgateway-native at
> the RBX infrastructure adapter layer.

### Target component shape

| Component | Language | Role |
|-----------|----------|------|
| `thalamus-core` | Rust crate | Domain types, policy model, envelopes, decisions, risk levels, audit event schemas, context authorization types, validation primitives |
| `thalamus-server` | Rust service | HTTP/gRPC APIs for policy decisions, pre-call mediation, post-call validation, evaluation hooks, auditing, gateway integration |
| `thalamus-sdk-python` | Python | SDK for Robson, Python agents, jobs, backend services, evaluation workflows |
| `thalamus-sdk-ts` | TypeScript | SDK for Strategos, Eden, admin tools, web apps |
| `thalamus-agentgateway-adapter` | Rust (Go only if ecosystem integration is clearly better) | Translates Thalamus decisions and policies into Agentgateway-compatible routes, headers, metadata, policy hooks, observability signals |
| `thalamus-eval` | Rust core, SDK-driven | Schema checks, output validation, quality scoring, hallucination signals, citation/source checks, future TruthMetal integration |
| `thalamus-console` | TypeScript | Administrative UI for policies, traces, audit events, costs, model/tool permissions, evaluation outcomes |

### Language decision

- Core: Rust
- Server: Rust
- Policy engine: Rust
- Gateway adapters: Rust first; Go only when an ecosystem integration makes it
  clearly better
- SDKs: Python and TypeScript
- Admin UI: TypeScript
- Zig: not for v0 or v1

Rationale: Thalamus is infrastructure-critical, not a web app. It enforces
policy, context boundaries, auditability, and operational safety. Strong typing
and explicit invariants matter. Rust aligns with Robson and with RBX's
reliability posture.

### Invariant

Thalamus domain logic must not depend directly on Agentgateway types. Backends
(Agentgateway, LiteLLM, OpenRouter, Azure API Management, Envoy, Kong, direct
provider calls, future RBX gateways) are reached only through adapters behind a
stable port.

## Consequences

### Positive

- A single control point answers: who can call which model, with which budget,
  under which policy, with which context, with which risk level, with which
  evaluation, with which traceability, with which output rule, which calls need
  human review, which responses become agent-executable actions, which events
  must be persisted into Strategos, which evidence feeds TruthMetal.
- Backends become replaceable. The data plane is an implementation detail.
- Audit and evaluation are first-class, not bolt-ons.

### Negative

- The Foundation-phase corpus (README, PURPOSE, ARCHITECTURE, BOUNDARIES,
  glossary, agent guidelines) is superseded and must be rewritten. History is
  preserved through "superseded by" notes, not deletion.
- "No technology decisions in Phase 0" no longer holds. Rust-first is now a
  governed decision recorded here.

### Neutral

- The biological metaphor remains available as historical context and as a loose
  intuition (a relay that filters and contextualizes), but it is no longer the
  normative model. The normative model is control plane vs data plane.

## Alternatives considered

1. **Keep the signal-router framing, add governance as a feature.**
   Rejected. Governance, policy, audit, and evaluation are the product, not a
   feature of a router. The signal abstraction hides the real unit of work.

2. **Make Thalamus the Agentgateway control plane (Agentgateway-native).**
   Rejected. Couples the domain to one data plane. Violates the
   gateway-agnostic requirement and the no-direct-dependency invariant.

3. **Build Thalamus into Strategos.**
   Rejected. Strategos is a consumer and a strategic memory, not the AI control
   plane. Multiple products (Robson, Eden, agents, jobs) need Thalamus
   independently of Strategos.

## Pre-call responsibilities

Before a model/tool/agent call, Thalamus is responsible for:

1. Identifying tenant, product, user, and workflow
2. Classifying intent
3. Selecting the applicable policy
4. Selecting the permitted model, tool, gateway, or backend
5. Enforcing budget, token, and latency limits
6. Building the prompt/envelope
7. Retrieving only authorized context
8. Redacting or blocking sensitive data
9. Making the routing decision
10. Creating `trace_id` and `audit_id`

Worked example:

```
Strategos requests a strategic analysis.

Thalamus verifies:
  tenant            = RBX
  module            = Business Plan
  sensitivity       = high
  permitted model   = Claude / GPT / Kimi, depending on policy
  audit required    = yes
  private context   = allowed only if policy authorizes it
  structured output = required

Only then does the request proceed to the selected
gateway / provider / tool / agent path.
```

## Post-call responsibilities

After a model/tool/agent response, Thalamus is responsible for:

1. Validating the response
2. Checking the response against a schema
3. Classifying operational risk
4. Detecting likely hallucination signals
5. Checking citations or sources where required
6. Applying business rules
7. Redacting sensitive information
8. Registering audit events
9. Sending data to automatic evaluation
10. Persisting events

Worked example:

```
A model returns an operational recommendation.

Thalamus checks:
  Does the response respect the schema?
  Did it cite non-existent data?
  Does it contain a prohibited recommendation?
  Does it require human review?
  Can it be executed by an agent?
  Should it become an event in Strategos?
```

## Related documents

- `../../BOUNDARIES.md` - Control boundary (what Thalamus is and is not)
- `../../ARCHITECTURE.md` - Control plane architecture
- `../02-architecture/target-architecture.md` - Components and ports
- `../02-architecture/pre-call-and-post-call-responsibilities.md`
- `../02-architecture/observability-and-evaluation.md`
- `../02-architecture/agentgateway-and-data-plane.md`
- `../03-integration/cross-product-positioning.md`
- `../99-reference/design-decisions.md` - Superseded Foundation decisions
