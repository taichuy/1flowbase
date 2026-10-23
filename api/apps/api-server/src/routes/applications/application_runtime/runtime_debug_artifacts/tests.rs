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
    assert!(callbacks[0]
        .summary_payload(Uuid::now_v7())
        .get("token_delta")
        .is_none());
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

#[test]
fn artifact_indexed_llm_rounds_count_host_internal_tool_callbacks() {
    let debug_payloads = vec![json!({
        "llm_rounds": {
            "artifact_ref": Uuid::now_v7(),
            "tool_callbacks": [
                {"id": "call-list", "name": "1flowbase_mcp_list"},
                {"id": "call-get", "name": "1flowbase_mcp_get"}
            ]
        }
    })];

    assert_eq!(
        count_llm_tool_callback_trace_items(&debug_payloads, &[], &[]),
        2
    );
}

#[test]
fn llm_tool_callback_count_preserves_mixed_source_identity_rules() {
    let debug_payloads = [
        json!({"llm_rounds": [
            null,
            {
                "assistant": {"tool_calls": [
                    null,
                    {"id": " call-a ", "tool_call_id": "wrong-call", "call_id": "wrong-third"},
                    {"id": "", "tool_call_id": " call-b ", "call_id": "wrong-third"},
                    {"id": 15, "tool_call_id": " ", "call_id": " call-c "},
                    {},
                    {"id": " ", "tool_call_id": "", "call_id": " "}
                ]},
                "assistant_message": {"tool_calls": [{"id": "wrong-assistant-message"}]},
                "tool_calls": [{"id": "wrong-round-call"}],
                "tool_results": [
                    false,
                    {"tool_call_id": " call-a ", "id": "wrong-result", "call_id": "wrong-third"},
                    {"tool_call_id": "", "id": " call-b ", "call_id": "wrong-third"},
                    {"tool_call_id": false, "id": " ", "call_id": " call-c "},
                    {}
                ]
            },
            {
                "round_index": 8,
                "assistant": null,
                "assistant_message": {"tool_calls": [{"id": "wrong-null-assistant"}]},
                "tool_calls": [false, {}]
            },
            {
                "assistant": {"tool_calls": []},
                "assistant_message": {"tool_calls": [{"id": "wrong-empty-assistant"}]},
                "tool_calls": [{"id": "call-d"}]
            },
            {
                "assistant_message": {"tool_calls": [{"id": "call-e"}]},
                "tool_calls": [{"id": "wrong-message-fallback"}]
            },
            {
                "assistant": {"tool_calls": "invalid"},
                "tool_calls": [{"id": "call-f"}]
            },
            {
                "assistant": {"tool_calls": [null]},
                "tool_calls": [{"id": "wrong-malformed-assistant-fallback"}]
            }
        ]}),
        json!({"llm_rounds": {"tool_callbacks": [
            {"id": "call-a"}, {"id": ""}, {"id": " call-a "},
            {"id": null, "tool_call_id": "wrong-artifact-fallback"}
        ]}}),
        json!({"llm_rounds": {"tool_callbacks": [
            null, {"tool_call_id": "call-b"}, {}
        ]}}),
    ];
    let task = |kind: &str, request_payload: Value, response_payload: Option<Value>| {
        domain::CallbackTaskRecord {
            id: Uuid::now_v7(),
            flow_run_id: Uuid::now_v7(),
            node_run_id: Uuid::now_v7(),
            callback_kind: kind.into(),
            status: domain::CallbackTaskStatus::Completed,
            request_payload,
            response_payload,
            external_ref_payload: None,
            created_at: OffsetDateTime::UNIX_EPOCH,
            completed_at: Some(OffsetDateTime::UNIX_EPOCH),
        }
    };
    let tasks = [
        task(
            "llm_tool_calls",
            json!({"tool_calls": [null, {"id": "call-a", "tool_call_id": "wrong-request"}, {}, {"id": "call-b"}]}),
            Some(
                json!({"tool_results": [false, {"tool_call_id": "call-a", "id": "wrong-response"}, {}, {"tool_call_id": "call-b"}]}),
            ),
        ),
        task(
            "llm_tool_calls",
            json!({"id": "call-c", "tool_call_id": "wrong-singleton-request"}),
            Some(json!({"tool_call_id": "call-d", "id": "wrong-singleton-response"})),
        ),
        task(
            "llm_tool_calls",
            json!({"id": "", "tool_call_id": " ", "call_id": " call-e ", "tool_calls": null}),
            Some(json!({"tool_results": {}, "tool_call_id": " ", "id": "", "call_id": " call-f "})),
        ),
        task(
            "llm_tool_calls",
            json!({"id": "wrong-empty-request", "tool_calls": []}),
            Some(json!({"tool_call_id": "wrong-empty-response", "tool_results": []})),
        ),
        task("llm_tool_calls", json!({}), Some(json!({}))),
        task(
            "llm_tool_calls",
            json!({"tool_calls": [{"id": "request-only"}]}),
            None,
        ),
        task(
            "other",
            json!({"tool_calls": [{"id": "wrong-kind-request"}]}),
            Some(json!({"tool_results": [{"tool_call_id": "wrong-kind-response"}]})),
        ),
    ];

    // Six named calls, four positional round IDs, and five additional artifact IDs.
    assert_eq!(
        count_llm_tool_callback_trace_items(&debug_payloads, &[], &[]),
        15
    );
    // Tasks add only tool-1-3, tool-result-1-3, and request-only.
    assert_eq!(
        count_llm_tool_callback_trace_items(&debug_payloads, &tasks, &[]),
        18
    );

    let mut native_messages = [
        "call-a",
        "tool-2-5",
        "tool-2-6",
        "tool-result-2-5",
        "tool-9-2",
        "tool-1-3",
        "tool-result-1-3",
        "artifact_tool_callback_3",
        "artifact_tool_callback_4",
        "artifact_tool_callback_6",
        "",
        " call-a ",
    ]
    .map(|id| json!({"_source_item": {"type": "function_call", "call_id": id}}))
    .to_vec();
    native_messages.extend([
        json!({"_source_item": {"type": "custom_tool_call", "call_id": "native-only"}}),
        json!({"_source_item": {"type": "function_call", "call_id": " "}}),
        json!({"_source_item": {"type": "function_call", "call_id": " "}}),
        json!({"_source_item": {"type": "function_call_output", "call_id": "wrong-output"}}),
        json!({"_source_item": {"type": "custom_tool_call", "call_id": 9}}),
        json!({"_source_item": {"type": "function_call", "id": "wrong-native-key"}}),
        json!({"type": "function_call", "call_id": "wrong-top-level"}),
    ]);
    assert_eq!(
        count_llm_tool_callback_trace_items(&debug_payloads, &tasks, &native_messages),
        20
    );
}

#[test]
fn llm_tool_callback_count_ignores_bodies_runtime_facts_and_route_traces() {
    let mut debug_payloads = [json!({"llm_rounds": [{
        "assistant": {"tool_calls": [{"id": "shared"}]},
        "tool_results": [{"tool_call_id": "shared"}]
    }]})];
    let mut tasks = [domain::CallbackTaskRecord {
        id: Uuid::now_v7(),
        flow_run_id: Uuid::now_v7(),
        node_run_id: Uuid::now_v7(),
        callback_kind: "llm_tool_calls".into(),
        status: domain::CallbackTaskStatus::Completed,
        request_payload: json!({"tool_calls": [{"id": "shared"}]}),
        response_payload: Some(json!({"tool_results": [{"tool_call_id": "shared"}]})),
        external_ref_payload: None,
        created_at: OffsetDateTime::UNIX_EPOCH,
        completed_at: Some(OffsetDateTime::UNIX_EPOCH),
    }];
    assert_eq!(
        count_llm_tool_callback_trace_items(&debug_payloads, &tasks, &[]),
        1
    );

    let body =
        json!({"nested": [{"tool_calls": [{"id": "body-only"}], "text": "x".repeat(256 * 1024)}]});
    debug_payloads[0]["llm_rounds"][0]["assistant"]["tool_calls"][0]["arguments"] = body.clone();
    debug_payloads[0]["llm_rounds"][0]["tool_results"][0]["content"] = body.clone();
    debug_payloads[0]["llm_rounds"][0]["usage"] = body.clone();
    debug_payloads[0]["unrelated"] = body.clone();
    debug_payloads[0]["visible_internal_llm_tool_trace"] = json!([
        {"tool_call_id": "shared", "status": "failed", "branch_traces": body},
        {"tool_call_id": "trace-only", "status": "succeeded", "arguments": body}
    ]);
    tasks[0].request_payload["tool_calls"][0]["arguments"] = body.clone();
    tasks[0].response_payload.as_mut().unwrap()["tool_results"][0]["content"] = body;
    tasks[0].completed_at = Some(OffsetDateTime::UNIX_EPOCH + Duration::hours(1));

    assert_eq!(
        count_llm_tool_callback_trace_items(&debug_payloads, &tasks, &[]),
        1
    );
}

#[test]
fn llm_tool_callback_summary_preserves_trace_fields_and_full_artifact() {
    let rounds = json!([{"tool_calls": [{"id": "call_summary", "name": "lookup"}]}]);
    let trace = json!({
        "tool_call_id": "call_summary",
        "arguments": {"private": "argument body"},
        "status": "succeeded",
        "branch_traces": [{"private": "branch body"}],
        "callback_requests": ["callback body"],
        "events": ["event body"],
        "final_output": "final body",
        "main_resume_output": "resume body",
        "route_output": "route body",
        "tool_call": {"private": "call body"},
        "tool_result": {"private": "result body"},
        "unknown_extension": {"retained": true},
        "nullable_extension": null
    });
    let mut callbacks = collect_llm_tool_callbacks(&rounds, &HashMap::new());
    attach_inline_route_traces(
        &mut callbacks,
        &[json!({"visible_internal_llm_tool_trace": [trace.clone()]})],
    );
    let summary = callbacks[0].summary_payload(Uuid::nil());
    assert_eq!(summary["execution_status"], "succeeded");
    assert_eq!(summary["artifact_ref"], Uuid::nil().to_string());
    assert_eq!(
        summary["route_trace"],
        json!({
            "tool_call_id": "call_summary", "status": "succeeded",
            "unknown_extension": {"retained": true}, "nullable_extension": null
        })
    );
    let mut retained = trace.as_object().unwrap().clone();
    for field in [
        "arguments",
        "branch_traces",
        "callback_requests",
        "events",
        "final_output",
        "main_resume_output",
        "route_output",
        "tool_call",
        "tool_result",
    ] {
        retained.remove(field);
    }
    assert_eq!(
        summary["route_trace"].to_string(),
        Value::Object(retained).to_string()
    );
    assert_eq!(callbacks[0].detail_payload()["route_trace"], trace);
    assert!(
        collect_llm_tool_callbacks(&rounds, &HashMap::new())[0].summary_payload(Uuid::nil())
            ["route_trace"]
            .is_null()
    );
}
