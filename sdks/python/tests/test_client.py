"""SDK contract tests -- every test is driven by contract-fixture.json, zero network."""

from __future__ import annotations

import pytest

from thalamus import (
    DecideRequest,
    ThalamusClient,
    ThalamusError,
)

BASE_URL = "http://thalamus.test"


# ---------------------------------------------------------------------------
# Helpers
# ---------------------------------------------------------------------------


def _register(fixture_data: dict, name: str, respx_mock):
    """Register a mock route from the contract fixture and return the body."""
    entry = fixture_data[name]
    endpoint: str = entry["endpoint"]  # e.g. "POST /v1/decide"
    method, path = endpoint.split(" ", 1)
    status: int = entry["response"]["status"]
    body: dict = entry["response"]["body"]

    route = respx_mock.request(method, f"{BASE_URL}{path}")
    route.respond(status_code=status, json=body)
    return entry


def _decide_req(fixture_data: dict, name: str) -> DecideRequest:
    """Build a DecideRequest from the fixture's request block."""
    r = fixture_data[name]["request"]
    return DecideRequest.model_validate(r)


# ---------------------------------------------------------------------------
# /v1/decide
# ---------------------------------------------------------------------------


class TestDecide:
    def test_decide_allow(self, fixture_data, respx_mock):
        _register(fixture_data, "decide_allow", respx_mock)
        with ThalamusClient(BASE_URL) as c:
            resp = c.decide(_decide_req(fixture_data, "decide_allow"))
        assert resp.decision == "Allow"
        assert resp.policy_id == "rbx-robson-default"
        assert resp.reason is None

    def test_decide_deny(self, fixture_data, respx_mock):
        _register(fixture_data, "decide_deny", respx_mock)
        with ThalamusClient(BASE_URL) as c:
            resp = c.decide(_decide_req(fixture_data, "decide_deny"))
        assert resp.decision == "Deny"
        assert resp.reason == "budget exceeded"
        assert resp.policy_ref == "rbx-robson-budget-v2"

    def test_decide_allow_with_review(self, fixture_data, respx_mock):
        _register(fixture_data, "decide_allow_with_review", respx_mock)
        with ThalamusClient(BASE_URL) as c:
            resp = c.decide(_decide_req(fixture_data, "decide_allow_with_review"))
        assert resp.decision == "AllowWithReview"
        assert resp.review_reason is not None
        assert "human review" in resp.review_reason
        assert resp.policy_ref == "rbx-strategos-review-v1"


# ---------------------------------------------------------------------------
# /v1/pre-call
# ---------------------------------------------------------------------------


class TestPreCall:
    def test_pre_call_allow(self, fixture_data, respx_mock):
        _register(fixture_data, "pre_call_allow", respx_mock)
        with ThalamusClient(BASE_URL) as c:
            resp = c.pre_call(_decide_req(fixture_data, "pre_call_allow"))
        assert resp.decision == "Allow"
        assert resp.trace_id == "a1b2c3d4-e5f6-7890-abcd-ef1234567890"
        assert resp.audit_id == "f1e2d3c4-b5a6-7890-abcd-ef1234567890"
        assert resp.envelope is not None
        assert resp.envelope.backend_handle_id == "gpt-4o"
        assert resp.envelope.budget_max_tokens == 4096

    def test_pre_call_no_permitted_backends(self, fixture_data, respx_mock):
        _register(fixture_data, "pre_call_no_permitted_backends", respx_mock)
        with ThalamusClient(BASE_URL) as c:
            with pytest.raises(ThalamusError) as exc_info:
                c.pre_call(_decide_req(fixture_data, "pre_call_no_permitted_backends"))
        assert exc_info.value.status_code == 422
        assert exc_info.value.code == "NO_PERMITTED_BACKENDS"


# ---------------------------------------------------------------------------
# /v1/post-call
# ---------------------------------------------------------------------------


class TestPostCall:
    def test_post_call_valid(self, fixture_data, respx_mock):
        from thalamus import PostCallRequest

        _register(fixture_data, "post_call_valid", respx_mock)
        req = PostCallRequest.model_validate(fixture_data["post_call_valid"]["request"])
        with ThalamusClient(BASE_URL) as c:
            resp = c.post_call(req)
        assert resp.status == "Valid"
        assert resp.audit_id == "f1e2d3c4-b5a6-7890-abcd-ef1234567890"
        assert resp.executable_by_agent is True

    def test_post_call_unknown_audit(self, fixture_data, respx_mock):
        from thalamus import PostCallRequest

        _register(fixture_data, "post_call_unknown_audit", respx_mock)
        req = PostCallRequest.model_validate(fixture_data["post_call_unknown_audit"]["request"])
        with ThalamusClient(BASE_URL) as c:
            with pytest.raises(ThalamusError) as exc_info:
                c.post_call(req)
        assert exc_info.value.status_code == 404
        assert exc_info.value.code == "UNKNOWN_AUDIT_ID"

    def test_post_call_invalid_audit_id(self, fixture_data, respx_mock):
        from thalamus import PostCallRequest

        _register(fixture_data, "post_call_invalid_audit_id", respx_mock)
        req = PostCallRequest.model_validate(fixture_data["post_call_invalid_audit_id"]["request"])
        with ThalamusClient(BASE_URL) as c:
            with pytest.raises(ThalamusError) as exc_info:
                c.post_call(req)
        assert exc_info.value.status_code == 400
        assert exc_info.value.code == "INVALID_AUDIT_ID"


# ---------------------------------------------------------------------------
# /v1/call (full call)
# ---------------------------------------------------------------------------


class TestFullCall:
    def test_full_call_allow(self, fixture_data, respx_mock):
        _register(fixture_data, "full_call_allow", respx_mock)
        with ThalamusClient(BASE_URL) as c:
            resp = c.call(_decide_req(fixture_data, "full_call_allow"))
        assert resp.decision == "Allow"
        assert resp.backend_content is not None
        assert "BTC" in resp.backend_content
        assert resp.post_call.status == "Valid"

    def test_full_call_deny(self, fixture_data, respx_mock):
        _register(fixture_data, "full_call_deny", respx_mock)
        with ThalamusClient(BASE_URL) as c:
            resp = c.call(_decide_req(fixture_data, "full_call_deny"))
        assert "Deny" in resp.decision
        assert resp.backend_content is None
        assert resp.post_call.status == "Denied"

    def test_full_call_allow_with_review(self, fixture_data, respx_mock):
        _register(fixture_data, "full_call_allow_with_review", respx_mock)
        with ThalamusClient(BASE_URL) as c:
            resp = c.call(_decide_req(fixture_data, "full_call_allow_with_review"))
        assert "AllowWithReview" in resp.decision
        assert resp.backend_content is None
        assert resp.post_call.status == "NeedsHumanReview"

    def test_full_call_no_permitted_backends(self, fixture_data, respx_mock):
        _register(fixture_data, "full_call_no_permitted_backends", respx_mock)
        with ThalamusClient(BASE_URL) as c:
            with pytest.raises(ThalamusError) as exc_info:
                c.call(_decide_req(fixture_data, "full_call_no_permitted_backends"))
        assert exc_info.value.status_code == 422
        assert exc_info.value.code == "NO_PERMITTED_BACKENDS"


# ---------------------------------------------------------------------------
# /v1/audit/{id}
# ---------------------------------------------------------------------------


class TestAudit:
    def test_audit_found(self, fixture_data, respx_mock):
        entry = fixture_data["audit_found"]
        audit_id = entry["request"]["audit_id"]
        respx_mock.get(f"{BASE_URL}/v1/audit/{audit_id}").respond(
            status_code=entry["response"]["status"],
            json=entry["response"]["body"],
        )
        with ThalamusClient(BASE_URL) as c:
            resp = c.get_audit(audit_id)
        assert resp.audit_id == "f1e2d3c4-b5a6-7890-abcd-ef1234567890"
        assert len(resp.events) == 2
        assert resp.events[0].kind == "PreCallDecision"
        assert resp.events[0].trace_id == "a1b2c3d4-e5f6-7890-abcd-ef1234567890"
        assert resp.events[1].kind == "PostCallOutcome"

    def test_audit_invalid_id(self, fixture_data, respx_mock):
        entry = fixture_data["audit_invalid_id"]
        audit_id = entry["request"]["audit_id"]
        respx_mock.get(f"{BASE_URL}/v1/audit/{audit_id}").respond(
            status_code=entry["response"]["status"],
            json=entry["response"]["body"],
        )
        with ThalamusClient(BASE_URL) as c:
            with pytest.raises(ThalamusError) as exc_info:
                c.get_audit(audit_id)
        assert exc_info.value.status_code == 400
        assert exc_info.value.code == "INVALID_AUDIT_ID"


# ---------------------------------------------------------------------------
# Configuration
# ---------------------------------------------------------------------------


class TestConfiguration:
    def test_base_url_swap(self, fixture_data, respx_mock):
        """Swapping base_url is the only config change needed to point at a
        different environment."""
        other_url = "http://thalamus-staging.test"
        entry = fixture_data["decide_allow"]
        body = entry["response"]["body"]
        respx_mock.post(f"{other_url}/v1/decide").respond(
            status_code=200, json=body,
        )
        with ThalamusClient(other_url) as c:
            resp = c.decide(_decide_req(fixture_data, "decide_allow"))
        assert resp.decision == "Allow"
