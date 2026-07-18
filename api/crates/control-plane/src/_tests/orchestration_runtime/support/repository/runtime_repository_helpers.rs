use super::*;

pub(super) fn force_status_before_next_flow_update(
    inner: &mut InMemoryOrchestrationRuntimeState,
    flow_run_id: Uuid,
) {
    let Some((race_flow_run_id, status)) = inner.status_before_next_flow_update.take() else {
        return;
    };
    if race_flow_run_id == flow_run_id {
        if let Some(stored) = inner.flow_runs_by_id.get_mut(&flow_run_id) {
            stored.status = status;
        }
    } else {
        inner.status_before_next_flow_update = Some((race_flow_run_id, status));
    }
}

pub(super) fn force_stream_terminal_failure_before_next_flow_update(
    inner: &mut InMemoryOrchestrationRuntimeState,
    flow_run_id: Uuid,
) {
    let Some(race_flow_run_id) = inner.stream_terminal_failure_before_next_flow_update.take()
    else {
        return;
    };
    if race_flow_run_id == flow_run_id {
        if let Some(stored) = inner.flow_runs_by_id.get_mut(&flow_run_id) {
            let now = OffsetDateTime::now_utc();
            stored.status = domain::FlowRunStatus::Failed;
            stored.error_payload = Some(serde_json::json!({
                "code": "stream_terminal_missing",
                "message": "runtime event stream ended without a terminal event"
            }));
            stored.finished_at = Some(now);
            stored.updated_at = now;
        }
    } else {
        inner.stream_terminal_failure_before_next_flow_update = Some(race_flow_run_id);
    }
}

pub(super) fn flow_run_record_from_create_input(
    input: &CreateFlowRunInput,
) -> domain::FlowRunRecord {
    domain::FlowRunRecord {
        id: Uuid::now_v7(),
        application_id: input.application_id,
        flow_id: input.flow_id,
        draft_id: input.flow_draft_id,
        compiled_plan_id: Some(input.compiled_plan_id),
        debug_session_id: input.debug_session_id.clone(),
        flow_schema_version: input.flow_schema_version.clone(),
        document_hash: input.document_hash.clone(),
        run_mode: input.run_mode,
        target_node_id: input.target_node_id.clone(),
        title: input.title.clone(),
        status: input.status,
        input_payload: input.input_payload.clone(),
        output_payload: json!({}),
        error_payload: None,
        created_by: input.actor_user_id,
        authorized_account: None,
        api_key_id: input.api_key_id,
        publication_version_id: input.publication_version_id,
        external_user: input.external_user.clone(),
        external_conversation_id: input.external_conversation_id.clone(),
        external_trace_id: input.external_trace_id.clone(),
        compatibility_mode: input.compatibility_mode.clone(),
        idempotency_key: input.idempotency_key.clone(),
        started_at: input.started_at,
        finished_at: None,
        created_at: input.started_at,
        updated_at: input.started_at,
    }
}

pub(super) fn flow_run_shell_record_from_input(
    input: &crate::ports::CreateFlowRunShellInput,
) -> domain::FlowRunRecord {
    domain::FlowRunRecord {
        id: Uuid::now_v7(),
        application_id: input.application_id,
        flow_id: input.flow_id,
        draft_id: input.flow_draft_id,
        compiled_plan_id: None,
        debug_session_id: input.debug_session_id.clone(),
        flow_schema_version: input.flow_schema_version.clone(),
        document_hash: input.document_hash.clone(),
        run_mode: input.run_mode,
        target_node_id: input.target_node_id.clone(),
        title: input.title.clone(),
        status: input.status,
        input_payload: input.input_payload.clone(),
        output_payload: json!({}),
        error_payload: None,
        created_by: input.actor_user_id,
        authorized_account: None,
        api_key_id: input.api_key_id,
        publication_version_id: input.publication_version_id,
        external_user: input.external_user.clone(),
        external_conversation_id: input.external_conversation_id.clone(),
        external_trace_id: input.external_trace_id.clone(),
        compatibility_mode: input.compatibility_mode.clone(),
        idempotency_key: input.idempotency_key.clone(),
        started_at: input.started_at,
        finished_at: None,
        created_at: input.started_at,
        updated_at: input.started_at,
    }
}

pub(super) fn node_run_record_from_create_input(
    input: &CreateNodeRunInput,
) -> domain::NodeRunRecord {
    domain::NodeRunRecord {
        id: Uuid::now_v7(),
        flow_run_id: input.flow_run_id,
        node_id: input.node_id.clone(),
        node_type: input.node_type.clone(),
        node_alias: input.node_alias.clone(),
        status: input.status,
        input_payload: input.input_payload.clone(),
        output_payload: json!({}),
        error_payload: None,
        metrics_payload: json!({}),
        debug_payload: input.debug_payload.clone(),
        started_at: input.started_at,
        finished_at: None,
    }
}
