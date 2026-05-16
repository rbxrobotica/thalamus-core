# Cross-Product Positioning

**Version**: 0.2.0 | **Last Updated**: 2026-05-16

Normative reference: [ADR-0001](../adr/ADR-0001-thalamus-as-semantic-control-layer.md).

Thalamus is the semantic control layer for AI traffic. Other RBX products
consume it; they do not reimplement it and they do not become the data plane.

## The questions Thalamus answers

For every AI-mediated call, the following must have an answer, and that answer
comes from Thalamus policy and validation:

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

If a product cannot answer these for a call, that call must go through Thalamus.

## Strategos

- Consumes Thalamus as a governed AI control service via `thalamus-sdk-ts`.
- Receives operational, audit, and evaluation events from Thalamus (for example
  `PostCallResult.strategos_event`).
- May expose policy, business plan, and situation-room views over Thalamus data.
- Does NOT become the low-level LLM gateway. Strategos remains the strategic
  brain and strategic memory.

Worked case: Strategos requests a strategic analysis (Business Plan module,
sensitivity high). Thalamus applies the policy (permitted model, audit required,
private context only if authorized, structured output required), routes, and
post-validates. Strategos receives a validated, audited result and records
strategic memory and rationale on its side. Thalamus does not store strategic
rationale; Strategos does.

Existing cross-reference: `strategos-core/docs/architecture/thalamus-interaction.md`
and `strategos-agents/docs/thalamus-interface.md` are being aligned to this
positioning. The `strategos-agents` interface (governance, validation, audit,
human review) is already consistent with the control-layer model; the older
"stateless signal router / cognitive chamber" framing in `strategos-core` is
superseded by this document and ADR-0001.

## TruthMetal

- Future ground-truth and evidence layer.
- May provide datasets, assertions, evaluation cases, citation checks, and
  factual validation hooks consumed by `thalamus-eval` through `EvalPort`.
- Is NOT the same thing as Thalamus. Thalamus governs and validates calls;
  TruthMetal supplies the truth basis some validations need.

Forward integration point: post-call citation/source checks and hallucination
signals can, in the future, query TruthMetal for factual grounding. Until
TruthMetal exists, these checks use schema and policy heuristics only.

## Robson

- May consume Thalamus (via `thalamus-sdk-python`) for AI-mediated analysis,
  risk classification, audit, and operational recommendations.
- Must keep hard trading/risk invariants separate from LLM suggestions. Those
  invariants live in Robson's domain and are deterministic.
- Thalamus responses that could affect execution require strong policy gates and
  human or deterministic validation. `risk_class` alone never makes a response
  auto-executable for trade-affecting workflows.

Boundary rule: an LLM suggestion routed through Thalamus is an input to Robson,
never a substitute for Robson's deterministic risk and position invariants. A
`PostCallResult` with `executable_by_agent = true` still passes through Robson's
own gates before any execution.

## Eden

- May use Thalamus as part of the platform baseline for agentic workflows,
  internal developer platform features, and governed AI execution.
- Eden scaffolds agents and registers them; agents that call AI backends do so
  through Thalamus, consistent with the rbx-harness Thalamus protocol.

## rbx-harness relationship

`rbx-harness/spec/protocol.md` defines the agent-facing Thalamus protocol
(envelopes, `trace_id`, governance block, human-review deferral, OTLP). That
protocol is the contract between agents and the Thalamus control layer. It is
consistent with this pivot: the protocol is the agent's view of the control
plane. The phrase "mediation layer" in rbx-harness now means the semantic
control layer defined here, not a transport router. Routing transport is the
data plane below `BackendPort`.

## What does NOT move into Thalamus

| Concern | Owner |
|---------|-------|
| Strategic memory, decision rationale | Strategos |
| Ground-truth datasets, factual oracles | TruthMetal (future) |
| Hard trading/risk and position invariants | Robson |
| Agent scaffolding and IDP ergonomics | Eden |
| Transport, proxy, rate-limit mechanics | Data plane (Agentgateway/LiteLLM) |
| Model inference | AI backends/providers |

Thalamus owns the decision and validation about AI-mediated calls, and the
audit and evaluation of them. Nothing more, nothing less.

## References

- [ADR-0001](../adr/ADR-0001-thalamus-as-semantic-control-layer.md)
- [../../BOUNDARIES.md](../../BOUNDARIES.md)
- [../02-architecture/pre-call-and-post-call-responsibilities.md](../02-architecture/pre-call-and-post-call-responsibilities.md)
- `strategos-core/docs/architecture/thalamus-interaction.md`
- `strategos-agents/docs/thalamus-interface.md`
- `rbx-harness/spec/protocol.md`
