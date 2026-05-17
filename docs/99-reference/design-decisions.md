# Design Decisions

**Version**: 0.2.0 | **Last Updated**: 2026-05-16

> **Pivot notice (2026-05-16)**: The Foundation-phase decisions dated
> 2026-02-02 below are **Superseded by**
> [ADR-0001](../adr/ADR-0001-thalamus-as-semantic-control-layer.md), which
> redefines Thalamus as the semantic control layer for AI traffic. The
> biological-thalamus model and the Five-Question Framework are no longer
> normative. History is preserved here, not deleted. New architectural
> decisions are recorded as ADRs in `docs/adr/`; this log records
> non-ADR-level decisions and points at ADRs. See the "Post-pivot decisions"
> section near the end.

## Purpose

This document records all significant architectural and design decisions made for Thalamus. Each decision includes context, reasoning, alternatives considered, and boundary analysis.

**Why This Matters**: Design decisions provide precedent for future work, ensure consistency, and create a traceable reasoning chain.

## How to Use This Document

### For Contributors

Before proposing a change:
1. Search this document for similar past decisions
2. Check if precedent exists
3. Reference relevant decisions in your proposal
4. Add new decisions after approval

### For Reviewers

When reviewing proposals:
1. Check for precedent in this document
2. Verify consistency with past decisions
3. Ensure new decisions are documented
4. Maintain decision quality standards

## Decision Template

Use this template for all documented decisions:

```markdown
## [YYYY-MM-DD] [Decision Name]

**Context**: [Why this decision is needed, background, problem statement]

**Considered Options**:
1. [Option A] - [Pros and cons]
2. [Option B] - [Pros and cons]
3. [Option C] - [Pros and cons]

**Decision**: [What was decided]

**Boundary Analysis**:
- Signal Question: [Is this about signal routing/normalization?]
- Decision Question: [Does this make business decisions?]
- Domain Question: [Is this specific to one product?]
- State Question: [Does this require long-term persistent state?]
- Reusability Question: [Would every RBX product need this?]
- Verdict: [PASSES all boundaries / VIOLATES X boundary]

**Rationale**: [Why this decision, how it maintains boundaries, alignment with architecture]

**Consequences**: [Impact on architecture, users, integration, technical debt]

**Decision Level**: [Autonomous / Reviewed / Governed]

**Decided By**: [Agent ID or Human name]

**Status**: [Accepted / Superseded / Deprecated]

**Related Decisions**: [Links to related decisions]
```

---

## Foundation Phase Decisions

### 2026-02-02: Establish Foundation First

**Context**: Starting with empty repository, need to determine first steps. Options include diving into code or establishing architecture first.

**Considered Options**:
1. **Start coding immediately** - Begin implementation to validate concepts
   - Pros: Fast feedback, concrete artifacts
   - Cons: Risk of wrong direction, hard to change, boundary violations likely
2. **Establish documentation foundation first** - Define architecture before code
   - Pros: Clear boundaries, aligned contributors, avoid rework
   - Cons: Delayed concrete artifacts, requires discipline
3. **Hybrid approach** - Document and code simultaneously
   - Pros: Balance between speed and clarity
   - Cons: Risk of documentation-code drift, unclear precedence

**Decision**: Establish documentation foundation first (Phase 0), implement later (Phase 1).

**Boundary Analysis**:
- N/A (meta-decision about process, not feature)

**Rationale**:
- Clear boundaries prevent costly rework
- AI-first development requires explicit guidelines
- Architecture clarity enables autonomous agent work
- Precedent from successful open-source projects
- RBX Systems values quality through discipline

**Consequences**:
- Delayed implementation (accepted trade-off)
- Comprehensive documentation (benefit)
- Clear contribution guidelines (benefit)
- Time investment in Phase 0 (pays off in Phase 1+)

**Decision Level**: Governed

**Decided By**: RBX Systems leadership

**Status**: Accepted

---

### 2026-02-02: Biological Thalamus as Architectural Model

**Context**: Need a conceptual model for signal mediation. Options include generic message bus patterns or domain-specific architectures.

**Considered Options**:
1. **Generic message bus pattern** - Traditional pub/sub, message queue
   - Pros: Well-understood, many implementations
   - Cons: Too generic, lacks semantic structure, no boundary guidance
2. **Biological thalamus model** - Sensory relay station pattern from neuroscience
   - Pros: Clear boundaries (relay not decision), proven pattern, conceptually elegant
   - Cons: May seem esoteric, requires explanation, not a direct analogy
3. **Domain-specific mediator** - Built around trading or coding concepts
   - Pros: Directly applicable to initial products
   - Cons: Not reusable, violates business-agnostic principle

**Decision**: Use biological thalamus as architectural inspiration and conceptual model.

**Boundary Analysis**:
- Signal Question: YES (thalamus is fundamentally about signal routing)
- Decision Question: NO (thalamus relays, doesn't decide)
- Domain Question: NO (biological pattern is universal)
- State Question: NO (working memory, not long-term storage)
- Reusability Question: YES (all systems need signal mediation)
- Verdict: PASSES all boundaries

**Rationale**:
- Biological thalamus has clear boundaries (relay vs decision)
- Proven pattern (millions of years of evolution)
- Provides intuitive mental model
- Separates mediation from decision naturally
- Aligns with reusability and business-agnostic principles

**Consequences**:
- Need to educate contributors on biological inspiration
- Must avoid over-literalizing the analogy
- Creates clear conceptual framework
- Enables boundary-based reasoning
- Provides memorable name and concept

**Decision Level**: Governed

**Decided By**: RBX Systems leadership

**Status**: Superseded by [ADR-0001](../adr/ADR-0001-thalamus-as-semantic-control-layer.md) (2026-05-16). The biological-thalamus model is no longer the normative architecture; the normative model is control plane vs data plane. Retained as historical context and as a loose intuition only.

**Related Decisions**: Foundation First (architectural thinking before implementation)

---

### 2026-02-02: Five-Question Boundary Framework

**Context**: Need explicit mechanism to determine if features belong in Thalamus. Implicit boundaries are insufficient for AI agent work.

**Considered Options**:
1. **Implicit boundaries** - "You know it when you see it"
   - Pros: Flexible, intuitive for experienced humans
   - Cons: Subjective, inconsistent, doesn't work for AI agents
2. **Checklist framework** - Specific questions to validate boundaries
   - Pros: Explicit, consistent, works for humans and agents
   - Cons: May feel bureaucratic, requires discipline
3. **Example-based** - List of do's and don'ts
   - Pros: Concrete, easy to understand
   - Cons: Never comprehensive, edge cases unclear

**Decision**: Implement Five-Question Framework (checklist) as primary boundary validation.

**Questions**:
1. Signal Question: Is this about signal routing/normalization?
2. Decision Question: Does this make business decisions?
3. Domain Question: Is this specific to one product?
4. State Question: Does this require long-term persistent state?
5. Reusability Question: Would every RBX product need this?

**Boundary Analysis**:
- N/A (meta-decision about boundary enforcement, not feature)

**Rationale**:
- Explicit questions enable AI agent autonomy
- Consistent framework prevents ambiguity
- All five questions align with core principles
- Traceable reasoning for all features
- Can be automated in future tooling

**Consequences**:
- Contributors must apply framework (discipline required)
- Documentation overhead (small, worthwhile)
- Clear accept/reject criteria (benefit)
- Enables autonomous agent contributions (major benefit)
- May evolve as we learn (acceptable)

**Decision Level**: Governed

**Decided By**: RBX Systems leadership

**Status**: Superseded by [ADR-0001](../adr/ADR-0001-thalamus-as-semantic-control-layer.md) (2026-05-16). Replaced by the control-boundary framework in [BOUNDARIES.md](../../BOUNDARIES.md) (Control / Data plane / Gateway-coupling / Ownership / Policy).

**Related Decisions**: Foundation First, Biological Model

---

### 2026-02-02: AI-First, Human-Centered Governance

**Context**: Need governance model that leverages AI agents while maintaining quality and strategic control.

**Considered Options**:
1. **Human-only governance** - All decisions require human review
   - Pros: Maximum control, familiar process
   - Cons: Bottleneck, underutilizes AI capabilities
2. **Autonomous AI agents** - AI agents make all decisions
   - Pros: Maximum speed, no bottlenecks
   - Cons: Risk of boundary violations, strategic drift
3. **Tiered decision model** - AI autonomous for some, human for others
   - Pros: Leverage AI for routine, human for strategic
   - Cons: Requires clear tier definitions

**Decision**: Implement three-tier decision model (Autonomous, Reviewed, Governed).

**Tiers**:
- **Autonomous**: AI agents decide (typos, examples, formatting)
- **Reviewed**: AI proposes, human reviews (features, architecture)
- **Governed**: Human decides (technology, phases, boundaries)

**Boundary Analysis**:
- N/A (meta-decision about process, not feature)

**Rationale**:
- Leverages AI agent capabilities appropriately
- Maintains human strategic control
- Clear decision authority prevents conflicts
- Enables rapid iteration on routine work
- Protects architectural integrity

**Consequences**:
- Requires clear tier definitions (GOVERNANCE.md)
- AI agents need guidelines (.claude/agent-guidelines.md)
- Training overhead for contributors (acceptable)
- Faster routine work (benefit)
- Maintained quality on strategic decisions (benefit)

**Decision Level**: Governed

**Decided By**: RBX Systems leadership

**Status**: Accepted

**Related Decisions**: Foundation First

---

### 2026-02-02: Documentation-First Architecture

**Context**: Need to define architecture. Options include code-first (implementation defines architecture) or documentation-first (architecture defines implementation).

**Considered Options**:
1. **Code-first** - Write code, extract architecture later
   - Pros: Fast concrete feedback
   - Cons: Risk of architectural drift, hard to refactor
2. **Documentation-first** - Define architecture, implement later
   - Pros: Clear direction, aligned implementation
   - Cons: Delayed validation, may need revision
3. **Parallel** - Document and implement simultaneously
   - Pros: Balance
   - Cons: Risk of documentation-code drift

**Decision**: Documentation-first (Phase 0: Foundation before Phase 1: Implementation).

**Boundary Analysis**:
- N/A (meta-decision about process, not feature)

**Rationale**:
- Aligns with "Foundation First" decision
- Enables boundary definition before boundary violations possible
- Allows architectural discussion without code constraints
- Better for AI-first development (clear guidelines before autonomy)
- Reduces rework risk

**Consequences**:
- Delayed implementation feedback (accepted)
- Comprehensive documentation (benefit)
- May need architecture refinement after implementation (expected)
- Clear foundation for contributors (benefit)

**Decision Level**: Governed

**Decided By**: RBX Systems leadership

**Status**: Accepted

**Related Decisions**: Foundation First

---

### 2026-02-02: Single Repository for Core

**Context**: Need to decide repository structure. Options include monorepo, multi-repo, or product-integrated.

**Considered Options**:
1. **Monorepo** - All Thalamus code (core + adapters + docs) in one repo
   - Pros: Easy coordination, atomic changes
   - Cons: Mixes concerns, larger surface area
2. **Separate core repository** - Core only, products integrate via dependency
   - Pros: Clear core boundary, independent versioning
   - Cons: Coordination overhead for changes
3. **Product-integrated** - Thalamus embedded in each product repo
   - Pros: Product-specific flexibility
   - Cons: Violates reusability, duplicates core

**Decision**: Single repository for Thalamus core (thalamus-core). Product adapters live in product repos.

**Boundary Analysis**:
- Signal Question: YES (repository structure supports signal mediation focus)
- Decision Question: NO (doesn't make business decisions)
- Domain Question: NO (core is domain-agnostic)
- State Question: NO (repository structure, not state management)
- Reusability Question: YES (single core serves all products)
- Verdict: PASSES all boundaries

**Rationale**:
- Clear boundary between core and product code
- Single source of truth for core
- Products depend on versioned core
- Independent evolution of core and products
- Enforces no product-specific code in core

**Consequences**:
- Need clear versioning strategy (semantic versioning)
- Product-core integration via dependency management
- Adapter code lives in product repos (correct separation)
- Core changes require product updates (manageable)

**Decision Level**: Governed

**Decided By**: RBX Systems leadership

**Status**: Accepted

**Related Decisions**: Biological Model (clear core boundaries)

---

## Post-pivot decisions

### 2026-05-16: Thalamus is the semantic control layer for AI traffic

**Context**: The Foundation signal-router framing did not name the control
boundary RBX needs (policy, audit, context authorization, evaluation, risk) for
AI-mediated calls.

**Decision**: Recorded as
[ADR-0001](../adr/ADR-0001-thalamus-as-semantic-control-layer.md). Thalamus is
the control plane for AI traffic; the data plane (Agentgateway, LiteLLM,
others) is replaceable behind `BackendPort`.

**Control-boundary analysis**: Control YES, Data plane NO, Gateway-coupling NO
(adapter-only), Ownership NO, variation expressed as Policy.

**Decision Level**: Governed. **Decided By**: RBX Systems leadership.
**Status**: Accepted. **Supersedes**: Biological Model, Five-Question
Framework, and the no-code Foundation phase.

### 2026-05-16: Rust-first language decision

**Context**: Thalamus is infrastructure-critical and enforces policy, context
boundaries, auditability, and operational safety. Strong typing and explicit
invariants matter; Rust aligns with Robson and RBX reliability posture.

**Decision**: Core, server, policy engine in Rust. SDKs in Python and
TypeScript. Admin UI in TypeScript. Gateway adapters Rust-first (Go only if an
ecosystem integration is clearly better). No Zig for v0/v1. Detail in
[ADR-0001](../adr/ADR-0001-thalamus-as-semantic-control-layer.md) and
[target-architecture.md](../02-architecture/target-architecture.md).

**Decision Level**: Governed. **Status**: Accepted. **Supersedes**: the
Foundation rule "no technology choices in Phase 0".

### 2026-05-16: Ports keep the data plane replaceable

**Context**: Direct provider/gateway calls couple products to backends.

**Decision**: `thalamus-core` defines port traits (`BackendPort`,
`ContextPort`, `PolicyPort`, `AuditPort`, `EvalPort`, `ObservabilityPort`).
Adapters implement them and depend on `thalamus-core`; `thalamus-core` depends
on no adapter and no gateway type.

**Decision Level**: Reviewed (architectural, within ADR-0001). **Status**:
Accepted.

### 2026-05-17: ureq as HTTP client for LiteLLM adapter (TH-S3)

**Context**: `BackendPort::call` is synchronous (returns `BackendResponse`, not a
Future). The adapter must make real HTTP calls to LiteLLM within this sync trait.
Options: reqwest (async, would need spawn_blocking bridge), ureq (sync, minimal),
minreq (very minimal but less maintained).

**Decision**: ureq v3. Sync, production-grade, minimal dependency tree, supports
TLS via rustls, has timeout support. Matches the sync `BackendPort` trait directly.

**Why not reqwest**: reqwest's blocking feature pulls in tokio, adding weight. The
async client would need `tokio::task::spawn_blocking` wrapping, adding complexity
for no benefit. ureq is purpose-built for sync use cases.

**Timeout/retry**: `timeout_global` set to 30s (configurable). No retry in the
adapter — retry policy belongs in the data plane (LiteLLM) or in a future
circuit-breaker wrapper, not in the adapter.

**Decision Level**: Autonomous. **Status**: Accepted. **Refs**: TH-S3.

### 2026-05-17: Adapter error taxonomy (TH-S3)

**Context**: The adapter must handle HTTP failures gracefully without panicking.
`BackendPort::call` returns `BackendResponse` (no error variant), so errors are
internal to the adapter: logged via tracing, surfaced as empty-content responses
that post_call marks as Invalid.

**Decision**: Five typed errors in `AdapterError`: Connection, Timeout,
ServerError (4xx/5xx), MalformedResponse, ModelMapping. The public `call()`
method catches all errors and returns an empty `BackendResponse`. The internal
`call_internal()` returns `Result<BackendResponse, AdapterError>` for unit testing.

**Decision Level**: Autonomous. **Status**: Accepted. **Refs**: TH-S3.

### 2026-05-17: Audit-store correlation for post-call (TH-S3 follow-up ii)

**Context**: `/v1/post-call` previously trusted caller-supplied budget and policy.
This violated the control-plane/data-plane boundary: the caller could lie about
budget to bypass post-call validation.

**Decision**: `AuditStore` now stores a `PreCallRecord` (envelope + policy) keyed
by `audit_id`. Both `/v1/pre-call` and `/v1/call` store records. `/v1/post-call`
looks up the record and uses the stored policy/budget, not caller-supplied values.
Unknown `audit_id` returns 404 with `UNKNOWN_AUDIT_ID` code. `PostCallRequest`
simplified to: audit_id + content + tokens_used + latency_ms.

**Decision Level**: Reviewed (within TH-S3 scope). **Status**: Accepted.
**Refs**: TH-S3, ADR-0001 (control-plane boundary).

### 2026-05-17: mockito for adapter tests (TH-S3)

**Context**: Tests must exercise the full stack (HTTP routes + adapter + mock LiteLLM)
without real network access. Options: mockito (HTTP mock server), wiremock-rs,
manual TCP listener.

**Decision**: mockito v1. Lightweight, async-compatible, request matching, runs
on localhost. The adapter (ureq) connects to mockito's mock server. No real
network.

**Decision Level**: Autonomous. **Status**: Accepted. **Refs**: TH-S3.

### Open items

- Policy language/representation and evaluation semantics: not yet decided.
- Audit store schema and retention (must respect Postgres-external constraint):
  not yet decided.
- Whether `PolicyEngine` stays in-process or becomes a separate service:
  deferred until load characteristics are known.

## Placeholder for Future Decisions

New decisions will be added here as the project evolves. All contributors must document significant decisions using the template above.

### Decision Categories

Decisions typically fall into these categories:

- **Architectural**: Core architecture, layers, interfaces
- **Boundary**: Boundary definitions, enforcement mechanisms
- **Process**: Development process, governance, contribution
- **Technical**: Technology choices (Phase 1+), implementation patterns
- **Integration**: Product integration patterns, adapter design

---

## Decision Status Lifecycle

- **Proposed**: Under consideration, not yet decided
- **Accepted**: Decided and active
- **Superseded**: Replaced by newer decision (link to replacement)
- **Deprecated**: No longer recommended but not replaced
- **Rejected**: Considered but not adopted (document why)

---

## How to Challenge a Decision

Past decisions can be revisited if circumstances change:

1. Reference the original decision
2. Explain what changed (new information, environment, requirements)
3. Propose alternative with reasoning
4. Show boundary compliance
5. Analyze impact of change
6. Request governed-level review

**Important**: Decisions should not be revisited lightly. Stability is valuable.

---

## Quick Reference

### Search This Document

When proposing a change, search for keywords:
- Feature type (routing, normalization, context)
- Boundary concern (business logic, product-specific)
- Technical area (architecture, integration, process)
- Decision date (recent decisions more relevant)

### Related Documents

- [BOUNDARIES.md](../../BOUNDARIES.md) - Boundary definitions
- [ARCHITECTURE.md](../../ARCHITECTURE.md) - Architecture principles
- [GOVERNANCE.md](../../GOVERNANCE.md) - Decision process
- [CONTRIBUTING.md](../../CONTRIBUTING.md) - Contribution guidelines

---

## Contributing Decisions

When adding new decisions:

1. Use the template provided above
2. Include all sections (context, options, analysis, etc.)
3. Apply Five-Question Framework for feature decisions
4. Link to related decisions
5. Update date and version
6. Place in appropriate category

**Decision Level**: Reviewed (new decisions require review)

---

**Remember**: This document is the project's architectural memory. Maintain it with care.

*Last updated: 2026-02-02*
