# Observability and Evaluation

**Version**: 0.2.0 | **Last Updated**: 2026-05-16

Normative reference: [ADR-0001](../adr/ADR-0001-thalamus-as-semantic-control-layer.md).

Thalamus treats observability and evaluation as first-class outputs of the
control plane, not as logging side effects.

## The four tools and their separation

```
OpenTelemetry   instrumentation and telemetry standard
                traces, metrics, logs correlation, trace propagation
                across Thalamus, the data plane, providers, RBX systems

Langfuse        LLM trace and evaluation layer
                prompt/generation inspection, scoring, datasets,
                evals, model usage and cost

Prometheus      metrics scraping and alerting
                SLO/SLA metrics, rate/error/duration/saturation,
                policy decision counts, validation failure counts,
                gateway/provider failure rates

Grafana         dashboards and operational views
                system health, cost/usage panels, correlation with
                traces and logs where available
```

Prometheus and Grafana are infrastructure metrics and dashboard tools. They are
NOT replacements for Langfuse. Langfuse is the LLM-specific trace and evaluation
layer.

## OpenTelemetry (observability backbone)

OpenTelemetry is the vendor-neutral backbone. Thalamus emits, through
`ObservabilityPort` over OTLP:

- traces and spans for every call
- metrics
- logs correlation
- `trace_id` propagation across Thalamus, the data plane, providers, and RBX
  systems
- `audit_id` correlation (audit event id attached as a span attribute)
- tenant / product / workflow metadata
- gateway / provider / model / tool metadata
- latency, retries, failures, policy decisions, validation outcomes

The rbx-harness Thalamus protocol already mandates a `trace_id` per message and
defines OTLP exporters (`rbx-harness/spec/protocol.md`). Thalamus aligns with
that: the same `trace_id` spans the agent message, the Thalamus decision, the
data plane hop, and the post-call validation.

```
trace: <trace_id>
  span: thalamus.pre_call            attrs: tenant, product, workflow,
  |     policy_ref, decision               selected_backend, risk_tier
  span: dataplane.route              attrs: backend, provider, model_or_tool
  span: provider.invoke              attrs: latency_ms, retries, tokens
  span: thalamus.post_call           attrs: schema_check, risk_class,
        |                                  hallucination_signals,
        |                                  citation_check, audit_id
        span: thalamus.eval.submit
```

Infrastructure status: no OpenTelemetry Collector is deployed in `rbx-infra`
today (verified 2026-05-16: `platform/monitoring/` contains
kube-prometheus-stack, Loki, Promtail only). Deploying an OTLP collector in the
`monitoring` namespace is a prerequisite for end-to-end tracing and is a
recommended next infrastructure step. Until then, Thalamus still produces spans;
they need a collector and a trace backend (for example Tempo) to be visualized.

## Langfuse (LLM observability and evaluation)

Langfuse is the LLM observability and evaluation layer, reached through
`EvalPort` and used by `thalamus-eval`:

- prompt and prompt-version tracking
- generations
- traces
- scoring
- datasets
- evaluation runs
- model comparison
- feedback
- cost and token analysis where applicable
- prompt and response inspection under RBX policy boundaries (only
  policy-authorized content is sent; redaction applies before submission)

Infrastructure status: Langfuse is not deployed in `rbx-infra` today (verified
2026-05-16). Adding it is a recommended next step. Langfuse requires a
PostgreSQL database. The RBX constraint is non-negotiable: PostgreSQL never runs
inside the production k3s cluster (`rbx-infra/docs/infra/ARCHITECTURE.md`). A
Langfuse deployment must use a dedicated external Postgres instance managed by
Ansible, following the same pattern as the LiteLLM data plane
(`rbx-infra/apps/prod/llm-gateway`: in-namespace Service with explicit
`Endpoints` pointing at the external host).

Policy boundary: Thalamus only submits to Langfuse what policy authorizes.
Redaction (post-call step 7) runs before any submission. Sensitive context is
never sent to the evaluation layer unless policy explicitly allows it.

## Prometheus (metrics and alerting)

Verified present in `rbx-infra`:
`platform/monitoring/kube-prometheus-stack.yml` deploys Prometheus, Grafana,
Alertmanager, node-exporter, and kube-state-metrics. Metrics retention is 7
days. Prometheus auto-scrapes pods carrying `prometheus.io/scrape: "true"`
annotations.

`thalamus-server` should expose a Prometheus metrics endpoint and carry the
scrape annotations so it is picked up automatically. Control-plane metrics to
export:

- policy decision counts by decision (allow / allow-with-review / deny)
- validation failure counts by check (schema / risk / hallucination / citation)
- gateway/provider failure rates
- pre-call and post-call latency
- budget-exceeded and rate-limit events
- rate, error, duration, saturation (RED/USE) for the service

These feed SLO/SLA alerting via Alertmanager.

## Grafana (dashboards)

Verified present (part of kube-prometheus-stack), exposed at
`grafana.rbxsystems.ch`, with Loki pre-configured as a datasource. Self-hosted,
budget-conscious (ADR-11 in `rbx-infra`).

Thalamus dashboards:

- control-plane health (RED/USE)
- policy decision breakdown and deny reasons
- validation failure breakdown
- cost and token usage panels (if exported as metrics)
- correlation with logs (Loki) and, once a collector and trace backend exist,
  with traces

## Logs

`rbx-infra` runs Loki and Promtail (verified). Thalamus structured logs are
shipped by Promtail to Loki and correlated to traces by `trace_id` and to audit
by `audit_id`.

## Audit vs observability

These are distinct and must not be conflated:

- Observability (OTel/Prometheus/Grafana/Loki) is operational. It can be
  sampled, has retention limits (7d metrics), and is for running the system.
- Audit (`AuditPort`, durable Postgres) is a governance record. It is not
  sampled, has its own retention policy, and is the source of truth for "who
  called which model, under which policy, with which outcome."

A dropped span is acceptable. A dropped audit event is not.

## Deferred and lateral integrations

OpenMetadata is a candidate AI governance catalog (asset catalog, lineage,
ownership, classifications, glossary, data contracts) for the assets Thalamus
governs: models, providers, eval datasets, RAG/context sources, tools, agents,
tenants, owners, risk classes. It is **not** a policy engine, gateway, or
runtime dependency, and it does not replace Langfuse, OpenTelemetry, or
Prometheus.

Position and timing:

- Lateral, not in the request path. Thalamus emits; an integration publishes to
  the catalog. Runtime decisions are served from Thalamus-owned policy
  snapshots, never from synchronous OpenMetadata calls.
- Deferred to a later phase (after `thalamus-core`, `thalamus-server`, and the
  first `BackendPort` adapter). Not part of the first slices.
- Reached, when introduced, behind a future lateral integration port (same
  discipline as `EventBusPort`: only when natural, never to future-proof). The
  port must not exist in slice 1.
- Must not shape the `Policy` model yet. The policy representation is an open
  question; an external catalog taxonomy must not pre-decide it.

## Infrastructure assumptions summary

| Tool | Status in rbx-infra (verified 2026-05-16) | Action |
|------|-------------------------------------------|--------|
| Prometheus | Present (kube-prometheus-stack) | Expose metrics + scrape annotations |
| Grafana | Present (grafana.rbxsystems.ch) | Add Thalamus dashboards |
| Alertmanager | Present (kube-prometheus-stack) | Add control-plane alerts |
| Loki + Promtail | Present | Ship structured logs |
| OpenTelemetry Collector | Not deployed | Recommended next infra step |
| Trace backend (Tempo/Jaeger) | Not deployed | Recommended with collector |
| Langfuse | Not deployed | Recommended; needs external Postgres |

## References

- [ADR-0001](../adr/ADR-0001-thalamus-as-semantic-control-layer.md)
- [pre-call-and-post-call-responsibilities.md](pre-call-and-post-call-responsibilities.md)
- `rbx-infra/platform/monitoring/kube-prometheus-stack.yml`
- `rbx-infra/docs/infra/ARCHITECTURE.md` (Postgres-external constraint)
- `rbx-harness/spec/protocol.md` (trace_id and OTLP in the agent protocol)
