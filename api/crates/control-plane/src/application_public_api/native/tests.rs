use serde_json::json;

use super::*;
use crate::application_public_api::mapping::{
    ApplicationApiMappingInput, ApplicationApiMappingOutput,
};

fn request_with_model(model: &str) -> NativeRunRequest {
    serde_json::from_value(json!({
        "query": "hello",
        "model": model,
        "execution": {
            "idempotency_key": "idem-1"
        },
        "metadata": {
            "trace_id": "trace-1"
        }
    }))
    .unwrap()
}

#[test]
fn mapper_rejects_selector_collisions() {
    let mapping = ApplicationApiMappingConfig {
        input: ApplicationApiMappingInput {
            query_target: "start.query".into(),
            model_target: Some("start.query".into()),
            inputs_target: None,
            history_target: None,
            attachments_target: None,
        },
        output: ApplicationApiMappingOutput::default(),
        extension: None,
    };

    let error = NativeInputMapper::map(&request_with_model("any/provider"), &mapping).unwrap_err();

    assert_eq!(
        error,
        NativeInputMappingError::SelectorCollision {
            selector: "start.query".into()
        }
    );
}

#[test]
fn mapper_preserves_model_metadata_when_model_target_is_null() {
    let mapping = ApplicationApiMappingConfig {
        input: ApplicationApiMappingInput {
            query_target: "start.query".into(),
            model_target: None,
            inputs_target: None,
            history_target: None,
            attachments_target: None,
        },
        output: ApplicationApiMappingOutput::default(),
        extension: None,
    };

    let mapped = NativeInputMapper::map(&request_with_model("unlisted-model"), &mapping).unwrap();

    assert!(mapped.node_input_payload["start"].get("model").is_none());
    assert_eq!(mapped.metadata["model"], json!("unlisted-model"));
    assert_eq!(mapped.metadata["idempotency_key"], json!("idem-1"));
    assert_eq!(mapped.metadata["external_trace_id"], json!("trace-1"));
}

#[test]
fn mapper_keeps_requested_model_in_the_existing_start_model_builtin() {
    let mapped = NativeInputMapper::map(
        &request_with_model("provider/requested-model"),
        &ApplicationApiMappingConfig::default_native(),
    )
    .unwrap();

    assert_eq!(
        mapped.node_input_payload["node-start"]["model"],
        json!("provider/requested-model")
    );
    assert!(mapped.node_input_payload["node-start"]
        .get("requested_model")
        .is_none());
}

#[test]
fn mapper_places_tool_registry_under_default_start_input() {
    let request: NativeRunRequest = serde_json::from_value(json!({
        "query": "hello",
        "inputs": {
            "tools": [
                {
                    "name": "read_file",
                    "source": "openai_compatible",
                    "input_schema": {
                        "type": "object"
                    }
                }
            ],
            "tool_choice": "auto"
        }
    }))
    .unwrap();

    let mapped =
        NativeInputMapper::map(&request, &ApplicationApiMappingConfig::default_native()).unwrap();

    assert_eq!(
        mapped.node_input_payload["node-start"]["tools"][0]["name"],
        json!("read_file")
    );
    assert_eq!(
        mapped.node_input_payload["node-start"]["tool_choice"],
        json!("auto")
    );
    assert!(mapped.node_input_payload["node-start"]
        .get("compatibility")
        .is_none());
}

#[test]
fn mapper_places_native_tools_under_query_start_when_inputs_target_is_absent() {
    let request: NativeRunRequest = serde_json::from_value(json!({
        "query": "hello",
        "inputs": {
            "tools": [{"name": "read_file", "input_schema": {"type": "object"}}],
            "tool_choice": "auto"
        }
    }))
    .unwrap();
    let mapping = ApplicationApiMappingConfig {
        input: ApplicationApiMappingInput {
            query_target: "node-start.query".into(),
            model_target: None,
            inputs_target: None,
            history_target: Some("node-start.history".into()),
            attachments_target: None,
        },
        output: ApplicationApiMappingOutput::default(),
        extension: None,
    };

    let mapped = NativeInputMapper::map(&request, &mapping).unwrap();

    assert_eq!(
        mapped.node_input_payload["node-start"]["tools"]
            .as_array()
            .map(Vec::len),
        Some(1)
    );
    assert_eq!(
        mapped.node_input_payload["node-start"]["tool_choice"],
        "auto"
    );
}

#[test]
fn mapper_materializes_empty_typed_start_context() {
    let request: NativeRunRequest = serde_json::from_value(json!({
        "query": "hello"
    }))
    .unwrap();

    let mapped =
        NativeInputMapper::map(&request, &ApplicationApiMappingConfig::default_native()).unwrap();

    assert_eq!(mapped.node_input_payload["node-start"]["system"], json!([]));
    assert_eq!(
        mapped.node_input_payload["node-start"]["operation"],
        json!({"kind": "generate", "profile": "standard"})
    );
    assert_eq!(
        mapped.node_input_payload["node-start"]["history"],
        json!([])
    );
}

#[test]
fn mapper_materializes_only_the_safe_canonical_operation_view() {
    let request: NativeRunRequest = serde_json::from_value(json!({
        "query": "compact",
        "execution": {
            "operation": {
                "kind": "compact",
                "profile": "responses_compaction_v2"
            }
        }
    }))
    .unwrap();

    let mapped =
        NativeInputMapper::map(&request, &ApplicationApiMappingConfig::default_native()).unwrap();

    assert_eq!(
        mapped.node_input_payload["node-start"]["operation"],
        json!({"kind": "compact", "profile": "responses_compaction_v2"})
    );
    assert_eq!(
        mapped.node_input_payload["node-start"]["operation"]
            .as_object()
            .map(|operation| operation.len()),
        Some(2)
    );
}

#[test]
fn mapper_promotes_system_context_out_of_native_history() {
    let request: NativeRunRequest = serde_json::from_value(json!({
        "query": "hello",
        "system": "Use the request system.",
        "history": [
            { "role": "system", "content": "Use the legacy history system." },
            { "role": "user", "content": "Earlier question" }
        ]
    }))
    .unwrap();

    let mapped =
        NativeInputMapper::map(&request, &ApplicationApiMappingConfig::default_native()).unwrap();

    assert_eq!(
        mapped.node_input_payload["node-start"]["system"],
        json!([
            { "type": "text", "text": "Use the request system." },
            { "type": "text", "text": "Use the legacy history system." }
        ])
    );
    assert_eq!(
        mapped.node_input_payload["node-start"]["history"],
        json!([{ "role": "user", "content": "Earlier question" }])
    );
    assert_eq!(
        mapped.node_input_payload[NATIVE_MODEL_PROMPT_CONTEXT_PAYLOAD_KEY],
        json!({
            "system": [
                { "type": "text", "text": "Use the request system." },
                { "type": "text", "text": "Use the legacy history system." }
            ],
            "messages": [
                { "role": "user", "content": "Earlier question" }
            ]
        })
    );
}

#[test]
fn mapper_rebuilds_history_without_unknown_raw_fields() {
    let sentinel = "D2-NATIVE-MAPPER-RAW-HISTORY-MUST-NOT-REACH-MODEL";
    let mut request: NativeRunRequest = serde_json::from_value(json!({
        "query": "hello"
    }))
    .unwrap();
    request.history.push(json!({
        "role": "assistant",
        "content": "prior answer",
        "content_blocks": [
            {
                "type": "image_url",
                "image_url": {"url": "https://example.invalid/image.png"}
            }
        ],
        "raw_provider_body": sentinel
    }));

    let mapped =
        NativeInputMapper::map(&request, &ApplicationApiMappingConfig::default_native()).unwrap();

    let mapped_history = &mapped.node_input_payload["node-start"]["history"];
    assert_eq!(
        mapped_history,
        &json!([
            {
                "role": "assistant",
                "content": "prior answer",
                "content_blocks": [
                    {
                        "type": "image_url",
                        "image_url": {"url": "https://example.invalid/image.png"}
                    }
                ]
            }
        ])
    );
    assert!(!serde_json::to_string(&mapped.node_input_payload)
        .expect("mapped Native input should serialize")
        .contains(sentinel));
}
