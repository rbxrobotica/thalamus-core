# AGENTS.md — Thalamus Core

This file is the **agnostic agent guide** for Codex, Claude Code, Aider, Cursor, Windsurf, Kimi, GLM and any other coding agent working in this repository. Read it before contributing.

## Read these first

- `PURPOSE.md`, `BOUNDARIES.md`, `GOVERNANCE.md`, `ARCHITECTURE.md` (root of this repo)
- `docs/adr/ADR-0001-thalamus-as-semantic-control-layer.md` — the canonical reason this repo exists
- `docs/02-architecture/agentgateway-and-data-plane.md` — the BackendPort seam
- `docs/02-architecture/target-architecture.md` — components and ports

## The institutional rules that apply to this repo (RBX ADR-0008)

The Architecture Council ratified [`rbx-governance/docs/adr/ADR-0008-agentic-mcp-governance-and-internal-domain-mcps.md`](https://github.com/rbxrobotica/rbx-governance/blob/main/docs/adr/ADR-0008-agentic-mcp-governance-and-internal-domain-mcps.md) on 2026-05-27. The companion human-readable standard is at [`rbx-governance/docs/governance/AI-AGENT-TOOLING-AND-MCP-GOVERNANCE.md`](https://github.com/rbxrobotica/rbx-governance/blob/main/docs/governance/AI-AGENT-TOOLING-AND-MCP-GOVERNANCE.md). The 10 rules in summary:

1. **Prefer internal RBX domain MCPs** over raw external technical MCPs in institutional flows.
2. **External MCPs sit behind a `BackendPort` adapter** (Agentgateway recommended; LiteLLM permitted for LLM traffic).
3. **Thalamus governs every relevant call** (pre-call + post-call), or it is explicitly exempted by recorded policy.
4. **Production is read-only by default** for debug paths; writes require an approved policy + recorded human approval (ADR-0007).
5. **`tenant_scope` is mandatory** for tenant-owned data; omission = refusal.
6. **Secrets and PII are redacted at the control plane**; never surfaced in tool outputs.
7. **Audit-class MCPs emit evidence bundles** tied to `trace_id` and `mission_id`.
8. **Context budget is governed** (≤ 70% of model window); sessions isolated by work type.
9. **Local-dev MCP usage is permitted** for development and reviewed sessions; never for automated production paths.
10. **Tool/MCP/Skill Registry** in `rbx-maestro` (module 4.5) is the single canonical catalogue; unregistered MCPs are not reachable through the institutional path.

## What this means specifically for `thalamus-core`

This repository **is** the AI control plane named by ADR-0008. Several invariants are direct constraints on what may be added here:

- **No transport** in `thalamus-core` or `thalamus-server`. No connection pools, no MCP multiplexing, no streaming proxies. That is the data plane (Agentgateway / LiteLLM / others) behind `BackendPort`.
- **No gateway types** in domain code. `thalamus-core` and `thalamus-server` must never import an Agentgateway / LiteLLM / provider type. Only adapter crates (`thalamus-agentgateway-adapter`, future LiteLLM adapter, etc.) may know those types.
- **Policy is data, not code branches.** Variation belongs in the policy engine; do not encode product-specific rules as `if`/`switch` in core or server.
- **Pre-call decision + post-call validation are first-class.** Every AI-mediated call passes both, unless an exemption is recorded by policy.
- **TruthMetal (planned) owns ground truth.** Do not absorb dataset/factual-oracle responsibilities into Thalamus.
- **Strategos owns strategic memory.** Do not absorb decision history or rationale.
- **Robson owns trading invariants.** Thalamus governs the call; Robson is the source of truth for risk.

When adding capabilities, run the **Control-Boundary Framework** in `BOUNDARIES.md` (Control / Data plane / Gateway-coupling / Ownership / Policy questions). Record the result for Level 2 / Level 3 work in `docs/99-reference/design-decisions.md`.

## Thalamus policy envelope (canonical)

Every institutional MCP / A2A / LLM call carries this envelope (see ADR-0008 §7 for full spec):

```json
{
  "actor": "agent_id or user_id",
  "purpose": "debug | review | incident | replay | governance_check | implementation",
  "environment": "local | staging | production",
  "tenant_scope": "tenant_id or none",
  "flow_scope": {
    "trace_id": "...",
    "mission_id": "...",
    "event_id": "...",
    "command_id": "..."
  },
  "access_mode": "read_only | proposed_write | approved_write",
  "risk_class": "low | medium | high | restricted",
  "evidence_required": true
}
```

`access_mode = approved_write` requires a `rbx-governance` approval record per ADR-0007. `risk_class = restricted` triggers additional per-MCP policy gates.

## Versioning of internal RBX domain MCPs

Internal MCPs (not part of this repo, but consumed via `BackendPort` adapters) are versioned semantically; consumers pin `MAJOR.MINOR`; breaking changes require an amending per-MCP technical ADR; deprecation windows are at least one release cycle; no silent compat shims. See ADR-0008 §"Versioning of internal RBX domain MCPs".

## Evidence bundle retention

Audit-class internal MCPs emit evidence bundles using the canonical `retention_class` enum from ADR-0001 (`permanent | 7years | 2years | ephemeral`). Defaults: debug → ephemeral, replay/lineage → 2years, quality/cost → 2years, incident → permanent.

## Local-dev exception (R9)

You may wire raw MCPs directly in your own Claude Code / Codex / Kimi / GLM workstation session — including the lean-ctx MCP, plugin marketplace MCPs, or Codex's internal GitHub MCP — for local development and reviewed sessions. You may not rely on that wiring for automated workflows, scheduled jobs, or production paths.

## Decision levels (from this repo's `GOVERNANCE.md`)

- **Level 1 (autonomous)**: docs, formatting, comments, in-module refactors, micro-opt, tests. Must respect BOUNDARIES.md.
- **Level 2 (reviewed)**: new features, API/port changes, deps, conceptual sections. Record control-boundary analysis; open a proposal.
- **Level 3 (governed)**: boundary redefinition, language/major-tech change, phase transitions, port add/remove from the domain contract. Requires a governed decision (RBX leadership).

## Where to push back

If you believe one of the 10 rules above is wrong for a specific case, open a counter-ADR in `rbx-governance/docs/adr/` rather than editing this file or working around the rule. Boundary changes are Level 3 in this repo's governance.

## Pointers

- Memory entry (Claude Code auto-memory): `~/.claude/projects/-home-psyctl-apps/memory/feedback_agentic_mcp_governance.md`
- Capability ownership: `rbx-governance/docs/governance/CAPABILITY-OWNERSHIP-MATRIX.md`
- Implementation plan: `rbx-governance/docs/roadmaps/rbx-agentic-mcp-governance-implementation-plan.md`
- Phase 1 inventory: `rbx-governance/docs/governance/AI-AGENT-TOOLING-INVENTORY.md`

---

**Last updated:** 2026-05-27 — added on ratification of ADR-0008.
