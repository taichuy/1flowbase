use anyhow::{anyhow, Result};
use serde_json::json;
use sha2::{Digest, Sha256};
use time::OffsetDateTime;
use uuid::Uuid;

use crate::flow_run_title::display_flow_run_title;
use crate::ports::{
    CompleteFlowRunInput, CompleteNodeRunInput, CreateFlowRunInput, CreateNodeRunInput,
    UpsertCompiledPlanInput,
};

use super::payloads::persisted_node_output_payload;

pub(crate) fn build_compiled_plan_input(
    actor_user_id: Uuid,
    editor_state: &domain::FlowEditorState,
    compiled_plan: &orchestration_runtime::compiled_plan::CompiledPlan,
    document: &serde_json::Value,
) -> Result<UpsertCompiledPlanInput> {
    let mut plan = serde_json::to_value(compiled_plan)?;
    if let Some(plan) = plan.as_object_mut() {
        plan.insert(
            "_runtime_metadata".to_string(),
            json!({
                "document_hash": flow_document_hash(document),
            }),
        );
    }

    Ok(UpsertCompiledPlanInput {
        actor_user_id,
        flow_id: editor_state.flow.id,
        flow_draft_id: editor_state.draft.id,
        schema_version: compiled_plan.schema_version.clone(),
        document_hash: flow_document_hash(document),
        document_updated_at: editor_state.draft.updated_at,
        plan,
    })
}

pub(crate) fn flow_document_hash(document: &serde_json::Value) -> String {
    let bytes = serde_json::to_vec(document).unwrap_or_default();
    format!("sha256:{:x}", Sha256::digest(bytes))
}

pub(crate) fn flow_document_schema_version(
    editor_state: &domain::FlowEditorState,
    document: &serde_json::Value,
) -> String {
    document
        .get("schemaVersion")
        .and_then(|value| value.as_str())
        .unwrap_or(&editor_state.draft.schema_version)
        .to_string()
}

pub(super) fn build_flow_run_input(
    actor_user_id: Uuid,
    application_id: Uuid,
    editor_state: &domain::FlowEditorState,
    compiled_record: &domain::CompiledPlanRecord,
    command: &crate::orchestration_runtime::StartNodeDebugPreviewCommand,
    document: &serde_json::Value,
    started_at: OffsetDateTime,
) -> CreateFlowRunInput {
    CreateFlowRunInput {
        actor_user_id,
        application_id,
        flow_id: editor_state.flow.id,
        flow_draft_id: editor_state.draft.id,
        compiled_plan_id: compiled_record.id,
        debug_session_id: command.debug_session_id.clone().unwrap_or_default(),
        flow_schema_version: compiled_record.schema_version.clone(),
        document_hash: flow_document_hash(document),
        run_mode: domain::FlowRunMode::DebugNodePreview,
        target_node_id: Some(command.node_id.clone()),
        title: display_flow_run_title("", &command.input_payload),
        status: domain::FlowRunStatus::Running,
        input_payload: command.input_payload.clone(),
        started_at,
        api_key_id: None,
        publication_version_id: None,
        assistant_conversation_id: None,
        external_user: None,
        external_conversation_id: None,
        external_trace_id: None,
        compatibility_mode: None,
        idempotency_key: None,
    }
}

pub(super) fn build_node_run_input(
    flow_run_id: Uuid,
    compiled_plan: &orchestration_runtime::compiled_plan::CompiledPlan,
    target_node_id: &str,
    preview: &orchestration_runtime::preview_executor::NodePreviewOutcome,
    started_at: OffsetDateTime,
) -> Result<CreateNodeRunInput> {
    let node = compiled_plan
        .nodes
        .get(target_node_id)
        .ok_or_else(|| anyhow!("target node not found in compiled plan: {target_node_id}"))?;

    Ok(CreateNodeRunInput {
        flow_run_id,
        node_id: node.node_id.clone(),
        node_type: node.node_type.clone(),
        node_alias: node.alias.clone(),
        status: domain::NodeRunStatus::Running,
        input_payload: json!(preview.resolved_inputs),
        debug_payload: json!({}),
        started_at,
    })
}

pub(super) fn build_complete_node_run_input(
    node_run: &domain::NodeRunRecord,
    preview: &orchestration_runtime::preview_executor::NodePreviewOutcome,
    finished_at: OffsetDateTime,
) -> CompleteNodeRunInput {
    let stored_metrics_payload = json!({
        "output_contract_count": preview.output_contract.len(),
        "provider_events": preview.provider_events.len(),
        "runtime": preview.metrics_payload,
    });

    CompleteNodeRunInput {
        node_run_id: node_run.id,
        status: if preview.is_failed() {
            domain::NodeRunStatus::Failed
        } else {
            domain::NodeRunStatus::Succeeded
        },
        output_payload: persisted_node_output_payload(
            &preview.node_output,
            &preview.metrics_payload,
            preview.error_payload.as_ref(),
            &preview.debug_payload,
        ),
        error_payload: preview.error_payload.clone(),
        metrics_payload: stored_metrics_payload,
        debug_payload: preview.debug_payload.clone(),
        finished_at,
    }
}

pub(super) fn build_complete_flow_run_input(
    flow_run: &domain::FlowRunRecord,
    preview: &orchestration_runtime::preview_executor::NodePreviewOutcome,
    finished_at: OffsetDateTime,
) -> CompleteFlowRunInput {
    CompleteFlowRunInput {
        flow_run_id: flow_run.id,
        status: if preview.is_failed() {
            domain::FlowRunStatus::Failed
        } else {
            domain::FlowRunStatus::Succeeded
        },
        output_payload: persisted_node_output_payload(
            &preview.node_output,
            &preview.metrics_payload,
            preview.error_payload.as_ref(),
            &preview.debug_payload,
        ),
        error_payload: preview.error_payload.clone(),
        finished_at,
    }
}
