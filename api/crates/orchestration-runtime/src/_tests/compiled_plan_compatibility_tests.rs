use std::collections::BTreeMap;

use crate::compiled_plan::{
    CompiledBinding, CompiledLlmRouteTarget, CompiledLlmRuntime, CompiledNode, VariableReference,
};
use serde_json::json;

#[test]
fn compiled_llm_runtime_accepts_legacy_payload_without_provider_display_name() {
    let runtime: CompiledLlmRuntime = serde_json::from_value(json!({
        "provider_instance_id": "provider-main",
        "provider_code": "openai_compatible",
        "protocol": "openai_chat",
        "model": "legacy-model",
        "routing": null
    }))
    .unwrap();

    assert_eq!(runtime.provider_instance_display_name, "");
}

#[test]
fn compiled_llm_route_target_accepts_legacy_payload_without_provider_display_name() {
    let target: CompiledLlmRouteTarget = serde_json::from_value(json!({
        "provider_instance_id": "provider-backup",
        "provider_code": "openai_compatible",
        "protocol": "openai_chat",
        "upstream_model_id": "legacy-backup-model",
        "runtime_capabilities": ["message_blocks.reasoning_history.v1"]
    }))
    .unwrap();

    assert_eq!(target.provider_instance_display_name, "");
    assert!(
        serde_json::to_value(target)
            .unwrap()
            .get("runtime_capabilities")
            .is_none(),
        "Provider capabilities belong to the live installation, not the compiled plan"
    );
}

#[test]
fn compiled_binding_accepts_legacy_payload_without_i18n_text_ref() {
    let binding: CompiledBinding = serde_json::from_value(json!({
        "kind": "templated_text",
        "raw_value": "Hello {{node-start.query}}",
        "selector_paths": [["node-start", "query"]]
    }))
    .unwrap();

    assert_eq!(binding.i18n_text_ref, None);
    assert_eq!(binding.kind, "templated_text");
}

#[test]
fn legacy_compiled_llm_node_without_protocol_context_defaults_to_system_context() {
    let node = CompiledNode {
        node_id: "legacy-llm".to_string(),
        node_type: "llm".to_string(),
        alias: "Legacy LLM".to_string(),
        container_id: None,
        dependency_node_ids: Vec::new(),
        downstream_node_ids: Vec::new(),
        bindings: BTreeMap::new(),
        outputs: Vec::new(),
        config: json!({}),
        plugin_runtime: None,
        llm_runtime: None,
        code_runtime: None,
    };

    assert_eq!(
        node.protocol_context_reference().unwrap(),
        Some(VariableReference::selector(vec![
            "sys".to_string(),
            "protocol_context".to_string(),
        ]))
    );
}
