# Sources and prior evidence

Consulted 2026-11-07. Upstream capabilities are not local RBX validation.

- [Laya upstream](https://github.com/NandhaKishorM/laya): structured classification;
  pin package/checkpoint/schema for experiments.
- [Laya routing](https://nandhakishorm.github.io/laya/routing/): selects Laya
  checkpoints. RBX tier/alias selection needs its own schema and evaluation.
- [Prior RBX IRC evidence](https://github.com/rbxrobotica/rbx-infra/blob/03a99d8201542624fdf32c1b73a1cd070f6c906b/infra/irc/pilots/evidence/laya-evaluation.json).
  Local report: `/home/psyctl/rbx/rbx-infra-wt-a2/infra/irc/pilots/LAYA-EVALUATION.md`.
  Laya 0.4.3, `convaiinnovations/laya-multilingual` revision
  `e4e9ddf21a7b1903b7acffd8814ad4307bf63a67`, CPU FP32/eager, two threads, batch 1.
  Results: Laya 8/32, rules 29/32. Synthetic IRC triage, small sample, single-author
  labels and cues favoring rules; not a test of LLM selection or newer checkpoints.
- [Robson Code wire contract](https://github.com/rbxrobotica/robson-code/blob/main/docs/dev/thalamus-wire-contract.md):
  granted lease before compilation; run-bound model honored or refused.
- Local authority: `AGENTS.md`, `BOUNDARIES.md`,
  `docs/adr/ADR-0001-thalamus-as-semantic-control-layer.md`,
  `docs/02-architecture/agentgateway-and-data-plane.md`,
  `docs/02-architecture/target-architecture.md`.
- Current selector: `crates/thalamus-core/src/flow.rs`, `select_backend`;
  inspect run-bound paths separately before implementing the proposed mode.
