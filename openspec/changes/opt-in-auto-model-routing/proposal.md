# Proposal: Governed opt-in auto-model routing with Laya evaluated as an advisor

Status: backlog/proposal; no router implementation or activation.
Owner: Thalamus; evaluation: Verentir; client coordination: Robson Code.
Assignee and priority are maintained in the linked GitHub issue. Created: 2026-11-07.
GitHub tracking: [Thalamus #25](https://github.com/rbxrobotica/thalamus-core/issues/25).
This change versions the technical scope and checklist. GitHub progress reconciliation remains agent-managed until the publisher is configured.

## Why

When the user selects auto-model, choose an eligible model using task quality and
total resource/cost evidence. Preserve explicit model selection. Evaluate whether
Laya improves on deterministic rules and a fixed-model baseline.

The prior RBX Laya pilot scored 8/32 versus rules 29/32 on synthetic Portuguese IRC
triage. That separate-domain, small-sample result neither validates LLM selection
nor disproves it. Require a new held-out evaluation. See [sources.md](sources.md).

## What Changes

Proposed flow:

1. Explicit user opt-in; collect a minimal authorized task descriptor before
   model-specific prompt compilation, under a defined input/privacy contract.
2. Policy filters aliases by tenant/workflow permissions, locality, required
   capabilities, context, resource limits and budget.
3. A replaceable rules/Laya selector recommends a tier/alias within that set.
4. Thalamus validates the decision and pins the alias/profile/capabilities in the
   run/route lease before the client compiles the prompt.
5. Execute through BackendPort/data plane and audit selection and outcome.

Laya's own Router selects Laya checkpoints, including by language; it is not a
ready-made Qwen/Ministral selector. RBX must define and evaluate the decision schema
and mapping to eligible aliases. A classifier recommendation grants no permission.

## Capabilities

### New Capabilities

- `opt-in-auto-model-routing`: explicit, policy-bounded model selection with
  auditable recommendations and compatible run/lease negotiation.

### Modified Capabilities

No accepted OpenSpec baseline is modified. Detailed design is pending. auto-model
is a proposed product mode; its request field/alias is not yet an accepted API.

## Acceptance Criteria

1. Auto is explicit opt-in. Manual aliases retain the run-bound honor-or-refuse
   contract. Choose before prompt compilation; pin the effective model to the run.
   Material profile changes require renegotiation/recompilation, never silent
   substitution during an inference.
2. Filter before ranking; validate policy before lease issuance and at call time.
   Empty sets, refusals and exhausted budgets cannot be bypassed. Inventory actual
   enforcement: missing capability/locality/budget controls must be implemented
   and tested or block the affected acceptance claim.
3. Timeout, invalid output, unavailability or insufficient evidence yields only
   an eligible configured fallback or typed refusal. Local-only never silently
   becomes cloud; no widened provider/data/budget scope. Distinguish pre-lease
   fallback from a new negotiated run after a lease has been issued.
4. Treat task/tool text as untrusted input, not policy. Minimize sensitive input,
   bound its length, and test misleading instructions, ambiguity, mixed languages
   and out-of-distribution tasks. Empirically calibrate scores/abstention; raw
   confidence is not assumed to be a probability.
5. Keep the model-backed selector outside the Rust domain behind an approved,
   replaceable boundary. Pin package/checkpoint/schema; govern classifier inference
   or record an explicit policy exemption. Prevent recursive auto selection and
   bound time, resources and retries.
6. Verentir compares fixed model, rules and Laya on held-out PT-BR tasks using the
   candidate capability matrix. Freeze criteria first; measure final-task quality,
   under/over-selection, abstention, calibration, cold/warm overhead, RAM/VRAM,
   model loading/swapping and cost per accepted task. CPU-first is a hypothesis
   to benchmark, preserving the 16 GB GPU for inference where beneficial.
7. Begin with offline evaluation, then shadow mode: record Laya proposals while
   existing approved routing still executes. Bounded local opt-in promotion needs
   an evidence-backed decision and rollback/kill switch; no default enablement.
8. Audit requested mode, eligible/effective aliases, policy/capability/schema and
   selector/checkpoint versions, reason/score, fallback, added latency, resource/
   cost evidence and trace/run references. Avoid raw sensitive prompt retention by
   default. Clients display the effective model and a concise reason.
9. Publish adoption/limitation/rejection evidence. Realized ROI needs attributable
   economic effects; routing accuracy or estimated token savings alone is not ROI.

## Control-Boundary Analysis (proposal, pending design review)

- Control: yes; model selection is a pre-call control decision.
- Data plane: no provider transport, credentials, pools or retries move into core/
  server; execution stays behind BackendPort adapters.
- Gateway coupling: no Laya/provider SDK or gateway types enter domain types.
- Ownership: Verentir evaluates; TruthMetal accepts rubrics. Thalamus does not own
  ground truth, financial facts or strategic memory.
- Policy: encode eligibility/preferences/budget/fallback as data, not product
  branches. Laya is replaceable and cannot authorize calls.

Before implementation, record the refined analysis in
`docs/99-reference/design-decisions.md` as required by AGENTS.md. New API/dependency
work needs Level 2 review; domain port or boundary changes require the applicable
Level 3 decision. This backlog proposal is not that implementation decision.

## Dependencies and Gaps

- [Verentir #11](https://github.com/rbxrobotica/verentir/issues/11) and the
  [Verentir #12](https://github.com/rbxrobotica/verentir/issues/12) model-comparison evidence matrix.
- Current core `select_backend` chooses a requested permitted backend or the first
  permitted backend; it is not an evaluated semantic router. Inspect run-bound
  paths separately; the Robson Code lease precedes model-specific compilation.
- Capability/locality/monetary/cumulative-budget controls require a code inventory;
  a conceptual envelope is not end-to-end enforcement evidence.

No downloads, service startup, production rollout or Mint bootstrap edits occur
in this change. Ledger cost reconciliation remains a separate workstream.
