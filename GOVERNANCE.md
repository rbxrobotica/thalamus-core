# Thalamus Governance

**Version**: 0.2.0 | **Last Updated**: 2026-05-16

This document supersedes the 0.1.0 governance model, which assumed a Phase 0
"no technology, no code" Foundation. That assumption is removed by
[ADR-0001](docs/adr/ADR-0001-thalamus-as-semantic-control-layer.md), which makes
Rust-first a governed decision. The three-tier decision model below is retained.

## Governance philosophy

- AI agents have autonomy within clear boundaries (see [BOUNDARIES.md](BOUNDARIES.md)).
- Humans (RBX Systems leadership) make architectural and strategic decisions.
- Decisions are recorded as ADRs in `docs/adr/` or in
  `docs/99-reference/design-decisions.md`.
- Process is lightweight but the control boundary is non-negotiable.

## Phases (post-pivot)

The pivot replaces the old Foundation/Implementation/Integration/Evolution
ladder, which was framed around deferring all technology and code.

| Phase | State | Focus |
|-------|-------|-------|
| Architecture (current) | Active | Domain model, ports, policy model, ADRs, implementation guides. Technology is decided (Rust-first, ADR-0001). |
| Implementation | Next | `thalamus-core` crate, `PolicyEngine`, `thalamus-server`, adapters, SDKs. |
| Integration | After | Wire Robson, Strategos, Eden, agents; LiteLLM then Agentgateway `BackendPort`. |
| Evolution | Ongoing | Policy language maturity, eval depth, TruthMetal integration. |

Phase transitions are governed decisions (RBX Systems leadership).

## Decision levels

### Level 1: Autonomous (AI agents may decide)

- Documentation clarifications, typos, formatting, link fixes, examples.
- Code (Implementation phase): formatting, comments, in-module refactors,
  micro-optimizations, test additions.
- Process: make the change, document it in the commit message.

Constraints: must respect [BOUNDARIES.md](BOUNDARIES.md); must not introduce a
gateway/provider dependency into `thalamus-core` or `thalamus-server`.

### Level 2: Reviewed (propose, then implement)

- New conceptual sections, architectural clarifications, boundary refinements
  short of a redefinition.
- Code: new features, API changes, dependency additions, port changes.
- Process: record an analysis (control-boundary framework result), open a
  proposal, implement after review.

### Level 3: Governed (human decision required)

- Boundary redefinition (the control-plane/data-plane split).
- Language or major technology change beyond ADR-0001.
- Phase transitions, license, governance changes.
- Adding or removing a port from the domain contract.
- Process: present options and trade-offs as an ADR; await RBX Systems
  leadership decision; record the outcome.

## Versioning

Semantic versioning `MAJOR.MINOR.PATCH`.

- `0.1.x`: Foundation (signal-mediation; superseded).
- `0.2.x`: Architecture phase (this pivot).
- `0.x` -> `1.0.0`: first production-ready control plane (`thalamus-core` +
  `thalamus-server` + at least one `BackendPort` adapter + SDKs).

Breaking changes to the domain contract or ports require a `MAJOR` bump, a
migration note, and a governed decision.

## Contribution process

1. Read [BOUNDARIES.md](BOUNDARIES.md).
2. Apply the control-boundary framework; record the result for Level 2/3 work.
3. Determine the decision level.
4. Execute (Level 1) or propose (Level 2/3).
5. Keep `thalamus-core` free of adapter/gateway dependencies.

## Quality gates

### Before commit
- [ ] Boundary framework applied and (Level 2/3) recorded
- [ ] No gateway/provider type in core or server
- [ ] Variation expressed as policy, not branches
- [ ] Related docs updated
- [ ] Commit message clear

### Before release
- [ ] Version and CHANGELOG updated
- [ ] Domain-contract changes have an ADR
- [ ] Migration notes for breaking changes
- [ ] RBX Systems leadership approval

## Conflict resolution

1. Apply the control-boundary framework.
2. Check `docs/adr/` and `docs/99-reference/design-decisions.md` for precedent.
3. Document the ambiguity.
4. Escalate to RBX Systems leadership for governed arbitration.

## References

- [ADR-0001](docs/adr/ADR-0001-thalamus-as-semantic-control-layer.md)
- [BOUNDARIES.md](BOUNDARIES.md)
- [CONTRIBUTING.md](CONTRIBUTING.md)
- [docs/99-reference/design-decisions.md](docs/99-reference/design-decisions.md)

---

*Last updated: 2026-05-16*
