import { describe, it, expect, vi, beforeEach } from "vitest";
import { ThalamusClient, ThalamusError } from "../src/client.js";
import fixtureData from "../../contract-fixture.json" with { type: "json" };
import type {
  DecideRequest,
  PostCallRequest,
} from "../src/types.js";

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

const BASE_URL = "http://thalamus.test";

interface FixtureEntry {
  endpoint: string;
  request: Record<string, unknown>;
  response: {
    status: number;
    body: unknown;
  };
}

function entry(name: string): FixtureEntry {
  return (fixtureData as Record<string, FixtureEntry>)[name];
}

function decideReq(name: string): DecideRequest {
  const r = entry(name).request;
  return {
    tenant: r.tenant as string,
    product: r.product as string,
    user: r.user as string,
    workflow: r.workflow as string,
    intent: r.intent as string,
    prompt: r.prompt as string,
    ...(r.requested_backend ? { requested_backend: r.requested_backend as { id: string; backend_type: string } } : {}),
    ...(r.budget_hint ? { budget_hint: r.budget_hint as { max_tokens?: number; max_latency_ms?: number } } : {}),
  };
}

function postCallReq(name: string): PostCallRequest {
  const r = entry(name).request;
  return {
    audit_id: r.audit_id as string,
    content: r.content as string,
    ...(r.tokens_used != null ? { tokens_used: r.tokens_used as number } : {}),
    ...(r.latency_ms != null ? { latency_ms: r.latency_ms as number } : {}),
  };
}

function mockResponse(status: number, body: unknown): Response {
  return new Response(JSON.stringify(body), {
    status,
    headers: { "Content-Type": "application/json" },
  });
}

function setupFetch(fixtureName: string): void {
  const e = entry(fixtureName);
  vi.stubGlobal("fetch", vi.fn().mockResolvedValue(mockResponse(e.response.status, e.response.body)));
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

beforeEach(() => {
  vi.restoreAllMocks();
});

// ---- /v1/decide ------------------------------------------------------------

describe("decide", () => {
  it("Allow", async () => {
    setupFetch("decide_allow");

    const client = new ThalamusClient({ baseUrl: BASE_URL });
    const resp = await client.decide(decideReq("decide_allow"));

    expect(resp.decision).toBe("Allow");
    expect(resp.policy_id).toBe("rbx-robson-default");
    expect(resp.reason).toBeNull();
  });

  it("Deny with reason and policy_ref", async () => {
    setupFetch("decide_deny");

    const client = new ThalamusClient({ baseUrl: BASE_URL });
    const resp = await client.decide(decideReq("decide_deny"));

    expect(resp.decision).toBe("Deny");
    expect(resp.reason).toBe("budget exceeded");
    expect(resp.policy_ref).toBe("rbx-robson-budget-v2");
  });

  it("AllowWithReview with review_reason", async () => {
    setupFetch("decide_allow_with_review");

    const client = new ThalamusClient({ baseUrl: BASE_URL });
    const resp = await client.decide(decideReq("decide_allow_with_review"));

    expect(resp.decision).toBe("AllowWithReview");
    expect(resp.review_reason).toContain("human review");
    expect(resp.policy_ref).toBe("rbx-strategos-review-v1");
  });
});

// ---- /v1/pre-call ----------------------------------------------------------

describe("preCall", () => {
  it("Allow with envelope", async () => {
    setupFetch("pre_call_allow");

    const client = new ThalamusClient({ baseUrl: BASE_URL });
    const resp = await client.preCall(decideReq("pre_call_allow"));

    expect(resp.decision).toBe("Allow");
    expect(resp.trace_id).toBe("a1b2c3d4-e5f6-7890-abcd-ef1234567890");
    expect(resp.audit_id).toBe("f1e2d3c4-b5a6-7890-abcd-ef1234567890");
    expect(resp.envelope).toBeDefined();
    expect(resp.envelope!.backend_handle_id).toBe("gpt-4o");
    expect(resp.envelope!.budget_max_tokens).toBe(4096);
  });

  it("throws ThalamusError on 422 NO_PERMITTED_BACKENDS", async () => {
    setupFetch("pre_call_no_permitted_backends");

    const client = new ThalamusClient({ baseUrl: BASE_URL });

    try {
      await client.preCall(decideReq("pre_call_no_permitted_backends"));
      expect.unreachable("should have thrown");
    } catch (e) {
      expect(e).toBeInstanceOf(ThalamusError);
      expect((e as ThalamusError).statusCode).toBe(422);
      expect((e as ThalamusError).errorCode).toBe("NO_PERMITTED_BACKENDS");
    }
  });
});

// ---- /v1/post-call ---------------------------------------------------------

describe("postCall", () => {
  it("Valid with audit_id", async () => {
    setupFetch("post_call_valid");

    const client = new ThalamusClient({ baseUrl: BASE_URL });
    const resp = await client.postCall(postCallReq("post_call_valid"));

    expect(resp.status).toBe("Valid");
    expect(resp.audit_id).toBe("f1e2d3c4-b5a6-7890-abcd-ef1234567890");
    expect(resp.executable_by_agent).toBe(true);
    expect(resp.schema_valid).toBe(true);
  });

  it("throws ThalamusError on 404 UNKNOWN_AUDIT_ID", async () => {
    setupFetch("post_call_unknown_audit");

    const client = new ThalamusClient({ baseUrl: BASE_URL });

    try {
      await client.postCall(postCallReq("post_call_unknown_audit"));
      expect.unreachable("should have thrown");
    } catch (e) {
      expect(e).toBeInstanceOf(ThalamusError);
      expect((e as ThalamusError).statusCode).toBe(404);
      expect((e as ThalamusError).errorCode).toBe("UNKNOWN_AUDIT_ID");
    }
  });

  it("throws ThalamusError on 400 INVALID_AUDIT_ID", async () => {
    setupFetch("post_call_invalid_audit_id");

    const client = new ThalamusClient({ baseUrl: BASE_URL });

    try {
      await client.postCall(postCallReq("post_call_invalid_audit_id"));
      expect.unreachable("should have thrown");
    } catch (e) {
      expect(e).toBeInstanceOf(ThalamusError);
      expect((e as ThalamusError).statusCode).toBe(400);
      expect((e as ThalamusError).errorCode).toBe("INVALID_AUDIT_ID");
    }
  });
});

// ---- /v1/call (full call) --------------------------------------------------

describe("call", () => {
  it("Allow with backend_content", async () => {
    setupFetch("full_call_allow");

    const client = new ThalamusClient({ baseUrl: BASE_URL });
    const resp = await client.call(decideReq("full_call_allow"));

    expect(resp.decision).toBe("Allow");
    expect(resp.backend_content).toBeDefined();
    expect(resp.backend_content).toContain("BTC");
    expect(resp.post_call.status).toBe("Valid");
    expect(resp.post_call.audit_id).toBe("f1e2d3c4-b5a6-7890-abcd-ef1234567890");
  });

  it("Deny with no backend_content", async () => {
    setupFetch("full_call_deny");

    const client = new ThalamusClient({ baseUrl: BASE_URL });
    const resp = await client.call(decideReq("full_call_deny"));

    expect(resp.decision).toContain("Deny");
    expect(resp.backend_content).toBeNull();
    expect(resp.post_call.status).toBe("Denied");
  });

  it("AllowWithReview with no backend_content", async () => {
    setupFetch("full_call_allow_with_review");

    const client = new ThalamusClient({ baseUrl: BASE_URL });
    const resp = await client.call(decideReq("full_call_allow_with_review"));

    expect(resp.decision).toContain("AllowWithReview");
    expect(resp.backend_content).toBeNull();
    expect(resp.post_call.status).toBe("NeedsHumanReview");
  });

  it("throws ThalamusError on 422 NO_PERMITTED_BACKENDS", async () => {
    setupFetch("full_call_no_permitted_backends");

    const client = new ThalamusClient({ baseUrl: BASE_URL });

    try {
      await client.call(decideReq("full_call_no_permitted_backends"));
      expect.unreachable("should have thrown");
    } catch (e) {
      expect(e).toBeInstanceOf(ThalamusError);
      expect((e as ThalamusError).statusCode).toBe(422);
      expect((e as ThalamusError).errorCode).toBe("NO_PERMITTED_BACKENDS");
    }
  });
});

// ---- /v1/audit/{id} --------------------------------------------------------

describe("getAudit", () => {
  it("returns events with trace_id", async () => {
    setupFetch("audit_found");

    const client = new ThalamusClient({ baseUrl: BASE_URL });
    const resp = await client.getAudit("f1e2d3c4-b5a6-7890-abcd-ef1234567890");

    expect(resp.audit_id).toBe("f1e2d3c4-b5a6-7890-abcd-ef1234567890");
    expect(resp.events).toHaveLength(2);
    expect(resp.events[0].kind).toBe("PreCallDecision");
    expect(resp.events[0].trace_id).toBe("a1b2c3d4-e5f6-7890-abcd-ef1234567890");
    expect(resp.events[1].kind).toBe("PostCallOutcome");
  });

  it("throws ThalamusError on 400 INVALID_AUDIT_ID", async () => {
    setupFetch("audit_invalid_id");

    const client = new ThalamusClient({ baseUrl: BASE_URL });

    try {
      await client.getAudit("not-a-uuid");
      expect.unreachable("should have thrown");
    } catch (e) {
      expect(e).toBeInstanceOf(ThalamusError);
      expect((e as ThalamusError).statusCode).toBe(400);
      expect((e as ThalamusError).errorCode).toBe("INVALID_AUDIT_ID");
    }
  });
});

// ---- Configuration: base URL swap ------------------------------------------

describe("configuration", () => {
  it("swapping baseUrl is the only change needed to point at a different environment", async () => {
    const f = entry("decide_allow");

    // Server A
    vi.stubGlobal("fetch", vi.fn().mockResolvedValue(mockResponse(f.response.status, f.response.body)));
    const clientA = new ThalamusClient({ baseUrl: "http://server-a:8080" });
    const respA = await clientA.decide(decideReq("decide_allow"));

    // Server B -- same SDK, different URL
    vi.stubGlobal("fetch", vi.fn().mockResolvedValue(mockResponse(f.response.status, f.response.body)));
    const clientB = new ThalamusClient({ baseUrl: "http://server-b:9090" });
    const respB = await clientB.decide(decideReq("decide_allow"));

    expect(respA.decision).toBe(respB.decision);
    expect(respA.policy_id).toBe(respB.policy_id);
  });
});
