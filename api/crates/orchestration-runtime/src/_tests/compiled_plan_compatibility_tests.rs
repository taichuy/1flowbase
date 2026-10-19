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

    assert_eq!(runtime.provider_instance_id, "provider-main");
    assert_eq!(runtime.provider_instance_display_name, "");
    assert_eq!(runtime.protocol, "openai_chat");
    assert!(runtime.routing.is_none());
    assert_eq!(
        serde_json::to_value(&runtime).unwrap(),
        json!({
            "provider_instance_id": "provider-main",
            "provider_code": "openai_compatible",
            "protocol": "openai_chat",
            "model": "legacy-model"
        })
    );
}

fn failover_runtime_wire() -> serde_json::Value {
    json!({
        "provider_instance_id": "provider-primary",
        "provider_instance_display_name": "Primary provider",
        "provider_code": "openai_compatible",
        "protocol": "openai_chat",
        "model": "primary-model",
        "routing": {
            "routing_mode": "failover_queue",
            "fixed_model_target": null,
            "queue_template_id": "queue-template",
            "queue_snapshot_id": "queue-snapshot",
            "queue_targets": [
                {
                    "provider_instance_id": "provider-primary",
                    "provider_instance_display_name": "Primary provider",
                    "provider_code": "openai_compatible",
                    "protocol": "openai_chat",
                    "upstream_model_id": "primary-model"
                },
                {
                    "provider_instance_id": "provider-backup",
                    "provider_instance_display_name": "Backup provider",
                    "provider_code": "anthropic",
                    "protocol": "anthropic_messages",
                    "upstream_model_id": "backup-model"
                }
            ],
            "distribution_rule": "retry_round_robin",
            "distribution_key": "queue-distribution",
            "context_policy": { "integration_context": "disabled" },
            "stream_policy": { "mode": "buffered" }
        }
    })
}

#[test]
fn compiled_llm_runtime_roundtrips_current_nonempty_failover_wire_fields() {
    let wire = failover_runtime_wire();
    let runtime: CompiledLlmRuntime = serde_json::from_value(wire.clone()).unwrap();

    assert!(!runtime.targets_main_instance());
    let serialized = serde_json::to_value(&runtime).unwrap();
    assert_eq!(serialized, wire);
    assert_eq!(
        serde_json::from_value::<CompiledLlmRuntime>(serialized).unwrap(),
        runtime
    );
}

#[test]
fn compiled_llm_runtime_roundtrips_main_wire_with_omitted_live_fields() {
    let wire = json!({
        "provider_code": "openai_compatible",
        "model": "main-model"
    });
    let runtime: CompiledLlmRuntime = serde_json::from_value(wire.clone()).unwrap();

    assert_eq!(runtime.provider_instance_id, "");
    assert_eq!(runtime.provider_instance_display_name, "");
    assert_eq!(runtime.protocol, "");
    assert!(runtime.routing.is_none());
    assert!(runtime.targets_main_instance());
    let serialized = serde_json::to_value(&runtime).unwrap();
    assert_eq!(serialized, wire);
    assert_eq!(
        serde_json::from_value::<CompiledLlmRuntime>(serialized).unwrap(),
        runtime
    );
}

#[test]
fn compiled_llm_runtime_rejects_wrong_wire_types_and_missing_required_fields() {
    for (pointer, invalid_value) in [
        ("/provider_instance_id", json!(42)),
        ("/provider_instance_id", json!(null)),
        ("/provider_instance_display_name", json!(false)),
        ("/protocol", json!(["openai_chat"])),
        ("/routing", json!("failover_queue")),
        ("/routing/routing_mode", json!("unknown_mode")),
        (
            "/routing/queue_targets/0/provider_instance_id",
            json!({ "id": "provider-primary" }),
        ),
    ] {
        let mut wire = failover_runtime_wire();
        *wire.pointer_mut(pointer).unwrap() = invalid_value;
        assert!(
            serde_json::from_value::<CompiledLlmRuntime>(wire).is_err(),
            "wrong wire type or enum at {pointer} must be rejected"
        );
    }

    for required_field in ["provider_code", "model"] {
        let mut wire = failover_runtime_wire();
        wire.as_object_mut().unwrap().remove(required_field);
        assert!(
            serde_json::from_value::<CompiledLlmRuntime>(wire).is_err(),
            "missing required field {required_field} must be rejected"
        );
    }
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
