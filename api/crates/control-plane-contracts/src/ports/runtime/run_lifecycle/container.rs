use super::*;

/// Adapter input for the single atomic container state owner. The opaque state
/// belongs to orchestration-runtime; the adapter only stores its current value.
#[derive(Debug, Clone)]
pub struct CommitContainerTransitionInput {
    pub flow_run_id: Uuid,
    pub resume_claim_owner: Option<RenewResumeClaimInput>,
    pub execution_identity: domain::NodeExecutionIdentity,
    pub checkpoint: serde_json::Value,
    pub admission: ContainerCommitAdmission,
    pub node: Option<ContainerNodeFact>,
    pub rounds: Vec<ContainerRoundFact>,
    pub control_failure: Option<ContainerControlFailureFact>,
    pub observation: Option<ContainerObservationInput>,
}

/// Business owner selects the deadline for this transition, not every round
/// carried in its recovery projection. Root work outside a round has no deadline.
#[derive(Debug, Clone, Copy)]
pub enum ContainerCommitAdmission {
    AdmitWork {
        effective_deadline: Option<ContainerCommitDeadline>,
    },
    ClosureOrObservation,
}

#[derive(Debug, Clone, Copy)]
pub struct ContainerCommitDeadline {
    pub deadline_unix_ms: i64,
    pub owner_execution_id: Uuid,
}

#[derive(Debug, Clone)]
pub enum ContainerNodeFact {
    Started {
        node_type: String,
        node_alias: String,
        input_payload: serde_json::Value,
    },
    Updated {
        status: domain::NodeRunStatus,
        output_payload: serde_json::Value,
        error_payload: Option<serde_json::Value>,
        metrics_payload: serde_json::Value,
        debug_payload: serde_json::Value,
        terminal: bool,
    },
}

#[derive(Debug, Clone)]
pub struct ContainerRoundFact {
    pub execution_id: Uuid,
    pub container_execution_id: Uuid,
    pub round_index: u64,
    pub deadline_unix_ms: Option<i64>,
    pub status: String,
    pub error_payload: Option<serde_json::Value>,
    pub output_payload: Option<serde_json::Value>,
    pub variables: Option<serde_json::Value>,
}

#[derive(Debug, Clone)]
pub struct ContainerControlFailureFact {
    pub after_round_index: u64,
    pub error_payload: serde_json::Value,
}

#[derive(Debug, Clone)]
pub struct ContainerObservationInput {
    pub observation_key: String,
    pub trace_payload: serde_json::Value,
}
#[derive(Debug, Clone)]
pub struct PendingContainerObservation {
    pub id: Uuid,
    pub flow_run_id: Uuid,
    pub node_run_id: Uuid,
    pub started_at: OffsetDateTime,
    pub trace_payload: serde_json::Value,
}

/// Only the existing node-observation writes needed by a scoped trace. The
/// receipt and deterministic record IDs make interrupted projections resumable.
#[derive(Debug, Clone)]
pub enum ContainerObservationOperation {
    Span(AppendRuntimeSpanInput),
    ContextProjection(AppendContextProjectionInput),
    Attempt(AppendModelFailoverAttemptLedgerInput),
    Usage(AppendUsageLedgerInput),
    RunEvent(AppendRunEventInput),
    RuntimeEvent(AppendRuntimeEventInput),
    Item(AppendRuntimeItemInput),
    Capability(AppendCapabilityInvocationInput),
}
#[derive(Debug, Clone)]
pub enum ContainerObservationResult {
    Span(domain::RuntimeSpanRecord),
    ContextProjection(domain::ContextProjectionRecord),
    Attempt(domain::ModelFailoverAttemptLedgerRecord),
    Usage(domain::UsageLedgerRecord),
    RunEvent(domain::RunEventRecord),
    RuntimeEvent(domain::RuntimeEventRecord),
    Item(domain::RuntimeItemRecord),
    Capability(domain::CapabilityInvocationRecord),
}
