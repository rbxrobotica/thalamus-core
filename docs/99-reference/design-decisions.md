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

### 2026-05-17: Agentgateway LLM surface is OpenAI-compatible (TH-S4)

**Context**: TH-S4 adds `thalamus-agentgateway-adapter` as a second BackendPort
implementation. Agentgateway's LLM gateway routing surface exposes an
OpenAI-compatible contract (POST /v1/chat/completions with standard
request/response shape). This is the same common denominator the LiteLLM
adapter targets.

**Decision**: Implement against the OpenAI-compatible contract. The adapter
sends `POST /v1/chat/completions` with model + messages, parses the standard
choices/usage response shape. Endpoint URL and optional Authorization header
are configurable.

**Assumption to confirm**: This is based on Agentgateway's documented
OpenAI-compatible LLM routing surface. The endpoint path and header names are
configurable via `AdapterConfig`. If the real Agentgateway contract differs
materially (e.g. different path, non-standard auth, extra required fields), the
adapter config and wire types must be updated — but `thalamus-core` and
`thalamus-server` route/app logic remain unchanged.

**Decision Level**: Autonomous. **Status**: Accepted (assumption pending
confirmation against official Agentgateway docs). **Refs**: TH-S4, ADR-0001.

### 2026-05-17: Feature-driven backend selection with litellm priority (TH-S4)

**Context**: Multiple BackendPort adapters now exist (litellm, agentgateway).
The server must select which to wire at build time. Options: (1) Cargo feature
flags, (2) runtime config, (3) both.

**Decision**: Feature-driven selection. `litellm` feature takes priority when
both are enabled (it was first; the `#[cfg]` ordering in main.rs reflects this).
When no feature is enabled, `app::build()` runs with no backend (503 on Allow).
This matches the TH-S3 pattern exactly and keeps the selection in main.rs only.

Runtime config is deferred: a future slice can add a config-driven factory
that reads a `backend` field and selects at runtime, but that requires more
design (what if both adapters are compiled but only one is configured?). For
now, compile-time selection via features is simple, explicit, and matches the
existing litellm wiring.

**Decision Level**: Autonomous. **Status**: Accepted. **Refs**: TH-S4.

### 2026-05-17: Agentgateway adapter config shape (TH-S4)

**Context**: The agentgateway adapter needs configuration for endpoint, model
mapping, timeout, and authentication. This mirrors the LiteLLM adapter but adds
an `auth_header` field.

**Decision**: `AdapterConfig` fields: `endpoint` (base URL), `model_map`
(handle ID → model name), `timeout`, `auth_header` (optional `Authorization`
header value). Env vars for server wiring: `AGENTGATEWAY_ENDPOINT`,
`AGENTGATEWAY_AUTH_HEADER`. No runtime config file parsing in the adapter —
the server reads env vars and constructs the config in main.rs.

**Decision Level**: Autonomous. **Status**: Accepted. **Refs**: TH-S4.

### 2026-05-17: Python SDK uses httpx + pydantic v2 (TH-S5)

**Context**: The Python SDK needs an HTTP client and typed models. Options:
(1) requests + dataclasses (heavier HTTP lib, no validation),
(2) httpx + pydantic v2 (modern sync client, validated models),
(3) aiohttp + attrs (async-first, over-engineered for a thin SDK).

**Decision**: httpx (sync) + pydantic v2. httpx is the modern Python HTTP client
with a clean sync API, timeout support, and a well-maintained mocking library
(respx). pydantic v2 provides validated, typed models that serialize/deserialize
JSON directly. Sync is chosen over async because the SDK is a thin transport
client; callers can wrap in asyncio if needed.

**Decision Level**: Autonomous. **Status**: Accepted. **Refs**: TH-S5, ADR-0001.

### 2026-05-17: TypeScript SDK uses native fetch, no runtime deps (TH-S5)

**Context**: The TS SDK needs HTTP and type systems. Options:
(1) native fetch + plain interfaces (zero deps, Node 18+),
(2) undici + zod (extra dep, validation),
(3) axios + io-ts (heavier).

**Decision**: Native fetch + plain TypeScript interfaces. Node 18+ ships fetch
globally. The SDK is a thin transport client — no runtime validation library is
needed. Types are enforced at compile time by tsc. Zero runtime dependencies
keeps the package minimal.

**Decision Level**: Autonomous. **Status**: Accepted. **Refs**: TH-S5, ADR-0001.

### 2026-05-17: Shared contract fixture in JSON (TH-S5)

**Context**: Both SDKs need to verify they match the server wire contract.
Options: (1) separate fixtures per SDK, (2) shared fixture, (3) codegen from
Rust types.

**Decision**: Single shared `sdks/contract-fixture.json` derived from
routes.rs handler structs. Both SDK test suites consume it. This makes
server↔SDK drift detectable: if either SDK diverges from the fixture, tests
fail. Codegen is out of scope for TH-S5.

**Decision Level**: Autonomous. **Status**: Accepted. **Refs**: TH-S5.

### 2026-05-17: Vitest for TypeScript SDK tests (TH-S5)

**Context**: TS SDK needs a test runner. Options: vitest, jest.
Vitest is ESM-native, faster, and has first-class TypeScript support without
babel transforms. Jest requires CJS interop for ESM packages.

**Decision**: vitest. Matches ESM module system, zero config for TS.

**Decision Level**: Autonomous. **Status**: Accepted. **Refs**: TH-S5.

### 2026-05-18: Bounded channel + dedicated worker thread for non-blocking eval (TH-S6a)

**Context**: `EvalPort::submit` is called inside `post_call`, which runs on the
async handler thread (the same tokio worker that serves the HTTP request). A slow
or blocking eval submission would re-create the TH-S3.1 starvation bug: the async
runtime stalls while eval runs synchronously.

**Considered Options**:
1. **`tokio::spawn` async task** — submit returns immediately, eval runs in a
   spawned task. Pro: idiomatic async. Con: `EvalPort::submit` is a sync trait
   method (returns `String`, not a Future). Spawning from a sync context inside
   an async runtime requires `tokio::runtime::Handle::current().spawn()`, which
   couples thalamus-eval to tokio — violating the crate's dep constraint
   (thalamus-eval depends on thalamus-core only).
2. **Bounded crossbeam channel + dedicated worker thread** — submit does a
   `try_send` through a bounded channel; a dedicated std thread receives and
   stores. Pro: no async dep, no tokio coupling, bounded backpressure, truly
   non-blocking (`try_send` never blocks). Con: one extra thread per eval port
   instance.
3. **`std::thread::spawn` per submission** — fire-and-forget threads. Pro:
   simple. Con: unbounded thread creation, no backpressure, no graceful
   shutdown.

**Decision**: Option 2. Bounded crossbeam-channel (`crossbeam-channel` crate)
with a dedicated worker thread. Capacity 256 (configurable via constant in
app.rs). `try_send` is O(1) and never blocks. Full channel => record dropped
with a tracing warning (eval is best-effort per
observability-and-evaluation.md). Worker thread named `thalamus-eval-worker`
for diagnostics.

**Boundary analysis**: No new port, no core change, no HTTP, no ML. EvalRecord
contains only deterministic facts (schema validity, budget-based risk class,
content length, citation placeholder). No fabricated "quality scores".

**Decision Level**: Autonomous. **Status**: Accepted. **Refs**: TH-S6a, ADR-0001.

### 2026-05-18: EvalRecord shape — deterministic facts only (TH-S6a)

**Context**: The eval record must be honest about what it knows. There is no ML,
no hallucination detection model, no scoring engine. The record must contain
only what can be derived deterministically from the response and policy.

**Decision**: EvalRecord fields: `eval_ref` (UUID), `schema_valid` (non-empty
content), `citation_check` (placeholder: always NotRequired), `hallucination_signals`
(empty Vec — placeholder), `risk_class` (derived from budget usage, same logic as
flow.rs), `response_metadata` (content_len, tokens_used, latency_ms), `trace_id`,
`audit_id`, `policy_id`, `created_at`. Every placeholder is explicitly documented
in code as a placeholder. No numeric "quality score" or "confidence" field.

**Decision Level**: Autonomous. **Status**: Accepted. **Refs**: TH-S6a.

### 2026-05-18: crossbeam-channel over std::sync::mpsc for eval (TH-S6a)

**Context**: Need a bounded channel for the eval worker. Options: `std::sync::mpsc`
(sync channel, bounded available), `crossbeam-channel` (more robust, better
performance, `try_send` semantics).

**Decision**: crossbeam-channel v0.5. More ergonomic `try_send` (returns
`TrySendError` without panic), better performance under contention, widely used
in the Rust ecosystem. `std::sync::mpsc::sync_channel` would work but
crossbeam's API is cleaner and the dep is minimal.

**Decision Level**: Autonomous. **Status**: Accepted. **Refs**: TH-S6a.

### 2026-05-18: EvalSink trait — abstract sink for eval forwarding (TH-S6b)

**Context**: TH-S6a established a non-blocking ChannelEvalPort with a dedicated
worker thread that stores records in EvalStore. External observability (Langfuse)
needs to receive the same records, but thalamus-eval must stay HTTP-free.

**Decision**: `EvalSink` trait in thalamus-eval: `fn accept(&self, submission:
&EvalSubmission)`. Object-safe, no HTTP or tokio dependencies. ChannelEvalPort
accepts an injected `Arc<dyn EvalSink + Send + Sync>` via `new_with_sink()`.
Default `new()` uses `NoOpSink` (store-only, backward compatible). The worker
thread always inserts into EvalStore first, then forwards to the sink.

`EvalSubmission` wraps `EvalRecord` + optional `authorized_content: Option<String>`.
When ContentPolicy is MetadataOnly (default), authorized_content is always None.

**Decision Level**: Autonomous. **Status**: Accepted. **Refs**: TH-S6b.

### 2026-05-18: ContentPolicy — metadata-only default with redaction boundary (TH-S6b)

**Context**: Raw prompt/response content must not leave the process unless policy
explicitly authorizes it AND redaction is applied. The boundary is enforced BEFORE
the record reaches any external sink.

**Decision**: `ContentPolicy` enum: `MetadataOnly` (default, no content) vs
`IncludeRedacted` (content included after redaction). Applied in `submit()` before
the channel send — the EvalSubmission that reaches the sink already reflects the
policy decision. Redaction uses literal substring matching on `Policy.redaction_rules`:
`Redact` action replaces with `[REDACTED]`, `Block` action drops content entirely
(returns None). Literal matching (not regex) avoids adding regex to thalamus-eval.

**Boundary analysis**: EvalRecord already contains only deterministic metadata
(no raw content). ContentPolicy controls the optional authorized_content field.
Default = no content leaves. Only MetadataOnly is wired in thalamus-server today.

**Decision Level**: Autonomous. **Status**: Accepted. **Refs**: TH-S6b.

### 2026-05-18: Langfuse ingestion contract — configurable with recorded assumptions (TH-S6b)

**Context**: The Langfuse ingestion API contract could not be authoritatively
confirmed from public docs (JS-rendered pages, 404s on API reference URLs).

**Decision**: Implement against the assumed contract: `POST /api/public/ingestion`
with Bearer `{public_key}:{secret_key}` auth, JSON body `{ "batch": [event...] }`
where each event has `id`, `type`, `name`, `metadata`, `output`, `timestamp`.
All parts (endpoint path, auth header, payload shape) are configurable via
`LangfuseConfig`. If the real contract differs, only `LangfuseSink::build_payload`
and `LangfuseClient::post` need updating — no core/eval/server changes.

**Assumption**: Batch ingestion endpoint at `/api/public/ingestion` with Bearer
key:secret auth and event shape as described above. To be confirmed against
official Langfuse docs when available.

**Decision Level**: Autonomous. **Status**: Accepted (assumption pending
confirmation). **Refs**: TH-S6b.

### 2026-05-18: Sync ureq on eval worker thread for Langfuse (TH-S6b)

**Context**: Langfuse HTTP calls must not block the async runtime. The eval worker
thread is already a dedicated std::thread (from TH-S6a). Options: (1) sync ureq
on the worker thread, (2) tokio::spawn_blocking, (3) async HTTP client.

**Decision**: Sync ureq on the worker thread (option 1). The worker already runs
outside the async runtime. No spawn_blocking needed. No async HTTP dep needed.
Failure is best-effort: logged via tracing, record dropped, no retry. This matches
the TH-S3 ureq pattern and keeps the adapter simple.

**Decision Level**: Autonomous. **Status**: Accepted. **Refs**: TH-S6b.

### 2026-05-18: Langfuse feature gate in thalamus-server (TH-S6b)

**Context**: Langfuse adapter must not leak ureq/langfuse dependencies when the
feature is disabled. Default build must have no ureq in normal deps.

**Decision**: `langfuse` Cargo feature in thalamus-server, gating
`thalamus-langfuse-adapter` dependency and `app::build_with_eval_sink()`. Main.rs
has six mutually-exclusive cfg blocks (3 langfuse × {litellm, agentgateway, none}).
`cargo tree --edges normal` with no features shows no ureq via eval/langfuse path.

**Decision Level**: Autonomous. **Status**: Accepted. **Refs**: TH-S6b.

### 2026-07-27: Governed embeddings use a dedicated authenticated route and port

**Context**: ADR-0023 requires `rbx-memory` to obtain embeddings through
Thalamus rather than calling LiteLLM or a provider. `EmbeddingPort` and the
LiteLLM implementation already exist, but the server had no authenticated HTTP
surface connecting policy to that port.

**Decision**: Mount `POST /v1/embeddings` only on the credential-gated
`THALAMUS_RBX_API` router. The credential must target audience `thalamus` and
carry `thalamus:embeddings`; this is the capability authorization before policy
resolution. The request carries tenant, product, workflow, an institutional
model alias, and one or more input strings. Thalamus resolves and
evaluates the exact policy tuple, refuses a missing policy, refuses an alias not
listed as a model backend without substituting another alias, applies policy
block/redact rules, creates trace/audit IDs, emits pre-call, route, and post-call
audit events, and invokes `EmbeddingPort` once for the bounded batch. The
response validates count, dimensions, finite values, and alias correlation; it
returns no provider metadata.

The synchronous port runs in `spawn_blocking`, matching the existing adapter
boundary. LiteLLM model-name resolution and credentials stay in the adapter.
The route is additive and has no existing consumers; `rbx-memory` is the first
planned consumer.

**Control-boundary analysis**: Control = yes (identity, policy, redaction,
routing permission, audit, response validation). Provider transport ownership =
no (`EmbeddingPort` only). Gateway coupling = no (server imports no LiteLLM or
provider type). Sibling ownership = no (`rbx-memory` retains persistence and
retrieval; Thalamus stores no vectors). Variation = policy data keyed by tenant,
product, and workflow. The trade-off is a purpose-built governed contract rather
than wire compatibility with the provider-facing OpenAI request shape; this
keeps caller scope and audit correlation explicit.

**Decision Level**: Reviewed, within accepted ADR-0023 and the existing
`EmbeddingPort`. **Status**: Proposed in implementation PR.

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
