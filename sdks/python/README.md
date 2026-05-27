# thalamus-sdk

Thin synchronous Python SDK for the [Thalamus](../../README.md) semantic control layer.

## Install

```bash
pip install thalamus-sdk
```

## Quick start

```python
from thalamus import (
    ThalamusClient,
    DecideRequest,
    PostCallRequest,
)

with ThalamusClient("http://localhost:8080") as client:
    # Simple policy decision
    decision = client.decide(DecideRequest(
        tenant="rbx",
        product="robson",
        user="agent-1",
        workflow="trade-analysis",
        intent="market-summary",
        prompt="Summarize BTC market conditions",
    ))

    # Pre-call (returns envelope on Allow)
    pre = client.pre_call(DecideRequest(
        tenant="rbx",
        product="robson",
        user="agent-1",
        workflow="trade-analysis",
        intent="market-summary",
        prompt="Summarize BTC market conditions",
    ))
    print(pre.trace_id, pre.audit_id)

    # Full call (decide + execute + post-call in one)
    full = client.call(DecideRequest(
        tenant="rbx",
        product="robson",
        user="agent-1",
        workflow="trade-analysis",
        intent="market-summary",
        prompt="Summarize BTC market conditions",
    ))
    print(full.backend_content)

    # Post-call audit
    result = client.post_call(PostCallRequest(
        audit_id=pre.audit_id,
        content="BTC is trading at $95,000...",
        tokens_used=512,
        latency_ms=1200,
    ))

    # Retrieve audit trail
    audit = client.get_audit(pre.audit_id)
    for event in audit.events:
        print(event.kind, event.timestamp)
```

## Configuration

| Parameter     | Default | Description                          |
|---------------|---------|--------------------------------------|
| `base_url`    | --      | Thalamus server URL (required)       |
| `auth_header` | `None`  | Value for `Authorization` header     |
| `timeout`     | `30.0`  | HTTP timeout in seconds              |

```python
client = ThalamusClient(
    base_url="https://thalamus.example.com",
    auth_header="Bearer tok_abc123",
    timeout=10.0,
)
```

## Error handling

Non-2xx responses raise `ThalamusError` with `.status_code`, `.error`, and `.code`:

```python
from thalamus import ThalamusClient, ThalamusError

try:
    client.decide(req)
except ThalamusError as exc:
    print(exc.status_code, exc.code, exc.error)
```

## Contract fixture

The test suite validates against the shared contract fixture at `../../contract-fixture.json`. To re-run:

```bash
pip install -e ".[dev]"
pytest
```
