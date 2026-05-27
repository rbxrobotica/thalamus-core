"""Synchronous Thalamus SDK client (httpx-based)."""

from __future__ import annotations

from typing import Any

import httpx

from .models import (
    AuditResponse,
    DecideRequest,
    DecideResponse,
    FullCallResponse,
    PostCallRequest,
    PostCallResponse,
    PreCallResponse,
)


class ThalamusError(Exception):
    """Raised when the Thalamus server returns a non-2xx response."""

    def __init__(self, status_code: int, error: str, code: str) -> None:
        self.status_code = status_code
        self.error = error
        self.code = code
        super().__init__(f"[{status_code}] {code}: {error}")


class ThalamusClient:
    """Thin sync client for the Thalamus semantic control layer.

    Usage::

        with ThalamusClient("http://localhost:8080") as c:
            resp = c.decide(DecideRequest(...))
    """

    def __init__(
        self,
        base_url: str,
        auth_header: str | None = None,
        timeout: float = 30.0,
    ) -> None:
        self._base_url = base_url.rstrip("/")
        self._auth_header = auth_header
        self._timeout = timeout
        self._client: httpx.Client | None = None

    # -- context manager ----------------------------------------------------

    def __enter__(self) -> ThalamusClient:
        return self

    def __exit__(self, *exc: Any) -> None:
        self.close()

    def close(self) -> None:
        if self._client is not None:
            self._client.close()
            self._client = None

    # -- internal helpers ---------------------------------------------------

    def _get_client(self) -> httpx.Client:
        if self._client is None:
            self._client = httpx.Client(timeout=self._timeout)
        return self._client

    def _headers(self) -> dict[str, str]:
        h: dict[str, str] = {"Content-Type": "application/json"}
        if self._auth_header is not None:
            h["Authorization"] = self._auth_header
        return h

    def _request(
        self,
        method: str,
        path: str,
        json_body: dict[str, Any] | None = None,
    ) -> httpx.Response:
        url = f"{self._base_url}{path}"
        return self._get_client().request(
            method,
            url,
            json=json_body,
            headers=self._headers(),
        )

    @staticmethod
    def _raise_on_error(resp: httpx.Response) -> None:
        if resp.status_code < 400:
            return
        body = resp.json()
        raise ThalamusError(
            status_code=resp.status_code,
            error=body.get("error", ""),
            code=body.get("code", ""),
        )

    # -- public API ---------------------------------------------------------

    def decide(self, req: DecideRequest) -> DecideResponse:
        resp = self._request("POST", "/v1/decide", req.model_dump(exclude_none=True))
        self._raise_on_error(resp)
        return DecideResponse.model_validate(resp.json())

    def pre_call(self, req: DecideRequest) -> PreCallResponse:
        resp = self._request("POST", "/v1/pre-call", req.model_dump(exclude_none=True))
        self._raise_on_error(resp)
        return PreCallResponse.model_validate(resp.json())

    def call(self, req: DecideRequest) -> FullCallResponse:
        resp = self._request("POST", "/v1/call", req.model_dump(exclude_none=True))
        self._raise_on_error(resp)
        return FullCallResponse.model_validate(resp.json())

    def post_call(self, req: PostCallRequest) -> PostCallResponse:
        resp = self._request("POST", "/v1/post-call", req.model_dump(exclude_none=True))
        self._raise_on_error(resp)
        return PostCallResponse.model_validate(resp.json())

    def get_audit(self, audit_id: str) -> AuditResponse:
        resp = self._request("GET", f"/v1/audit/{audit_id}")
        self._raise_on_error(resp)
        return AuditResponse.model_validate(resp.json())
