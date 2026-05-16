# Contributing to Thalamus

**Version**: 0.2.0 | **Last Updated**: 2026-05-16

This supersedes the 0.1.0 contribution guide, which assumed Phase 0 was
documentation-only with no technology decisions. See
[ADR-0001](docs/adr/ADR-0001-thalamus-as-semantic-control-layer.md).

## Mandatory reading

1. [BOUNDARIES.md](BOUNDARIES.md) - the control boundary (read first)
2. [ADR-0001](docs/adr/ADR-0001-thalamus-as-semantic-control-layer.md) - the pivot
3. [ARCHITECTURE.md](ARCHITECTURE.md) - control plane architecture
4. [GOVERNANCE.md](GOVERNANCE.md) - decision levels and phases

AI agents also read [.claude/agent-guidelines.md](.claude/agent-guidelines.md).

## Current phase

Architecture phase. Technology is decided: Rust-first for core, server, and
policy engine; Python and TypeScript SDKs; TypeScript admin UI; no Zig for
v0/v1 (ADR-0001).

You CAN: refine the domain model, ports, policy model, ADRs, implementation
guides; start Rust crate scaffolding consistent with the target architecture.

You MUST NOT: put a gateway or provider type into `thalamus-core` or
`thalamus-server`; hardcode policy as product branches; skip post-call
validation; absorb sibling-product ownership (Strategos memory, TruthMetal
truth, Robson invariants).

## The control-boundary framework

Before adding a capability, answer (from [BOUNDARIES.md](BOUNDARIES.md)):

1. **Control**: is this a decision/validation about an AI-mediated call?
2. **Data plane**: does this move bytes / hold connections / proxy / transport
   rate-limit? (must be NO)
3. **Gateway-coupling**: does this need a gateway/provider type in domain code?
   (must be NO)
4. **Ownership**: is this Strategos memory, TruthMetal truth, or Robson trading
   invariants? (must be NO)
5. **Policy**: can this be policy instead of a code branch? (prefer YES)

Record the result for Level 2/3 work in
[docs/99-reference/design-decisions.md](docs/99-reference/design-decisions.md)
or as an ADR in `docs/adr/`.

## Decision levels

From [GOVERNANCE.md](GOVERNANCE.md):

- **Autonomous**: doc fixes, examples, in-module refactors, tests. Just do it,
  document in the commit.
- **Reviewed**: new sections, features, API/port changes, dependencies. Propose
  with a recorded boundary analysis, implement after review.
- **Governed**: boundary redefinition, technology beyond ADR-0001, phase
  transitions, port add/remove. Present as an ADR; await leadership.

## Recording reasoning

For non-trivial changes:

```markdown
## [YYYY-MM-DD] [Change name]

**Type**: Documentation | Code | Process
**Decision Level**: Autonomous | Reviewed | Governed

**Context**: why this is needed

**Control-boundary analysis**:
- Control: ...
- Data plane: ...
- Gateway-coupling: ...
- Ownership: ...
- Policy: ...

**Decision**: what is done
**Alternatives**: what else was considered
**Impact**: what this affects
```

## Documentation style

Match the repository's style:

- Precise engineering language. No hype. No vague "AI platform" language
  without naming the control boundary.
- Prefer concrete terms: policy, audit, trace, tenant, workflow, context,
  schema, route, evaluation, risk, budget, gateway adapter, data plane, control
  plane.
- No em dashes.
- ASCII diagrams where useful.
- Markdown tables only when they improve clarity.
- Keep docs operational enough that another agent can implement from them.

## Code (Implementation phase)

- `thalamus-core`: pure domain. Standard library plus minimal audited crates.
  No adapter or gateway dependency. Ever.
- Adapters depend on `thalamus-core`, never the reverse.
- Policy is data evaluated by `PolicyEngine`, not branches.
- Every governed call path has pre-call and post-call phases.
- Run the repository's standard formatting and lint before committing if such
  tooling exists. Do not introduce new tooling as a side effect.

## Good vs bad contributions

```
GOOD: Add a citation-check port and a schema-only default implementation,
      with a recorded boundary analysis (Control=YES, Data plane=NO,
      Gateway-coupling=NO, Ownership=NO, Policy: which workflows require it).

BAD:  Add MCP connection pooling to thalamus-server.
      (Data plane = YES -> belongs below BackendPort.)

BAD:  if product == "robson" { model = "claude" }
      (Policy expressed as a branch -> belongs in policy data.)
```

## Commit messages

```
[Type] Brief description

Why, and boundary note if relevant.

Ref: docs/adr/ADR-XXXX or design-decisions.md#YYYY-MM-DD-name
```

Types: `Docs`, `Fix`, `Feature`, `Refactor`, `Test`.

## Git

Do not push. Do not commit unrelated working-tree changes. If a file has
pre-existing user modifications, inspect and preserve them; do not overwrite
unrelated work.

---

*Contributing to Thalamus means protecting the control boundary: decide and
validate, never transport, never absorb another product's ownership.*

*Last updated: 2026-05-16*
