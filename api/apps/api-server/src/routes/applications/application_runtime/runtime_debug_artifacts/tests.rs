use serde_json::json;
use time::{Duration, OffsetDateTime};

use super::payloads::{
    application_run_model, application_run_query, application_run_system,
    is_safe_to_persist_debug_artifact_previews, should_keep_runtime_payload_field_inline,
    with_application_run_input_summary, with_debug_artifact_field_path,
};
use super::*;

#[test]
fn application_query_prefers_start_query_over_tool_schema_payload() {
    let payload = json!({
        "node-start": {
            "query": "ping",
            "model": "gpt-test",
            "compatibility": {
                "tools": [
                    {
                        "function": {
                            "description": "path to the file to read.",
                            "parameters": {
                                "properties": {
                                    "file_path": {
                                        "description": "path to the file to read."
                                    }
                                }
                            }
                        }
                    }
                ]
            }
        }
    });

    assert_eq!(application_run_query(&payload), Some("ping".into()));
    assert_eq!(application_run_model(&payload), Some("gpt-test".into()));
}

#[test]
fn application_query_reads_persisted_artifact_preview_summary() {
    let payload = json!({
        "__runtime_debug_artifact": true,
        "artifact_ref": Uuid::now_v7().to_string(),
        "preview": "{\"node-start\":{\"compatibility\":{\"tools\":[]}}}",
        "query": "总结退款政策",
        "model": "deepseek-chat",
        "system": "Use concise Chinese."
    });

    assert_eq!(application_run_query(&payload), Some("总结退款政策".into()));
    assert_eq!(
        application_run_model(&payload),
        Some("deepseek-chat".into())
    );
    assert_eq!(
        application_run_system(&payload),
        Some("Use concise Chinese.".into())
    );
}

#[test]
fn flow_input_artifact_preview_keeps_application_query_model_and_system() {
    let preview = json!({
        "__runtime_debug_artifact": true,
        "artifact_ref": Uuid::now_v7().to_string(),
        "preview": "{\"node-start\":{\"compatibility\":{\"tools\":[]}}}"
    });

    let preview = with_application_run_input_summary(
        preview,
        Some("ping"),
        Some("gpt-test"),
        Some("Use concise Chinese."),
    );

    assert_eq!(preview["query"], json!("ping"));
    assert_eq!(preview["model"], json!("gpt-test"));
    assert_eq!(preview["system"], json!("Use concise Chinese."));
}

#[test]
fn runtime_payload_field_artifact_selection_keeps_truth_fields_inline() {
    assert!(should_keep_runtime_payload_field_inline(&["query".into()]));
    assert!(should_keep_runtime_payload_field_inline(&[
        "node-start".into(),
        "model".into()
    ]));
    assert!(should_keep_runtime_payload_field_inline(&[
        "node-start".into(),
        "system".into()
    ]));
    assert!(should_keep_runtime_payload_field_inline(&[
        "input".into(),
        "sys".into(),
        "workflow_run_id".into()
    ]));
    assert!(should_keep_runtime_payload_field_inline(&[
        "env".into(),
        "ApiBaseUrl".into()
    ]));
    assert!(should_keep_runtime_payload_field_inline(&["files".into()]));
    assert!(!should_keep_runtime_payload_field_inline(&[
        "history".into()
    ]));
    assert!(!should_keep_runtime_payload_field_inline(&[
        "compatibility".into(),
        "tools".into()
    ]));
}

#[test]
fn runtime_payload_field_artifact_wrapper_keeps_original_field_path() {
    let payload = json!({
        "__runtime_debug_artifact": true,
        "artifact_ref": Uuid::now_v7().to_string(),
        "preview": "[{\"role\":\"user\"}]"
    });

    let payload = with_debug_artifact_field_path(payload, &["history".into()]);

    assert_eq!(payload["artifact_scope"], json!("field"));
    assert_eq!(payload["field_path"], json!(["history"]));
}

#[test]
fn paused_callback_statuses_can_persist_debug_artifact_previews() {
    assert!(is_safe_to_persist_debug_artifact_previews(
        domain::FlowRunStatus::WaitingCallback
    ));
    assert!(is_safe_to_persist_debug_artifact_previews(
        domain::FlowRunStatus::WaitingHuman
    ));
    assert!(!is_safe_to_persist_debug_artifact_previews(
        domain::FlowRunStatus::Running
    ));
}

#[test]
fn llm_tool_callback_execution_status_requires_explicit_execution_fact() {
    assert_eq!(
        execution_status_from_callback_payload(Some(&json!({
            "tool_call_id": "call_weather",
            "content": "{\"temperature\":21}"
        }))),
        "unknown"
    );
    assert_eq!(
        execution_status_from_callback_payload(Some(&json!({
            "tool_call_id": "call_read",
            "execution": {
                "status": "failed"
            }
        }))),
        "failed"
    );
    assert_eq!(
        execution_status_from_callback_payload(Some(&json!({
            "tool_call_id": "call_glob",
            "exit_code": 0,
            "content": []
        }))),
        "succeeded"
    );
    assert_eq!(
        execution_status_from_callback_payload(Some(&json!({
            "tool_call_id": "call_http",
            "http_status": 500
        }))),
        "failed"
    );
}

#[test]
fn llm_tool_callback_execution_status_prefers_intercepted_route_trace() {
    assert_eq!(
        execution_status_from_callback_payload_and_route_trace(
            Some(&json!({
                "tool_call_id": "call_image",
                "execution_status": "succeeded"
            })),
            Some(&json!({
                "kind": "visible_internal_llm_tool_trace",
                "tool_call_id": "call_image",
                "status": "intercepted"
            })),
        ),
        "intercepted"
    );
    assert_eq!(
        execution_status_from_callback_payload_and_route_trace(
            Some(&json!({
                "tool_call_id": "call_weather",
                "content": "{\"temperature\":21}"
            })),
            Some(&json!({
                "kind": "visible_internal_llm_tool_trace",
                "tool_call_id": "call_weather",
                "status": "failed"
            })),
        ),
        "failed"
    );
}

#[test]
fn llm_tool_callback_payloads_keep_context_usage_without_token_delta() {
    let rounds = json!([
        {
            "round_index": 0,
            "usage": {
                "total_tokens": 8122
            },
            "assistant": {
                "role": "assistant",
                "content": "need tool",
                "tool_calls": [
                    {
                        "id": "call_weather",
                        "name": "lookup_weather"
                    }
                ]
            }
        },
        {
            "round_index": 1,
            "tool_results": [
                {
                    "role": "tool",
                    "tool_call_id": "call_weather",
                    "content": "{\"temperature\":21}"
                }
            ]
        },
        {
            "round_index": 2,
            "usage": {
                "total_tokens": 8224
            },
            "assistant": {
                "role": "assistant",
                "content": "continue"
            }
        }
    ]);

    let callbacks = collect_llm_tool_callbacks(&rounds, &std::collections::HashMap::new());

    assert_eq!(callbacks.len(), 1);
    assert_eq!(
        callbacks[0].detail_payload()["call_usage"]["total_tokens"],
        json!(8122)
    );
    assert_eq!(
        callbacks[0].detail_payload()["result_context_usage"]["total_tokens"],
        json!(8224)
    );
    assert!(callbacks[0].detail_payload().get("token_delta").is_none());
    assert!(
        callbacks[0]
            .summary_payload(Uuid::now_v7())
            .get("token_delta")
            .is_none()
    );
}

#[test]
fn llm_tool_callback_payloads_include_callback_duration_ms() {
    let task = domain::CallbackTaskRecord {
        id: Uuid::now_v7(),
        flow_run_id: Uuid::now_v7(),
        node_run_id: Uuid::now_v7(),
        callback_kind: "llm_tool_calls".into(),
        status: domain::CallbackTaskStatus::Completed,
        request_payload: json!({}),
        response_payload: Some(json!({
            "tool_results": [
                {
                    "tool_call_id": "call_weather",
                    "content": "{\"temperature\":21}"
                }
            ]
        })),
        external_ref_payload: None,
        created_at: OffsetDateTime::UNIX_EPOCH,
        completed_at: Some(OffsetDateTime::UNIX_EPOCH + Duration::milliseconds(1234)),
    };
    let rounds = json!([
        {
            "round_index": 0,
            "usage": { "total_tokens": 8122 },
            "assistant": {
                "tool_calls": [
                    {
                        "id": "call_weather",
                        "name": "lookup_weather"
                    }
                ]
            }
        },
        {
            "round_index": 1,
            "tool_results": [
                {
                    "role": "tool",
                    "tool_call_id": "call_weather",
                    "result_context_usage": { "total_tokens": 8224 },
                    "content": "{\"temperature\":21}"
                }
            ]
        }
    ]);

    let callback_facts = collect_llm_tool_callback_runtime_facts(&[task]);
    let callbacks = collect_llm_tool_callbacks(&rounds, &callback_facts);
    let (enriched_rounds, changed) = with_llm_tool_callback_runtime_facts(rounds, &callback_facts);

    assert!(changed);
    assert_eq!(callbacks[0].detail_payload()["duration_ms"], json!(1234));
    assert_eq!(
        callbacks[0].summary_payload(Uuid::now_v7())["duration_ms"],
        json!(1234)
    );
    assert_eq!(
        enriched_rounds[1]["tool_results"][0]["duration_ms"],
        json!(1234)
    );
}
