# Quick Start for AI Agents

**Version**: 0.3.0 | **Last Updated**: 2026-07-12

This supersedes the 0.1.0 quick start. See
[ADR-0001](../adr/ADR-0001-thalamus-as-semantic-control-layer.md).

## What Thalamus is now

Thalamus is the **semantic control layer for AI traffic**. It decides and
validates AI-mediated calls (pre-call and post-call). It is not the model, not a
proxy, not a gateway, not Agentgateway.

If you read older docs claiming Thalamus is a biological signal router: that is
superseded. Trust [BOUNDARIES.md](../../BOUNDARIES.md) and ADR-0001.

## First steps

1. Read [BOUNDARIES.md](../../BOUNDARIES.md) (control boundary).
2. Read [ADR-0001](../adr/ADR-0001-thalamus-as-semantic-control-layer.md).
3. Skim [ARCHITECTURE.md](../../ARCHITECTURE.md) and
   [target-architecture.md](../02-architecture/target-architecture.md).
4. Apply the control-boundary framework before any change.

## The control-boundary framework

```
1. Control:          is this a decision/validation about an AI call?      want YES
2. Provider transport: does this own provider protocol/credentials/types? want NO
3. Gateway-coupling: does this need a gateway type in domain code?        want NO
4. Ownership:        is this Strategos/TruthMetal/Robson territory?       want NO
5. Policy:           can this be policy instead of a code branch?         prefer YES
```

Belongs in Thalamus only if Control=YES, provider transport ownership=NO,
Gateway-coupling=NO, Ownership=NO, and variation is Policy. Inline model payload
mediation is allowed only through `BackendPort`.

## Decision levels

- **Autonomous**: doc fixes, examples, in-module refactors, tests.
- **Reviewed**: new sections, features, API/port changes. Record a boundary
  analysis, propose, wait.
- **Governed**: boundary redefinition, tech beyond ADR-0001, phase transitions,
  port add/remove. ADR + leadership.

## Red flags (stop)

- Putting a gateway/provider type into `thalamus-core` or `thalamus-server`.
- Adding provider transport ownership (provider SDKs, connection pooling,
  gateway types, provider credentials, or provider-specific retry/fallback) to
  the control plane.
- Hardcoding policy as `if product == ...`.
- Skipping post-call validation.
- Storing strategic memory, ground truth, or trading invariants in Thalamus.

## Phase

Architecture phase. Technology is decided (Rust-first; Python/TS SDKs; TS UI;
no Zig v0/v1). You may scaffold consistent with the target architecture. The old
"no code, no technology in Phase 0" rule is removed.

## Essential documents

1. [BOUNDARIES.md](../../BOUNDARIES.md)
2. [ADR-0001](../adr/ADR-0001-thalamus-as-semantic-control-layer.md)
3. [ARCHITECTURE.md](../../ARCHITECTURE.md)
4. [docs/02-architecture/target-architecture.md](../02-architecture/target-architecture.md)
5. [docs/02-architecture/pre-call-and-post-call-responsibilities.md](../02-architecture/pre-call-and-post-call-responsibilities.md)
6. [docs/03-integration/cross-product-positioning.md](../03-integration/cross-product-positioning.md)
7. [GOVERNANCE.md](../../GOVERNANCE.md)
8. [.claude/agent-guidelines.md](../../.claude/agent-guidelines.md)

---

*Decide and validate. Never transport. Never absorb another product's
ownership.*

*Last updated: 2026-05-16*
