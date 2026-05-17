/**
 * Thalamus SDK — Wire-contract types.
 * Mirrors routes.rs JSON shapes exactly. No invented fields.
 */

// ---------------------------------------------------------------------------
// Request types
// ---------------------------------------------------------------------------

export interface BackendHandle {
  id: string;
  backend_type: string;
}

export interface BudgetHint {
  max_tokens?: number;
  max_latency_ms?: number;
}

export interface DecideRequest {
  tenant: string;
  product: string;
  user: string;
  workflow: string;
  intent: string;
  prompt: string;
  requested_backend?: BackendHandle;
  budget_hint?: BudgetHint;
}

export interface PostCallRequest {
  audit_id: string;
  content: string;
  tokens_used?: number;
  latency_ms?: number;
}

// ---------------------------------------------------------------------------
// Response types
// ---------------------------------------------------------------------------

export interface DecideResponse {
  decision: string;
  policy_id: string;
  reason?: string | null;
  review_reason?: string | null;
  policy_ref?: string | null;
}

export interface Envelope {
  trace_id: string;
  audit_id: string;
  backend_handle_id: string;
  prompt: string;
  policy_ref: string;
  budget_max_tokens: number;
  budget_max_latency_ms: number;
}

export interface PreCallResponse {
  decision: string;
  trace_id: string;
  audit_id: string;
  policy_id: string;
  envelope?: Envelope;
  review_reason?: string | null;
  policy_ref?: string | null;
}

export interface PostCallResponse {
  status: string;
  risk_class: string;
  executable_by_agent: boolean;
  schema_valid: boolean;
  audit_id: string;
}

export interface FullCallResponse {
  decision: string;
  post_call: PostCallResponse;
  backend_content?: string | null;
}

export interface AuditEvent {
  kind: string;
  trace_id: string;
  timestamp: string;
  details: Record<string, unknown>;
}

export interface AuditResponse {
  audit_id: string;
  events: AuditEvent[];
}

export interface ErrorResponse {
  error: string;
  code: string;
}
