use std::{
    collections::BTreeMap,
    sync::{Arc, Mutex},
};

use anyhow::Result;
use async_trait::async_trait;
use plugin_framework::provider_contract::{
    ProviderFinishReason, ProviderInvocationInput, ProviderInvocationResult, ProviderStreamEvent,
};
use serde_json::json;
use uuid::Uuid;

use crate::{
    compiled_plan::{
        CompiledBinding, CompiledCodeRuntime, CompiledLlmRuntime, CompiledNode, CompiledOutput,
        CompiledPlan,
    },
    execution_engine::{
        start_flow_debug_run, CapabilityInvocationOutput, CapabilityInvoker, CodeInvocationOutput,
        CodeInvoker, ProviderInvocationOutput, ProviderInvoker,
    },
    execution_state::ExecutionStopReason,
};

struct CapturingProviderInvoker {
    captured_input: Arc<Mutex<Option<ProviderInvocationInput>>>,
}

#[async_trait]
impl ProviderInvoker for CapturingProviderInvoker {
    async fn invoke_llm(
        &self,
        _runtime: &CompiledLlmRuntime,
        input: ProviderInvocationInput,
    ) -> Result<ProviderInvocationOutput> {
        *self
            .captured_input
            .lock()
            .expect("captured input mutex poisoned") = Some(input);

        Ok(ProviderInvocationOutput {
            events: vec![
                ProviderStreamEvent::TextDelta {
                    delta: "ok".to_string(),
                },
                ProviderStreamEvent::Finish {
                    reason: ProviderFinishReason::Stop,
                },
            ],
            result: ProviderInvocationResult {
                final_content: Some("ok".to_string()),
                finish_reason: Some(ProviderFinishReason::Stop),
                ..ProviderInvocationResult::default()
            },
            first_token_at: None,
            time_to_first_token_ms: None,
        })
    }
}

#[async_trait]
impl CapabilityInvoker for CapturingProviderInvoker {
    async fn invoke_capability_node(
        &self,
        _runtime: &crate::compiled_plan::CompiledPluginRuntime,
        _config_payload: serde_json::Value,
        input_payload: serde_json::Value,
    ) -> Result<CapabilityInvocationOutput> {
        Ok(CapabilityInvocationOutput {
            output_payload: input_payload,
        })
    }
}

#[async_trait]
impl CodeInvoker for CapturingProviderInvoker {
    async fn invoke_code_node(
        &self,
        _runtime: &CompiledCodeRuntime,
        _config_payload: serde_json::Value,
        _input_payload: serde_json::Value,
    ) -> Result<CodeInvocationOutput> {
        unreachable!("prompt message validation plan does not execute code nodes")
    }
}

fn plan_with_empty_prompt_messages() -> CompiledPlan {
    let mut nodes = BTreeMap::new();
    nodes.insert(
        "node-start".to_string(),
        CompiledNode {
            node_id: "node-start".to_string(),
            node_type: "start".to_string(),
            alias: "Start".to_string(),
            container_id: None,
            dependency_node_ids: vec![],
            downstream_node_ids: vec!["node-llm".to_string()],
            bindings: BTreeMap::new(),
            outputs: vec![CompiledOutput {
                key: "query".to_string(),
                title: "用户输入".to_string(),
                value_type: "string".to_string(),
                selector: Vec::new(),
                json_schema: None,
            }],
            config: json!({}),
            plugin_runtime: None,
            llm_runtime: None,
            code_runtime: None,
        },
    );
    nodes.insert(
        "node-llm".to_string(),
        CompiledNode {
            node_id: "node-llm".to_string(),
            node_type: "llm".to_string(),
            alias: "LLM".to_string(),
            container_id: None,
            dependency_node_ids: vec!["node-start".to_string()],
            downstream_node_ids: vec![],
            bindings: BTreeMap::from([(
                "prompt_messages".to_string(),
                CompiledBinding {
                    kind: "prompt_messages".to_string(),
                    selector_paths: vec![],
                    raw_value: json!([
                        {
                            "id": "system-1",
                            "role": "system",
                            "content": {
                                "kind": "templated_text",
                                "value": ""
                            }
                        },
                        {
                            "id": "user-2",
                            "role": "assistant",
                            "content": {
                                "kind": "templated_text",
                                "value": ""
                            }
                        }
                    ]),
                },
            )]),
            outputs: vec![CompiledOutput {
                key: "text".to_string(),
                title: "模型输出".to_string(),
                value_type: "string".to_string(),
                selector: Vec::new(),
                json_schema: None,
            }],
            config: json!({}),
            plugin_runtime: None,
            llm_runtime: Some(CompiledLlmRuntime {
                provider_instance_id: "provider-ready".to_string(),
                provider_instance_display_name: String::new(),
                provider_code: "fixture_provider".to_string(),
                protocol: "openai_compatible".to_string(),
                model: "gpt-5.4-mini".to_string(),
                routing: None,
            }),
            code_runtime: None,
        },
    );

    CompiledPlan {
        flow_id: Uuid::nil(),
        source_draft_id: "draft-1".to_string(),
        schema_version: "1flowbase.flow/v2".to_string(),
        topological_order: vec!["node-start".to_string(), "node-llm".to_string()],
        edges: Vec::new(),
        nodes,
        compile_issues: Vec::new(),
    }
}

fn plan_with_templated_prompt_message() -> CompiledPlan {
    let mut plan = plan_with_empty_prompt_messages();
    let node = plan
        .nodes
        .get_mut("node-llm")
        .expect("llm node should exist");
    node.bindings.insert(
        "prompt_messages".to_string(),
        CompiledBinding {
            kind: "prompt_messages".to_string(),
            selector_paths: vec![vec!["node-start".to_string(), "query".to_string()]],
            raw_value: json!([
                {
                    "id": "user-1",
                    "role": "user",
                    "content": {
                        "kind": "templated_text",
                        "value": "{{node-start.query}}"
                    }
                }
            ]),
        },
    );
    plan
}

fn plan_with_system_only_prompt_message() -> CompiledPlan {
    let mut plan = plan_with_empty_prompt_messages();
    let node = plan
        .nodes
        .get_mut("node-llm")
        .expect("llm node should exist");
    node.bindings.insert(
        "prompt_messages".to_string(),
        CompiledBinding {
            kind: "prompt_messages".to_string(),
            selector_paths: vec![
                vec!["node-start".to_string(), "query".to_string()],
                vec!["node-start".to_string(), "previous_answer".to_string()],
            ],
            raw_value: json!([
                {
                    "id": "system-1",
                    "role": "system",
                    "content": {
                        "kind": "templated_text",
                        "value": "Polish the answer for this question:\n{{node-start.query}}\n\nAnswer:\n{{node-start.previous_answer}}"
                    }
                }
            ]),
        },
    );
    plan
}

#[tokio::test]
async fn llm_runtime_fails_before_provider_when_prompt_messages_are_empty() {
    let plan = plan_with_empty_prompt_messages();
    let captured_input = Arc::new(Mutex::new(None));
    let invoker = CapturingProviderInvoker {
        captured_input: captured_input.clone(),
    };

    let outcome = start_flow_debug_run(
        &plan,
        &json!({ "node-start": { "query": "hello" } }),
        &invoker,
    )
    .await
    .unwrap();

    assert!(captured_input
        .lock()
        .expect("captured input mutex poisoned")
        .is_none());

    match outcome.stop_reason {
        ExecutionStopReason::Failed(ref failure) => {
            assert_eq!(failure.node_id, "node-llm");
            assert_eq!(
                failure.error_payload["error_code"],
                json!("prompt_messages_empty")
            );
            assert_eq!(outcome.node_traces[1].output_payload, json!({}));
            assert!(outcome.variable_pool.get("node-llm").is_none());
        }
        other => panic!("expected failed stop reason, got {other:?}"),
    }
}

#[tokio::test]
async fn llm_runtime_executes_system_only_node_prompt_as_user_turn() {
    let plan = plan_with_system_only_prompt_message();
    let captured_input = Arc::new(Mutex::new(None));
    let invoker = CapturingProviderInvoker {
        captured_input: captured_input.clone(),
    };

    let outcome = start_flow_debug_run(
        &plan,
        &json!({
            "node-start": {
                "query": "hello",
                "previous_answer": "raw answer"
            }
        }),
        &invoker,
    )
    .await
    .unwrap();

    assert!(matches!(
        outcome.stop_reason,
        ExecutionStopReason::Completed
    ));
    let input = captured_input
        .lock()
        .expect("captured input mutex poisoned")
        .clone()
        .expect("provider input should be captured");

    assert_eq!(
        input.messages.len(),
        1,
        "system-only node prompt should become an executable provider turn"
    );
    assert_eq!(
        input.messages[0].role,
        plugin_framework::provider_contract::ProviderMessageRole::User
    );
    assert!(
        input
            .system_text()
            .as_deref()
            .is_some_and(|system| system.contains("raw answer")),
        "system-only node prompt should still feed provider instructions"
    );
    assert!(
        input.messages[0].content.contains("raw answer"),
        "rendered system-only prompt should remain visible to the provider"
    );

    let trace = outcome
        .node_traces
        .iter()
        .find(|trace| trace.node_id == "node-llm")
        .expect("llm trace should exist");
    assert_eq!(
        trace.debug_payload["llm_context"]["compatibility_promotions"][0]["source_kind"],
        json!("node_prompt_system_only")
    );
    assert!(
        trace.debug_payload["llm_context"]["effective_system"]
            .as_array()
            .is_some_and(|blocks| blocks.iter().any(|block| {
                block["text"]
                    .as_str()
                    .is_some_and(|text| text.contains("raw answer"))
            })),
        "debug payload should show the preserved provider instructions"
    );
}

#[tokio::test]
async fn llm_runtime_fails_before_provider_when_prompt_template_selector_is_missing() {
    let plan = plan_with_templated_prompt_message();
    let captured_input = Arc::new(Mutex::new(None));
    let invoker = CapturingProviderInvoker {
        captured_input: captured_input.clone(),
    };

    let outcome = start_flow_debug_run(
        &plan,
        &json!({ "different-start": { "query": "hello" } }),
        &invoker,
    )
    .await
    .unwrap();

    assert!(captured_input
        .lock()
        .expect("captured input mutex poisoned")
        .is_none());

    match outcome.stop_reason {
        ExecutionStopReason::Failed(ref failure) => {
            assert_eq!(failure.node_id, "node-llm");
            assert_eq!(
                failure.error_payload["error_code"],
                json!("prompt_template_unresolved")
            );
            assert!(failure.error_payload["message"]
                .as_str()
                .expect("message should be a string")
                .contains("node-start.query"));
        }
        other => panic!("expected failed stop reason, got {other:?}"),
    }
}

#[tokio::test]
async fn ac_003_prompt_binding_rejects_implicit_object_stringification() {
    let plan = plan_with_templated_prompt_message();
    let captured_input = Arc::new(Mutex::new(None));
    let invoker = CapturingProviderInvoker {
        captured_input: captured_input.clone(),
    };

    let outcome = start_flow_debug_run(
        &plan,
        &json!({ "node-start": { "query": { "unsafe": "object" } } }),
        &invoker,
    )
    .await
    .unwrap();

    assert!(captured_input
        .lock()
        .expect("captured input mutex poisoned")
        .is_none());
    match outcome.stop_reason {
        ExecutionStopReason::Failed(failure) => {
            assert_eq!(
                failure.error_payload["error_code"],
                json!("binding_resolution_failed")
            );
            assert!(failure.error_payload["message"]
                .as_str()
                .is_some_and(|message| message.contains("cannot implicitly stringify an object")));
        }
        other => panic!("expected prompt binding failure, got {other:?}"),
    }
}

#[tokio::test]
async fn ac_003_prompt_binding_rejects_untyped_array_stringification() {
    let plan = plan_with_templated_prompt_message();
    let captured_input = Arc::new(Mutex::new(None));
    let invoker = CapturingProviderInvoker {
        captured_input: captured_input.clone(),
    };

    let outcome = start_flow_debug_run(
        &plan,
        &json!({ "node-start": { "query": ["unsafe", "array"] } }),
        &invoker,
    )
    .await
    .unwrap();

    assert!(captured_input
        .lock()
        .expect("captured input mutex poisoned")
        .is_none());
    match outcome.stop_reason {
        ExecutionStopReason::Failed(failure) => {
            assert_eq!(
                failure.error_payload["error_code"],
                json!("binding_resolution_failed")
            );
            assert!(failure.error_payload["message"]
                .as_str()
                .is_some_and(|message| message.contains("not an arbitrary array")));
        }
        other => panic!("expected prompt binding failure, got {other:?}"),
    }
}
