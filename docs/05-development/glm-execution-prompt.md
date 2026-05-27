# GLM 5.1 Execution Prompt: TH-S1 (thalamus-core crate)

This file is the orchestration handoff. Paste the fenced block below into the
GLM 5.1 session (Codex / llm-proxy). It is self-contained. It executes only
slice TH-S1. It must not reopen the pivot, propose new architecture, or push.

---

```
ROLE
You are implementing slice TH-S1 of Thalamus in the repository ~/apps/thalamus-core.
Branch is already created context: create and work on `thalamus-th-s1-core-crate`
off the current `thalamus-semantic-control-layer-pivot` branch.

READ FIRST (binding, do not skip; if any conflict, the doc wins, stop and ask):
- docs/adr/ADR-0001-thalamus-as-semantic-control-layer.md
- BOUNDARIES.md
- docs/02-architecture/target-architecture.md
- docs/02-architecture/pre-call-and-post-call-responsibilities.md
- docs/05-development/implementation-guide.md   (nomenclature, VC, slice plan)

DEFINITION (do not relitigate)
Thalamus is the semantic control layer for AI traffic. It decides (pre-call),
validates (post-call), audits, evaluates, classifies risk, and produces routing
decisions. It does NOT transport bytes. Agentgateway/LiteLLM are BackendPort
implementations, never domain dependencies.

SCOPE: TH-S1 ONLY = the `thalamus-core` Rust crate. Nothing else.

DELIVERABLES
1. Cargo workspace at repo root with exactly one member: crates/thalamus-core.
2. crates/thalamus-core with modules: domain, policy, ports, flow, audit
   (exact layout in implementation-guide.md section 3).
3. Domain types: CallRequest, Envelope, PolicyDecision (Allow|Deny|
   AllowWithReview), PostCallResult (status Valid|Invalid|NeedsHumanReview,
   risk_class, executable_by_agent, strategos_event), AuditEvent, RiskLevel
   (Low|Medium|High|Prohibited).
4. policy: Policy, Budget, ContextGrant, RedactionRule + trait PolicyEngine.
   Minimal typed structs only. NO DSL, NO external taxonomy.
5. ports: trait BackendPort, ContextPort, PolicyPort, AuditPort, EvalPort,
   ObservabilityPort (definitions only).
6. flow: pure pre_call() and post_call() orchestrating the ports, matching the
   step lists in pre-call-and-post-call-responsibilities.md.
7. In-memory fake impls of the ports for tests only (under #[cfg(test)] or a
   `testing` module), never wired to real I/O.
8. Unit tests proving the acceptance gate below.

HARD RULES
- thalamus-core Cargo.toml: NO gateway/provider/HTTP-client/async-runtime-IO
  deps. std + minimal audited crates only (serde, time/chrono, uuid/ulid).
- No module/type may import or name a gateway/provider type.
- Policy is data evaluated by PolicyEngine, never `if product == ...`.
- Deny path must never construct or call BackendPort.
- post_call must always run on the Allow path before returning.
- Do not implement thalamus-server, adapters, SDKs, eval, console, EventBusPort,
  or any OpenMetadata/lateral integration.
- Forbidden names (review fails if present anywhere in crate): SignalRouter,
  SignalBus, MessageBroker, Relay, CognitiveChamber, Nervous*,
  analytical_signals, Gateway (as a domain type), LlmProxy, AgentgatewayClient.

ACCEPTANCE GATE (all must hold)
- `cargo build` and `cargo test` pass with thalamus-core alone (no adapter).
- Test: Deny PolicyDecision => BackendPort fake recorded zero calls.
- Test: Allow path => post_call ran and produced a PostCallResult.
- Test: AllowWithReview => status NeedsHumanReview, not executable_by_agent.
- `grep -rniE 'agentgateway|litellm|reqwest|tonic|openai|anthropic' crates/thalamus-core/src` is empty.
- No forbidden name present.

VERSION CONTROL
- Work on branch `thalamus-th-s1-core-crate`.
- Conventional Commits, scope `core`, trailer `Refs ADR-0001, Refs TH-S1`, and
  the agent co-author trailer. Small, reviewable commits.
- DO NOT push. DO NOT open a PR. Leave commits local for operator review.
- DO NOT modify docs/, ADRs, or any file outside crates/ and the workspace
  Cargo.toml.

WHEN BLOCKED
If a needed decision is not in an ADR (e.g. policy representation, audit
schema), STOP, write the ambiguity into docs/99-reference/design-decisions.md
under a dated heading, and ask. Do not invent the answer.

DONE = acceptance gate green, commits local on the slice branch, a short
summary of files added and test results.
```

---

## Orchestration notes (for the operator, not for GLM)

- I cannot invoke GLM 5.1 from here (the Agent tool is Claude-only). Run the
  block above in your GLM 5.1 / Codex session pointed at `~/apps/thalamus-core`.
- GLM is instructed NOT to push and NOT to PR (per
  `[[feedback_agent_git_remote_write_policy]]` and the Codex push incident).
  Review locally, then push/PR yourself or authorize it explicitly.
- Post-GLM review: use the "Review checklist for the post-GLM Opus session"
  from the Transition Contract (diff inspection, regression list, blockers).
- GLM is told to keep the rustfmt-clean expectation via `cargo test`; if CI
  uses nightly rustfmt, apply the standard post-push rustfmt fix
  (`[[feedback_rustfmt_pattern]]`) during the Opus review pass, not GLM's.

*Last updated: 2026-05-16*
