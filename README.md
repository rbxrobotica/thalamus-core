# Thalamus

**The semantic control layer for AI traffic in RBX Systems**

---

## What is Thalamus?

Thalamus applies business rules, policies, context, validation, routing
decisions, observability, evaluation, and auditability **before and after**
calls to language models, tools, MCP servers, A2A agents, or other AI execution
backends.

Thalamus is the control plane for AI traffic. It is the single point that
answers, for every AI-mediated call:

- Who can call which model?
- With which budget?
- Under which policy?
- With which context?
- With which risk level?
- With which evaluation?
- With which traceability?
- With which output rule?
- Which calls require human review?
- Which responses can become agent-executable actions?
- Which events must be persisted into Strategos?
- Which evidence should feed TruthMetal in the future?

## What Thalamus is NOT

- Not the language model.
- Not merely an LLM proxy.
- Not merely a gateway.
- Not based on Agentgateway.

Thalamus is gateway-agnostic at the product and domain layer. Agentgateway is a
low-level data plane backend that Thalamus may support as a privileged backend
through an adapter.

> Thalamus is gateway-agnostic at the product layer, but Agentgateway-native at
> the RBX infrastructure adapter layer.

See [BOUNDARIES.md](BOUNDARIES.md) for the full control boundary.

## Control plane vs data plane

```
                 +-------------------------------------------+
   product /     |              THALAMUS                     |
   agent / job   |        semantic control layer             |
       |         |                                           |
       |  call   |  pre-call:  identity, intent, policy,      |
       +-------->|             model/tool selection, budget,  |
                 |             context auth, redaction,       |
                 |             routing decision, trace/audit  |
                 |                                           |
                 |  -- delegates transport to data plane --> |
                 +---------------------+---------------------+
                                       |
                                       v
                 +-------------------------------------------+
                 |   DATA PLANE (replaceable backend)        |
                 |   Agentgateway | LiteLLM | OpenRouter |   |
                 |   Envoy | Kong | direct provider calls    |
                 |   connectivity, proxy, rate limits,       |
                 |   transport-level enforcement             |
                 +---------------------+---------------------+
                                       |
                                       v
                 +-------------------------------------------+
                 |   AI backend: LLM | tool | MCP | A2A      |
                 +---------------------+---------------------+
                                       |
                 +---------------------v---------------------+
                 |              THALAMUS                     |
                 |  post-call: schema check, risk class,     |
                 |             hallucination signals,        |
                 |             citation/source check,        |
                 |             business rules, redaction,    |
                 |             audit events, evaluation,     |
                 |             persistence                   |
                 +-------------------------------------------+
```

## Target components

| Component | Language | Role |
|-----------|----------|------|
| `thalamus-core` | Rust crate | Domain types, policy model, envelopes, decisions, risk levels, audit event schemas, context authorization types, validation primitives |
| `thalamus-server` | Rust service | HTTP/gRPC APIs: policy decisions, pre-call mediation, post-call validation, evaluation hooks, auditing, gateway integration |
| `thalamus-sdk-python` | Python | SDK for Robson, Python agents, jobs, backend services, evaluation workflows |
| `thalamus-sdk-ts` | TypeScript | SDK for Strategos, Eden, admin tools, web apps |
| `thalamus-agentgateway-adapter` | Rust | Translates Thalamus decisions into Agentgateway routes, headers, metadata, policy hooks, observability signals |
| `thalamus-eval` | Rust + SDK | Schema checks, output validation, quality scoring, hallucination signals, citation checks, future TruthMetal integration |
| `thalamus-console` | TypeScript | Admin UI: policies, traces, audit events, costs, model/tool permissions, evaluation outcomes |

This repository (`thalamus-core`) is the canonical home for the domain model and
the architecture of the control layer. Implementation crates and services are
added here or in sibling repositories as the architecture lands.

## Key documents

| Document | Purpose |
|----------|---------|
| [BOUNDARIES.md](BOUNDARIES.md) | What Thalamus is and is not. Read first. |
| [PURPOSE.md](PURPOSE.md) | Why the control layer exists |
| [ARCHITECTURE.md](ARCHITECTURE.md) | Control plane architecture |
| [GOVERNANCE.md](GOVERNANCE.md) | Decision framework and phases |
| [CONTRIBUTING.md](CONTRIBUTING.md) | Contribution process |
| [CHANGELOG.md](CHANGELOG.md) | Version history |
| [docs/adr/ADR-0001-thalamus-as-semantic-control-layer.md](docs/adr/ADR-0001-thalamus-as-semantic-control-layer.md) | The pivot decision |
| [docs/02-architecture/target-architecture.md](docs/02-architecture/target-architecture.md) | Components, ports, adapters |
| [docs/02-architecture/pre-call-and-post-call-responsibilities.md](docs/02-architecture/pre-call-and-post-call-responsibilities.md) | The two mediation phases |
| [docs/02-architecture/observability-and-evaluation.md](docs/02-architecture/observability-and-evaluation.md) | OpenTelemetry, Langfuse, Prometheus, Grafana |
| [docs/02-architecture/agentgateway-and-data-plane.md](docs/02-architecture/agentgateway-and-data-plane.md) | Backend adapter model |
| [docs/03-integration/cross-product-positioning.md](docs/03-integration/cross-product-positioning.md) | Strategos, TruthMetal, Robson, Eden |
| [docs/99-reference/design-decisions.md](docs/99-reference/design-decisions.md) | Decision log (Foundation entries superseded) |

## Status

**Phase**: Architecture (post-pivot). The Foundation-phase signal-router
definition is superseded by
[ADR-0001](docs/adr/ADR-0001-thalamus-as-semantic-control-layer.md).

**Language**: Rust-first for core, server, and policy engine. Python and
TypeScript SDKs. TypeScript admin UI. This is a governed decision, recorded in
ADR-0001. No implementation code exists yet; the next step is the
`thalamus-core` Rust crate (domain types and policy model).

## Use across RBX

- **Robson**: consumes Thalamus for AI-mediated analysis, risk classification,
  audit, and operational recommendations. Hard trading/risk invariants stay
  separate from LLM suggestions. Execution-affecting responses require strong
  policy gates and deterministic or human validation.
- **Strategos**: consumes Thalamus as a governed AI control service, receives
  operational/audit/evaluation events, and may expose policy, business plan, and
  situation-room views. Strategos does not become the low-level LLM gateway.
- **TruthMetal**: future ground-truth and evidence layer. Provides datasets,
  assertions, evaluation cases, citation checks, and factual validation hooks.
  Distinct from Thalamus.
- **Eden**: uses Thalamus as part of the platform baseline for agentic
  workflows, internal developer platform features, and governed AI execution.

See [docs/03-integration/cross-product-positioning.md](docs/03-integration/cross-product-positioning.md).

## License

To be determined by RBX Systems. See [LICENSE](LICENSE).

---

*Thalamus governs AI traffic. It does not run the model and it does not move the
bytes. It decides what is allowed, with which context, and whether the result is
acceptable.*

*Last updated: 2026-05-16*
