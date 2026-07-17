use anyhow::{anyhow, Result};
use time::OffsetDateTime;
use uuid::Uuid;

#[derive(Debug, Clone)]
pub struct StoredCompiledPlanRow {
    pub id: Uuid,
    pub flow_id: Uuid,
    pub flow_draft_id: Uuid,
    pub schema_version: String,
    pub document_hash: String,
    pub document_updated_at: OffsetDateTime,
    pub plan: serde_json::Value,
    pub created_by: Uuid,
    pub created_at: OffsetDateTime,
    pub updated_at: OffsetDateTime,
}

#[derive(Debug, Clone)]
pub struct StoredFlowRunRow {
    pub id: Uuid,
    pub application_id: Uuid,
    pub flow_id: Uuid,
    pub flow_draft_id: Uuid,
    pub compiled_plan_id: Option<Uuid>,
    pub debug_session_id: String,
    pub flow_schema_version: String,
    pub document_hash: String,
    pub run_mode: String,
    pub target_node_id: Option<String>,
    pub title: String,
    pub status: String,
    pub input_payload: serde_json::Value,
    pub output_payload: serde_json::Value,
    pub error_payload: Option<serde_json::Value>,
    pub created_by: Uuid,
    pub authorized_account: Option<String>,
    pub api_key_id: Option<Uuid>,
    pub publication_version_id: Option<Uuid>,
    pub external_user: Option<String>,
    pub external_conversation_id: Option<String>,
    pub external_trace_id: Option<String>,
    pub compatibility_mode: Option<String>,
    pub idempotency_key: Option<String>,
    pub started_at: OffsetDateTime,
    pub finished_at: Option<OffsetDateTime>,
    pub created_at: OffsetDateTime,
    pub updated_at: OffsetDateTime,
}

#[derive(Debug, Clone)]
pub struct StoredNodeRunRow {
    pub id: Uuid,
    pub flow_run_id: Uuid,
    pub node_id: String,
    pub node_type: String,
    pub node_alias: String,
    pub status: String,
    pub input_payload: serde_json::Value,
    pub output_payload: serde_json::Value,
    pub error_payload: Option<serde_json::Value>,
    pub metrics_payload: serde_json::Value,
    pub debug_payload: serde_json::Value,
    pub started_at: OffsetDateTime,
    pub finished_at: Option<OffsetDateTime>,
}

#[derive(Debug, Clone)]
pub struct StoredCheckpointRow {
    pub id: Uuid,
    pub flow_run_id: Uuid,
    pub node_run_id: Option<Uuid>,
    pub status: String,
    pub reason: String,
    pub locator_payload: serde_json::Value,
    pub variable_snapshot: serde_json::Value,
    pub external_ref_payload: Option<serde_json::Value>,
    pub created_at: OffsetDateTime,
}

#[derive(Debug, Clone)]
pub struct StoredCallbackTaskRow {
    pub id: Uuid,
    pub flow_run_id: Uuid,
    pub node_run_id: Uuid,
    pub callback_kind: String,
    pub status: String,
    pub request_payload: serde_json::Value,
    pub response_payload: Option<serde_json::Value>,
    pub external_ref_payload: Option<serde_json::Value>,
    pub created_at: OffsetDateTime,
    pub completed_at: Option<OffsetDateTime>,
}

#[derive(Debug, Clone)]
pub struct StoredRunEventRow {
    pub id: Uuid,
    pub flow_run_id: Uuid,
    pub node_run_id: Option<Uuid>,
    pub sequence: i64,
    pub event_type: String,
    pub payload: serde_json::Value,
    pub created_at: OffsetDateTime,
}

#[derive(Debug, Clone)]
pub struct StoredRuntimeSpanRow {
    pub id: Uuid,
    pub flow_run_id: Uuid,
    pub node_run_id: Option<Uuid>,
    pub parent_span_id: Option<Uuid>,
    pub kind: String,
    pub name: String,
    pub status: String,
    pub capability_id: Option<String>,
    pub input_ref: Option<String>,
    pub output_ref: Option<String>,
    pub error_payload: Option<serde_json::Value>,
    pub metadata: serde_json::Value,
    pub started_at: OffsetDateTime,
    pub finished_at: Option<OffsetDateTime>,
}

#[derive(Debug, Clone)]
pub struct StoredRuntimeEventRow {
    pub id: Uuid,
    pub flow_run_id: Uuid,
    pub node_run_id: Option<Uuid>,
    pub span_id: Option<Uuid>,
    pub parent_span_id: Option<Uuid>,
    pub sequence: i64,
    pub event_type: String,
    pub layer: String,
    pub source: String,
    pub trust_level: String,
    pub item_id: Option<Uuid>,
    pub ledger_ref: Option<String>,
    pub payload: serde_json::Value,
    pub visibility: String,
    pub durability: String,
    pub created_at: OffsetDateTime,
}

#[derive(Debug, Clone)]
pub struct StoredRuntimeItemRow {
    pub id: Uuid,
    pub flow_run_id: Uuid,
    pub span_id: Option<Uuid>,
    pub kind: String,
    pub status: String,
    pub source_event_id: Option<Uuid>,
    pub input_ref: Option<String>,
    pub output_ref: Option<String>,
    pub usage_ledger_id: Option<Uuid>,
    pub trust_level: String,
    pub created_at: OffsetDateTime,
    pub updated_at: OffsetDateTime,
}

#[derive(Debug, Clone)]
pub struct StoredContextProjectionRow {
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

#[derive(Debug, Clone)]
pub struct StoredUsageLedgerRow {
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
    pub usage_status: String,
    pub raw_usage: serde_json::Value,
    pub normalized_usage: serde_json::Value,
    pub created_at: OffsetDateTime,
}

#[derive(Debug, Clone)]
pub struct StoredCostLedgerRow {
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

#[derive(Debug, Clone)]
pub struct StoredCreditLedgerRow {
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

#[derive(Debug, Clone)]
pub struct StoredBillingSessionRow {
    pub id: Uuid,
    pub workspace_id: Uuid,
    pub flow_run_id: Option<Uuid>,
    pub client_request_id: Option<String>,
    pub idempotency_key: String,
    pub route_id: Option<Uuid>,
    pub provider_account_id: Option<Uuid>,
    pub status: String,
    pub reserved_credit_ledger_id: Option<Uuid>,
    pub settled_credit_ledger_id: Option<Uuid>,
    pub refund_credit_ledger_id: Option<Uuid>,
    pub metadata: serde_json::Value,
    pub created_at: OffsetDateTime,
    pub updated_at: OffsetDateTime,
}

#[derive(Debug, Clone)]
pub struct StoredAuditHashRow {
    pub id: Uuid,
    pub flow_run_id: Uuid,
    pub fact_table: String,
    pub fact_id: Uuid,
    pub prev_hash: Option<String>,
    pub row_hash: String,
    pub created_at: OffsetDateTime,
}

#[derive(Debug, Clone)]
pub struct StoredModelFailoverAttemptLedgerRow {
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

#[derive(Debug, Clone)]
pub struct StoredCapabilityInvocationRow {
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

#[derive(Debug, Clone)]
pub struct StoredApplicationRunSummaryRow {
    pub id: Uuid,
    pub run_mode: String,
    pub status: String,
    pub target_node_id: Option<String>,
    pub title: String,
    pub input_payload: serde_json::Value,
    pub external_user: Option<String>,
    pub authorized_account: Option<String>,
    pub api_key_id: Option<Uuid>,
    pub publication_version_id: Option<Uuid>,
    pub external_conversation_id: Option<String>,
    pub external_trace_id: Option<String>,
    pub compatibility_mode: Option<String>,
    pub idempotency_key: Option<String>,
    pub started_at: OffsetDateTime,
    pub finished_at: Option<OffsetDateTime>,
    pub created_at: OffsetDateTime,
    pub updated_at: OffsetDateTime,
}

#[derive(Debug, Clone)]
pub struct StoredApplicationRunLogSummaryRow {
    pub run: StoredApplicationRunSummaryRow,
    pub total_tokens: Option<i64>,
    pub input_tokens: Option<i64>,
    pub output_tokens: Option<i64>,
    pub input_cache_hit_tokens: Option<i64>,
    pub input_cache_hit_rate: Option<f64>,
    pub unique_node_count: i64,
    pub tool_callback_count: i64,
}

pub struct PgOrchestrationRuntimeMapper;

impl PgOrchestrationRuntimeMapper {
    pub fn to_compiled_plan_record(row: StoredCompiledPlanRow) -> domain::CompiledPlanRecord {
        domain::CompiledPlanRecord {
            id: row.id,
            flow_id: row.flow_id,
            draft_id: row.flow_draft_id,
            schema_version: row.schema_version,
            document_hash: row.document_hash,
            document_updated_at: row.document_updated_at,
            plan: row.plan,
            created_by: row.created_by,
            created_at: row.created_at,
            updated_at: row.updated_at,
        }
    }

    pub fn to_flow_run_record(row: StoredFlowRunRow) -> Result<domain::FlowRunRecord> {
        let title =
            control_plane::flow_run_title::display_flow_run_title(&row.title, &row.input_payload);
        Ok(domain::FlowRunRecord {
            id: row.id,
            application_id: row.application_id,
            flow_id: row.flow_id,
            draft_id: row.flow_draft_id,
            compiled_plan_id: row.compiled_plan_id,
            debug_session_id: row.debug_session_id,
            flow_schema_version: row.flow_schema_version,
            document_hash: row.document_hash,
            run_mode: parse_flow_run_mode(&row.run_mode)?,
            target_node_id: row.target_node_id,
            title,
            status: parse_flow_run_status(&row.status)?,
            input_payload: row.input_payload,
            output_payload: row.output_payload,
            error_payload: row.error_payload,
            created_by: row.created_by,
            authorized_account: row.authorized_account,
            api_key_id: row.api_key_id,
            publication_version_id: row.publication_version_id,
            external_user: row.external_user,
            external_conversation_id: row.external_conversation_id,
            external_trace_id: row.external_trace_id,
            compatibility_mode: row.compatibility_mode,
            idempotency_key: row.idempotency_key,
            started_at: row.started_at,
            finished_at: row.finished_at,
            created_at: row.created_at,
            updated_at: row.updated_at,
        })
    }

    pub fn to_node_run_record(row: StoredNodeRunRow) -> Result<domain::NodeRunRecord> {
        Ok(domain::NodeRunRecord {
            id: row.id,
            flow_run_id: row.flow_run_id,
            node_id: row.node_id,
            node_type: row.node_type,
            node_alias: row.node_alias,
            status: parse_node_run_status(&row.status)?,
            input_payload: row.input_payload,
            output_payload: row.output_payload,
            error_payload: row.error_payload,
            metrics_payload: row.metrics_payload,
            debug_payload: row.debug_payload,
            started_at: row.started_at,
            finished_at: row.finished_at,
        })
    }

    pub fn to_checkpoint_record(row: StoredCheckpointRow) -> domain::CheckpointRecord {
        domain::CheckpointRecord {
            id: row.id,
            flow_run_id: row.flow_run_id,
            node_run_id: row.node_run_id,
            status: row.status,
            reason: row.reason,
            locator_payload: row.locator_payload,
            variable_snapshot: row.variable_snapshot,
            external_ref_payload: row.external_ref_payload,
            created_at: row.created_at,
        }
    }

    pub fn to_callback_task_record(
        row: StoredCallbackTaskRow,
    ) -> Result<domain::CallbackTaskRecord> {
        Ok(domain::CallbackTaskRecord {
            id: row.id,
            flow_run_id: row.flow_run_id,
            node_run_id: row.node_run_id,
            callback_kind: row.callback_kind,
            status: parse_callback_task_status(&row.status)?,
            request_payload: row.request_payload,
            response_payload: row.response_payload,
            external_ref_payload: row.external_ref_payload,
            created_at: row.created_at,
            completed_at: row.completed_at,
        })
    }

    pub fn to_run_event_record(row: StoredRunEventRow) -> domain::RunEventRecord {
        domain::RunEventRecord {
            id: row.id,
            flow_run_id: row.flow_run_id,
            node_run_id: row.node_run_id,
            sequence: row.sequence,
            event_type: row.event_type,
            payload: row.payload,
            created_at: row.created_at,
        }
    }

    pub fn to_runtime_span_record(row: StoredRuntimeSpanRow) -> Result<domain::RuntimeSpanRecord> {
        Ok(domain::RuntimeSpanRecord {
            id: row.id,
            flow_run_id: row.flow_run_id,
            node_run_id: row.node_run_id,
            parent_span_id: row.parent_span_id,
            kind: parse_runtime_span_kind(&row.kind)?,
            name: row.name,
            status: parse_runtime_span_status(&row.status)?,
            capability_id: row.capability_id,
            input_ref: row.input_ref,
            output_ref: row.output_ref,
            error_payload: row.error_payload,
            metadata: row.metadata,
            started_at: row.started_at,
            finished_at: row.finished_at,
        })
    }

    pub fn to_runtime_event_record(
        row: StoredRuntimeEventRow,
    ) -> Result<domain::RuntimeEventRecord> {
        Ok(domain::RuntimeEventRecord {
            id: row.id,
            flow_run_id: row.flow_run_id,
            node_run_id: row.node_run_id,
            span_id: row.span_id,
            parent_span_id: row.parent_span_id,
            sequence: row.sequence,
            event_type: row.event_type,
            layer: parse_runtime_event_layer(&row.layer)?,
            source: parse_runtime_event_source(&row.source)?,
            trust_level: parse_runtime_trust_level(&row.trust_level)?,
            item_id: row.item_id,
            ledger_ref: row.ledger_ref,
            payload: row.payload,
            visibility: parse_runtime_event_visibility(&row.visibility)?,
            durability: parse_runtime_event_durability(&row.durability)?,
            created_at: row.created_at,
        })
    }

    pub fn to_runtime_item_record(row: StoredRuntimeItemRow) -> Result<domain::RuntimeItemRecord> {
        Ok(domain::RuntimeItemRecord {
            id: row.id,
            flow_run_id: row.flow_run_id,
            span_id: row.span_id,
            kind: parse_runtime_item_kind(&row.kind)?,
            status: parse_runtime_item_status(&row.status)?,
            source_event_id: row.source_event_id,
            input_ref: row.input_ref,
            output_ref: row.output_ref,
            usage_ledger_id: row.usage_ledger_id,
            trust_level: parse_runtime_trust_level(&row.trust_level)?,
            created_at: row.created_at,
            updated_at: row.updated_at,
        })
    }

    pub fn to_context_projection_record(
        row: StoredContextProjectionRow,
    ) -> domain::ContextProjectionRecord {
        domain::ContextProjectionRecord {
            id: row.id,
            flow_run_id: row.flow_run_id,
            node_run_id: row.node_run_id,
            llm_turn_span_id: row.llm_turn_span_id,
            projection_kind: row.projection_kind,
            merge_stage_ref: row.merge_stage_ref,
            source_transcript_ref: row.source_transcript_ref,
            source_item_refs: row.source_item_refs,
            compaction_event_id: row.compaction_event_id,
            summary_version: row.summary_version,
            model_input_ref: row.model_input_ref,
            model_input_hash: row.model_input_hash,
            compacted_summary_ref: row.compacted_summary_ref,
            previous_projection_id: row.previous_projection_id,
            token_estimate: row.token_estimate,
            provider_continuation_metadata: row.provider_continuation_metadata,
            created_at: row.created_at,
        }
    }

    pub fn to_usage_ledger_record(row: StoredUsageLedgerRow) -> Result<domain::UsageLedgerRecord> {
        Ok(domain::UsageLedgerRecord {
            id: row.id,
            flow_run_id: row.flow_run_id,
            node_run_id: row.node_run_id,
            span_id: row.span_id,
            failover_attempt_id: row.failover_attempt_id,
            provider_instance_id: row.provider_instance_id,
            gateway_route_id: row.gateway_route_id,
            model_id: row.model_id,
            upstream_model_id: row.upstream_model_id,
            upstream_request_id: row.upstream_request_id,
            input_tokens: row.input_tokens,
            cached_input_tokens: row.cached_input_tokens,
            output_tokens: row.output_tokens,
            reasoning_output_tokens: row.reasoning_output_tokens,
            total_tokens: row.total_tokens,
            input_cache_hit_tokens: row.input_cache_hit_tokens,
            input_cache_miss_tokens: row.input_cache_miss_tokens,
            cache_read_tokens: row.cache_read_tokens,
            cache_write_tokens: row.cache_write_tokens,
            price_snapshot: row.price_snapshot,
            cost_snapshot: row.cost_snapshot,
            usage_status: parse_usage_ledger_status(&row.usage_status)?,
            raw_usage: row.raw_usage,
            normalized_usage: row.normalized_usage,
            created_at: row.created_at,
        })
    }

    pub fn to_cost_ledger_record(row: StoredCostLedgerRow) -> domain::CostLedgerRecord {
        domain::CostLedgerRecord {
            id: row.id,
            flow_run_id: row.flow_run_id,
            span_id: row.span_id,
            usage_ledger_id: row.usage_ledger_id,
            workspace_id: row.workspace_id,
            provider_instance_id: row.provider_instance_id,
            provider_account_id: row.provider_account_id,
            gateway_route_id: row.gateway_route_id,
            model_id: row.model_id,
            upstream_model_id: row.upstream_model_id,
            price_snapshot: row.price_snapshot,
            raw_cost: row.raw_cost,
            normalized_cost: row.normalized_cost,
            settlement_currency: row.settlement_currency,
            cost_source: row.cost_source,
            cost_status: row.cost_status,
            created_at: row.created_at,
        }
    }

    pub fn to_credit_ledger_record(row: StoredCreditLedgerRow) -> domain::CreditLedgerRecord {
        domain::CreditLedgerRecord {
            id: row.id,
            workspace_id: row.workspace_id,
            user_id: row.user_id,
            application_id: row.application_id,
            agent_id: row.agent_id,
            flow_run_id: row.flow_run_id,
            span_id: row.span_id,
            cost_ledger_id: row.cost_ledger_id,
            transaction_type: row.transaction_type,
            amount: row.amount,
            balance_after: row.balance_after,
            credit_unit: row.credit_unit,
            reason: row.reason,
            idempotency_key: row.idempotency_key,
            status: row.status,
            created_at: row.created_at,
        }
    }

    pub fn to_billing_session_record(
        row: StoredBillingSessionRow,
    ) -> Result<domain::BillingSessionRecord> {
        Ok(domain::BillingSessionRecord {
            id: row.id,
            workspace_id: row.workspace_id,
            flow_run_id: row.flow_run_id,
            client_request_id: row.client_request_id,
            idempotency_key: row.idempotency_key,
            route_id: row.route_id,
            provider_account_id: row.provider_account_id,
            status: parse_billing_session_status(&row.status)?,
            reserved_credit_ledger_id: row.reserved_credit_ledger_id,
            settled_credit_ledger_id: row.settled_credit_ledger_id,
            refund_credit_ledger_id: row.refund_credit_ledger_id,
            metadata: row.metadata,
            created_at: row.created_at,
            updated_at: row.updated_at,
        })
    }

    pub fn to_audit_hash_record(row: StoredAuditHashRow) -> domain::AuditHashRecord {
        domain::AuditHashRecord {
            id: row.id,
            flow_run_id: row.flow_run_id,
            fact_table: row.fact_table,
            fact_id: row.fact_id,
            prev_hash: row.prev_hash,
            row_hash: row.row_hash,
            created_at: row.created_at,
        }
    }

    pub fn to_model_failover_attempt_ledger_record(
        row: StoredModelFailoverAttemptLedgerRow,
    ) -> domain::ModelFailoverAttemptLedgerRecord {
        domain::ModelFailoverAttemptLedgerRecord {
            id: row.id,
            flow_run_id: row.flow_run_id,
            node_run_id: row.node_run_id,
            llm_turn_span_id: row.llm_turn_span_id,
            queue_snapshot_id: row.queue_snapshot_id,
            attempt_index: row.attempt_index,
            provider_instance_id: row.provider_instance_id,
            provider_code: row.provider_code,
            upstream_model_id: row.upstream_model_id,
            protocol: row.protocol,
            request_ref: row.request_ref,
            request_hash: row.request_hash,
            started_at: row.started_at,
            first_token_at: row.first_token_at,
            finished_at: row.finished_at,
            status: row.status,
            failed_after_first_token: row.failed_after_first_token,
            upstream_request_id: row.upstream_request_id,
            error_code: row.error_code,
            error_message_ref: row.error_message_ref,
            usage_ledger_id: row.usage_ledger_id,
            cost_ledger_id: row.cost_ledger_id,
            response_ref: row.response_ref,
        }
    }

    pub fn to_capability_invocation_record(
        row: StoredCapabilityInvocationRow,
    ) -> domain::CapabilityInvocationRecord {
        domain::CapabilityInvocationRecord {
            id: row.id,
            flow_run_id: row.flow_run_id,
            span_id: row.span_id,
            capability_id: row.capability_id,
            requested_by_span_id: row.requested_by_span_id,
            requester_kind: row.requester_kind,
            arguments_ref: row.arguments_ref,
            authorization_status: row.authorization_status,
            authorization_reason: row.authorization_reason,
            result_ref: row.result_ref,
            normalized_result: row.normalized_result,
            started_at: row.started_at,
            finished_at: row.finished_at,
            error_payload: row.error_payload,
            created_at: row.created_at,
        }
    }

    pub fn to_application_run_summary(
        row: StoredApplicationRunSummaryRow,
    ) -> Result<domain::ApplicationRunSummary> {
        Ok(domain::ApplicationRunSummary {
            id: row.id,
            run_mode: parse_flow_run_mode(&row.run_mode)?,
            status: parse_flow_run_status(&row.status)?,
            target_node_id: row.target_node_id,
            title: control_plane::flow_run_title::display_flow_run_title(
                &row.title,
                &row.input_payload,
            ),
            user_id: row.external_user,
            authorized_account: row.authorized_account,
            api_key_id: row.api_key_id,
            publication_version_id: row.publication_version_id,
            external_conversation_id: row.external_conversation_id,
            external_trace_id: row.external_trace_id,
            compatibility_mode: row.compatibility_mode,
            idempotency_key: row.idempotency_key,
            started_at: row.started_at,
            finished_at: row.finished_at,
            created_at: row.created_at,
            updated_at: row.updated_at,
        })
    }

    pub fn to_application_run_log_summary(
        row: StoredApplicationRunLogSummaryRow,
    ) -> Result<domain::ApplicationRunLogSummary> {
        Ok(domain::ApplicationRunLogSummary {
            run: Self::to_application_run_summary(row.run)?,
            total_tokens: row.total_tokens,
            input_tokens: row.input_tokens,
            output_tokens: row.output_tokens,
            input_cache_hit_tokens: row.input_cache_hit_tokens,
            input_cache_hit_rate: row.input_cache_hit_rate,
            unique_node_count: row.unique_node_count,
            tool_callback_count: row.tool_callback_count,
        })
    }
}

pub fn parse_flow_run_mode(value: &str) -> Result<domain::FlowRunMode> {
    match value {
        "debug_node_preview" => Ok(domain::FlowRunMode::DebugNodePreview),
        "debug_flow_run" => Ok(domain::FlowRunMode::DebugFlowRun),
        "published_api_run" => Ok(domain::FlowRunMode::PublishedApiRun),
        _ => Err(anyhow!("unknown flow run mode: {value}")),
    }
}

pub fn parse_callback_task_status(value: &str) -> Result<domain::CallbackTaskStatus> {
    match value {
        "pending" => Ok(domain::CallbackTaskStatus::Pending),
        "completed" => Ok(domain::CallbackTaskStatus::Completed),
        "cancelled" => Ok(domain::CallbackTaskStatus::Cancelled),
        _ => Err(anyhow!("unknown callback task status: {value}")),
    }
}

pub fn parse_flow_run_callback_resume_attempt_status(
    value: &str,
) -> Result<domain::FlowRunCallbackResumeAttemptStatus> {
    match value {
        "received" => Ok(domain::FlowRunCallbackResumeAttemptStatus::Received),
        "processing" => Ok(domain::FlowRunCallbackResumeAttemptStatus::Processing),
        "succeeded" => Ok(domain::FlowRunCallbackResumeAttemptStatus::Succeeded),
        "failed" => Ok(domain::FlowRunCallbackResumeAttemptStatus::Failed),
        "cancelled" => Ok(domain::FlowRunCallbackResumeAttemptStatus::Cancelled),
        _ => Err(anyhow!(
            "unknown flow run callback resume attempt status: {value}"
        )),
    }
}

pub fn parse_flow_run_status(value: &str) -> Result<domain::FlowRunStatus> {
    match value {
        "queued" => Ok(domain::FlowRunStatus::Queued),
        "running" => Ok(domain::FlowRunStatus::Running),
        "waiting_callback" => Ok(domain::FlowRunStatus::WaitingCallback),
        "waiting_human" => Ok(domain::FlowRunStatus::WaitingHuman),
        "paused" => Ok(domain::FlowRunStatus::Paused),
        "succeeded" => Ok(domain::FlowRunStatus::Succeeded),
        "incomplete" => Ok(domain::FlowRunStatus::Incomplete),
        "failed" => Ok(domain::FlowRunStatus::Failed),
        "cancelled" => Ok(domain::FlowRunStatus::Cancelled),
        _ => Err(anyhow!("unknown flow run status: {value}")),
    }
}

pub fn parse_node_run_status(value: &str) -> Result<domain::NodeRunStatus> {
    match value {
        "pending" => Ok(domain::NodeRunStatus::Pending),
        "ready" => Ok(domain::NodeRunStatus::Ready),
        "running" => Ok(domain::NodeRunStatus::Running),
        "streaming" => Ok(domain::NodeRunStatus::Streaming),
        "waiting_tool" => Ok(domain::NodeRunStatus::WaitingTool),
        "waiting_callback" => Ok(domain::NodeRunStatus::WaitingCallback),
        "waiting_human" => Ok(domain::NodeRunStatus::WaitingHuman),
        "retrying" => Ok(domain::NodeRunStatus::Retrying),
        "succeeded" => Ok(domain::NodeRunStatus::Succeeded),
        "failed" => Ok(domain::NodeRunStatus::Failed),
        "skipped" => Ok(domain::NodeRunStatus::Skipped),
        _ => Err(anyhow!("unknown node run status: {value}")),
    }
}

pub fn parse_runtime_span_kind(value: &str) -> Result<domain::RuntimeSpanKind> {
    match value {
        "flow" => Ok(domain::RuntimeSpanKind::Flow),
        "node" => Ok(domain::RuntimeSpanKind::Node),
        "llm_turn" => Ok(domain::RuntimeSpanKind::LlmTurn),
        "provider_request" => Ok(domain::RuntimeSpanKind::ProviderRequest),
        "gateway_forward" => Ok(domain::RuntimeSpanKind::GatewayForward),
        "tool_call" => Ok(domain::RuntimeSpanKind::ToolCall),
        "mcp_call" => Ok(domain::RuntimeSpanKind::McpCall),
        "skill_load" => Ok(domain::RuntimeSpanKind::SkillLoad),
        "skill_action" => Ok(domain::RuntimeSpanKind::SkillAction),
        "workflow_tool" => Ok(domain::RuntimeSpanKind::WorkflowTool),
        "data_retrieval" => Ok(domain::RuntimeSpanKind::DataRetrieval),
        "approval" => Ok(domain::RuntimeSpanKind::Approval),
        "compaction" => Ok(domain::RuntimeSpanKind::Compaction),
        "subagent" => Ok(domain::RuntimeSpanKind::Subagent),
        "system_agent" => Ok(domain::RuntimeSpanKind::SystemAgent),
        _ => Err(anyhow!("unknown runtime span kind: {value}")),
    }
}

pub fn parse_runtime_span_status(value: &str) -> Result<domain::RuntimeSpanStatus> {
    match value {
        "running" => Ok(domain::RuntimeSpanStatus::Running),
        "succeeded" => Ok(domain::RuntimeSpanStatus::Succeeded),
        "failed" => Ok(domain::RuntimeSpanStatus::Failed),
        "cancelled" => Ok(domain::RuntimeSpanStatus::Cancelled),
        "waiting" => Ok(domain::RuntimeSpanStatus::Waiting),
        _ => Err(anyhow!("unknown runtime span status: {value}")),
    }
}

pub fn parse_runtime_event_layer(value: &str) -> Result<domain::RuntimeEventLayer> {
    match value {
        "provider_raw" => Ok(domain::RuntimeEventLayer::ProviderRaw),
        "runtime_item" => Ok(domain::RuntimeEventLayer::RuntimeItem),
        "capability" => Ok(domain::RuntimeEventLayer::Capability),
        "agent_transition" => Ok(domain::RuntimeEventLayer::AgentTransition),
        "ledger" => Ok(domain::RuntimeEventLayer::Ledger),
        "diagnostic" => Ok(domain::RuntimeEventLayer::Diagnostic),
        _ => Err(anyhow!("unknown runtime event layer: {value}")),
    }
}

pub fn parse_runtime_event_source(value: &str) -> Result<domain::RuntimeEventSource> {
    match value {
        "host" => Ok(domain::RuntimeEventSource::Host),
        "provider_plugin" => Ok(domain::RuntimeEventSource::ProviderPlugin),
        "gateway_relay" => Ok(domain::RuntimeEventSource::GatewayRelay),
        "internal_agent" => Ok(domain::RuntimeEventSource::InternalAgent),
        "external_agent" => Ok(domain::RuntimeEventSource::ExternalAgent),
        _ => Err(anyhow!("unknown runtime event source: {value}")),
    }
}

pub fn parse_runtime_trust_level(value: &str) -> Result<domain::RuntimeTrustLevel> {
    match value {
        "host_fact" => Ok(domain::RuntimeTrustLevel::HostFact),
        "verified_bridge" => Ok(domain::RuntimeTrustLevel::VerifiedBridge),
        "agent_reported" => Ok(domain::RuntimeTrustLevel::AgentReported),
        "external_opaque" => Ok(domain::RuntimeTrustLevel::ExternalOpaque),
        "inferred" => Ok(domain::RuntimeTrustLevel::Inferred),
        _ => Err(anyhow!("unknown runtime trust level: {value}")),
    }
}

pub fn parse_runtime_event_visibility(value: &str) -> Result<domain::RuntimeEventVisibility> {
    match value {
        "internal" => Ok(domain::RuntimeEventVisibility::Internal),
        "workspace" => Ok(domain::RuntimeEventVisibility::Workspace),
        "user" => Ok(domain::RuntimeEventVisibility::User),
        "public" => Ok(domain::RuntimeEventVisibility::Public),
        _ => Err(anyhow!("unknown runtime event visibility: {value}")),
    }
}

pub fn parse_runtime_event_durability(value: &str) -> Result<domain::RuntimeEventDurability> {
    match value {
        "ephemeral" => Ok(domain::RuntimeEventDurability::Ephemeral),
        "durable" => Ok(domain::RuntimeEventDurability::Durable),
        "sampled" => Ok(domain::RuntimeEventDurability::Sampled),
        _ => Err(anyhow!("unknown runtime event durability: {value}")),
    }
}

pub fn parse_runtime_item_kind(value: &str) -> Result<domain::RuntimeItemKind> {
    match value {
        "message" => Ok(domain::RuntimeItemKind::Message),
        "reasoning" => Ok(domain::RuntimeItemKind::Reasoning),
        "tool_call" => Ok(domain::RuntimeItemKind::ToolCall),
        "tool_result" => Ok(domain::RuntimeItemKind::ToolResult),
        "mcp_call" => Ok(domain::RuntimeItemKind::McpCall),
        "skill_load" => Ok(domain::RuntimeItemKind::SkillLoad),
        "skill_action" => Ok(domain::RuntimeItemKind::SkillAction),
        "approval" => Ok(domain::RuntimeItemKind::Approval),
        "handoff" => Ok(domain::RuntimeItemKind::Handoff),
        "agent_as_tool" => Ok(domain::RuntimeItemKind::AgentAsTool),
        "compaction" => Ok(domain::RuntimeItemKind::Compaction),
        "gateway_forward" => Ok(domain::RuntimeItemKind::GatewayForward),
        _ => Err(anyhow!("unknown runtime item kind: {value}")),
    }
}

pub fn parse_runtime_item_status(value: &str) -> Result<domain::RuntimeItemStatus> {
    match value {
        "created" => Ok(domain::RuntimeItemStatus::Created),
        "running" => Ok(domain::RuntimeItemStatus::Running),
        "waiting" => Ok(domain::RuntimeItemStatus::Waiting),
        "succeeded" => Ok(domain::RuntimeItemStatus::Succeeded),
        "failed" => Ok(domain::RuntimeItemStatus::Failed),
        "cancelled" => Ok(domain::RuntimeItemStatus::Cancelled),
        _ => Err(anyhow!("unknown runtime item status: {value}")),
    }
}

pub fn parse_usage_ledger_status(value: &str) -> Result<domain::UsageLedgerStatus> {
    match value {
        "recorded" => Ok(domain::UsageLedgerStatus::Recorded),
        "unavailable_error" => Ok(domain::UsageLedgerStatus::UnavailableError),
        _ => Err(anyhow!("unknown usage ledger status: {value}")),
    }
}

pub fn parse_billing_session_status(value: &str) -> Result<domain::BillingSessionStatus> {
    match value {
        "reserved" => Ok(domain::BillingSessionStatus::Reserved),
        "settled" => Ok(domain::BillingSessionStatus::Settled),
        "refunded" => Ok(domain::BillingSessionStatus::Refunded),
        "failed" => Ok(domain::BillingSessionStatus::Failed),
        _ => Err(anyhow!("unknown billing session status: {value}")),
    }
}
