use super::*;

pub(super) fn build_ready_provider_flow_document(
    flow_id: Uuid,
    _provider_instance_id: Uuid,
) -> Value {
    json!({
        "schemaVersion": "1flowbase.flow/v2",
        "meta": { "flowId": flow_id.to_string(), "name": "Support Agent", "description": "", "tags": [] },
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
                    "id": "node-llm",
                    "type": "llm",
                    "alias": "LLM",
                    "description": "",
                    "containerId": null,
                    "position": { "x": 240, "y": 0 },
                    "configVersion": 1,
                    "config": {
                        "model_provider": {
                            "provider_code": "fixture_provider",
                            "model_id": "gpt-5.4-mini"
                        },
                        "temperature": 0.2
                    },
                    "bindings": {
                        "prompt_messages": { "kind": "prompt_messages", "value": [{ "id": "user-1", "role": "user", "content": { "kind": "templated_text", "value": "{{node-start.query}}" } }] }
                    },
                    "outputs": [{ "key": "text", "title": "模型输出", "valueType": "string" }]
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
                        "answer_template": { "kind": "selector", "value": ["node-llm", "text"] }
                    },
                    "outputs": [{ "key": "answer", "title": "对话输出", "valueType": "string" }]
                }
            ],
            "edges": [
                { "id": "edge-start-llm", "source": "node-start", "target": "node-llm", "sourceHandle": null, "targetHandle": null, "containerId": null, "points": [] },
                { "id": "edge-llm-answer", "source": "node-llm", "target": "node-answer", "sourceHandle": null, "targetHandle": null, "containerId": null, "points": [] }
            ]
        },
        "editor": { "viewport": { "x": 0, "y": 0, "zoom": 1 }, "annotations": [], "activeContainerPath": [] }
    })
}

pub(super) fn build_human_input_flow_document(flow_id: Uuid, _provider_instance_id: Uuid) -> Value {
    json!({
        "schemaVersion": "1flowbase.flow/v2",
        "meta": { "flowId": flow_id.to_string(), "name": "Support Agent", "description": "", "tags": [] },
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
                    "id": "node-llm",
                    "type": "llm",
                    "alias": "LLM",
                    "description": "",
                    "containerId": null,
                    "position": { "x": 240, "y": 0 },
                    "configVersion": 1,
                    "config": {
                        "model_provider": {
                            "provider_code": "fixture_provider",
                            "model_id": "gpt-5.4-mini"
                        },
                        "temperature": 0.2
                    },
                    "bindings": {
                        "prompt_messages": { "kind": "prompt_messages", "value": [{ "id": "user-1", "role": "user", "content": { "kind": "templated_text", "value": "{{node-start.query}}" } }] }
                    },
                    "outputs": [{ "key": "text", "title": "模型输出", "valueType": "string" }]
                },
                {
                    "id": "node-human",
                    "type": "human_input",
                    "alias": "Human Input",
                    "description": "",
                    "containerId": null,
                    "position": { "x": 480, "y": 0 },
                    "configVersion": 1,
                    "config": {},
                    "bindings": {
                        "prompt": { "kind": "templated_text", "value": "请审核：{{ node-llm.text }}" }
                    },
                    "outputs": [{ "key": "input", "title": "人工输入", "valueType": "string" }]
                },
                {
                    "id": "node-answer",
                    "type": "answer",
                    "alias": "Answer",
                    "description": "",
                    "containerId": null,
                    "position": { "x": 720, "y": 0 },
                    "configVersion": 1,
                    "config": {},
                    "bindings": {
                        "answer_template": { "kind": "selector", "value": ["node-human", "input"] }
                    },
                    "outputs": [{ "key": "answer", "title": "对话输出", "valueType": "string" }]
                }
            ],
            "edges": [
                { "id": "edge-start-llm", "source": "node-start", "target": "node-llm", "sourceHandle": null, "targetHandle": null, "containerId": null, "points": [] },
                { "id": "edge-llm-human", "source": "node-llm", "target": "node-human", "sourceHandle": null, "targetHandle": null, "containerId": null, "points": [] },
                { "id": "edge-human-answer", "source": "node-human", "target": "node-answer", "sourceHandle": null, "targetHandle": null, "containerId": null, "points": [] }
            ]
        },
        "editor": { "viewport": { "x": 0, "y": 0, "zoom": 1 }, "annotations": [], "activeContainerPath": [] }
    })
}

pub(super) fn build_second_llm_failure_flow_document(
    flow_id: Uuid,
    _provider_instance_id: Uuid,
) -> Value {
    json!({
        "schemaVersion": "1flowbase.flow/v2",
        "meta": { "flowId": flow_id.to_string(), "name": "Support Agent", "description": "", "tags": [] },
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
                    "id": "node-llm",
                    "type": "llm",
                    "alias": "LLM",
                    "description": "",
                    "containerId": null,
                    "position": { "x": 240, "y": 0 },
                    "configVersion": 1,
                    "config": {
                        "model_provider": {
                            "provider_code": "fixture_provider",
                            "model_id": "gpt-5.4-mini"
                        }
                    },
                    "bindings": {
                        "prompt_messages": { "kind": "prompt_messages", "value": [{ "id": "user-1", "role": "user", "content": { "kind": "templated_text", "value": "{{node-start.query}}" } }] }
                    },
                    "outputs": [{ "key": "text", "title": "模型输出", "valueType": "string" }]
                },
                {
                    "id": "node-llm-2",
                    "type": "llm",
                    "alias": "LLM2",
                    "description": "",
                    "containerId": null,
                    "position": { "x": 480, "y": 0 },
                    "configVersion": 1,
                    "config": {
                        "model_provider": {
                            "provider_code": "fixture_provider",
                            "model_id": "gpt-5.4-mini"
                        }
                    },
                    "bindings": {
                        "prompt_messages": { "kind": "prompt_messages", "value": [{ "id": "user-2", "role": "user", "content": { "kind": "templated_text", "value": "{{node-llm.text}}" } }] }
                    },
                    "outputs": [{ "key": "text", "title": "模型输出", "valueType": "string" }]
                },
                {
                    "id": "node-answer",
                    "type": "answer",
                    "alias": "Answer",
                    "description": "",
                    "containerId": null,
                    "position": { "x": 720, "y": 0 },
                    "configVersion": 1,
                    "config": {},
                    "bindings": {
                        "answer_template": { "kind": "templated_text", "value": "{{ node-llm.text }}\n----\n{{ node-llm-2.text }}" }
                    },
                    "outputs": [{ "key": "answer", "title": "对话输出", "valueType": "string" }]
                }
            ],
            "edges": [
                { "id": "edge-start-llm", "source": "node-start", "target": "node-llm", "sourceHandle": null, "targetHandle": null, "containerId": null, "points": [] },
                { "id": "edge-llm-llm2", "source": "node-llm", "target": "node-llm-2", "sourceHandle": null, "targetHandle": null, "containerId": null, "points": [] },
                { "id": "edge-llm2-answer", "source": "node-llm-2", "target": "node-answer", "sourceHandle": null, "targetHandle": null, "containerId": null, "points": [] }
            ]
        },
        "editor": { "viewport": { "x": 0, "y": 0, "zoom": 1 }, "annotations": [], "activeContainerPath": [] }
    })
}

pub(super) fn build_callback_flow_document(flow_id: Uuid, _provider_instance_id: Uuid) -> Value {
    json!({
        "schemaVersion": "1flowbase.flow/v2",
        "meta": { "flowId": flow_id.to_string(), "name": "Support Agent", "description": "", "tags": [] },
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
                    "id": "node-tool",
                    "type": "tool",
                    "alias": "Tool",
                    "description": "",
                    "containerId": null,
                    "position": { "x": 240, "y": 0 },
                    "configVersion": 1,
                    "config": { "tool_name": "lookup_order" },
                    "bindings": {
                        "order_id": { "kind": "selector", "value": ["node-start", "query"] }
                    },
                    "outputs": [{ "key": "result", "title": "工具输出", "valueType": "json" }]
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
                        "answer_template": { "kind": "selector", "value": ["node-tool", "result"] }
                    },
                    "outputs": [{ "key": "answer", "title": "对话输出", "valueType": "json" }]
                }
            ],
            "edges": [
                { "id": "edge-start-tool", "source": "node-start", "target": "node-tool", "sourceHandle": null, "targetHandle": null, "containerId": null, "points": [] },
                { "id": "edge-tool-answer", "source": "node-tool", "target": "node-answer", "sourceHandle": null, "targetHandle": null, "containerId": null, "points": [] }
            ]
        },
        "editor": { "viewport": { "x": 0, "y": 0, "zoom": 1 }, "annotations": [], "activeContainerPath": [] }
    })
}

pub(super) fn build_plugin_capability_flow_document(
    flow_id: Uuid,
    _provider_instance_id: Uuid,
) -> Value {
    json!({
        "schemaVersion": "1flowbase.flow/v2",
        "meta": { "flowId": flow_id.to_string(), "name": "Support Agent", "description": "", "tags": [] },
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
                    "id": "node-plugin",
                    "type": "plugin_node",
                    "alias": "Plugin Node",
                    "description": "",
                    "containerId": null,
                    "position": { "x": 240, "y": 0 },
                    "configVersion": 1,
                    "plugin_unique_identifier": "fixture_capability",
                    "package_id": "fixture_capability@0.1.0",
                    "plugin_id": "fixture_capability@0.1.0",
                    "plugin_version": "0.1.0",
                    "contribution_code": "fixture_action",
                    "node_shell": "action",
                    "schema_version": "1flowbase.node-contribution/v2",
                    "contribution_checksum": "sha256:contribution",
                    "compiled_contribution_hash": "sha256:compiled",
                    "output_schema_snapshot": {
                        "outputs": [{ "key": "answer", "title": "回答", "valueType": "string" }]
                    },
                    "config": { "prompt": "Hello {{ node-start.query }}" },
                    "bindings": {
                        "query": { "kind": "selector", "value": ["node-start", "query"] }
                    },
                    "outputs": [{ "key": "answer", "title": "回答", "valueType": "string" }]
                }
            ],
            "edges": [
                { "id": "edge-start-plugin", "source": "node-start", "target": "node-plugin", "sourceHandle": null, "targetHandle": null, "containerId": null, "points": [] }
            ]
        },
        "editor": { "viewport": { "x": 0, "y": 0, "zoom": 1 }, "annotations": [], "activeContainerPath": [] }
    })
}
