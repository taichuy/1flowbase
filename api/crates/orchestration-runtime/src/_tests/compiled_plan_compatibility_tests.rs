use crate::compiled_plan::{CompiledBinding, CompiledLlmRouteTarget, CompiledLlmRuntime};
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
        "upstream_model_id": "legacy-backup-model"
    }))
    .unwrap();

    assert_eq!(target.provider_instance_display_name, "");
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
