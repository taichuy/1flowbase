use super::*;
use anyhow::Result;
use async_trait::async_trait;
use extension_contracts::provider_contract::{
    ProviderFinishReason, ProviderInvocationInput, ProviderInvocationResult, ProviderMessageRole,
};
use orchestration_runtime::{
    compiled_plan::{CompiledCodeRuntime, CompiledLlmRuntime, CompiledPlan, CompiledPluginRuntime},
    execution_engine::{
        start_flow_debug_run, CapabilityInvocationOutput, CapabilityInvoker, CodeInvocationOutput,
        CodeInvoker, ProviderInvocationOutput, ProviderInvoker,
    },
};
use std::sync::Mutex;

struct Capture(Mutex<Vec<ProviderInvocationInput>>);
#[async_trait]
impl ProviderInvoker for Capture {
    async fn resolve_llm_route(
        &self,
        _runtime: &CompiledLlmRuntime,
    ) -> Result<orchestration_runtime::execution_engine::ResolvedProviderRoute> {
        Ok(
            orchestration_runtime::execution_engine::ResolvedProviderRoute::new(
                Default::default(),
                (),
            )
            .with_runtime_plugin_id("fixture@1.0.0"),
        )
    }
    async fn invoke_llm(
        &self,
        _runtime: &CompiledLlmRuntime,
        input: ProviderInvocationInput,
    ) -> Result<ProviderInvocationOutput> {
        self.0.lock().unwrap().push(input);
        Ok(ProviderInvocationOutput {
            events: Vec::new(),
            result: ProviderInvocationResult {
                final_content: Some("final answer".into()),
                finish_reason: Some(ProviderFinishReason::Stop),
                ..Default::default()
            },
            first_token_at: None,
            time_to_first_token_ms: None,
        })
    }
}
#[async_trait]
impl CapabilityInvoker for Capture {
    async fn invoke_capability_node(
        &self,
        _runtime: &CompiledPluginRuntime,
        _config: Value,
        _input: Value,
    ) -> Result<CapabilityInvocationOutput> {
        unreachable!()
    }
}
#[async_trait]
impl CodeInvoker for Capture {
    async fn invoke_code_node(
        &self,
        _runtime: &CompiledCodeRuntime,
        _config: Value,
        _input: Value,
    ) -> Result<CodeInvocationOutput> {
        unreachable!()
    }
}
fn plan() -> CompiledPlan {
    serde_json::from_value(json!({
        "flow_id":"00000000-0000-0000-0000-000000000001","source_draft_id":"draft","schema_version":"1flowbase.flow/v2",
        "topological_order":["node-start","llm","answer"],"edges":[],"compile_issues":[],
        "nodes":{
            "node-start":{"node_id":"node-start","node_type":"start","alias":"Start","container_id":null,"dependency_node_ids":[],"downstream_node_ids":["llm"],"bindings":{},"outputs":[{"key":"query","title":"Query","value_type":"string","selector":[]}],"config":{}},
            "llm":{"node_id":"llm","node_type":"llm","alias":"LLM","container_id":null,"dependency_node_ids":["node-start"],"downstream_node_ids":["answer"],
                "bindings":{"prompt_messages":{"kind":"prompt_messages","selector_paths":[["node-start","query"]],"raw_value":[
                    {"id":"system","role":"system","content":{"kind":"templated_text","value":"Original application instruction."}},
                    {"id":"user","role":"user","content":{"kind":"templated_text","value":"{{node-start.query}}"}}
                ]}},"outputs":[{"key":"text","title":"Text","value_type":"string","selector":[]}],"config":{},
                "llm_runtime":{"provider_instance_id":"fixture","provider_instance_display_name":"Fixture","provider_code":"fixture","protocol":"openai_compatible","model":"test","routing":null}},
            "answer":{"node_id":"answer","node_type":"answer","alias":"Answer","container_id":null,"dependency_node_ids":["llm"],"downstream_node_ids":[],
                "bindings":{"answer_template":{"kind":"selector","selector_paths":[["llm","text"]],"raw_value":["llm","text"]}},"outputs":[{"key":"answer","title":"Answer","value_type":"string","selector":[]}],"config":{}}
        }
    })).unwrap()
}

#[tokio::test]
async fn communication_native_mapping_reaches_provider_as_stable_system_session_user() {
    let capture = Capture(Mutex::new(Vec::new()));
    let mapping = ApplicationApiMappingConfig::default_native();
    for query in ["first input", "second input"] {
        let request: NativeRunRequest = serde_json::from_value(json!({"query":query,"system":"Plugin instruction.","history":[{"role":"system","content":"{\"session_id\":\"stable\",\"user_id\":\"scanner\"}"}]})).unwrap();
        let mapped = NativeInputMapper::map_communication(&request, &mapping).unwrap();
        let result = start_flow_debug_run(&plan(), &mapped.node_input_payload, &capture)
            .await
            .unwrap();
        assert!(matches!(
            result.stop_reason,
            orchestration_runtime::execution_state::ExecutionStopReason::Completed
        ));
    }
    let requests = capture.0.lock().unwrap();
    assert_eq!(requests.len(), 2);
    for (index, input) in requests.iter().enumerate() {
        assert_eq!(
            input.system_text().as_deref(),
            Some("Original application instruction.\n\nPlugin instruction.")
        );
        assert_eq!(input.messages.len(),2,"one separate session system plus current user; stable system uses provider system blocks");
        assert_eq!(input.messages[0].role, ProviderMessageRole::System);
        assert_eq!(
            input.messages[0].content,
            "{\"session_id\":\"stable\",\"user_id\":\"scanner\"}"
        );
        assert_eq!(input.messages[1].role, ProviderMessageRole::User);
        assert_eq!(
            input.messages[1].content,
            if index == 0 {
                "first input"
            } else {
                "second input"
            }
        );
        assert!(!serde_json::to_string(input)
            .unwrap()
            .contains("__communication_session"));
    }
    assert_eq!(requests[0].system, requests[1].system);
    assert_eq!(requests[0].messages[0], requests[1].messages[0]);
}

#[test]
fn public_native_cannot_forge_communication_marker_or_change_history_normalization() {
    assert!(serde_json::from_value::<NativeRunRequest>(json!({"query":"x","history":[{"role":"system","content":"x","__communication_session":true}]})).is_err());
    let request: NativeRunRequest = serde_json::from_value(
        json!({"query":"x","system":"stable","history":[{"role":"system","content":"session"}]}),
    )
    .unwrap();
    let normal =
        NativeInputMapper::map(&request, &ApplicationApiMappingConfig::default_native()).unwrap();
    assert_eq!(
        normal.node_input_payload["__native_model_prompt_context"]["messages"],
        json!([])
    );
    assert_eq!(
        normal.node_input_payload["__native_model_prompt_context"]["system"]
            .as_array()
            .unwrap()
            .len(),
        2
    );
    let mut invalid = request;
    invalid
        .history
        .push(json!({"role":"assistant","content":"prior answer"}));
    assert!(NativeInputMapper::map_communication(
        &invalid,
        &ApplicationApiMappingConfig::default_native()
    )
    .is_err());
}
