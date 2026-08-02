use anyhow::Result;
use sqlx::{postgres::PgRow, Row};
use uuid::Uuid;

use crate::mappers::orchestration_runtime_mapper::{
    parse_flow_run_callback_resume_attempt_status, parse_flow_run_status,
    PgOrchestrationRuntimeMapper, StoredApplicationRunLogSummaryRow,
    StoredApplicationRunSummaryRow, StoredAuditHashRow, StoredBillingSessionRow,
    StoredCallbackTaskRow, StoredCapabilityInvocationRow, StoredCheckpointRow,
    StoredCompiledPlanRow, StoredContextProjectionRow, StoredCostLedgerRow, StoredCreditLedgerRow,
    StoredFlowRunRow, StoredModelFailoverAttemptLedgerRow, StoredNodeRunRow, StoredRunEventRow,
    StoredRuntimeEventRow, StoredRuntimeItemRow, StoredRuntimeSpanRow, StoredUsageLedgerRow,
};

pub(super) fn map_compiled_plan_record(row: PgRow) -> Result<domain::CompiledPlanRecord> {
    Ok(PgOrchestrationRuntimeMapper::to_compiled_plan_record(
        StoredCompiledPlanRow {
            id: row.get("id"),
            flow_id: row.get("flow_id"),
            flow_draft_id: row.get("flow_draft_id"),
            schema_version: row.get("schema_version"),
            document_hash: row.get("document_hash"),
            document_updated_at: row.get("document_updated_at"),
            plan: row.get("plan"),
            created_by: row.get("created_by"),
            created_at: row.get("created_at"),
            updated_at: row.get("updated_at"),
        },
    ))
}

pub(super) fn map_flow_run_record(row: PgRow) -> Result<domain::FlowRunRecord> {
    PgOrchestrationRuntimeMapper::to_flow_run_record(StoredFlowRunRow {
        id: row.get("id"),
        application_id: row.get("application_id"),
        flow_id: row.get("flow_id"),
        flow_draft_id: row.get("flow_draft_id"),
        compiled_plan_id: row.get::<Option<Uuid>, _>("compiled_plan_id"),
        debug_session_id: row.get("debug_session_id"),
        flow_schema_version: row.get("flow_schema_version"),
        document_hash: row.get("document_hash"),
        run_mode: row.get("run_mode"),
        target_node_id: row.get("target_node_id"),
        title: row.get("title"),
        status: row.get("status"),
        input_payload: row.get("input_payload"),
        output_payload: row.get("output_payload"),
        error_payload: row.get("error_payload"),
        created_by: row.get("created_by"),
        authorized_account: row.get("authorized_account"),
        api_key_id: row.get("api_key_id"),
        publication_version_id: row.get("publication_version_id"),
        external_user: row.get("external_user"),
        external_conversation_id: row.get("external_conversation_id"),
        external_trace_id: row.get("external_trace_id"),
        compatibility_mode: row.get("compatibility_mode"),
        idempotency_key: row.get("idempotency_key"),
        started_at: row.get("started_at"),
        finished_at: row.get("finished_at"),
        created_at: row.get("created_at"),
        updated_at: row.get("updated_at"),
    })
}

pub(super) fn map_node_run_record(row: PgRow) -> Result<domain::NodeRunRecord> {
    PgOrchestrationRuntimeMapper::to_node_run_record(StoredNodeRunRow {
        id: row.get("id"),
        flow_run_id: row.get("flow_run_id"),
        node_id: row.get("node_id"),
        node_type: row.get("node_type"),
        node_alias: row.get("node_alias"),
        status: row.get("status"),
        input_payload: row.get("input_payload"),
        output_payload: row.get("output_payload"),
        error_payload: row.get("error_payload"),
        metrics_payload: row.get("metrics_payload"),
        debug_payload: row.get("debug_payload"),
        started_at: row.get("started_at"),
        finished_at: row.get("finished_at"),
    })
}

pub(super) fn map_checkpoint_record(row: PgRow) -> domain::CheckpointRecord {
    PgOrchestrationRuntimeMapper::to_checkpoint_record(StoredCheckpointRow {
        id: row.get("id"),
        flow_run_id: row.get("flow_run_id"),
        node_run_id: row.get("node_run_id"),
        status: row.get("status"),
        reason: row.get("reason"),
        locator_payload: row.get("locator_payload"),
        variable_snapshot: row.get("variable_snapshot"),
        external_ref_payload: row.get("external_ref_payload"),
        created_at: row.get("created_at"),
    })
}

pub(super) fn fetch_checkpoint_record(row: PgRow) -> domain::CheckpointRecord {
    PgOrchestrationRuntimeMapper::to_checkpoint_record(StoredCheckpointRow {
        id: row.get("id"),
        flow_run_id: row.get("flow_run_id"),
        node_run_id: row.get("node_run_id"),
        status: row.get("status"),
        reason: row.get("reason"),
        locator_payload: row.get("locator_payload"),
        variable_snapshot: row.get("variable_snapshot"),
        external_ref_payload: row.get("external_ref_payload"),
        created_at: row.get("created_at"),
    })
}

pub(super) fn map_callback_task_record(row: PgRow) -> Result<domain::CallbackTaskRecord> {
    PgOrchestrationRuntimeMapper::to_callback_task_record(StoredCallbackTaskRow {
        id: row.get("id"),
        flow_run_id: row.get("flow_run_id"),
        node_run_id: row.get("node_run_id"),
        callback_kind: row.get("callback_kind"),
        status: row.get("status"),
        request_payload: row.get("request_payload"),
        response_payload: row.get("response_payload"),
        external_ref_payload: row.get("external_ref_payload"),
        created_at: row.get("created_at"),
        completed_at: row.get("completed_at"),
    })
}

pub(super) fn map_flow_run_callback_resume_attempt_record(
    row: &PgRow,
) -> Result<domain::FlowRunCallbackResumeAttemptRecord> {
    Ok(domain::FlowRunCallbackResumeAttemptRecord {
        id: row.get("id"),
        flow_run_id: row.get("flow_run_id"),
        callback_task_id: row.get("callback_task_id"),
        source: row.get("source"),
        status: parse_flow_run_callback_resume_attempt_status(
            row.get::<String, _>("status").as_str(),
        )?,
        response_payload: row.get("response_payload"),
        idempotency_key: row.get("idempotency_key"),
        error_payload: row.get("error_payload"),
        created_at: row.get("created_at"),
        updated_at: row.get("updated_at"),
        completed_at: row.get("completed_at"),
    })
}

pub(super) fn map_run_event_record(row: PgRow) -> domain::RunEventRecord {
    PgOrchestrationRuntimeMapper::to_run_event_record(StoredRunEventRow {
        id: row.get("id"),
        flow_run_id: row.get("flow_run_id"),
        node_run_id: row.get("node_run_id"),
        sequence: row.get("sequence"),
        event_type: row.get("event_type"),
        payload: row.get("payload"),
        created_at: row.get("created_at"),
    })
}

pub(super) fn map_runtime_span_record(row: PgRow) -> Result<domain::RuntimeSpanRecord> {
    PgOrchestrationRuntimeMapper::to_runtime_span_record(StoredRuntimeSpanRow {
        id: row.get("id"),
        flow_run_id: row.get("flow_run_id"),
        node_run_id: row.get("node_run_id"),
        parent_span_id: row.get("parent_span_id"),
        kind: row.get("kind"),
        name: row.get("name"),
        status: row.get("status"),
        capability_id: row.get("capability_id"),
        input_ref: row.get("input_ref"),
        output_ref: row.get("output_ref"),
        error_payload: row.get("error_payload"),
        metadata: row.get("metadata"),
        started_at: row.get("started_at"),
        finished_at: row.get("finished_at"),
    })
}

pub(super) fn map_runtime_event_record(row: PgRow) -> Result<domain::RuntimeEventRecord> {
    PgOrchestrationRuntimeMapper::to_runtime_event_record(StoredRuntimeEventRow {
        id: row.get("id"),
        flow_run_id: row.get("flow_run_id"),
        node_run_id: row.get("node_run_id"),
        span_id: row.get("span_id"),
        parent_span_id: row.get("parent_span_id"),
        sequence: row.get("sequence"),
        event_type: row.get("event_type"),
        layer: row.get("layer"),
        source: row.get("source"),
        trust_level: row.get("trust_level"),
        item_id: row.get("item_id"),
        ledger_ref: row.get("ledger_ref"),
        payload: row.get("payload"),
        visibility: row.get("visibility"),
        durability: row.get("durability"),
        created_at: row.get("created_at"),
    })
}

pub(super) fn map_runtime_item_record(row: PgRow) -> Result<domain::RuntimeItemRecord> {
    PgOrchestrationRuntimeMapper::to_runtime_item_record(StoredRuntimeItemRow {
        id: row.get("id"),
        flow_run_id: row.get("flow_run_id"),
        span_id: row.get("span_id"),
        kind: row.get("kind"),
        status: row.get("status"),
        source_event_id: row.get("source_event_id"),
        input_ref: row.get("input_ref"),
        output_ref: row.get("output_ref"),
        usage_ledger_id: row.get("usage_ledger_id"),
        trust_level: row.get("trust_level"),
        created_at: row.get("created_at"),
        updated_at: row.get("updated_at"),
    })
}

pub(super) fn map_context_projection_record(row: PgRow) -> domain::ContextProjectionRecord {
    PgOrchestrationRuntimeMapper::to_context_projection_record(StoredContextProjectionRow {
        id: row.get("id"),
        flow_run_id: row.get("flow_run_id"),
        node_run_id: row.get("node_run_id"),
        llm_turn_span_id: row.get("llm_turn_span_id"),
        projection_kind: row.get("projection_kind"),
        merge_stage_ref: row.get("merge_stage_ref"),
        source_transcript_ref: row.get("source_transcript_ref"),
        source_item_refs: row.get("source_item_refs"),
        compaction_event_id: row.get("compaction_event_id"),
        summary_version: row.get("summary_version"),
        model_input_ref: row.get("model_input_ref"),
        model_input_hash: row.get("model_input_hash"),
        compacted_summary_ref: row.get("compacted_summary_ref"),
        previous_projection_id: row.get("previous_projection_id"),
        token_estimate: row.get("token_estimate"),
        provider_continuation_metadata: row.get("provider_continuation_metadata"),
        created_at: row.get("created_at"),
    })
}

pub(super) fn map_usage_ledger_record(row: PgRow) -> Result<domain::UsageLedgerRecord> {
    PgOrchestrationRuntimeMapper::to_usage_ledger_record(StoredUsageLedgerRow {
        id: row.get("id"),
        flow_run_id: row.get("flow_run_id"),
        node_run_id: row.get("node_run_id"),
        span_id: row.get("span_id"),
        failover_attempt_id: row.get("failover_attempt_id"),
        provider_instance_id: row.get("provider_instance_id"),
        gateway_route_id: row.get("gateway_route_id"),
        model_id: row.get("model_id"),
        upstream_model_id: row.get("upstream_model_id"),
        upstream_request_id: row.get("upstream_request_id"),
        input_tokens: row.get("input_tokens"),
        cached_input_tokens: row.get("cached_input_tokens"),
        output_tokens: row.get("output_tokens"),
        reasoning_output_tokens: row.get("reasoning_output_tokens"),
        total_tokens: row.get("total_tokens"),
        input_cache_hit_tokens: row.get("input_cache_hit_tokens"),
        input_cache_miss_tokens: row.get("input_cache_miss_tokens"),
        cache_read_tokens: row.get("cache_read_tokens"),
        cache_write_tokens: row.get("cache_write_tokens"),
        price_snapshot: row.get("price_snapshot"),
        cost_snapshot: row.get("cost_snapshot"),
        usage_status: row.get("usage_status"),
        raw_usage: row.get("raw_usage"),
        normalized_usage: row.get("normalized_usage"),
        created_at: row.get("created_at"),
    })
}

pub(super) fn map_cost_ledger_record(row: PgRow) -> domain::CostLedgerRecord {
    PgOrchestrationRuntimeMapper::to_cost_ledger_record(StoredCostLedgerRow {
        id: row.get("id"),
        flow_run_id: row.get("flow_run_id"),
        span_id: row.get("span_id"),
        usage_ledger_id: row.get("usage_ledger_id"),
        workspace_id: row.get("workspace_id"),
        provider_instance_id: row.get("provider_instance_id"),
        provider_account_id: row.get("provider_account_id"),
        gateway_route_id: row.get("gateway_route_id"),
        model_id: row.get("model_id"),
        upstream_model_id: row.get("upstream_model_id"),
        price_snapshot: row.get("price_snapshot"),
        raw_cost: row.get("raw_cost"),
        normalized_cost: row.get("normalized_cost"),
        settlement_currency: row.get("settlement_currency"),
        cost_source: row.get("cost_source"),
        cost_status: row.get("cost_status"),
        created_at: row.get("created_at"),
    })
}

pub(super) fn map_credit_ledger_record(row: PgRow) -> domain::CreditLedgerRecord {
    PgOrchestrationRuntimeMapper::to_credit_ledger_record(StoredCreditLedgerRow {
        id: row.get("id"),
        workspace_id: row.get("workspace_id"),
        user_id: row.get("user_id"),
        application_id: row.get("application_id"),
        agent_id: row.get("agent_id"),
        flow_run_id: row.get("flow_run_id"),
        span_id: row.get("span_id"),
        cost_ledger_id: row.get("cost_ledger_id"),
        transaction_type: row.get("transaction_type"),
        amount: row.get("amount"),
        balance_after: row.get("balance_after"),
        credit_unit: row.get("credit_unit"),
        reason: row.get("reason"),
        idempotency_key: row.get("idempotency_key"),
        status: row.get("status"),
        created_at: row.get("created_at"),
    })
}

pub(super) fn map_billing_session_record(row: PgRow) -> Result<domain::BillingSessionRecord> {
    PgOrchestrationRuntimeMapper::to_billing_session_record(StoredBillingSessionRow {
        id: row.get("id"),
        workspace_id: row.get("workspace_id"),
        flow_run_id: row.get("flow_run_id"),
        client_request_id: row.get("client_request_id"),
        idempotency_key: row.get("idempotency_key"),
        route_id: row.get("route_id"),
        provider_account_id: row.get("provider_account_id"),
        status: row.get("status"),
        reserved_credit_ledger_id: row.get("reserved_credit_ledger_id"),
        settled_credit_ledger_id: row.get("settled_credit_ledger_id"),
        refund_credit_ledger_id: row.get("refund_credit_ledger_id"),
        metadata: row.get("metadata"),
        created_at: row.get("created_at"),
        updated_at: row.get("updated_at"),
    })
}

pub(super) fn map_audit_hash_record(row: PgRow) -> domain::AuditHashRecord {
    PgOrchestrationRuntimeMapper::to_audit_hash_record(StoredAuditHashRow {
        id: row.get("id"),
        flow_run_id: row.get("flow_run_id"),
        fact_table: row.get("fact_table"),
        fact_id: row.get("fact_id"),
        prev_hash: row.get("prev_hash"),
        row_hash: row.get("row_hash"),
        created_at: row.get("created_at"),
    })
}

pub(super) fn map_model_failover_attempt_ledger_record(
    row: PgRow,
) -> domain::ModelFailoverAttemptLedgerRecord {
    PgOrchestrationRuntimeMapper::to_model_failover_attempt_ledger_record(
        StoredModelFailoverAttemptLedgerRow {
            id: row.get("id"),
            flow_run_id: row.get("flow_run_id"),
            node_run_id: row.get("node_run_id"),
            llm_turn_span_id: row.get("llm_turn_span_id"),
            queue_snapshot_id: row.get("queue_snapshot_id"),
            attempt_index: row.get("attempt_index"),
            provider_instance_id: row.get("provider_instance_id"),
            provider_code: row.get("provider_code"),
            upstream_model_id: row.get("upstream_model_id"),
            protocol: row.get("protocol"),
            request_ref: row.get("request_ref"),
            request_hash: row.get("request_hash"),
            started_at: row.get("started_at"),
            first_token_at: row.get("first_token_at"),
            finished_at: row.get("finished_at"),
            status: row.get("status"),
            failed_after_first_token: row.get("failed_after_first_token"),
            upstream_request_id: row.get("upstream_request_id"),
            error_code: row.get("error_code"),
            error_message_ref: row.get("error_message_ref"),
            usage_ledger_id: row.get("usage_ledger_id"),
            cost_ledger_id: row.get("cost_ledger_id"),
            response_ref: row.get("response_ref"),
        },
    )
}

pub(super) fn map_capability_invocation_record(row: PgRow) -> domain::CapabilityInvocationRecord {
    PgOrchestrationRuntimeMapper::to_capability_invocation_record(StoredCapabilityInvocationRow {
        id: row.get("id"),
        flow_run_id: row.get("flow_run_id"),
        span_id: row.get("span_id"),
        capability_id: row.get("capability_id"),
        requested_by_span_id: row.get("requested_by_span_id"),
        requester_kind: row.get("requester_kind"),
        arguments_ref: row.get("arguments_ref"),
        authorization_status: row.get("authorization_status"),
        authorization_reason: row.get("authorization_reason"),
        result_ref: row.get("result_ref"),
        normalized_result: row.get("normalized_result"),
        started_at: row.get("started_at"),
        finished_at: row.get("finished_at"),
        error_payload: row.get("error_payload"),
        created_at: row.get("created_at"),
    })
}

pub(super) fn map_application_run_summary(row: PgRow) -> Result<domain::ApplicationRunSummary> {
    PgOrchestrationRuntimeMapper::to_application_run_summary(StoredApplicationRunSummaryRow {
        id: row.get("id"),
        run_mode: row.get("run_mode"),
        status: row.get("status"),
        target_node_id: row.get("target_node_id"),
        title: row.get("title"),
        input_payload: row.get("input_payload"),
        external_user: row.get("external_user"),
        created_by: row.get("created_by"),
        authorized_account: row.get("authorized_account"),
        api_key_id: row.get("api_key_id"),
        publication_version_id: row.get("publication_version_id"),
        external_conversation_id: row.get("external_conversation_id"),
        external_trace_id: row.get("external_trace_id"),
        compatibility_mode: row.get("compatibility_mode"),
        idempotency_key: row.get("idempotency_key"),
        started_at: row.get("started_at"),
        finished_at: row.get("finished_at"),
        created_at: row.get("created_at"),
        updated_at: row.get("updated_at"),
    })
}

pub(super) fn map_application_run_log_summary(
    row: PgRow,
) -> Result<domain::ApplicationRunLogSummary> {
    PgOrchestrationRuntimeMapper::to_application_run_log_summary(
        StoredApplicationRunLogSummaryRow {
            run: StoredApplicationRunSummaryRow {
                id: row.get("id"),
                run_mode: row.get("run_mode"),
                status: row.get("status"),
                target_node_id: row.get("target_node_id"),
                title: row.get("title"),
                input_payload: row.get("input_payload"),
                external_user: row.get("external_user"),
                created_by: row.get("created_by"),
                authorized_account: row.get("authorized_account"),
                api_key_id: row.get("api_key_id"),
                publication_version_id: row.get("publication_version_id"),
                external_conversation_id: row.get("external_conversation_id"),
                external_trace_id: row.get("external_trace_id"),
                compatibility_mode: row.get("compatibility_mode"),
                idempotency_key: row.get("idempotency_key"),
                started_at: row.get("started_at"),
                finished_at: row.get("finished_at"),
                created_at: row.get("created_at"),
                updated_at: row.get("updated_at"),
            },
            count_tokens_input_tokens: None,
            total_tokens: row.get("total_tokens"),
            input_tokens: row.get("input_tokens"),
            output_tokens: row.get("output_tokens"),
            input_cache_hit_tokens: row.get("input_cache_hit_tokens"),
            input_cache_hit_rate: row.get("input_cache_hit_rate"),
            unique_node_count: row.get("unique_node_count"),
            tool_callback_count: row.get("tool_callback_count"),
        },
    )
}

pub(super) fn map_application_conversation_run_summary(
    row: PgRow,
) -> Result<domain::ApplicationConversationRunSummary> {
    Ok(domain::ApplicationConversationRunSummary {
        id: row.get("id"),
        status: parse_flow_run_status(row.get::<String, _>("status").as_str())?,
        query: row.get("query"),
        model: row.get("model"),
        answer: row.get("answer"),
        started_at: row.get("started_at"),
        finished_at: row.get("finished_at"),
    })
}
