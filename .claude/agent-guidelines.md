# Agent Guidelines for Thalamus

**Version**: 0.2.0 | **Last Updated**: 2026-05-16

This supersedes the 0.1.0 guidelines (signal mediation, Phase 0 no-code). See
[ADR-0001](../docs/adr/ADR-0001-thalamus-as-semantic-control-layer.md).

## Operating definition

Thalamus is the semantic control layer for AI traffic. You are working on a
control plane: it decides (pre-call) and validates (post-call) AI-mediated
calls. It does not run models and it does not move bytes.

## Before any contribution

- [ ] Read [BOUNDARIES.md](../BOUNDARIES.md)
- [ ] Read [ADR-0001](../docs/adr/ADR-0001-thalamus-as-semantic-control-layer.md)
- [ ] Apply the control-boundary framework
- [ ] Determine the decision level ([GOVERNANCE.md](../GOVERNANCE.md))
- [ ] Record a boundary analysis for Reviewed/Governed work

## Control-boundary framework

```
Control:          decision/validation about an AI-mediated call?   YES
Data plane:       moves bytes / proxy / transport rate-limit?       NO
Gateway-coupling: needs a gateway/provider type in domain code?     NO
Ownership:        Strategos memory / TruthMetal truth / Robson      NO
                  trading invariants?
Policy:           expressible as policy, not a code branch?          prefer YES
```

## Hard rules

1. `thalamus-core` and `thalamus-server` must never import a gateway or
   provider type. Only `thalamus-agentgateway-adapter` (and other
   `BackendPort` adapters) may.
2. Adapters depend on `thalamus-core`. `thalamus-core` depends on no adapter.
3. Policy is data evaluated by the policy engine, not `if product == ...`.
4. Every governed call path has pre-call and post-call phases.
5. Do not move strategic memory (Strategos), ground truth (TruthMetal), or
   trading/risk invariants (Robson) into Thalamus.
6. Transport (connections, streams, MCP/A2A multiplexing, rate-limit
   mechanics) is data plane, below `BackendPort`. Never in the control plane.

## Language

Rust-first: core, server, policy engine. SDKs: Python and TypeScript. Admin UI:
TypeScript. Gateway adapters: Rust first; Go only if an ecosystem integration
is clearly better. No Zig for v0/v1. This is governed (ADR-0001); do not
relitigate it autonomously.

## Decision levels

- **Autonomous**: docs clarifications, examples, in-module refactors, tests.
  Make the change, document it in the commit.
- **Reviewed**: new sections, features, API/port changes, dependencies. Record
  a boundary analysis in `docs/99-reference/design-decisions.md`, propose,
  wait.
- **Governed**: boundary redefinition, technology beyond ADR-0001, phase
  transitions, port add/remove. Present an ADR; await RBX leadership.

## Documentation style

Precise engineering language. No hype. No vague "AI platform" wording without
naming the control boundary. Concrete terms (policy, audit, trace, tenant,
workflow, context, schema, route, evaluation, risk, budget, gateway adapter,
data plane, control plane). No em dashes. ASCII diagrams. Tables only when they
add clarity. Operational enough that another agent can implement from them.

## Git

Do not push. Do not commit unrelated working-tree changes. Preserve any
pre-existing user modifications; inspect before touching a dirty file.

## When uncertain

1. Apply the control-boundary framework.
2. Check `docs/adr/` and `docs/99-reference/design-decisions.md`.
3. Document the ambiguity.
4. Propose; do not assume. Escalate governed questions to RBX leadership.

## Resources

- [BOUNDARIES.md](../BOUNDARIES.md)
- [ADR-0001](../docs/adr/ADR-0001-thalamus-as-semantic-control-layer.md)
- [ARCHITECTURE.md](../ARCHITECTURE.md)
- [docs/02-architecture/target-architecture.md](../docs/02-architecture/target-architecture.md)
- [docs/03-integration/cross-product-positioning.md](../docs/03-integration/cross-product-positioning.md)
- [GOVERNANCE.md](../GOVERNANCE.md)

---

*Decide and validate. Never transport. Protect the control boundary.*

*Last updated: 2026-05-16*
