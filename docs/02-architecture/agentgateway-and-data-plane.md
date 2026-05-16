# Agentgateway and the Data Plane

**Version**: 0.2.0 | **Last Updated**: 2026-05-16

Normative reference: [ADR-0001](../adr/ADR-0001-thalamus-as-semantic-control-layer.md).

## The separation

```
THALAMUS (control plane)            DATA PLANE (replaceable backend)
----------------------------        ----------------------------------
business rules                      connectivity
policy                              proxy
evaluation                          routing (transport)
context authorization               MCP gateway
audit                               A2A gateway
routing decisions                   LLM gateway
risk classification                 rate limits
pre-call validation                 low-level traffic observability
post-call validation                transport-level enforcement
traceability
model and tool governance
```

Agentgateway is a privileged data plane backend, not Thalamus. Thalamus supports
it through `thalamus-agentgateway-adapter`, which implements `BackendPort`.

> Thalamus is gateway-agnostic at the product layer, but Agentgateway-native at
> the RBX infrastructure adapter layer.

## The invariant

Thalamus domain logic must not depend directly on Agentgateway types.

```
thalamus-core           ---- defines ---->  trait BackendPort
thalamus-server         ---- uses    ---->  BackendPort
thalamus-agentgateway-  ---- impl    ---->  BackendPort (knows Agentgateway)
  adapter
```

`thalamus-core` and `thalamus-server` never import an Agentgateway crate or
type. Only the adapter does. The same rule applies to any provider SDK.

## What the adapter may do

`thalamus-agentgateway-adapter` may:

- translate Thalamus policies into Agentgateway-compatible config or policy
  hooks
- inject tenant / workflow / risk headers
- propagate `trace_id` and `audit_id`
- route LLM, MCP, A2A, and tool traffic
- consume traffic telemetry
- support rate-limit and budget enforcement at the transport edge
- expose route / provider / tool metadata to Thalamus audit and evaluation

The adapter consumes a `PolicyDecision` and an approved `Envelope`. It returns a
raw backend response. It does not make policy.

## Replaceable backends

The data plane can be:

- Agentgateway (recommended RBX low-level backend when the deployment requires
  MCP, A2A, LLM traffic routing, rate limits, and traffic observability)
- LiteLLM (the current experimental RBX data plane in
  `rbx-infra/apps/prod/llm-gateway`, namespace `llm-gateway`, ClusterIP only)
- OpenRouter
- Azure API Management
- Envoy
- Kong
- direct provider calls
- future RBX gateways

Each is a `BackendPort` implementation. Swapping one for another is an adapter
change, never a product or domain change. Thalamus remains the semantic and
governance layer regardless of which backend is wired.

## Current RBX data plane reality

`rbx-infra` today runs an experimental LiteLLM proxy:

- namespace: `llm-gateway`
- exposure: internal only (ClusterIP:4000), no Ingress
- status: candidate, non-critical, safe to scale to zero
- model aliases: Groq, GLM (Z.AI), Kimi (Moonshot) active; OpenAI/Anthropic/
  DeepSeek/Qwen commented out
- Postgres: external on jaguar via in-namespace Service + explicit `Endpoints`

This is a data plane. It has no policy, no context authorization, no audit, no
evaluation, no risk classification. Thalamus is the missing control plane above
it. The first `BackendPort` adapter can target this LiteLLM deployment so
governance can be adopted before Agentgateway is introduced; an Agentgateway
adapter follows when MCP/A2A/LLM routing and transport observability are
required.

`llm-proxy` (the local Go proxy in `~/apps/llm-proxy`) is a developer-machine
tool for IDE assistants. It is out of scope as an RBX data plane backend, though
the same `BackendPort` abstraction would apply if it were ever promoted.

## Migration path

```
phase A (now):  callers -> provider/LiteLLM directly. No governance.

phase B:        callers -> thalamus-sdk -> thalamus-server
                  -> /v1/decide (governance) -> caller executes via LiteLLM
                  -> /v1/post-call (validation)
                LiteLLM BackendPort adapter exists but split path used first.

phase C:        callers -> thalamus-sdk -> /v1/call
                  -> Thalamus owns round trip via LiteLLM BackendPort.

phase D:        introduce Agentgateway for MCP/A2A/LLM routing.
                add thalamus-agentgateway-adapter (BackendPort).
                swap backend by policy/config. No caller change.
```

Phase B exists specifically so existing direct-to-data-plane callers can adopt
governance incrementally without surrendering the round trip immediately.

## Boundary checks for adapter work

Before adding to an adapter, confirm against [BOUNDARIES.md](../../BOUNDARIES.md):

- Does this put a gateway type into `thalamus-core` or `thalamus-server`? If
  yes, stop. It belongs in the adapter only.
- Is this policy logic? If yes, it belongs in the policy engine, not the
  adapter. The adapter executes decisions; it does not make them.
- Is this transport (connections, streams, rate-limit mechanics)? Then it is
  data plane and stays below `BackendPort`.

## References

- [ADR-0001](../adr/ADR-0001-thalamus-as-semantic-control-layer.md)
- [target-architecture.md](target-architecture.md)
- [../../BOUNDARIES.md](../../BOUNDARIES.md)
- `rbx-infra/apps/prod/llm-gateway/README.md`
