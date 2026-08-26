use extension_contracts::provider_contract::ProviderStreamEvent;
use orchestration_runtime::{
    compiled_plan::CompiledOutput,
    payload_builder::{PublicOutputContract, RawNodeExecutionResult},
};
use serde_json::{json, Map, Value};

fn output(key: &str) -> CompiledOutput {
    CompiledOutput {
        key: key.to_string(),
        title: key.to_string(),
        value_type: "string".to_string(),
        selector: Vec::new(),
        json_schema: None,
    }
}

fn output_with_selector(key: &str, selector: &[&str]) -> CompiledOutput {
    CompiledOutput {
        key: key.to_string(),
        title: key.to_string(),
        value_type: "json".to_string(),
        selector: selector.iter().map(|segment| segment.to_string()).collect(),
        json_schema: None,
    }
}

fn object(entries: impl IntoIterator<Item = (&'static str, Value)>) -> Map<String, Value> {
    entries
        .into_iter()
        .map(|(key, value)| (key.to_string(), value))
        .collect()
}

#[test]
fn payload_builder_keeps_complete_output_object_and_only_declares_selector_values() {
    let contract = PublicOutputContract::from_compiled_outputs(&[output("text")]).unwrap();
    let raw = RawNodeExecutionResult {
        executor_output: object([
            ("text", json!("accepted")),
            ("usage", json!({ "input_tokens": 3, "output_tokens": 5 })),
            ("route", json!({ "provider_code": "openai" })),
            ("error", json!({ "message": "provider failed" })),
            ("raw_response_ref", json!("artifact-1")),
            ("unexpected", json!("kept for output inspection")),
        ]),
        metrics_facts: Map::new(),
        error_facts: Map::new(),
        debug_facts: Map::new(),
        provider_events: Vec::new(),
    };

    let built = contract.build_node_payloads(raw).unwrap();

    assert_eq!(
        built.output_payload,
        json!({
            "error": { "message": "provider failed" },
            "raw_response_ref": "artifact-1",
            "route": { "provider_code": "openai" },
            "text": "accepted",
            "unexpected": "kept for output inspection",
            "usage": { "input_tokens": 3, "output_tokens": 5 }
        })
    );
    assert_eq!(
        contract
            .project_variable_payload(&built.output_payload)
            .unwrap(),
        json!({ "text": "accepted" })
    );
}

#[test]
fn payload_builder_allows_unknown_keys_when_structured_expansion_is_explicit() {
    let contract = PublicOutputContract::from_compiled_outputs(&[output("text")])
        .unwrap()
        .with_structured_expansion(true);
    let raw = RawNodeExecutionResult {
        executor_output: object([
            ("text", json!("accepted")),
            ("dynamic_field", json!("expanded")),
        ]),
        metrics_facts: Map::new(),
        error_facts: Map::new(),
        debug_facts: Map::new(),
        provider_events: Vec::new(),
    };

    let built = contract.build_node_payloads(raw).unwrap();

    assert_eq!(
        built.output_payload,
        json!({ "dynamic_field": "expanded", "text": "accepted" })
    );
}

#[test]
fn payload_builder_allows_runtime_fields_as_declared_selectors() {
    let contract =
        PublicOutputContract::from_compiled_outputs(&[output("text"), output("usage")]).unwrap();
    let raw = RawNodeExecutionResult {
        executor_output: object([
            ("text", json!("accepted")),
            ("usage", json!({ "total_tokens": 8 })),
            ("route", json!({ "provider_code": "openai" })),
        ]),
        metrics_facts: Map::new(),
        error_facts: Map::new(),
        debug_facts: Map::new(),
        provider_events: Vec::new(),
    };

    let built = contract.build_node_payloads(raw).unwrap();

    assert_eq!(
        contract
            .project_variable_payload(&built.output_payload)
            .unwrap(),
        json!({
            "text": "accepted",
            "usage": { "total_tokens": 8 }
        })
    );
}

#[test]
fn payload_builder_projects_declared_selector_paths_without_copying_output_payload() {
    let contract = PublicOutputContract::from_compiled_outputs(&[output_with_selector(
        "token_usage",
        &["usage", "total_tokens"],
    )])
    .unwrap();
    let built = contract
        .build_node_payloads(RawNodeExecutionResult {
            executor_output: object([
                ("text", json!("accepted")),
                ("usage", json!({ "total_tokens": 128 })),
            ]),
            metrics_facts: Map::new(),
            error_facts: Map::new(),
            debug_facts: Map::new(),
            provider_events: Vec::new(),
        })
        .unwrap();

    assert_eq!(built.output_payload["usage"]["total_tokens"], json!(128));
    assert_eq!(
        contract
            .project_variable_payload(&built.output_payload)
            .unwrap(),
        json!({ "token_usage": 128 })
    );
}

#[test]
fn payload_builder_keeps_runtime_fields_in_complete_output_payload() {
    let contract = PublicOutputContract::from_compiled_outputs(&[output("text")])
        .unwrap()
        .with_structured_expansion(true);
    let raw = RawNodeExecutionResult {
        executor_output: object([
            ("text", json!("accepted")),
            ("metadata", json!({ "provider": "internal" })),
            ("provider_code", json!("openai_compatible")),
            ("queue_snapshot_id", json!("queue-snapshot-1")),
            ("raw_response_ref", json!("artifact-1")),
        ]),
        metrics_facts: Map::new(),
        error_facts: Map::new(),
        debug_facts: Map::new(),
        provider_events: Vec::new(),
    };

    let built = contract.build_node_payloads(raw).unwrap();

    assert_eq!(
        built.output_payload,
        json!({
            "text": "accepted",
            "metadata": { "provider": "internal" },
            "provider_code": "openai_compatible",
            "queue_snapshot_id": "queue-snapshot-1",
            "raw_response_ref": "artifact-1"
        })
    );
}

#[test]
fn payload_builder_allows_context_keys_across_non_public_buckets() {
    let contract = PublicOutputContract::from_compiled_outputs(&[output("text")]).unwrap();
    let raw = RawNodeExecutionResult {
        executor_output: object([("text", json!("visible output"))]),
        metrics_facts: object([("provider_code", json!("openai_compatible"))]),
        error_facts: object([
            ("provider_code", json!("openai_compatible")),
            ("message", json!("provider failed")),
        ]),
        debug_facts: object([("provider_code", json!("openai_compatible"))]),
        provider_events: Vec::new(),
    };

    let built = contract.build_node_payloads(raw).unwrap();

    assert_eq!(
        built.output_payload,
        json!({
            "text": "visible output"
        })
    );
    assert_eq!(
        built.metrics_payload,
        json!({ "provider_code": "openai_compatible" })
    );
    assert_eq!(
        built.error_payload,
        json!({
            "message": "provider failed",
            "provider_code": "openai_compatible"
        })
    );
    assert_eq!(
        built.debug_payload,
        json!({ "provider_code": "openai_compatible" })
    );
}

#[test]
fn payload_builder_keeps_runtime_facts_out_of_output_payload() {
    let contract = PublicOutputContract::from_compiled_outputs(&[output("text")]).unwrap();
    let raw = RawNodeExecutionResult {
        executor_output: object([
            ("text", json!("visible output")),
            ("usage", json!({ "input_tokens": 3, "output_tokens": 5 })),
            ("error", json!({ "message": "provider failed" })),
            ("__raw_response", json!({ "id": "debug-1" })),
        ]),
        metrics_facts: object([("latency_ms", json!(42))]),
        error_facts: object([("retryable", json!(false))]),
        debug_facts: object([("trace_id", json!("trace-1"))]),
        provider_events: Vec::new(),
    };

    let built = contract.build_node_payloads(raw).unwrap();

    assert_eq!(
        built.output_payload,
        json!({
            "__raw_response": { "id": "debug-1" },
            "error": { "message": "provider failed" },
            "text": "visible output",
            "usage": { "input_tokens": 3, "output_tokens": 5 }
        })
    );
}

#[test]
fn payload_builder_keeps_stream_events_in_process_payload_not_output_payload() {
    let contract = PublicOutputContract::from_compiled_outputs(&[output("text")]).unwrap();
    let built = contract
        .build_node_payloads(RawNodeExecutionResult {
            executor_output: object([("text", json!("accepted"))]),
            metrics_facts: Map::new(),
            error_facts: Map::new(),
            debug_facts: Map::new(),
            provider_events: vec![ProviderStreamEvent::TextDelta {
                delta: "accepted".to_string(),
            }],
        })
        .unwrap();

    assert!(built.output_payload.get("provider_events").is_none());
    assert_eq!(
        built.debug_payload["provider_events"][0]["type"],
        json!("text_delta")
    );
}
