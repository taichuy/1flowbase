use std::collections::{BTreeMap, BTreeSet};

use orchestration_runtime::compiled_plan::{
    CodeIsolationProfile, CompileIssueCode, CompiledCodeDependency, CompiledLlmRouting,
};
use orchestration_runtime::compiler::{
    FlowCompileContext, FlowCompileJsDependency, FlowCompileNodeContribution,
    FlowCompileProviderFamily, FlowCompileProviderInstance, FlowCompiler,
};
use serde_json::{json, Value};
use uuid::Uuid;

fn compile_context() -> FlowCompileContext {
    FlowCompileContext {
        workspace_id: None,
        provider_families: BTreeMap::from([(
            "fixture_provider".to_string(),
            FlowCompileProviderFamily {
                provider_code: "fixture_provider".to_string(),
                protocol: "openai_compatible".to_string(),
                is_ready: true,
                available_models: BTreeSet::from(["gpt-5.4-mini".to_string()]),
                allow_custom_models: false,
            },
        )]),
        provider_instances: BTreeMap::from([(
            "provider-selected".to_string(),
            FlowCompileProviderInstance {
                provider_instance_id: "provider-selected".to_string(),
                display_name: String::new(),
                provider_code: "fixture_provider".to_string(),
                protocol: "openai_compatible".to_string(),
                is_ready: true,
                is_runnable: true,
                included_in_main: true,
                available_models: BTreeSet::from(["gpt-5.4-mini".to_string()]),
                allow_custom_models: false,
            },
        )]),
        model_distribution_rules: BTreeMap::new(),
        node_contributions: BTreeMap::new(),
        js_dependencies: BTreeMap::new(),
    }
}

fn plugin_compile_context() -> FlowCompileContext {
    let mut context = compile_context();
    context.node_contributions.insert(
        "prompt_pack@0.1.0::0.1.0::openai_prompt::action::1flowbase.node-contribution/v2"
            .to_string(),
        FlowCompileNodeContribution {
            installation_id: Uuid::now_v7(),
            plugin_unique_identifier: "prompt_pack".to_string(),
            package_id: "prompt_pack@0.1.0".to_string(),
            plugin_id: "prompt_pack@0.1.0".to_string(),
            plugin_version: "0.1.0".to_string(),
            contribution_code: "openai_prompt".to_string(),
            node_shell: "action".to_string(),
            schema_version: "1flowbase.node-contribution/v2".to_string(),
            contribution_checksum: "sha256:contribution".to_string(),
            compiled_contribution_hash: "sha256:compiled".to_string(),
            output_schema_snapshot: vec![orchestration_runtime::compiled_plan::CompiledOutput {
                key: "answer".to_string(),
                title: "回答".to_string(),
                value_type: "string".to_string(),
                selector: Vec::new(),
                json_schema: None,
            }],
            side_effect_policy: "external_read".to_string(),
            dependency_status: "ready".to_string(),
        },
    );
    context
}

fn sample_document(flow_id: Uuid) -> serde_json::Value {
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
                        "prompt_messages": {
                            "kind": "prompt_messages",
                            "value": [
                                {
                                    "id": "system-1",
                                    "role": "system",
                                    "content": {
                                        "kind": "templated_text",
                                        "value": "You are helpful."
                                    }
                                },
                                {
                                    "id": "user-1",
                                    "role": "user",
                                    "content": {
                                        "kind": "templated_text",
                                        "value": "Question: {{node-start.query}}"
                                    }
                                }
                            ]
                        }
                    },
                    "outputs": [{ "key": "text", "title": "模型输出", "valueType": "string" }]
                }
            ],
            "edges": [
                {
                    "id": "edge-start-llm",
                    "source": "node-start",
                    "target": "node-llm",
                    "sourceHandle": null,
                    "targetHandle": null,
                    "containerId": null,
                    "points": []
                }
            ]
        },
        "editor": { "viewport": { "x": 0, "y": 0, "zoom": 1 }, "annotations": [], "activeContainerPath": [] }
    })
}

fn workflow_document(flow_id: Uuid) -> serde_json::Value {
    json!({
        "schemaVersion": "1flowbase.flow/v2",
        "meta": { "flowId": flow_id.to_string(), "name": "Ticket Workflow", "description": "", "tags": [] },
        "graph": {
            "nodes": [
                {
                    "id": "node-workflow-start",
                    "type": "workflow_start",
                    "alias": "Workflow Start",
                    "description": "",
                    "containerId": null,
                    "position": { "x": 0, "y": 0 },
                    "configVersion": 1,
                    "config": {
                        "input_fields": [
                            {
                                "key": "customer_id",
                                "label": "Customer ID",
                                "inputType": "text",
                                "valueType": "string",
                                "required": true
                            }
                        ],
                        "sync_timeout_ms": 30000
                    },
                    "bindings": {},
                    "outputs": []
                },
                {
                    "id": "node-workflow-end",
                    "type": "workflow_end",
                    "alias": "Workflow End",
                    "description": "",
                    "containerId": null,
                    "position": { "x": 240, "y": 0 },
                    "configVersion": 1,
                    "config": {},
                    "bindings": {},
                    "outputs": [
                        {
                            "key": "ticket_id",
                            "title": "Ticket ID",
                            "valueType": "string"
                        }
                    ]
                }
            ],
            "edges": [
                {
                    "id": "edge-workflow-start-end",
                    "source": "node-workflow-start",
                    "target": "node-workflow-end",
                    "sourceHandle": null,
                    "targetHandle": null,
                    "containerId": null,
                    "points": []
                }
            ]
        },
        "editor": { "viewport": { "x": 0, "y": 0, "zoom": 1 }, "annotations": [], "activeContainerPath": [] }
    })
}

fn add_second_llm_and_answer(
    document: &mut Value,
    answer_template: &str,
    llm2_depends_on_llm1: bool,
) {
    let nodes = document["graph"]["nodes"]
        .as_array_mut()
        .expect("sample graph nodes should be an array");
    nodes.push(json!({
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
            "prompt_messages": {
                "kind": "prompt_messages",
                "value": [{
                    "id": "user-2",
                    "role": "user",
                    "content": {
                        "kind": "templated_text",
                        "value": if llm2_depends_on_llm1 { "{{ node-llm.text }}" } else { "{{ node-start.query }}" }
                    }
                }]
            }
        },
        "outputs": [{ "key": "text", "title": "模型输出", "valueType": "string" }]
    }));
    nodes.push(json!({
        "id": "node-answer",
        "type": "answer",
        "alias": "Answer",
        "description": "",
        "containerId": null,
        "position": { "x": 720, "y": 0 },
        "configVersion": 1,
        "config": {},
        "bindings": {
            "answer_template": { "kind": "templated_text", "value": answer_template }
        },
        "outputs": [{ "key": "answer", "title": "对话输出", "valueType": "string" }]
    }));

    let edges = document["graph"]["edges"]
        .as_array_mut()
        .expect("sample graph edges should be an array");
    if llm2_depends_on_llm1 {
        edges.push(json!({
            "id": "edge-llm-llm2",
            "source": "node-llm",
            "target": "node-llm-2",
            "sourceHandle": null,
            "targetHandle": null,
            "containerId": null,
            "points": []
        }));
    } else {
        edges.push(json!({
            "id": "edge-start-llm2",
            "source": "node-start",
            "target": "node-llm-2",
            "sourceHandle": null,
            "targetHandle": null,
            "containerId": null,
            "points": []
        }));
    }
    edges.push(json!({
        "id": "edge-llm-answer",
        "source": "node-llm",
        "target": "node-answer",
        "sourceHandle": null,
        "targetHandle": null,
        "containerId": null,
        "points": []
    }));
    edges.push(json!({
        "id": "edge-llm2-answer",
        "source": "node-llm-2",
        "target": "node-answer",
        "sourceHandle": null,
        "targetHandle": null,
        "containerId": null,
        "points": []
    }));
}

fn plugin_document(flow_id: Uuid) -> serde_json::Value {
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
                    "position": { "x": 480, "y": 0 },
                    "configVersion": 1,
                    "plugin_unique_identifier": "prompt_pack",
                    "package_id": "prompt_pack@0.1.0",
                    "plugin_id": "prompt_pack@0.1.0",
                    "plugin_version": "0.1.0",
                    "contribution_code": "openai_prompt",
                    "node_shell": "action",
                    "schema_version": "1flowbase.node-contribution/v2",
                    "contribution_checksum": "sha256:contribution",
                    "compiled_contribution_hash": "sha256:compiled",
                    "output_schema_snapshot": {
                        "outputs": [{ "key": "answer", "title": "回答", "valueType": "string" }]
                    },
                    "config": {
                        "prompt": "Hello {{ node-start.query }}"
                    },
                    "bindings": {
                        "query": { "kind": "selector", "value": ["node-start", "query"] }
                    },
                    "outputs": [{ "key": "answer", "title": "回答", "valueType": "string" }]
                }
            ],
            "edges": [
                {
                    "id": "edge-start-plugin",
                    "source": "node-start",
                    "target": "node-plugin",
                    "sourceHandle": null,
                    "targetHandle": null,
                    "containerId": null,
                    "points": []
                }
            ]
        },
        "editor": { "viewport": { "x": 0, "y": 0, "zoom": 1 }, "annotations": [], "activeContainerPath": [] }
    })
}

fn code_js_dependency_document(
    flow_id: Uuid,
    node_type: &str,
    imports: Option<Value>,
) -> serde_json::Value {
    let mut config = json!({});
    if let Some(imports) = imports {
        config["imports"] = imports;
    }

    json!({
        "schemaVersion": "1flowbase.flow/v2",
        "meta": { "flowId": flow_id.to_string(), "name": "Code Imports", "description": "", "tags": [] },
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
                    "id": "node-code",
                    "type": node_type,
                    "alias": "Code",
                    "description": "",
                    "containerId": null,
                    "position": { "x": 240, "y": 0 },
                    "configVersion": 1,
                    "config": config,
                    "bindings": {},
                    "outputs": [{ "key": "result", "title": "Result", "valueType": "json" }]
                }
            ],
            "edges": [
                {
                    "id": "edge-start-code",
                    "source": "node-start",
                    "target": "node-code",
                    "sourceHandle": null,
                    "targetHandle": null,
                    "containerId": null,
                    "points": []
                }
            ]
        },
        "editor": { "viewport": { "x": 0, "y": 0, "zoom": 1 }, "annotations": [], "activeContainerPath": [] }
    })
}

fn code_js_dependency_context(alias: &str, target: &str) -> FlowCompileContext {
    let mut context = compile_context();
    context.js_dependencies.insert(
        format!("{target}::{alias}"),
        FlowCompileJsDependency {
            alias: alias.to_string(),
            target: target.to_string(),
            artifact_path: format!("artifacts/{alias}.backend.mjs"),
            artifact_hash: format!("sha256:{alias}"),
            integrity: format!("sha256:{alias}"),
        },
    );
    context
}

mod bindings_and_outputs;
mod branches;
mod code_runtime;
mod provider_and_plugin;
