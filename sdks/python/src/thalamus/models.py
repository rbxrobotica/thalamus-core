"""Pydantic v2 models mirroring the Thalamus wire contract (routes.rs)."""

from __future__ import annotations

from typing import Any

from pydantic import BaseModel, Field


# ---------------------------------------------------------------------------
# Nested / shared types
# ---------------------------------------------------------------------------


class BackendHandle(BaseModel):
    """Requested backend selector."""

    id: str
    backend_type: str


class BudgetHint(BaseModel):
    """Optional budget constraints."""

    max_tokens: int | None = None
    max_latency_ms: int | None = None


class Envelope(BaseModel):
    """Pre-call envelope returned on Allow decisions."""

    trace_id: str
    audit_id: str
    backend_handle_id: str
    prompt: str
    policy_ref: str
    budget_max_tokens: int
    budget_max_latency_ms: int


class AuditEvent(BaseModel):
    """Single event in an audit trail."""

    kind: str
    trace_id: str
    timestamp: str
    details: dict[str, Any]


# ---------------------------------------------------------------------------
# Request types
# ---------------------------------------------------------------------------


class DecideRequest(BaseModel):
    """Request body for /v1/decide, /v1/pre-call, /v1/call."""

    tenant: str
    product: str
    user: str
    workflow: str
    intent: str
    prompt: str
    requested_backend: BackendHandle | None = None
    budget_hint: BudgetHint | None = None


class PostCallRequest(BaseModel):
    """Request body for /v1/post-call."""

    audit_id: str
    content: str
    tokens_used: int | None = None
    latency_ms: int | None = None


# ---------------------------------------------------------------------------
# Response types
# ---------------------------------------------------------------------------


class DecideResponse(BaseModel):
    """Response from /v1/decide."""

    decision: str
    policy_id: str
    reason: str | None = None
    review_reason: str | None = None
    policy_ref: str | None = None


class PreCallResponse(BaseModel):
    """Response from /v1/pre-call."""

    decision: str
    trace_id: str
    audit_id: str
    policy_id: str
    envelope: Envelope | None = None
    review_reason: str | None = None
    policy_ref: str | None = None


class PostCallResponse(BaseModel):
    """Response from /v1/post-call."""

    status: str
    risk_class: str
    executable_by_agent: bool
    schema_valid: bool
    audit_id: str


class FullCallResponse(BaseModel):
    """Response from /v1/call."""

    decision: str
    post_call: PostCallResponse
    backend_content: str | None = None


class AuditResponse(BaseModel):
    """Response from /v1/audit/{id}."""

    audit_id: str
    events: list[AuditEvent]


class ErrorResponse(BaseModel):
    """Error body returned on 4xx/5xx."""

    error: str
    code: str
