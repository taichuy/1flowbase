use serde_json::json;
use uuid::Uuid;

use crate::ApplicationType;

pub const FLOW_SCHEMA_VERSION: &str = "1flowbase.flow/v2";
pub const WORKFLOW_SYNC_TIMEOUT_MS: u32 = 30_000;

pub fn default_flow_document(flow_id: Uuid) -> serde_json::Value {
    default_agent_flow_document(flow_id)
}

pub fn default_flow_document_for_application(
    application_type: ApplicationType,
    flow_id: Uuid,
) -> serde_json::Value {
    match application_type {
        ApplicationType::AgentFlow => default_agent_flow_document(flow_id),
        ApplicationType::Workflow => default_workflow_document(flow_id),
    }
}

pub fn default_agent_flow_document(flow_id: Uuid) -> serde_json::Value {
    json!({
        "schemaVersion": FLOW_SCHEMA_VERSION,
        "meta": {
            "flowId": flow_id.to_string(),
            "name": "Untitled agentFlow",
            "description": "",
            "tags": [],
        },
        "graph": {
            "nodes": [
                {
                    "id": "node-start",
                    "type": "start",
                    "alias": "Start",
                    "description": "",
                    "containerId": serde_json::Value::Null,
                    "position": { "x": 80, "y": 220 },
                    "configVersion": 1,
                    "config": {
                        "input_fields": [],
                        "model_list": [],
                    },
                    "bindings": {},
                    "outputs": [],
                },
                {
                    "id": "node-llm",
                    "type": "llm",
                    "alias": "LLM",
                    "description": "",
                    "containerId": serde_json::Value::Null,
                    "position": { "x": 360, "y": 220 },
                    "configVersion": 1,
                    "config": {
                        "model_provider": {
                            "provider_code": "",
                            "source_instance_id": "",
                            "model_id": ""
                        },
                        "llm_parameters": {
                            "schema_version": "1.0.0",
                            "items": {}
                        },
                        "response_format": {
                            "mode": "text"
                        }
                    },
                    "bindings": {
                        "prompt_messages": {
                            "kind": "prompt_messages",
                            "value": [
                                {
                                    "id": "system-1",
                                    "role": "system",
                                    "content": { "kind": "templated_text", "value": "" },
                                },
                                {
                                    "id": "user-1",
                                    "role": "user",
                                    "content": {
                                        "kind": "templated_text",
                                        "value": "{{node-start.query}}",
                                    },
                                },
                            ],
                        },
                    },
                    "outputs": [
                        { "key": "text", "title": "模型输出", "valueType": "string" },
                        { "key": "usage", "title": "用量", "valueType": "json" },
                    ],
                },
                {
                    "id": "node-answer",
                    "type": "answer",
                    "alias": "Answer",
                    "description": "",
                    "containerId": serde_json::Value::Null,
                    "position": { "x": 640, "y": 220 },
                    "configVersion": 1,
                    "config": {},
                    "bindings": {
                        "answer_template": { "kind": "templated_text", "value": "{{node-llm.text}}" },
                    },
                    "outputs": [{ "key": "answer", "title": "对话输出", "valueType": "string" }],
                },
            ],
            "edges": [
                {
                    "id": "edge-start-llm",
                    "source": "node-start",
                    "target": "node-llm",
                    "sourceHandle": serde_json::Value::Null,
                    "targetHandle": serde_json::Value::Null,
                    "containerId": serde_json::Value::Null,
                    "points": [],
                },
                {
                    "id": "edge-llm-answer",
                    "source": "node-llm",
                    "target": "node-answer",
                    "sourceHandle": serde_json::Value::Null,
                    "targetHandle": serde_json::Value::Null,
                    "containerId": serde_json::Value::Null,
                    "points": [],
                },
            ],
        },
        "editor": {
            "viewport": { "x": 0, "y": 0, "zoom": 1 },
            "annotations": [],
            "activeContainerPath": [],
        },
    })
}

pub fn default_workflow_document(flow_id: Uuid) -> serde_json::Value {
    json!({
        "schemaVersion": FLOW_SCHEMA_VERSION,
        "meta": {
            "flowId": flow_id.to_string(),
            "name": "Untitled workflow",
            "description": "",
            "tags": [],
        },
        "graph": {
            "nodes": [
                {
                    "id": "node-workflow-start",
                    "type": "workflow_start",
                    "alias": "Workflow Start",
                    "description": "",
                    "containerId": serde_json::Value::Null,
                    "position": { "x": 120, "y": 220 },
                    "configVersion": 1,
                    "config": {
                        "input_fields": [],
                        "sync_timeout_ms": WORKFLOW_SYNC_TIMEOUT_MS,
                    },
                    "bindings": {},
                    "outputs": [],
                },
                {
                    "id": "node-workflow-end",
                    "type": "workflow_end",
                    "alias": "Workflow End",
                    "description": "",
                    "containerId": serde_json::Value::Null,
                    "position": { "x": 440, "y": 220 },
                    "configVersion": 1,
                    "config": {},
                    "bindings": {},
                    "outputs": [],
                },
            ],
            "edges": [
                {
                    "id": "edge-workflow-start-end",
                    "source": "node-workflow-start",
                    "target": "node-workflow-end",
                    "sourceHandle": serde_json::Value::Null,
                    "targetHandle": serde_json::Value::Null,
                    "containerId": serde_json::Value::Null,
                    "points": [],
                },
            ],
        },
        "editor": {
            "viewport": { "x": 0, "y": 0, "zoom": 1 },
            "annotations": [],
            "activeContainerPath": [],
        },
    })
}
