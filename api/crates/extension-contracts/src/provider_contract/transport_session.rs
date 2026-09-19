use super::*;

pub const PROVIDER_TRANSPORT_SESSION_CONTEXT_KEY: &str = "physical_transport_session";
pub const PROVIDER_TRANSPORT_SESSION_RECEIPT_METADATA_KEY: &str =
    "1flowbase_physical_transport_session";
pub const PROVIDER_INVOCATION_TIMING_RECEIPT_METADATA_KEY: &str =
    "1flowbase_provider_invocation_timing";

const MAX_OPAQUE_ID_BYTES: usize = 256;
const MAX_CONNECTION_LIFETIME_MS: u64 = 24 * 60 * 60 * 1_000;
pub const PROVIDER_INVOCATION_TIMING_SCHEMA_VERSION: u8 = 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProviderLogicalSessionState {
    Active,
    Waiting,
    Idle,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProviderTransportSessionDirective {
    pub logical_session_id: String,
    pub generation: u64,
    pub task_id: String,
    pub state: ProviderLogicalSessionState,
    pub physical_deadline_unix_ms: i64,
}

impl ProviderTransportSessionDirective {
    pub fn validate(&self) -> Result<(), String> {
        validate_opaque_id("logical_session_id", &self.logical_session_id)?;
        if self.generation == 0 {
            return Err("transport session directive generation must be positive".into());
        }
        validate_opaque_id("task_id", &self.task_id)?;
        if self.physical_deadline_unix_ms <= 0 {
            return Err("transport session physical_deadline_unix_ms must be positive".into());
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProviderTransportSessionAction {
    Drain,
    Close,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProviderTransportSessionCommand {
    pub logical_session_id: String,
    pub generation: u64,
    pub action: ProviderTransportSessionAction,
    pub deadline_unix_ms: i64,
}

impl ProviderTransportSessionCommand {
    pub fn validate(&self) -> Result<(), String> {
        validate_opaque_id("logical_session_id", &self.logical_session_id)?;
        if self.generation == 0 {
            return Err("transport session generation must be positive".into());
        }
        if self.deadline_unix_ms <= 0 {
            return Err("transport session deadline_unix_ms must be positive".into());
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProviderPhysicalTransportState {
    Ready,
    Draining,
    Closing,
    Closed,
    Faulted,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProviderTransportSessionCloseReason {
    RequestedDrain,
    RequestedClose,
    TtlExpired,
    CapacityRejected,
    TransportFault,
    InvocationDeadline,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProviderTransportSessionReceipt {
    pub generation: u64,
    pub reused: bool,
    pub physical_state: ProviderPhysicalTransportState,
    pub connection_age_ms: u64,
    pub ttl_remaining_ms: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub close_reason: Option<ProviderTransportSessionCloseReason>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub close_acknowledged: Option<bool>,
}

/// Host-side classification of a provider invocation's physical transport facts.
///
/// HTTP fallback is deliberately a recovery outcome, not a synthetic WebSocket
/// receipt. This keeps connection ACK and recovery semantics in separate domains.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum ProviderInvocationTransportOutcome {
    Ready {
        receipt: ProviderTransportSessionReceipt,
    },
    ReceiptMissing,
    StaleGeneration {
        expected_generation: u64,
        received_generation: u64,
    },
    PhysicalConnectionFault {
        generation: u64,
        state: ProviderPhysicalTransportState,
    },
    HttpFallback {
        recovery: ProviderRecoveryReceipt,
    },
}

/// Execution-mode classification. Session outcomes retain the existing receipt
/// contract; direct HTTP has no physical WebSocket session to acknowledge.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProviderInvocationTransportClassification {
    Http,
    Session(ProviderInvocationTransportOutcome),
}

/// Typed result of a drain/close lifecycle command.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum ProviderLifecycleAckOutcome {
    Acknowledged,
    AcknowledgementMissing,
    StaleGeneration {
        expected_generation: u64,
        received_generation: u64,
    },
    PhysicalConnectionFault {
        generation: u64,
        state: ProviderPhysicalTransportState,
    },
}

impl ProviderTransportSessionReceipt {
    pub fn validate(&self) -> Result<(), String> {
        if self.generation == 0 {
            return Err("transport session receipt generation must be positive".into());
        }
        if self.connection_age_ms > MAX_CONNECTION_LIFETIME_MS
            || self.ttl_remaining_ms > MAX_CONNECTION_LIFETIME_MS
        {
            return Err("transport session receipt lifetime exceeds the bounded contract".into());
        }
        if self.close_acknowledged.is_some() && self.close_reason.is_none() {
            return Err("transport session close ACK requires a close reason".into());
        }
        Ok(())
    }

    pub fn lifecycle_ack_outcome(
        &self,
        expected_generation: u64,
    ) -> Result<ProviderLifecycleAckOutcome, String> {
        self.validate()?;
        if expected_generation == 0 {
            return Err("expected transport generation must be positive".to_string());
        }
        if self.generation != expected_generation {
            return Ok(ProviderLifecycleAckOutcome::StaleGeneration {
                expected_generation,
                received_generation: self.generation,
            });
        }
        if self.physical_state == ProviderPhysicalTransportState::Faulted {
            return Ok(ProviderLifecycleAckOutcome::PhysicalConnectionFault {
                generation: self.generation,
                state: self.physical_state,
            });
        }
        Ok(if self.close_acknowledged == Some(true) {
            ProviderLifecycleAckOutcome::Acknowledged
        } else {
            ProviderLifecycleAckOutcome::AcknowledgementMissing
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProviderInvocationTerminationKind {
    Completed,
    UpstreamError,
    TransportError,
    Deadline,
}

/// Provider-owned timing facts. Durations use the provider process monotonic
/// clock and are deliberately independent rather than wall-clock timestamps.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProviderInvocationTimingReceipt {
    pub schema_version: u8,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub connect_ms: Option<u64>,
    pub upstream_ms: u64,
    pub termination_kind: ProviderInvocationTerminationKind,
}

impl ProviderInvocationTimingReceipt {
    pub fn validate(&self) -> Result<(), String> {
        if self.schema_version != PROVIDER_INVOCATION_TIMING_SCHEMA_VERSION {
            return Err("provider invocation timing schema version is unsupported".into());
        }
        if self
            .connect_ms
            .is_some_and(|value| value > MAX_CONNECTION_LIFETIME_MS)
            || self.upstream_ms > MAX_CONNECTION_LIFETIME_MS
        {
            return Err("provider invocation timing exceeds the bounded contract".into());
        }
        Ok(())
    }
}

fn validate_opaque_id(field: &str, value: &str) -> Result<(), String> {
    if value.is_empty() || value.len() > MAX_OPAQUE_ID_BYTES {
        return Err(format!(
            "transport session {field} must contain 1 through {MAX_OPAQUE_ID_BYTES} bytes"
        ));
    }
    if !value
        .bytes()
        .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.' | b':'))
    {
        return Err(format!(
            "transport session {field} must be an opaque URL-safe identifier"
        ));
    }
    Ok(())
}
