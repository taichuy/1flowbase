use control_plane::orchestration_runtime::{
    CompleteCallbackTaskCommand, ContinueFlowDebugRunCommand,
    FinalizePublishedRunMissingStreamTerminalCommand, OrchestrationRuntimeService,
    PrepareFlowDebugRunCommand, StartFlowDebugRunCommand, StartNodeDebugPreviewCommand,
};
use control_plane::{
    capability_plugin_runtime::CapabilityPluginRuntimePort,
    errors::ControlPlaneError,
    ports::{
        ApplicationJsDependencySelectionRepository, ApplicationRepository, FlowRepository,
        ModelDefinitionRepository, ModelProviderRepository, NodeContributionRepository,
        OrchestrationRuntimeRepository, PluginRepository, ProviderRuntimePort,
        ReplaceApplicationJsDependencySelectionInput, RuntimeEventCloseReason,
        RuntimeEventDurability, RuntimeEventEnvelope, RuntimeEventPayload, RuntimeEventSource,
        RuntimeEventStream, UpsertDataModelSideEffectReceiptInput,
    },
};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use time::Duration;
use uuid::Uuid;

fn runtime_text_delta(run_id: Uuid, node_run_id: Uuid, text: &str) -> RuntimeEventEnvelope {
    runtime_text_delta_with_payload(
        run_id,
        1,
        serde_json::json!({
            "type": "text_delta",
            "node_run_id": node_run_id,
            "node_id": "node-llm",
            "text": text,
        }),
    )
}

fn runtime_text_delta_with_payload(
    run_id: Uuid,
    sequence: i64,
    payload: Value,
) -> RuntimeEventEnvelope {
    RuntimeEventEnvelope::new(
        run_id,
        sequence,
        RuntimeEventPayload {
            event_type: "text_delta".to_string(),
            source: RuntimeEventSource::Provider,
            durability: RuntimeEventDurability::DurableRequired,
            persist_required: true,
            trace_visible: false,
            payload,
        },
    )
}

fn runtime_reasoning_delta(run_id: Uuid, node_run_id: Uuid, text: &str) -> RuntimeEventEnvelope {
    RuntimeEventEnvelope::new(
        run_id,
        1,
        RuntimeEventPayload {
            event_type: "reasoning_delta".to_string(),
            source: RuntimeEventSource::Provider,
            durability: RuntimeEventDurability::DurableRequired,
            persist_required: true,
            trace_visible: false,
            payload: serde_json::json!({
                "type": "reasoning_delta",
                "node_run_id": node_run_id,
                "node_id": "node-llm",
                "text": text,
            }),
        },
    )
}

mod billing_plugin_nodes;
mod branching_failover;
mod callback_tasks;
mod code_nodes;
mod data_model_nodes;
mod debug_lifecycle;
mod http_request_nodes;
mod runtime_events;
mod sql_nodes;

async fn run_data_model_flow(
    service: &OrchestrationRuntimeService<impl RuntimeRepositoryBounds, impl RuntimeHostBounds>,
    actor_user_id: Uuid,
    application_id: Uuid,
    flow_id: Uuid,
    nodes: Vec<Value>,
    edges: Vec<(&str, &str)>,
) -> domain::ApplicationRunDetail {
    let started = service
        .start_flow_debug_run(StartFlowDebugRunCommand {
            actor_user_id,
            application_id,
            input_payload: json!({}),
            document_snapshot: Some(data_model_flow_document(flow_id, nodes, edges)),
            debug_session_id: None,
        })
        .await
        .expect("data model debug run should start");

    service
        .continue_flow_debug_run(ContinueFlowDebugRunCommand {
            application_id,
            flow_run_id: started.flow_run.id,
            workspace_id: Uuid::nil(),
        })
        .await
        .expect("data model debug run should return persisted detail")
}

trait RuntimeRepositoryBounds:
    ApplicationRepository
    + ApplicationJsDependencySelectionRepository
    + control_plane::ports::FileManagementRepository
    + FlowRepository
    + OrchestrationRuntimeRepository
    + ModelDefinitionRepository
    + ModelProviderRepository
    + NodeContributionRepository
    + PluginRepository
    + Clone
    + Send
    + Sync
    + 'static
{
}

impl<T> RuntimeRepositoryBounds for T where
    T: ApplicationRepository
        + ApplicationJsDependencySelectionRepository
        + control_plane::ports::FileManagementRepository
        + FlowRepository
        + OrchestrationRuntimeRepository
        + ModelDefinitionRepository
        + ModelProviderRepository
        + NodeContributionRepository
        + PluginRepository
        + Clone
        + Send
        + Sync
        + 'static
{
}

trait RuntimeHostBounds: ProviderRuntimePort + CapabilityPluginRuntimePort + Clone {}

impl<T> RuntimeHostBounds for T where T: ProviderRuntimePort + CapabilityPluginRuntimePort + Clone {}

fn node_run<'a>(
    detail: &'a domain::ApplicationRunDetail,
    node_id: &str,
) -> &'a domain::NodeRunRecord {
    detail
        .node_runs
        .iter()
        .find(|node_run| node_run.node_id == node_id)
        .unwrap_or_else(|| panic!("node run {node_id} should exist"))
}

fn assert_resolved_llm_debug_refs(
    debug_payload: &Value,
    projections: &[domain::ContextProjectionRecord],
    attempts: &[domain::ModelFailoverAttemptLedgerRecord],
    node_run_id: Uuid,
) {
    let projection = projections
        .iter()
        .find(|projection| projection.node_run_id == Some(node_run_id))
        .unwrap_or_else(|| panic!("projection for node run {node_run_id} should exist"));
    let node_attempts = attempts
        .iter()
        .filter(|attempt| attempt.node_run_id == Some(node_run_id))
        .collect::<Vec<_>>();
    let winner = node_attempts
        .iter()
        .find(|attempt| attempt.status == "succeeded");
    let expected_attempt_refs = node_attempts
        .iter()
        .map(|attempt| json!(format!("model_failover_attempt:{}", attempt.id)))
        .collect::<Vec<_>>();

    assert_eq!(
        debug_payload["context_projection_ref"],
        json!(format!("runtime_context_projection:{}", projection.id))
    );
    assert_eq!(
        debug_payload["attempt_refs"],
        Value::Array(expected_attempt_refs)
    );
    if let Some(winner) = winner {
        assert_eq!(
            debug_payload["winner_attempt_ref"],
            json!(format!("model_failover_attempt:{}", winner.id))
        );
    } else {
        assert!(debug_payload
            .get("winner_attempt_ref")
            .is_none_or(Value::is_null));
    }
    assert_no_pending_debug_ref(debug_payload);
}

fn assert_no_pending_debug_ref(value: &Value) {
    match value {
        Value::String(text) => {
            assert!(
                !text.starts_with("pending_attempt_id:")
                    && !text.starts_with("pending_projection_id:"),
                "debug payload kept unresolved observability ref: {text}"
            );
        }
        Value::Array(items) => {
            for item in items {
                assert_no_pending_debug_ref(item);
            }
        }
        Value::Object(object) => {
            for item in object.values() {
                assert_no_pending_debug_ref(item);
            }
        }
        Value::Null | Value::Bool(_) | Value::Number(_) => {}
    }
}

fn data_model_flow_document(
    flow_id: Uuid,
    data_model_nodes: Vec<Value>,
    edges: Vec<(&str, &str)>,
) -> Value {
    let mut nodes = vec![json!({
        "id": "node-start",
        "type": "start",
        "alias": "Start",
        "description": "",
        "containerId": null,
        "position": { "x": 0, "y": 0 },
        "configVersion": 1,
        "config": {},
        "bindings": {},
        "outputs": []
    })];
    nodes.extend(data_model_nodes);

    json!({
        "schemaVersion": "1flowbase.flow/v2",
        "meta": {
            "flowId": flow_id.to_string(),
            "name": "Data Model Agent",
            "description": "",
            "tags": []
        },
        "graph": {
            "nodes": nodes,
            "edges": edges.into_iter().enumerate().map(|(index, (source, target))| {
                json!({
                    "id": format!("edge-{index}"),
                    "source": source,
                    "target": target,
                    "sourceHandle": null,
                    "targetHandle": null,
                    "containerId": null,
                    "points": []
                })
            }).collect::<Vec<_>>()
        },
        "editor": {
            "viewport": { "x": 0, "y": 0, "zoom": 1 },
            "annotations": [],
            "activeContainerPath": []
        }
    })
}

fn data_model_node(id: &str, action: &str, config_patch: Value, bindings: Value) -> Value {
    let mut config = serde_json::Map::from_iter([("data_model_code".to_string(), json!("orders"))]);
    if matches!(action, "create" | "update" | "delete") {
        config.insert(
            "side_effect_policy".to_string(),
            json!("allow_with_idempotency"),
        );
    }
    if let Some(patch) = config_patch.as_object() {
        config.extend(patch.clone());
    }

    json!({
        "id": id,
        "type": data_model_node_type(action),
        "alias": format!("Data Model {}", action),
        "description": "",
        "containerId": null,
        "position": { "x": 240, "y": 0 },
        "configVersion": 1,
        "config": Value::Object(config),
        "bindings": bindings,
        "outputs": [{ "key": "record", "title": "Record", "valueType": "object" }]
    })
}

fn data_model_node_type(action: &str) -> &'static str {
    match action {
        "list" => "data_model_list",
        "get" => "data_model_get",
        "create" => "data_model_create",
        "update" => "data_model_update",
        "delete" => "data_model_delete",
        _ => panic!("unsupported data model action in test: {action}"),
    }
}

fn code_flow_document(flow_id: Uuid, source: &str) -> Value {
    let mut nodes = vec![json!({
        "id": "node-start",
        "type": "start",
        "alias": "Start",
        "description": "",
        "containerId": null,
        "position": { "x": 0, "y": 0 },
        "configVersion": 1,
        "config": {},
        "bindings": {},
        "outputs": []
    })];
    nodes.push(code_node("node-code", source));

    json!({
        "schemaVersion": "1flowbase.flow/v2",
        "meta": {
            "flowId": flow_id.to_string(),
            "name": "Code Agent",
            "description": "",
            "tags": []
        },
        "graph": {
            "nodes": nodes,
            "edges": [{
                "id": "edge-start-code",
                "source": "node-start",
                "target": "node-code",
                "sourceHandle": null,
                "targetHandle": null,
                "containerId": null,
                "points": []
            }]
        },
        "editor": {
            "viewport": { "x": 0, "y": 0, "zoom": 1 },
            "annotations": [],
            "activeContainerPath": []
        }
    })
}

fn code_flow_document_with_imports(flow_id: Uuid, source: &str, imports: Vec<&str>) -> Value {
    let mut document = code_flow_document(flow_id, source);
    document["graph"]["nodes"][1]["config"]["imports"] = json!(imports);
    document
}

fn code_to_answer_flow_document(flow_id: Uuid, source: &str) -> Value {
    let mut document = code_flow_document(flow_id, source);
    let nodes = document["graph"]["nodes"]
        .as_array_mut()
        .expect("code flow document nodes should be an array");
    nodes.push(json!({
        "id": "node-answer",
        "type": "answer",
        "alias": "Answer",
        "description": "",
        "containerId": null,
        "position": { "x": 480, "y": 0 },
        "configVersion": 1,
        "config": {},
        "bindings": {
            "answer": {
                "kind": "templated_text",
                "value": "Code said: {{node-code.result.result}}"
            }
        },
        "outputs": [{ "key": "answer", "title": "Answer", "valueType": "string" }]
    }));
    document["graph"]["edges"] = json!([
        {
            "id": "edge-start-code",
            "source": "node-start",
            "target": "node-code",
            "sourceHandle": null,
            "targetHandle": null,
            "containerId": null,
            "points": []
        },
        {
            "id": "edge-code-answer",
            "source": "node-code",
            "target": "node-answer",
            "sourceHandle": null,
            "targetHandle": null,
            "containerId": null,
            "points": []
        }
    ]);
    document
}

fn variable_assigner_to_answer_flow_document(flow_id: Uuid) -> Value {
    json!({
        "schemaVersion": "1flowbase.flow/v2",
        "meta": {
            "flowId": flow_id.to_string(),
            "name": "Conversation State Agent",
            "description": "",
            "tags": []
        },
        "variables": {
            "conversation": [{
                "name": "ApiBaseUrl",
                "valueType": "string",
                "description": ""
            }]
        },
        "graph": {
            "nodes": [
                {
                    "id": "node-start",
                    "type": "start",
                    "alias": "Start",
                    "description": "",
                    "containerId": null,
                    "position": { "x": 0, "y": 0 },
                    "configVersion": 1,
                    "config": {},
                    "bindings": {},
                    "outputs": []
                },
                {
                    "id": "node-variable",
                    "type": "variable_assigner",
                    "alias": "Set conversation",
                    "description": "",
                    "containerId": null,
                    "position": { "x": 240, "y": 0 },
                    "configVersion": 1,
                    "config": {},
                    "bindings": {
                        "operations": {
                            "kind": "state_write",
                            "value": [{
                                "path": ["conversation", "ApiBaseUrl"],
                                "operator": "set",
                                "value": {
                                    "kind": "templated_text",
                                    "value": "https://{{node-start.query}}/v1"
                                }
                            }]
                        }
                    },
                    "outputs": [{
                        "key": "ApiBaseUrl",
                        "title": "conversation.ApiBaseUrl",
                        "valueType": "string"
                    }]
                },
                {
                    "id": "node-answer",
                    "type": "answer",
                    "alias": "Answer",
                    "description": "",
                    "containerId": null,
                    "position": { "x": 480, "y": 0 },
                    "configVersion": 1,
                    "config": {},
                    "bindings": {
                        "answer": {
                            "kind": "templated_text",
                            "value": "{{conversation.ApiBaseUrl}}"
                        }
                    },
                    "outputs": [{ "key": "answer", "title": "Answer", "valueType": "string" }]
                }
            ],
            "edges": [
                {
                    "id": "edge-start-variable",
                    "source": "node-start",
                    "target": "node-variable",
                    "sourceHandle": null,
                    "targetHandle": null,
                    "containerId": null,
                    "points": []
                },
                {
                    "id": "edge-variable-answer",
                    "source": "node-variable",
                    "target": "node-answer",
                    "sourceHandle": null,
                    "targetHandle": null,
                    "containerId": null,
                    "points": []
                }
            ]
        },
        "editor": {
            "viewport": { "x": 0, "y": 0, "zoom": 1 },
            "annotations": [],
            "activeContainerPath": []
        }
    })
}

fn code_node(id: &str, source: &str) -> Value {
    json!({
        "id": id,
        "type": "code",
        "alias": "Code",
        "description": "",
        "containerId": null,
        "position": { "x": 240, "y": 0 },
        "configVersion": 1,
        "config": {
            "language": "javascript",
            "source": source,
            "entrypoint": "main"
        },
        "bindings": {
            "query": {
                "kind": "selector",
                "value": ["node-start", "query"]
            }
        },
        "outputs": [{ "key": "result", "title": "结果", "valueType": "json" }]
    })
}

fn write_js_dependency_artifact_for_test(alias: &str, artifact_source: &str) -> String {
    let path = std::env::temp_dir().join(format!(
        "1flowbase-live-debug-js-dependency-{alias}-{}.mjs",
        Uuid::now_v7()
    ));
    std::fs::write(&path, artifact_source).expect("test dependency artifact should be written");
    path.to_string_lossy().into_owned()
}

fn selector_binding<const N: usize>(path: [&str; N]) -> Value {
    let path = path.into_iter().collect::<Vec<_>>();

    json!({
        "kind": "selector",
        "value": path
    })
}
