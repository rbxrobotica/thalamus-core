# Pre-Call and Post-Call Responsibilities

**Version**: 0.2.0 | **Last Updated**: 2026-05-16

Normative reference: [ADR-0001](../adr/ADR-0001-thalamus-as-semantic-control-layer.md).

Every governed AI-mediated call has two Thalamus phases. Neither is optional
unless policy explicitly exempts the workflow.

## Pre-call responsibilities

Before a model/tool/agent call, Thalamus is responsible for:

1. Identifying tenant, product, user, and workflow
2. Classifying intent
3. Selecting the applicable policy
4. Selecting the permitted model, tool, gateway, or backend
5. Enforcing budget, token, and latency limits
6. Building the prompt/envelope
7. Retrieving only authorized context
8. Redacting or blocking sensitive data
9. Making the routing decision
10. Creating `trace_id` and `audit_id`

### Pre-call flow

```
CallRequest
  | 1  resolve identity: tenant, product, user, workflow
  | 2  classify intent
  | 3  PolicyPort.resolve(request) -> Policy
  | 4  select permitted model / tool / gateway / backend (from Policy)
  | 5  enforce budget / token / latency limits (from Policy)
  | 6  build Envelope (prompt/payload)
  | 7  ContextPort.fetch(ContextGrant)  // only authorized context
  | 8  apply RedactionRule; block on prohibited content
  | 9  routing decision -> opaque backend handle (no gateway type)
  | 10 create trace_id, audit_id
  v
AuditPort.emit(PreCallDecision)
  |
  +--> Allow            -> hand Envelope to BackendPort
  +--> AllowWithReview  -> queue human review; hold execution
  +--> Deny             -> return typed denial (reason, policy_ref); no backend
```

### Worked example

```
Strategos requests a strategic analysis.

Thalamus verifies:
  tenant            = RBX
  module            = Business Plan
  sensitivity       = high
  permitted model   = Claude / GPT / Kimi, depending on policy
  audit required    = yes
  private context   = allowed only if policy authorizes it
  structured output = required

Only after this does the request proceed to the selected
gateway / provider / tool / agent path.
```

If policy does not authorize private context for this module/tenant,
`ContextPort` returns nothing for that reference and the envelope is built
without it. If the model requested by the caller is not in the permitted set,
Thalamus substitutes the policy-permitted model or denies, per policy.

## Post-call responsibilities

After a model/tool/agent response, Thalamus is responsible for:

1. Validating the response
2. Checking the response against a schema
3. Classifying operational risk
4. Detecting likely hallucination signals
5. Checking citations or sources where required
6. Applying business rules
7. Redacting sensitive information
8. Registering audit events
9. Sending data to automatic evaluation
10. Persisting events

### Post-call flow

```
backend response
  | 1  validate response (well-formed, within budget actuals)
  | 2  schema check (against workflow output schema)
  | 3  classify operational risk: Low | Medium | High | Prohibited
  | 4  hallucination signals
  | 5  citation / source check where policy requires
  | 6  apply business rules
  | 7  redact sensitive information
  | 8  AuditPort.emit(PostCallOutcome)
  | 9  EvalPort.submit(response, policy)   // automatic evaluation
  | 10 persist events
  v
PostCallResult { status, risk_class, executable_by_agent, strategos_event, ... }
```

### Worked example

```
A model returns an operational recommendation.

Thalamus checks:
  Does the response respect the schema?
  Did it cite non-existent data?
  Does it contain a prohibited recommendation?
  Does it require human review?
  Can it be executed by an agent?
  Should it become an event in Strategos?
```

Mapping the checks to outputs:

| Check | Field in `PostCallResult` |
|-------|---------------------------|
| Schema respected | `schema_check`, `status` |
| Cited non-existent data | `citation_check`, `hallucination_signals` |
| Prohibited recommendation | `risk_class = Prohibited`, `status = Invalid` |
| Requires human review | `status = NeedsHumanReview` |
| Executable by an agent | `executable_by_agent: bool` |
| Should become a Strategos event | `strategos_event: Option<...>` |

## The two API shapes

| Path | Caller responsibility | Thalamus responsibility |
|------|-----------------------|-------------------------|
| `/v1/call` | Provide `CallRequest` | Pre-call, delegate to `BackendPort`, post-call |
| `/v1/decide` + `/v1/post-call` | Execute backend itself (for example via an existing data plane) | Pre-call decision; later post-call validation of the externally produced response |

The split path exists so existing callers that already reach a data plane can
adopt governance incrementally without surrendering the round trip on day one.
The end state is `/v1/call`.

## Risk classification gate

`risk_class` controls what the caller may do with the response:

| `risk_class` | Default policy effect |
|--------------|-----------------------|
| `Low` | Returnable; agent-executable if policy allows |
| `Medium` | Returnable; not agent-executable without explicit policy grant |
| `High` | Requires deterministic or human validation before any execution |
| `Prohibited` | Not returnable as actionable; audited; surfaced for review |

Robson-specific rule: responses that could affect trade execution are never
auto-executable on `risk_class` alone. They require Robson's deterministic
validation or human review. See
[../03-integration/cross-product-positioning.md](../03-integration/cross-product-positioning.md).

## Audit and trace correlation

- `trace_id`: OpenTelemetry trace, propagated across Thalamus, the data plane,
  the provider/tool, and back.
- `audit_id`: stable identifier joining the `PreCallDecision` and
  `PostCallOutcome` audit events for one logical call.

Both are created in pre-call step 10 and carried through the entire round trip.
See [observability-and-evaluation.md](observability-and-evaluation.md).

## References

- [ADR-0001](../adr/ADR-0001-thalamus-as-semantic-control-layer.md)
- [../../ARCHITECTURE.md](../../ARCHITECTURE.md)
- [target-architecture.md](target-architecture.md)
