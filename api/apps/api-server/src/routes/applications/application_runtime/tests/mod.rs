use super::*;
use axum::http::HeaderValue;

fn application_trace_tree_endpoint_source<'a>(
    log_endpoint_source: &'a str,
    function_name: &str,
) -> &'a str {
    application_runtime_function_source(
        log_endpoint_source,
        &format!("pub async fn {function_name}"),
    )
}

fn application_runtime_function_source<'a>(
    log_endpoint_source: &'a str,
    function_marker: &str,
) -> &'a str {
    let start = log_endpoint_source
        .find(function_marker)
        .unwrap_or_else(|| panic!("{function_marker} source exists"));
    let remaining_source = &log_endpoint_source[start..];
    let end = remaining_source
        .find("\n#[utoipa::path(")
        .unwrap_or(remaining_source.len());

    &remaining_source[..end]
}

fn test_application_record() -> domain::ApplicationRecord {
    domain::ApplicationRecord {
        id: Uuid::now_v7(),
        workspace_id: Uuid::now_v7(),
        application_type: domain::ApplicationType::AgentFlow,
        workflow_trigger_type: None,
        name: "Support Agent".to_string(),
        description: "runtime".to_string(),
        icon: None,
        icon_type: None,
        icon_background: None,
        created_by: Uuid::now_v7(),
        updated_at: OffsetDateTime::UNIX_EPOCH,
        tags: Vec::new(),
        sections: domain::ApplicationSections {
            orchestration: domain::ApplicationOrchestrationSection {
                status: "enabled".to_string(),
                subject_kind: "flow".to_string(),
                subject_status: "draft".to_string(),
                current_subject_id: Some(Uuid::now_v7()),
                current_draft_id: Some(Uuid::now_v7()),
            },
            api: domain::ApplicationApiSection {
                status: "enabled".to_string(),
                credential_kind: "api_key".to_string(),
                invoke_routing_mode: "application".to_string(),
                invoke_path_template: None,
                api_capability_status: "enabled".to_string(),
                credentials_status: "enabled".to_string(),
            },
            logs: domain::ApplicationLogsSection {
                status: "enabled".to_string(),
                runs_capability_status: "enabled".to_string(),
                run_object_kind: "application_run".to_string(),
                log_retention_status: "default".to_string(),
            },
            monitoring: domain::ApplicationMonitoringSection {
                status: "enabled".to_string(),
                metrics_capability_status: "enabled".to_string(),
                metrics_object_kind: "application_run".to_string(),
                tracing_config_status: "default".to_string(),
            },
        },
    }
}

fn test_flow_run_record(
    application_id: Uuid,
    flow_run_id: Uuid,
    status: domain::FlowRunStatus,
    output_payload: serde_json::Value,
) -> domain::FlowRunRecord {
    domain::FlowRunRecord {
        id: flow_run_id,
        application_id,
        flow_id: Uuid::now_v7(),
        draft_id: Uuid::now_v7(),
        compiled_plan_id: Some(Uuid::now_v7()),
        debug_session_id: "debug-session".to_string(),
        flow_schema_version: "1flowbase.flow/v2".to_string(),
        document_hash: "hash".to_string(),
        run_mode: domain::FlowRunMode::DebugFlowRun,
        target_node_id: None,
        title: "天气？".to_string(),
        status,
        input_payload: serde_json::json!({
            "node-start": {
                "query": "天气？"
            }
        }),
        output_payload,
        error_payload: None,
        created_by: Uuid::now_v7(),
        authorized_account: Some("root".to_string()),
        api_key_id: None,
        publication_version_id: None,
        external_user: None,
        external_conversation_id: None,
        external_trace_id: None,
        compatibility_mode: None,
        idempotency_key: None,
        started_at: OffsetDateTime::UNIX_EPOCH,
        finished_at: None,
        created_at: OffsetDateTime::UNIX_EPOCH,
        updated_at: OffsetDateTime::UNIX_EPOCH,
    }
}

fn test_runtime_event_record(
    flow_run_id: Uuid,
    node_run_id: Option<Uuid>,
    event_type: &str,
    payload: serde_json::Value,
) -> domain::RuntimeEventRecord {
    domain::RuntimeEventRecord {
        id: Uuid::now_v7(),
        flow_run_id,
        node_run_id,
        span_id: None,
        parent_span_id: None,
        sequence: 1,
        event_type: event_type.to_string(),
        layer: domain::RuntimeEventLayer::RuntimeItem,
        source: domain::RuntimeEventSource::Host,
        trust_level: domain::RuntimeTrustLevel::HostFact,
        item_id: None,
        ledger_ref: None,
        payload,
        visibility: domain::RuntimeEventVisibility::Workspace,
        durability: domain::RuntimeEventDurability::Durable,
        created_at: OffsetDateTime::UNIX_EPOCH,
    }
}

mod conversation_context;
mod run_archive_import;
mod run_detail_response;
mod runtime_helpers;
mod start_node_response;
