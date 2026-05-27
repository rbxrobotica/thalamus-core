# @rbx/thalamus-sdk

Thin TypeScript SDK for the Thalamus semantic control layer. Zero policy logic — serialize, call, deserialize.

## Install

```bash
npm install @rbx/thalamus-sdk
```

## Quick Start

```typescript
import {
  ThalamusClient,
  type DecideRequest,
  type PostCallRequest,
} from "@rbx/thalamus-sdk";

const client = new ThalamusClient({ baseUrl: "http://localhost:8080" });

// Policy decision only
const decision = await client.decide({
  tenant: "rbx",
  product: "robson",
  user: "agent-1",
  workflow: "trade-analysis",
  intent: "market-summary",
  prompt: "Summarize BTC market conditions",
});
console.log(decision.decision); // "Allow" | "Deny" | "AllowWithReview"

// Pre-call: decision + envelope
const pre = await client.preCall({
  tenant: "rbx",
  product: "robson",
  user: "agent-1",
  workflow: "trade-analysis",
  intent: "market-summary",
  prompt: "Summarize BTC conditions",
  requested_backend: { id: "gpt-4o", backend_type: "Model" },
  budget_hint: { max_tokens: 4096 },
});
console.log(pre.trace_id, pre.audit_id);

// Full call: pre-call + backend + post-call
const result = await client.call({
  tenant: "rbx",
  product: "robson",
  user: "agent-1",
  workflow: "trade-analysis",
  intent: "market-summary",
  prompt: "Summarize BTC conditions",
});
console.log(result.backend_content);
console.log(result.post_call.status);

// Post-call (split path)
const post = await client.postCall({
  audit_id: pre.audit_id,
  content: "Response text...",
  tokens_used: 512,
});
console.log(post.status, post.risk_class);

// Audit trail
const audit = await client.getAudit(pre.audit_id);
for (const event of audit.events) {
  console.log(event.kind, event.trace_id);
}
```

## Configuration

```typescript
const client = new ThalamusClient({
  baseUrl: "http://localhost:8080",  // Required
  authHeader: "Bearer my-token",     // Optional
  timeout: 30_000,                   // Optional, ms (default 30000)
});
```

Swapping `baseUrl` is the only change needed to point at a different thalamus-server.

## Error Handling

```typescript
import { ThalamusError } from "@rbx/thalamus-sdk";

try {
  await client.decide(req);
} catch (e) {
  if (e instanceof ThalamusError) {
    console.log(e.statusCode);   // HTTP status
    console.log(e.errorCode);    // e.g. "NO_PERMITTED_BACKENDS"
    console.log(e.errorMessage); // Human-readable message
  }
}
```

## Contract Fixture

Both SDK test suites consume `../contract-fixture.json`, which is derived from the server's `routes.rs` handler structs. To verify the SDK matches the current server contract:

1. Re-derive `contract-fixture.json` from `crates/thalamus-server/src/routes.rs` request/response types.
2. Run `npm test` — if types diverge, tests fail.

## Running Tests

```bash
npm install
npm test
```
