# Thalamus Boundaries

**Status**: Living Document | **Version**: 0.2.0 | **Last Updated**: 2026-05-16

This document supersedes the 0.1.0 signal-mediation boundary framework and its
Five-Question Framework. See
[ADR-0001](docs/adr/ADR-0001-thalamus-as-semantic-control-layer.md). This is the
north-star document. Read it before any contribution.

## The core principle

> Thalamus is the semantic control layer for AI traffic. It decides and
> validates. It does not run the model and it does not move the bytes.

Thalamus applies business rules, policies, context, validation, routing
decisions, observability, evaluation, and auditability before and after calls to
language models, tools, MCP servers, A2A agents, or other AI execution backends.

## What Thalamus IS

### Control plane for AI traffic
- The single point that decides whether an AI-mediated call is allowed
- Selects the permitted model, tool, gateway, or backend
- Enforces budget, token, and latency limits
- Authorizes and scopes context
- Makes the routing decision

### Pre-call mediation
- Identity (tenant, product, user, workflow), intent classification
- Policy selection and enforcement
- Envelope construction, context authorization, redaction
- `trace_id` and `audit_id` creation

### Post-call validation
- Schema validation, operational risk classification
- Hallucination signals, citation/source checks
- Business-rule application, redaction
- Audit event registration, evaluation submission, persistence

### Governance and auditability
- Model and tool governance
- Risk classification
- Traceability
- Audit events as first-class output

### Gateway-agnostic at the product layer
- Backends are reached through a port; domain code never references a gateway
  type

## What Thalamus IS NOT

### NOT the language model
Thalamus does not generate completions. Providers and AI backends do.

**Violation example**: "Thalamus should host a fine-tuned model and serve
inference."
**Why**: That is an AI backend. Thalamus governs calls to it.

### NOT merely an LLM proxy
A proxy forwards requests. Thalamus decides whether the request is allowed,
under which policy, with which context, and whether the response is acceptable.

**Violation example**: "Thalamus is a thin pass-through to OpenAI with logging."
**Why**: That is a data plane. Thalamus is the control plane above it.

### NOT merely a gateway
A gateway provides connectivity, routing transport, rate limits. Thalamus
provides policy, audit, context authorization, risk classification, and
evaluation.

**Violation example**: "Add MCP transport multiplexing and connection pooling to
Thalamus."
**Why**: Transport is the data plane (Agentgateway, LiteLLM, Envoy, Kong).

### NOT based on Agentgateway
Agentgateway is a privileged data plane backend that Thalamus may support
through an adapter. Thalamus domain logic must not depend directly on
Agentgateway types.

**Violation example**: "Import `agentgateway::Route` in the policy engine."
**Why**: Couples the control plane to one data plane. Only
`thalamus-agentgateway-adapter` may know Agentgateway.

### NOT strategic memory, ground truth, or trading invariants
- Strategos owns decision history and rationale.
- TruthMetal will own datasets, assertions, and factual oracles.
- Robson owns hard trading/risk invariants, independent of any LLM suggestion.

## The control-boundary framework

Before adding any capability, answer these questions. The capability belongs in
Thalamus only if the answers match.

### 1. Control question
**Is this a decision or validation about an AI-mediated call (allow, route,
budget, context, risk, schema, audit, evaluation)?**
- YES -> may belong in Thalamus
- NO -> belongs elsewhere

### 2. Data plane question
**Does this move bytes, hold connections, proxy streams, or enforce transport
rate limits?**
- YES -> does NOT belong in Thalamus (data plane)
- NO -> may belong in Thalamus

### 3. Gateway-coupling question
**Does this require domain or product code to reference a specific gateway or
provider type?**
- YES -> does NOT belong in Thalamus core (adapter only)
- NO -> may belong in Thalamus

### 4. Ownership question
**Is this strategic memory (Strategos), ground truth (TruthMetal), or hard
trading/risk invariants (Robson)?**
- YES -> does NOT belong in Thalamus
- NO -> may belong in Thalamus

### 5. Policy question
**Can this be expressed as policy evaluated by the policy engine rather than
hardcoded product logic?**
- YES -> belongs as policy, not as a code branch
- NO -> reconsider; most variation is policy

A capability belongs in Thalamus only if: Control = YES, Data plane = NO,
Gateway-coupling = NO, Ownership = NO, and variation is expressed as Policy.

## Common boundary violations

### Violation: transport in the control plane

```
WRONG: Thalamus opens an HTTP/2 stream to the provider and relays tokens.
RIGHT: Thalamus produces a routing decision and hands the envelope to
       BackendPort. The data plane streams.
```

### Violation: gateway type in domain code

```
WRONG: fn decide(req) -> agentgateway::RouteConfig
RIGHT: fn decide(req) -> PolicyDecision   // backend is an opaque handle
```

### Violation: policy hardcoded as branches

```
WRONG: if product == "strategos" && workflow == "business_plan" {
           model = "claude"; require_audit = true;
       }
RIGHT: let decision = policy_engine.evaluate(request);
       // model, audit requirement, budget come from policy data
```

### Violation: skipping post-call validation

```
WRONG: return backend_response;   // straight back to caller
RIGHT: let result = post_call.validate(backend_response, policy);
       // schema, risk, hallucination, citations, audit, eval
```

### Violation: absorbing another product's ownership

```
WRONG: Thalamus stores strategic decision rationale for future sessions.
RIGHT: Thalamus emits an audit/operational event; Strategos persists
       strategic memory.
```

## The architectural boundary

```
+-------------------------------------+
|  Caller layer (Robson, Strategos,   |  business intent
|  Eden, agents, jobs)                |
+-------------------------------------+
                 |  Thalamus SDK
+-------------------------------------+
|  THALAMUS CONTROL PLANE             |  policy, audit, context auth,
|  decide / pre-call / post-call      |  risk, evaluation, routing decision
+-------------------------------------+
                 |  BackendPort (no gateway types cross this line)
+-------------------------------------+
|  DATA PLANE (Agentgateway,          |  connectivity, proxy, rate limits,
|  LiteLLM, Envoy, Kong, direct)      |  transport enforcement
+-------------------------------------+
                 |
+-------------------------------------+
|  AI backends (LLM, tool, MCP, A2A)  |  inference / execution
+-------------------------------------+
```

Thalamus must NOT reach down into transport, and must NOT absorb caller business
ownership or sibling-product ownership.

## Enforcement

### For contributors (human and AI)
1. Read this document before contributing.
2. Apply the control-boundary framework and record the result for non-trivial
   changes in `docs/99-reference/design-decisions.md`.
3. Keep `thalamus-core` free of adapter and gateway dependencies.
4. Express variation as policy.

### For reviewers
1. Reject changes that put transport, gateway types, or sibling-product
   ownership into the control plane.
2. Require a recorded boundary analysis for new capabilities.
3. Maintain boundary integrity over feature velocity.

## Living document

When the boundary needs to change, propose it as an ADR (governed decision),
record it in `docs/adr/`, and add a superseded note rather than deleting the
prior reasoning.

---

*Thalamus governs AI traffic. It does not generate it and it does not transport
it.*
