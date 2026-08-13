use serde::{Deserialize, Serialize};
use time::OffsetDateTime;
use uuid::Uuid;

macro_rules! string_enum {
    ($name:ident { $($variant:ident => $value:literal),+ $(,)? }) => {
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
        #[serde(rename_all = "snake_case")]
        pub enum $name {
            $($variant),+
        }

        impl $name {
            pub fn as_str(self) -> &'static str {
                match self {
                    $(Self::$variant => $value),+
                }
            }
        }
    };
}

string_enum!(RuntimeSpanKind {
    Flow => "flow",
    Node => "node",
    LlmTurn => "llm_turn",
    ProviderRequest => "provider_request",
    GatewayForward => "gateway_forward",
    ToolCall => "tool_call",
    McpCall => "mcp_call",
    SkillLoad => "skill_load",
    SkillAction => "skill_action",
    WorkflowTool => "workflow_tool",
    DataRetrieval => "data_retrieval",
    Approval => "approval",
    Compaction => "compaction",
    Subagent => "subagent",
    SystemAgent => "system_agent",
});

string_enum!(RuntimeSpanStatus {
    Running => "running",
    Succeeded => "succeeded",
    Failed => "failed",
    Cancelled => "cancelled",
    Waiting => "waiting",
});

string_enum!(RuntimeEventLayer {
    ProviderRaw => "provider_raw",
    RuntimeItem => "runtime_item",
    Capability => "capability",
    AgentTransition => "agent_transition",
    Ledger => "ledger",
    Diagnostic => "diagnostic",
});

string_enum!(RuntimeEventSource {
    Host => "host",
    ProviderPlugin => "provider_plugin",
    GatewayRelay => "gateway_relay",
    InternalAgent => "internal_agent",
    ExternalAgent => "external_agent",
});

string_enum!(RuntimeTrustLevel {
    HostFact => "host_fact",
    VerifiedBridge => "verified_bridge",
    AgentReported => "agent_reported",
    ExternalOpaque => "external_opaque",
    Inferred => "inferred",
});

string_enum!(RuntimeEventVisibility {
    Internal => "internal",
    Workspace => "workspace",
    User => "user",
    Public => "public",
});

string_enum!(RuntimeEventDurability {
    Ephemeral => "ephemeral",
    Durable => "durable",
    Sampled => "sampled",
});

string_enum!(RuntimeItemKind {
    Message => "message",
    Reasoning => "reasoning",
    ToolCall => "tool_call",
    ToolResult => "tool_result",
    McpCall => "mcp_call",
    SkillLoad => "skill_load",
    SkillAction => "skill_action",
    Approval => "approval",
    Handoff => "handoff",
    AgentAsTool => "agent_as_tool",
    Compaction => "compaction",
    GatewayForward => "gateway_forward",
});

string_enum!(RuntimeItemStatus {
    Created => "created",
    Running => "running",
    Waiting => "waiting",
    Succeeded => "succeeded",
    Failed => "failed",
    Cancelled => "cancelled",
});

string_enum!(UsageLedgerStatus {
    Recorded => "recorded",
    UnavailableError => "unavailable_error",
});

string_enum!(BillingSessionStatus {
    Reserved => "reserved",
    Settled => "settled",
    Refunded => "refunded",
    Failed => "failed",
});

string_enum!(ContextTransitionKind {
    Initial => "initial",
    Append => "append",
    Callback => "callback",
    Retry => "retry",
    DeclaredCompaction => "declared_compaction",
    ObservedReplacement => "observed_replacement",
});

string_enum!(ContextTransitionActor {
    Host => "host",
    Client => "client",
    Provider => "provider",
});

string_enum!(RecoveryStateCode {
    Running => "running",
    WaitingCallback => "waiting_callback",
    WaitingHuman => "waiting_human",
    Paused => "paused",
    Retrying => "retrying",
    Succeeded => "succeeded",
    Failed => "failed",
    Cancelled => "cancelled",
});

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CanonicalRuntimeContentRecord {
    pub id: Uuid,
    pub scope_id: Uuid,
    pub application_id: Uuid,
    pub content_hash: String,
    pub content: serde_json::Value,
    pub byte_size: i64,
    pub created_at: OffsetDateTime,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContextVersionRecord {
    pub id: Uuid,
    pub scope_id: Uuid,
    pub application_id: Uuid,
    pub flow_run_id: Uuid,
    pub parent_context_version_id: Option<Uuid>,
    pub sequence: i64,
    pub transition_kind: ContextTransitionKind,
    pub transition_actor: ContextTransitionActor,
    pub declared_compaction_provenance: Option<serde_json::Value>,
    pub actual_content_id: Uuid,
    pub created_at: OffsetDateTime,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InvocationContextBindingRecord {
    pub invocation_span_id: Uuid,
    pub scope_id: Uuid,
    pub application_id: Uuid,
    pub flow_run_id: Uuid,
    pub context_version_id: Uuid,
    pub created_at: OffsetDateTime,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct RecoveryCoordinate {
    pub node_sequence: i64,
    pub iteration_index: i64,
    pub attempt_index: i32,
    pub resume_sequence: i64,
    pub event_sequence: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RecoveryHistoryRecord {
    pub id: Uuid,
    pub scope_id: Uuid,
    pub application_id: Uuid,
    pub flow_run_id: Uuid,
    pub node_run_id: Option<Uuid>,
    pub sequence: i64,
    pub state_code: RecoveryStateCode,
    pub coordinate: RecoveryCoordinate,
    pub context_version_id: Uuid,
    pub recovery_content_id: Option<Uuid>,
    pub idempotency_key: String,
    pub created_at: OffsetDateTime,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RuntimeSpanRecord {
    pub id: Uuid,
    pub flow_run_id: Uuid,
    pub node_run_id: Option<Uuid>,
    pub parent_span_id: Option<Uuid>,
    pub kind: RuntimeSpanKind,
    pub name: String,
    pub status: RuntimeSpanStatus,
    pub capability_id: Option<String>,
    pub input_ref: Option<String>,
    pub output_ref: Option<String>,
    pub error_payload: Option<serde_json::Value>,
    pub metadata: serde_json::Value,
    pub started_at: OffsetDateTime,
    pub finished_at: Option<OffsetDateTime>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RuntimeEventRecord {
    pub id: Uuid,
    pub flow_run_id: Uuid,
    pub node_run_id: Option<Uuid>,
    pub span_id: Option<Uuid>,
    pub parent_span_id: Option<Uuid>,
    pub sequence: i64,
    pub event_type: String,
    pub layer: RuntimeEventLayer,
    pub source: RuntimeEventSource,
    pub trust_level: RuntimeTrustLevel,
    pub item_id: Option<Uuid>,
    pub ledger_ref: Option<String>,
    pub payload: serde_json::Value,
    pub visibility: RuntimeEventVisibility,
    pub durability: RuntimeEventDurability,
    pub created_at: OffsetDateTime,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RuntimeItemRecord {
    pub id: Uuid,
    pub flow_run_id: Uuid,
    pub span_id: Option<Uuid>,
    pub kind: RuntimeItemKind,
    pub status: RuntimeItemStatus,
    pub source_event_id: Option<Uuid>,
    pub input_ref: Option<String>,
    pub output_ref: Option<String>,
    pub usage_ledger_id: Option<Uuid>,
    pub trust_level: RuntimeTrustLevel,
    pub created_at: OffsetDateTime,
    pub updated_at: OffsetDateTime,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContextProjectionRecord {
    pub id: Uuid,
    pub flow_run_id: Uuid,
    pub node_run_id: Option<Uuid>,
    pub llm_turn_span_id: Option<Uuid>,
    pub projection_kind: String,
    pub merge_stage_ref: Option<String>,
    pub source_transcript_ref: Option<String>,
    pub source_item_refs: serde_json::Value,
    pub compaction_event_id: Option<Uuid>,
    pub summary_version: Option<String>,
    pub model_input_ref: String,
    pub model_input_hash: String,
    pub compacted_summary_ref: Option<String>,
    pub previous_projection_id: Option<Uuid>,
    pub token_estimate: Option<i64>,
    pub provider_continuation_metadata: serde_json::Value,
    pub created_at: OffsetDateTime,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UsageLedgerRecord {
    pub id: Uuid,
    pub flow_run_id: Uuid,
    pub node_run_id: Option<Uuid>,
    pub span_id: Option<Uuid>,
    pub failover_attempt_id: Option<Uuid>,
    pub provider_instance_id: Option<Uuid>,
    pub gateway_route_id: Option<Uuid>,
    pub model_id: Option<String>,
    pub upstream_model_id: Option<String>,
    pub upstream_request_id: Option<String>,
    pub input_tokens: Option<i64>,
    pub cached_input_tokens: Option<i64>,
    pub output_tokens: Option<i64>,
    pub reasoning_output_tokens: Option<i64>,
    pub total_tokens: Option<i64>,
    pub input_cache_hit_tokens: Option<i64>,
    pub input_cache_miss_tokens: Option<i64>,
    pub cache_read_tokens: Option<i64>,
    pub cache_write_tokens: Option<i64>,
    pub price_snapshot: Option<serde_json::Value>,
    pub cost_snapshot: Option<serde_json::Value>,
    pub usage_status: UsageLedgerStatus,
    pub raw_usage: serde_json::Value,
    pub normalized_usage: serde_json::Value,
    pub created_at: OffsetDateTime,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CostLedgerRecord {
    pub id: Uuid,
    pub flow_run_id: Option<Uuid>,
    pub span_id: Option<Uuid>,
    pub usage_ledger_id: Option<Uuid>,
    pub workspace_id: Uuid,
    pub provider_instance_id: Option<Uuid>,
    pub provider_account_id: Option<Uuid>,
    pub gateway_route_id: Option<Uuid>,
    pub model_id: Option<String>,
    pub upstream_model_id: Option<String>,
    pub price_snapshot: serde_json::Value,
    pub raw_cost: Option<String>,
    pub normalized_cost: Option<String>,
    pub settlement_currency: Option<String>,
    pub cost_source: String,
    pub cost_status: String,
    pub created_at: OffsetDateTime,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CreditLedgerRecord {
    pub id: Uuid,
    pub workspace_id: Uuid,
    pub user_id: Option<Uuid>,
    pub application_id: Option<Uuid>,
    pub agent_id: Option<Uuid>,
    pub flow_run_id: Option<Uuid>,
    pub span_id: Option<Uuid>,
    pub cost_ledger_id: Option<Uuid>,
    pub transaction_type: String,
    pub amount: String,
    pub balance_after: Option<String>,
    pub credit_unit: String,
    pub reason: String,
    pub idempotency_key: String,
    pub status: String,
    pub created_at: OffsetDateTime,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BillingSessionRecord {
    pub id: Uuid,
    pub workspace_id: Uuid,
    pub flow_run_id: Option<Uuid>,
    pub client_request_id: Option<String>,
    pub idempotency_key: String,
    pub route_id: Option<Uuid>,
    pub provider_account_id: Option<Uuid>,
    pub status: BillingSessionStatus,
    pub reserved_credit_ledger_id: Option<Uuid>,
    pub settled_credit_ledger_id: Option<Uuid>,
    pub refund_credit_ledger_id: Option<Uuid>,
    pub metadata: serde_json::Value,
    pub created_at: OffsetDateTime,
    pub updated_at: OffsetDateTime,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AuditHashRecord {
    pub id: Uuid,
    pub flow_run_id: Uuid,
    pub fact_table: String,
    pub fact_id: Uuid,
    pub prev_hash: Option<String>,
    pub row_hash: String,
    pub created_at: OffsetDateTime,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModelFailoverAttemptLedgerRecord {
    pub id: Uuid,
    pub flow_run_id: Uuid,
    pub node_run_id: Option<Uuid>,
    pub llm_turn_span_id: Option<Uuid>,
    pub queue_snapshot_id: Option<Uuid>,
    pub attempt_index: i32,
    pub provider_instance_id: Option<Uuid>,
    pub provider_code: String,
    pub upstream_model_id: String,
    pub protocol: String,
    pub request_ref: Option<String>,
    pub request_hash: Option<String>,
    pub started_at: OffsetDateTime,
    pub first_token_at: Option<OffsetDateTime>,
    pub finished_at: Option<OffsetDateTime>,
    pub status: String,
    pub failed_after_first_token: bool,
    pub upstream_request_id: Option<String>,
    pub error_code: Option<String>,
    pub error_message_ref: Option<String>,
    pub usage_ledger_id: Option<Uuid>,
    pub cost_ledger_id: Option<Uuid>,
    pub response_ref: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CapabilityInvocationRecord {
    pub id: Uuid,
    pub flow_run_id: Uuid,
    pub span_id: Option<Uuid>,
    pub capability_id: String,
    pub requested_by_span_id: Option<Uuid>,
    pub requester_kind: String,
    pub arguments_ref: Option<String>,
    pub authorization_status: String,
    pub authorization_reason: Option<String>,
    pub result_ref: Option<String>,
    pub normalized_result: Option<serde_json::Value>,
    pub started_at: Option<OffsetDateTime>,
    pub finished_at: Option<OffsetDateTime>,
    pub error_payload: Option<serde_json::Value>,
    pub created_at: OffsetDateTime,
}
