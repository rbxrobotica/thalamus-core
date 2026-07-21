//! Idempotency fingerprints for governance records
//! (`POST /rbx/v1/tool-decisions`, `POST /rbx/v1/approvals`).
//!
//! A `request_fingerprint` pins down which fields define a governance
//! record's identity for idempotent-replay comparison. `tenant` and
//! `source_system` are always server-derived (the owning session's tenant,
//! the verified caller's `client_app_id`) and never taken from the request
//! body. `metadata` (free-form) and an approval's `reason` (free text) are
//! deliberately excluded: neither changes the fact that was decided.
//! `schema_version` is explicit so a future field change is a new,
//! non-colliding fingerprint shape rather than a silent redefinition.

use serde::Serialize;
use sha2::{Digest, Sha256};
use uuid::Uuid;

pub const TOOL_DECISION_FINGERPRINT_SCHEMA: &str = "rbx.tool_decision_fingerprint.v1";
pub const APPROVAL_FINGERPRINT_SCHEMA: &str = "rbx.approval_fingerprint.v1";

/// Identity of a `POST /rbx/v1/tool-decisions` request for idempotent replay.
/// Field order is declaration order, which `serde_json` preserves for
/// structs, so the serialization (and therefore the hash) is deterministic.
#[derive(Serialize)]
pub struct ToolDecisionFingerprint<'a> {
    pub schema_version: &'static str,
    pub tenant: &'a str,
    pub source_system: &'a str,
    pub session_id: Uuid,
    pub run_id: Option<Uuid>,
    pub tool: &'a str,
    pub decision: &'a str,
}

impl<'a> ToolDecisionFingerprint<'a> {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        tenant: &'a str,
        source_system: &'a str,
        session_id: Uuid,
        run_id: Option<Uuid>,
        tool: &'a str,
        decision: &'a str,
    ) -> Self {
        Self {
            schema_version: TOOL_DECISION_FINGERPRINT_SCHEMA,
            tenant,
            source_system,
            session_id,
            run_id,
            tool,
            decision,
        }
    }

    pub fn hash_hex(&self) -> String {
        hash_hex(self)
    }
}

/// Identity of a `POST /rbx/v1/approvals` request for idempotent replay.
/// `session_id` is not optional here: idempotency for approvals requires a
/// session (decision recorded upstream — a bare `idempotency_key` without a
/// `session_id` is refused before a fingerprint is ever built).
#[derive(Serialize)]
pub struct ApprovalFingerprint<'a> {
    pub schema_version: &'static str,
    pub tenant: &'a str,
    pub source_system: &'a str,
    pub session_id: Uuid,
    pub run_id: Option<Uuid>,
    pub subject: &'a str,
    pub decision: &'a str,
}

impl<'a> ApprovalFingerprint<'a> {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        tenant: &'a str,
        source_system: &'a str,
        session_id: Uuid,
        run_id: Option<Uuid>,
        subject: &'a str,
        decision: &'a str,
    ) -> Self {
        Self {
            schema_version: APPROVAL_FINGERPRINT_SCHEMA,
            tenant,
            source_system,
            session_id,
            run_id,
            subject,
            decision,
        }
    }

    pub fn hash_hex(&self) -> String {
        hash_hex(self)
    }
}

/// Outcome of an idempotent-write attempt. `Created` is a fresh insert — the
/// caller should emit its usual side effects (lifecycle audit event, etc.).
/// `Replayed` is a retry that matched an existing row's fingerprint exactly —
/// no new side effects, since nothing new happened.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RecordOutcome {
    Created(Uuid),
    Replayed(Uuid),
}

impl RecordOutcome {
    pub fn id(self) -> Uuid {
        match self {
            Self::Created(id) | Self::Replayed(id) => id,
        }
    }

    pub fn is_new(self) -> bool {
        matches!(self, Self::Created(_))
    }
}

fn hash_hex(value: &impl Serialize) -> String {
    let json = serde_json::to_vec(value).expect("fingerprint structs always serialize");
    hex::encode(Sha256::digest(json))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tool_decision_fingerprint_is_deterministic() {
        let session_id = Uuid::new_v4();
        let a =
            ToolDecisionFingerprint::new("rbx", "robson-code", session_id, None, "shell", "denied")
                .hash_hex();
        let b =
            ToolDecisionFingerprint::new("rbx", "robson-code", session_id, None, "shell", "denied")
                .hash_hex();
        assert_eq!(a, b);
    }

    #[test]
    fn tool_decision_fingerprint_changes_with_decision() {
        let session_id = Uuid::new_v4();
        let allowed = ToolDecisionFingerprint::new(
            "rbx",
            "robson-code",
            session_id,
            None,
            "shell",
            "allowed",
        )
        .hash_hex();
        let denied =
            ToolDecisionFingerprint::new("rbx", "robson-code", session_id, None, "shell", "denied")
                .hash_hex();
        assert_ne!(allowed, denied);
    }

    #[test]
    fn approval_fingerprint_changes_with_tenant_or_source_system() {
        let session_id = Uuid::new_v4();
        let f1 = ApprovalFingerprint::new(
            "tenant-a",
            "robson-code",
            session_id,
            None,
            "patch:abc",
            "approved",
        )
        .hash_hex();
        let f2 = ApprovalFingerprint::new(
            "tenant-b",
            "robson-code",
            session_id,
            None,
            "patch:abc",
            "approved",
        )
        .hash_hex();
        let f3 = ApprovalFingerprint::new(
            "tenant-a",
            "other-app",
            session_id,
            None,
            "patch:abc",
            "approved",
        )
        .hash_hex();
        assert_ne!(f1, f2);
        assert_ne!(f1, f3);
    }
}
