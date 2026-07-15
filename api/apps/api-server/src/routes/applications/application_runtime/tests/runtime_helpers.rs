use super::*;

#[test]
fn runtime_event_cursor_accepts_numeric_and_run_scoped_event_ids() {
    let run_id = Uuid::now_v7();

    assert_eq!(parse_runtime_event_cursor(run_id, "7"), Some(7));
    assert_eq!(
        parse_runtime_event_cursor(run_id, &format!("{run_id}:8")),
        Some(8)
    );
    assert_eq!(
        parse_runtime_event_cursor(Uuid::now_v7(), &format!("{run_id}:8")),
        None
    );
    assert_eq!(parse_runtime_event_cursor(run_id, "not-a-cursor"), None);
}

#[test]
fn metrics_payload_cache_hit_tokens_accepts_cache_read_tokens() {
    assert_eq!(
        metrics_payload_cache_hit_tokens(&serde_json::json!({
            "usage": {
                "input_cache_hit_tokens": null,
                "cache_read_tokens": 29_504
            }
        })),
        Some(29_504)
    );
    assert_eq!(
        metrics_payload_cache_hit_tokens(&serde_json::json!({
            "usage": {
                "input_cache_hit_tokens": 11,
                "cache_read_tokens": 29_504
            }
        })),
        Some(11)
    );
}

#[test]
fn debug_run_stream_cursor_prefers_query_before_last_event_id_header() {
    let run_id = Uuid::now_v7();
    let mut headers = HeaderMap::new();
    headers.insert(
        "last-event-id",
        HeaderValue::from_str(&format!("{run_id}:11")).unwrap(),
    );

    assert_eq!(
        debug_run_stream_from_sequence(
            run_id,
            &DebugRunStreamQuery {
                from_sequence: Some(9),
                last_event_id: Some(format!("{run_id}:10")),
            },
            &headers,
        ),
        Some(9)
    );
    assert_eq!(
        debug_run_stream_from_sequence(
            run_id,
            &DebugRunStreamQuery {
                from_sequence: None,
                last_event_id: Some(format!("{run_id}:10")),
            },
            &headers,
        ),
        Some(10)
    );
    assert_eq!(
        debug_run_stream_from_sequence(
            run_id,
            &DebugRunStreamQuery {
                from_sequence: None,
                last_event_id: None,
            },
            &headers,
        ),
        Some(11)
    );
}

#[test]
fn usage_total_tokens_uses_total_or_known_segments() {
    assert_eq!(
        usage_total_tokens(&serde_json::json!({
            "total_tokens": 128,
            "input_tokens": 40,
            "output_tokens": 12
        })),
        Some(128)
    );
    assert_eq!(
        usage_total_tokens(&serde_json::json!({
            "input_tokens": 40,
            "output_tokens": 12,
            "reasoning_tokens": 6
        })),
        Some(58)
    );
    assert_eq!(usage_total_tokens(&serde_json::json!({})), None);
}

#[test]
fn callback_task_tool_callback_count_reads_offloaded_tool_call_count() {
    let base_task = domain::CallbackTaskRecord {
        id: Uuid::now_v7(),
        flow_run_id: Uuid::now_v7(),
        node_run_id: Uuid::now_v7(),
        callback_kind: "llm_tool_calls".to_string(),
        status: domain::CallbackTaskStatus::Completed,
        request_payload: serde_json::json!({
            "tool_calls": [
                { "id": "call-1" },
                { "id": "call-2" }
            ]
        }),
        response_payload: None,
        external_ref_payload: None,
        created_at: OffsetDateTime::UNIX_EPOCH,
        completed_at: Some(OffsetDateTime::UNIX_EPOCH),
    };
    assert_eq!(callback_task_tool_callback_count(&base_task), 2);

    let offloaded_task = domain::CallbackTaskRecord {
        request_payload: serde_json::json!({
            "tool_calls": {
                "__runtime_debug_artifact": true,
                "artifact_ref": Uuid::now_v7().to_string(),
                "tool_call_count": 3
            }
        }),
        ..base_task
    };
    assert_eq!(callback_task_tool_callback_count(&offloaded_task), 3);
}

#[test]
fn application_run_statistics_counts_indexed_llm_tool_callbacks() {
    let application = test_application_record();
    let flow_run_id = Uuid::now_v7();
    let node_run_id = Uuid::now_v7();
    let callback_task = domain::CallbackTaskRecord {
        id: Uuid::now_v7(),
        flow_run_id,
        node_run_id,
        callback_kind: "llm_tool_calls".to_string(),
        status: domain::CallbackTaskStatus::Completed,
        request_payload: serde_json::json!({
            "tool_calls": [
                { "id": "call-1", "name": "Read" }
            ]
        }),
        response_payload: None,
        external_ref_payload: None,
        created_at: OffsetDateTime::UNIX_EPOCH,
        completed_at: Some(OffsetDateTime::UNIX_EPOCH),
    };
    let detail = domain::ApplicationRunDetail {
        flow_run: test_flow_run_record(
            application.id,
            flow_run_id,
            domain::FlowRunStatus::Succeeded,
            serde_json::json!({}),
        ),
        node_runs: vec![domain::NodeRunRecord {
            id: node_run_id,
            flow_run_id,
            node_id: "node-llm".to_string(),
            node_type: "llm".to_string(),
            node_alias: "LLM".to_string(),
            status: domain::NodeRunStatus::Succeeded,
            input_payload: serde_json::json!({}),
            output_payload: serde_json::json!({}),
            error_payload: None,
            metrics_payload: serde_json::json!({}),
            debug_payload: serde_json::json!({
                "llm_rounds": [
                    {
                        "round_index": 0,
                        "assistant": {
                            "tool_calls": [
                                { "id": "call-1", "name": "Read" },
                                { "id": "call-2", "name": "problem_review" }
                            ]
                        }
                    }
                ]
            }),
            started_at: OffsetDateTime::UNIX_EPOCH,
            finished_at: Some(OffsetDateTime::UNIX_EPOCH),
        }],
        checkpoints: Vec::new(),
        callback_tasks: vec![callback_task],
        events: Vec::new(),
        stitched_trace: Vec::new(),
        subagent_traces: Vec::new(),
    };

    let statistics = application_run_statistics(&detail);

    assert_eq!(statistics.tool_callback_count, 2);
}

#[test]
fn trace_tree_endpoints_read_projection_without_full_detail_fallback() {
    let log_endpoint_source = include_str!("../log_handlers.rs");

    for function_name in [
        "get_application_run_trace_tree",
        "get_application_run_trace_node_children",
        "get_application_run_trace_node_content",
        "get_application_run_trace_tool_callback_content",
    ] {
        let function_source =
            application_trace_tree_endpoint_source(log_endpoint_source, function_name);

        assert!(
            function_source.contains("ensure_application_run_trace_projection_status"),
            "{function_name} must enter through projection status"
        );
        assert!(
            !function_source.contains("get_application_run_detail"),
            "{function_name} must not fallback to full run detail reads"
        );
    }

    assert!(application_trace_tree_endpoint_source(
        log_endpoint_source,
        "get_application_run_trace_tree"
    )
    .contains("list_application_run_trace_roots"));
    assert!(application_trace_tree_endpoint_source(
        log_endpoint_source,
        "get_application_run_trace_tree"
    )
    .contains("get_application_run_trace_statistics"));
    assert!(!application_trace_tree_endpoint_source(
        log_endpoint_source,
        "get_application_run_trace_tree"
    )
    .contains("list_application_run_trace_nodes_for_statistics"));
    assert!(application_trace_tree_endpoint_source(
        log_endpoint_source,
        "get_application_run_trace_node_children"
    )
    .contains("list_application_run_trace_children"));
    assert!(
        application_trace_tree_endpoint_source(
            log_endpoint_source,
            "get_application_run_trace_node_content"
        )
        .contains(
            "<MainDurableStore as OrchestrationRuntimeRepository>::get_application_run_trace_node_content"
        )
    );
    assert!(
        application_trace_tree_endpoint_source(
            log_endpoint_source,
            "get_application_run_trace_tool_callback_content"
        )
        .contains(
            "<MainDurableStore as OrchestrationRuntimeRepository>::get_application_run_trace_node_content"
        )
    );
}

#[test]
fn run_conversation_messages_endpoint_reads_projection_without_full_detail_fallback() {
    let function_source = application_runtime_function_source(
        include_str!("../log_handlers.rs"),
        "pub async fn list_application_run_conversation_messages",
    );
    let function_source = function_source
        .split("\nasync fn ensure_application_run_trace_projection_status")
        .next()
        .expect("run conversation endpoint source should precede trace projection helper");

    assert!(
        function_source.contains("list_application_run_conversation_message_items_page"),
        "run conversation endpoint must read the run-specific projection page"
    );
    assert!(
        function_source.contains("get_application_run_conversation_current_item"),
        "run conversation endpoint must use bounded current-item fallback for non-terminal or missing projection"
    );
    assert!(
        !function_source.contains("get_application_run_detail"),
        "run conversation endpoint must not materialize full run detail"
    );
    assert!(
        !function_source.contains("conversation_messages_from_run_detail"),
        "run conversation endpoint fallback must not scan node runs or debug detail fragments"
    );
}

#[test]
fn trace_projection_status_ensure_checks_lightweight_watermark_before_full_source() {
    let function_source = application_runtime_function_source(
        include_str!("../log_handlers.rs"),
        "async fn ensure_application_run_trace_projection_status",
    );
    let watermark_query = function_source
        .find("get_application_run_trace_projection_source_watermark")
        .expect("trace projection ensure must read a lightweight source watermark");
    let full_source_query = function_source
        .find("get_application_run_trace_projection_source(")
        .expect("trace projection ensure may read the full source only for rebuild");

    assert!(
        watermark_query < full_source_query,
        "trace projection ensure must decide unchanged succeeded projections before loading the full detail source"
    );
}

#[test]
fn trace_node_content_response_serializes_refs_without_heavy_containers() {
    let trace_node_id = Uuid::now_v7();
    let flow_run_id = Uuid::now_v7();
    let now = OffsetDateTime::UNIX_EPOCH;
    let response = trace_projection_node_content_response(
        domain::ApplicationRunTraceNodeRecord {
            trace_node_id,
            flow_run_id,
            parent_trace_node_id: None,
            stable_locator: format!("run:{flow_run_id}/node:{}", Uuid::now_v7()),
            node_kind: "node_run".to_string(),
            owner_kind: Some("node_run".to_string()),
            owner_id: Some(Uuid::now_v7().to_string()),
            order_key: "000001".to_string(),
            node_id: Some("node-llm".to_string()),
            node_type: Some("llm".to_string()),
            node_mode: None,
            node_alias: "LLM".to_string(),
            status: "succeeded".to_string(),
            started_at: now,
            finished_at: Some(now),
            duration_ms: Some(0),
            metrics_payload: serde_json::json!({}),
            has_children: false,
            child_count: 0,
            has_content: true,
            content_ref: None,
            source_flow_run_id: None,
            source_trace_node_id: None,
            parent_callback_task_id: None,
            parent_tool_call_id: None,
            trace_relation_kind: None,
            projection_version: APPLICATION_RUN_TRACE_PROJECTION_VERSION,
            source_watermark: "source:1".to_string(),
            created_at: now,
            updated_at: now,
        },
        domain::ApplicationRunTraceNodeContentRecord {
            trace_node_id,
            content_kind: "node_run".to_string(),
            payload: serde_json::json!({
                "payload_index": {
                    "node_run_count": 1,
                    "checkpoint_count": 1,
                    "event_count": 1
                },
                "detail_refs": [
                    {
                        "detail_kind": "node_run",
                        "source_kind": "node_run",
                        "source_locator": "node-run-1",
                        "count": 1
                    }
                ]
            }),
            source_refs: serde_json::json!([
                {
                    "source_kind": "node_run",
                    "source_locator": "node-run-1"
                }
            ]),
            created_at: now,
            updated_at: now,
        },
        ApplicationRunTraceProjectionStatusResponse {
            projection_status: "succeeded".to_string(),
            projection_version: APPLICATION_RUN_TRACE_PROJECTION_VERSION,
            source_watermark: "source:1".to_string(),
            attempt_count: 1,
            last_attempt_at: Some(format_time(now)),
            last_success_at: Some(format_time(now)),
            last_error_code: None,
            last_error_stage: None,
            last_error_source_kind: None,
            last_error_source_locator: None,
            last_error_ref: None,
            retriable: false,
        },
    )
    .expect("trace node content response should serialize");
    let serialized = serde_json::to_value(response).expect("response should be JSON");
    let data = serialized
        .as_object()
        .expect("trace node content response should be an object");

    assert!(!data.contains_key("node_run"));
    assert!(!data.contains_key("checkpoints"));
    assert!(!data.contains_key("events"));
    assert_eq!(serialized["content_kind"], serde_json::json!("node_run"));
    assert_eq!(
        serialized["payload"]["payload_index"]["node_run_count"],
        serde_json::json!(1)
    );
    assert_eq!(
        serialized["detail_refs"][0]["detail_kind"],
        serde_json::json!("node_run")
    );
    assert_eq!(
        serialized["source_refs"][0]["source_kind"],
        serde_json::json!("node_run")
    );
}

#[test]
fn trace_node_content_raw_payload_keeps_empty_payload_as_object() {
    let payload = trace_node_content_raw_payload_response(serde_json::json!({
        "node_run": {
            "id": "node-run-1"
        },
        "checkpoints": [],
        "events": [],
        "detail_refs": [],
        "source_refs": []
    }));

    assert_eq!(payload, serde_json::json!({}));
}
