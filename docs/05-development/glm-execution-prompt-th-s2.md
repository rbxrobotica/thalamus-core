# GLM 5.1 Execution Prompt: TH-S2 (thalamus-server)

Orchestration handoff for slice TH-S2. Paste the fenced block into GLM 5.1
(Codex / llm-proxy) pointed at `~/apps/thalamus-core`. Self-contained. Executes
only TH-S2. Must not reopen the pivot, add ports beyond ADR-0001, or push.

Prerequisite: TH-S1 (`thalamus-core`, commit `a791ddc`) is accepted and on
`origin/thalamus-th-s1-core-crate`. TH-S2 builds on it.

---

```
ROLE
You implement slice TH-S2 of Thalamus in ~/apps/thalamus-core.
Create and work on branch `thalamus-th-s2-server` off `thalamus-th-s1-core-crate`.

READ FIRST (binding; if any conflict, the doc wins, stop and ask):
- docs/adr/ADR-0001-thalamus-as-semantic-control-layer.md
- BOUNDARIES.md
- docs/02-architecture/target-architecture.md   (endpoint list, ports table)
- docs/02-architecture/pre-call-and-post-call-responsibilities.md
- docs/05-development/implementation-guide.md    (nomenclature, VC, slice plan)
- crates/thalamus-core/src/*                      (the API you build on)

DEFINITION (do not relitigate): Thalamus is the semantic control layer for AI
traffic. It decides (pre-call), validates (post-call), audits, evaluates,
classifies risk, produces routing decisions. It does NOT own provider transport; inline model payload mediation is allowed only through BackendPort.
Agentgateway/LiteLLM are BackendPort impls, never domain deps.

SCOPE: TH-S2 ONLY = the `thalamus-server` crate + the minimal `thalamus-core`
changes named below. NO data-plane adapter (that is TH-S3). NO SDKs, eval,
console, EventBusPort, OpenMetadata.

DELIVERABLES
1. New workspace member crates/thalamus-server (binary).
2. HTTP API (axum + tokio, or equivalently minimal audited stack) exposing:
   - POST /v1/decide      -> PolicyDecision only, no execution
   - POST /v1/pre-call    -> PreCallOutcome (decision + Envelope when allowed)
   - POST /v1/post-call   -> PostCallResult for an externally executed response
   - POST /v1/call        -> pre_call, then BackendPort, then post_call
   - GET  /v1/audit/{id}  -> audit events for an audit_id
   gRPC is OPTIONAL and may be deferred; do not block TH-S2 on it.
3. thalamus-core stays pure and synchronous. All async/IO lives in
   thalamus-server only. thalamus-core must still `cargo test` standalone.
4. Concrete PolicyPort: load typed `Policy` values from a config file
   (JSON/YAML into the existing structs). NO policy DSL, NO external taxonomy.
5. AuditPort impl = append-only structured log + in-memory store for
   /v1/audit. NO database (Postgres is later and external-constrained; do not
   add a DB). EvalPort and ObservabilityPort = logging stubs for TH-S2.
6. BackendPort: server takes a BackendPort via configuration/injection. With
   none configured, /v1/call returns a structured 503 (no data plane). A dev
   EchoBackend MAY exist ONLY under #[cfg(test)] or a `dev` feature, never the
   default path. Do not implement a real backend.

FOLD IN THESE TH-S1 FOLLOW-UPS (required, not optional)
- A. thalamus-core: replace the panic in `select_backend`
  (`.expect("policy must have at least one permitted backend")`) with a typed
  error. Introduce a `PreCallError` (or `thalamus_core::Error`) and make
  pre_call return Result. Update TH-S1 tests accordingly. Scope `core`.
- B. Set `publish = false` in crates/thalamus-core/Cargo.toml and
  crates/thalamus-server/Cargo.toml, and remove the `license = "MIT OR
  Apache-2.0"` line (repo license is TBD/governed; do not assert one).
- C. Structural enforcement in /v1/call:
  * post_call is non-bypassable: there is no code path that returns a backend
    response to the caller without post_call having run.
  * PolicyDecision::Deny  -> no BackendPort call, structured deny response.
  * PolicyDecision::AllowWithReview -> no BackendPort call; respond with a
    held/NeedsHumanReview outcome (review id), execution not performed.
  * Only PolicyDecision::Allow performs BackendPort then post_call.

HARD RULES
- No gateway/provider type anywhere in thalamus-core OR thalamus-server.
  thalamus-server depends on thalamus-core, never on an adapter.
- Policy is data evaluated by PolicyEngine/PolicyPort, never `if product ==`.
- thalamus-core Cargo.toml keeps only std + serde/uuid/time (no new deps).
- Forbidden names anywhere: SignalRouter, SignalBus, MessageBroker, Relay,
  CognitiveChamber, Nervous*, analytical_signals, Gateway (domain type),
  LlmProxy, AgentgatewayClient.

ACCEPTANCE GATE (all must hold)
- cargo build and cargo test pass for the workspace.
- thalamus-core still builds/tests STANDALONE (no server/async deps leaked in).
- Integration tests (axum test client or equivalent):
  * /v1/call Deny  => BackendPort fake recorded 0 calls, structured deny.
  * /v1/call AllowWithReview => 0 backend calls, NeedsHumanReview + review id.
  * /v1/call Allow => backend called once, post_call ran, PostCallResult
    returned; no path returns the raw backend response without post_call.
  * /v1/decide returns a decision and performs no backend call.
  * /v1/post-call validates an externally supplied response.
  * /v1/audit/{id} returns the pre/post audit events for that id.
  * empty permitted_backends on Allow => typed error surfaced as a structured
    4xx, NOT a panic/500-crash.
- grep -rniE 'agentgateway|litellm|reqwest|tonic|openai|anthropic'
  crates/thalamus-core/src crates/thalamus-server/src
  shows no gateway/provider client (tonic only if gRPC is implemented, and
  never in thalamus-core).
- No forbidden name present. publish=false set; no license line.

VERSION CONTROL
- Branch thalamus-th-s2-server. Conventional Commits: scope `server` for
  server work, scope `core` for follow-up A/B in thalamus-core. Trailers
  `Refs ADR-0001, Refs TH-S2` + agent co-author trailer. Small commits.
- DO NOT push. DO NOT open a PR. Commits stay local for operator review.
- DO NOT modify docs/, ADRs, or files outside crates/ and workspace Cargo.toml
  (Cargo.toml workspace member addition is allowed).

WHEN BLOCKED
If a needed decision is not in an ADR (e.g. config file format for policies,
gRPC inclusion, error taxonomy shape), STOP, record the ambiguity in
docs/99-reference/design-decisions.md under a dated heading, and ask. Do not
invent a policy DSL, a database, or a data-plane adapter.

DONE = acceptance gate green, commits local on thalamus-th-s2-server, short
summary of files added/changed and test results.
```

---

## Orchestration notes (operator, not GLM)

- I cannot invoke GLM 5.1 (Agent tool is Claude-only). Run the block in your
  GLM 5.1 / Codex session on `~/apps/thalamus-core`.
- GLM will not push or PR (per `[[feedback_agent_git_remote_write_policy]]` and
  the Codex push incident). Review locally; authorize push explicitly.
- Post-GLM review: reuse the Transition Contract checklist plus verify the
  three TH-S1 follow-ups (A typed error, B publish=false/no license, C
  non-bypassable post_call + AllowWithReview holds execution) are actually met.
- If CI enforces nightly rustfmt, apply the standard post-push rustfmt fix
  (`[[feedback_rustfmt_pattern]]`) during the Opus review pass, not GLM's.

*Last updated: 2026-05-16*
