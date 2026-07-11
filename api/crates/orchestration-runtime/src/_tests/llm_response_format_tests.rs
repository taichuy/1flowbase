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
};

struct CaptureInvoker {
    captured_input: Arc<Mutex<Option<ProviderInvocationInput>>>,
}

#[async_trait]
impl ProviderInvoker for CaptureInvoker {
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
                    delta: "你好".to_string(),
                },
                ProviderStreamEvent::Finish {
                    reason: ProviderFinishReason::Stop,
                },
            ],
            result: ProviderInvocationResult {
                final_content: Some("你好".to_string()),
                finish_reason: Some(ProviderFinishReason::Stop),
                ..ProviderInvocationResult::default()
            },
            first_token_at: None,
            time_to_first_token_ms: None,
        })
    }
}

#[async_trait]
impl CapabilityInvoker for CaptureInvoker {
    async fn invoke_capability_node(
        &self,
        _runtime: &crate::compiled_plan::CompiledPluginRuntime,
        _config_payload: serde_json::Value,
        _input_payload: serde_json::Value,
    ) -> Result<CapabilityInvocationOutput> {
        unreachable!("response format plan does not execute capability nodes")
    }
}

#[async_trait]
impl CodeInvoker for CaptureInvoker {
    async fn invoke_code_node(
        &self,
        _runtime: &CompiledCodeRuntime,
        _config_payload: serde_json::Value,
        _input_payload: serde_json::Value,
    ) -> Result<CodeInvocationOutput> {
        unreachable!("response format plan does not execute code nodes")
    }
}

fn llm_plan(response_format: serde_json::Value) -> CompiledPlan {
    let mut nodes = BTreeMap::new();
    nodes.insert(
        "node-start".to_string(),
        CompiledNode {
            node_id: "node-start".to_string(),
            node_type: "start".to_string(),
            alias: "Start".to_string(),
            container_id: None,
            dependency_node_ids: Vec::new(),
            downstream_node_ids: vec!["node-llm".to_string()],
            bindings: BTreeMap::new(),
            outputs: Vec::new(),
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
            downstream_node_ids: Vec::new(),
            bindings: BTreeMap::from([(
                "prompt_messages".to_string(),
                CompiledBinding {
                    kind: "prompt_messages".to_string(),
                    raw_value: json!([
                        {
                            "id": "user-1",
                            "role": "user",
                            "content": {
                                "kind": "templated_text",
                                "value": "{{node-start.query}}你好？"
                            }
                        }
                    ]),
                    selector_paths: vec![vec!["node-start".to_string(), "query".to_string()]],
                },
            )]),
            outputs: vec![CompiledOutput {
                key: "text".to_string(),
                title: "模型输出".to_string(),
                value_type: "string".to_string(),
                selector: Vec::new(),
                json_schema: None,
            }],
            config: json!({
                "response_format": response_format
            }),
            plugin_runtime: None,
            llm_runtime: Some(CompiledLlmRuntime {
                provider_instance_id: "provider-ready".to_string(),
                provider_instance_display_name: String::new(),
                provider_code: "openai_compatible".to_string(),
                protocol: "openai_compatible".to_string(),
                model: "qwen3.5-27b".to_string(),
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

#[tokio::test]
async fn text_response_format_is_not_forwarded_to_provider() {
    let captured_input = Arc::new(Mutex::new(None));
    let invoker = CaptureInvoker {
        captured_input: captured_input.clone(),
    };

    start_flow_debug_run(
        &llm_plan(json!({ "mode": "text" })),
        &json!({ "node-start": { "query": "你好？" } }),
        &invoker,
    )
    .await
    .unwrap();

    let input = captured_input
        .lock()
        .expect("captured input mutex poisoned")
        .clone()
        .expect("provider input should be captured");

    assert_eq!(input.response_format, None);
}
