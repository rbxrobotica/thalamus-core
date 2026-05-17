import type {
  AuditResponse,
  DecideRequest,
  DecideResponse,
  ErrorResponse,
  FullCallResponse,
  PostCallRequest,
  PostCallResponse,
  PreCallResponse,
} from "./types.js";

// ---------------------------------------------------------------------------
// Error
// ---------------------------------------------------------------------------

export class ThalamusError extends Error {
  /** HTTP status code returned by the server. */
  public readonly statusCode: number;
  /** Machine-readable error code from the response body. */
  public readonly errorCode: string;
  /** Human-readable error message from the response body. */
  public readonly errorMessage: string;

  constructor(statusCode: number, code: string, message: string) {
    super(`Thalamus ${statusCode}: ${code} — ${message}`);
    this.name = "ThalamusError";
    this.statusCode = statusCode;
    this.errorCode = code;
    this.errorMessage = message;
  }
}

// ---------------------------------------------------------------------------
// Client configuration
// ---------------------------------------------------------------------------

export interface ThalamusClientConfig {
  /** Base URL of the Thalamus server, e.g. "http://localhost:3000". */
  baseUrl: string;
  /** Optional Authorization header value (e.g. "Bearer <token>"). */
  authHeader?: string;
  /** Request timeout in milliseconds. Defaults to 30 000. */
  timeout?: number;
}

// ---------------------------------------------------------------------------
// Client
// ---------------------------------------------------------------------------

export class ThalamusClient {
  private readonly baseUrl: string;
  private readonly authHeader: string | undefined;
  private readonly timeout: number;

  constructor(config: ThalamusClientConfig) {
    this.baseUrl = config.baseUrl.replace(/\/+$/, "");
    this.authHeader = config.authHeader;
    this.timeout = config.timeout ?? 30_000;
  }

  // -- Public API ----------------------------------------------------------

  /** Evaluate a policy decision without executing a backend call. */
  public decide(req: DecideRequest): Promise<DecideResponse> {
    return this.post<DecideResponse>("/v1/decide", req);
  }

  /** Pre-call gate: returns a trace + envelope if allowed, or 422 if no backends. */
  public preCall(req: DecideRequest): Promise<PreCallResponse> {
    return this.post<PreCallResponse>("/v1/pre-call", req);
  }

  /** Full call: pre-call + backend execution + post-call in one shot. */
  public call(req: DecideRequest): Promise<FullCallResponse> {
    return this.post<FullCallResponse>("/v1/call", req);
  }

  /** Submit post-call feedback for an existing audit record. */
  public postCall(req: PostCallRequest): Promise<PostCallResponse> {
    return this.post<PostCallResponse>("/v1/post-call", req);
  }

  /** Retrieve the audit trail for a given audit ID. */
  public getAudit(auditId: string): Promise<AuditResponse> {
    return this.get<AuditResponse>(`/v1/audit/${encodeURIComponent(auditId)}`);
  }

  // -- Internal helpers ----------------------------------------------------

  private async post<T>(path: string, body: unknown): Promise<T> {
    const url = `${this.baseUrl}${path}`;
    const controller = new AbortController();
    const timer = setTimeout(() => controller.abort(), this.timeout);

    const headers: Record<string, string> = {
      "Content-Type": "application/json",
    };
    if (this.authHeader) {
      headers["Authorization"] = this.authHeader;
    }

    try {
      const res = await fetch(url, {
        method: "POST",
        headers,
        body: JSON.stringify(body),
        signal: controller.signal,
      });

      return this.handleResponse<T>(res);
    } finally {
      clearTimeout(timer);
    }
  }

  private async get<T>(path: string): Promise<T> {
    const url = `${this.baseUrl}${path}`;
    const controller = new AbortController();
    const timer = setTimeout(() => controller.abort(), this.timeout);

    const headers: Record<string, string> = {};
    if (this.authHeader) {
      headers["Authorization"] = this.authHeader;
    }

    try {
      const res = await fetch(url, {
        method: "GET",
        headers,
        signal: controller.signal,
      });

      return this.handleResponse<T>(res);
    } finally {
      clearTimeout(timer);
    }
  }

  private async handleResponse<T>(res: Response): Promise<T> {
    const payload = await res.json();

    if (res.ok) {
      return payload as T;
    }

    const err = payload as ErrorResponse;
    throw new ThalamusError(
      res.status,
      err.code ?? "UnknownError",
      err.error ?? "Unknown error",
    );
  }
}
