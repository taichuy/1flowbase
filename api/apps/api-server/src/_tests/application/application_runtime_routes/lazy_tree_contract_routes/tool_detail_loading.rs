use super::*;

#[tokio::test]
async fn application_runtime_routes_trace_node_content_excludes_tool_index_and_keeps_lazy_tool_detail(
) {
    let (state, _) = test_api_state_with_database_url().await;
    let app = crate::app_with_state_and_config(state.clone(), &test_config());
    let (cookie, csrf) = login_and_capture_cookie(&app, "root", "change-me").await;
    let provider_instance_id = create_ready_provider_instance(&app, &cookie, &csrf).await;
    let application_id =
        seed_agent_flow_application(&app, &cookie, &csrf, &provider_instance_id).await;

    let preview_payload =
        start_llm_preview(&app, &cookie, &csrf, &application_id, "总结退款政策").await;
    let flow_run_id = preview_payload["data"]["flow_run"]["id"].as_str().unwrap();
    let flow_run_uuid = Uuid::parse_str(flow_run_id).unwrap();
    let node_run_id =
        Uuid::parse_str(preview_payload["data"]["node_run"]["id"].as_str().unwrap()).unwrap();

    <MainDurableStore as OrchestrationRuntimeRepository>::update_node_run(
        &state.store,
        &UpdateNodeRunInput {
            node_run_id,
            status: domain::NodeRunStatus::WaitingCallback,
            output_payload: json!({
                "tool_calls": [
                    {
                        "id": "call-refund-policy",
                        "name": "refund_policy_lookup"
                    }
                ]
            }),
            error_payload: None,
            metrics_payload: json!({
                "usage": {
                    "total_tokens": 42
                }
            }),
            debug_payload: json!({
                "tool_callbacks": [
                    {
                        "id": "call-refund-policy",
                        "name": "refund_policy_lookup",
                        "callback_status": "returned",
                        "execution_status": "succeeded"
                    }
                ],
                "llm_rounds": [
                    {
                        "round_index": 0,
                        "assistant": {
                            "role": "assistant",
                            "tool_calls": [
                                {
                                    "id": "call-refund-policy",
                                    "name": "refund_policy_lookup",
                                    "arguments": {
                                        "topic": "refund"
                                    }
                                }
                            ]
                        }
                    }
                ]
            }),
            finished_at: None,
        },
    )
    .await
    .unwrap();

    let callback_task = <MainDurableStore as OrchestrationRuntimeRepository>::create_callback_task(
        &state.store,
        &CreateCallbackTaskInput {
            flow_run_id: flow_run_uuid,
            node_run_id,
            callback_kind: "llm_tool_calls".to_string(),
            request_payload: json!({
                "tool_calls": [
                    {
                        "id": "call-refund-policy",
                        "name": "refund_policy_lookup",
                        "arguments": {
                            "topic": "refund"
                        }
                    }
                ]
            }),
            external_ref_payload: None,
        },
    )
    .await
    .unwrap();
    <MainDurableStore as OrchestrationRuntimeRepository>::complete_callback_task(
        &state.store,
        &CompleteCallbackTaskInput {
            callback_task_id: callback_task.id,
            response_payload: json!({
                "tool_results": [
                    {
                        "tool_call_id": "call-refund-policy",
                        "name": "refund_policy_lookup",
                        "content": "30 days refund window",
                        "execution_status": "succeeded"
                    }
                ]
            }),
            completed_at: callback_task.created_at + time::Duration::milliseconds(1234),
        },
    )
    .await
    .unwrap();
    <MainDurableStore as OrchestrationRuntimeRepository>::append_runtime_events(
        &state.store,
        &[
            AppendRuntimeEventInput {
                flow_run_id: flow_run_uuid,
                node_run_id: Some(node_run_id),
                span_id: None,
                parent_span_id: None,
                event_type: "visible_internal_llm_tool_started".to_string(),
                layer: domain::RuntimeEventLayer::AgentTransition,
                source: domain::RuntimeEventSource::Host,
                trust_level: domain::RuntimeTrustLevel::HostFact,
                item_id: None,
                ledger_ref: None,
                payload: json!({
                    "tool_call_id": "call-refund-policy",
                    "tool_name": "refund_policy_lookup",
                    "main_node_id": "node-llm",
                    "target_node_id": "node-refund-panel",
                    "route_kind": "fusion",
                    "execution_mode": "bounded_parallel_panel",
                    "arguments": {
                        "topic": "refund"
                    }
                }),
                visibility: domain::RuntimeEventVisibility::Workspace,
                durability: domain::RuntimeEventDurability::Durable,
            },
            AppendRuntimeEventInput {
                flow_run_id: flow_run_uuid,
                node_run_id: Some(node_run_id),
                span_id: None,
                parent_span_id: None,
                event_type: "visible_internal_llm_tool_completed".to_string(),
                layer: domain::RuntimeEventLayer::AgentTransition,
                source: domain::RuntimeEventSource::Host,
                trust_level: domain::RuntimeTrustLevel::HostFact,
                item_id: None,
                ledger_ref: None,
                payload: json!({
                    "tool_call_id": "call-refund-policy",
                    "tool_name": "refund_policy_lookup",
                    "main_node_id": "node-llm",
                    "target_node_id": "node-refund-panel",
                    "route_kind": "fusion",
                    "execution_mode": "bounded_parallel_panel",
                    "node_id": "node-refund-panel",
                    "node_alias": "Refund Panel",
                    "node_type": "llm",
                    "provider_route": {
                        "model": "refund-review-v1"
                    },
                    "content": "refund panel says 30 days"
                }),
                visibility: domain::RuntimeEventVisibility::Workspace,
                durability: domain::RuntimeEventDurability::Durable,
            },
        ],
    )
    .await
    .unwrap();

    let trace_tree = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!(
                    "/api/console/applications/{application_id}/logs/runs/{flow_run_id}/trace-tree"
                ))
                .header("cookie", &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(trace_tree.status(), StatusCode::OK);
    let trace_tree_body = to_bytes(trace_tree.into_body(), usize::MAX).await.unwrap();
    let trace_tree_payload: Value = serde_json::from_slice(&trace_tree_body).unwrap();
    let root_nodes = trace_tree_payload["data"]["nodes"].as_array().unwrap();
    let llm_trace_node_id = root_nodes
        .iter()
        .find(|node| node["node_id"] == json!("node-llm"))
        .expect("LLM node should be present")["trace_node_id"]
        .as_str()
        .unwrap();

    let children = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!(
                    "/api/console/applications/{application_id}/logs/runs/{flow_run_id}/trace-tree/nodes?parent_trace_node_id={llm_trace_node_id}"
                ))
                .header("cookie", &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(children.status(), StatusCode::OK);
    let children_body = to_bytes(children.into_body(), usize::MAX).await.unwrap();
    let children_payload: Value = serde_json::from_slice(&children_body).unwrap();
    let children_items = children_payload["data"]["items"].as_array().unwrap();
    assert_eq!(
        children_items.len(),
        1,
        "llm_tool_calls must come back through the projection tools group"
    );
    assert_eq!(children_items[0]["node_kind"], json!("tool_group"));
    let tools_trace_node_id = children_items[0]["trace_node_id"].as_str().unwrap();

    let tool_children = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!(
                    "/api/console/applications/{application_id}/logs/runs/{flow_run_id}/trace-tree/nodes?parent_trace_node_id={tools_trace_node_id}"
                ))
                .header("cookie", &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(tool_children.status(), StatusCode::OK);
    let tool_children_body = to_bytes(tool_children.into_body(), usize::MAX)
        .await
        .unwrap();
    let tool_children_payload: Value = serde_json::from_slice(&tool_children_body).unwrap();
    let tool_child_items = tool_children_payload["data"]["items"].as_array().unwrap();
    assert_eq!(tool_child_items.len(), 1);
    assert_eq!(tool_child_items[0]["node_kind"], json!("tool_callback"));
    assert_eq!(
        tool_child_items[0]["node_alias"],
        json!("refund_policy_lookup")
    );
    assert_eq!(tool_child_items[0]["has_children"], json!(true));
    assert_eq!(tool_child_items[0]["child_count"], json!(1));
    assert_eq!(tool_child_items[0]["has_content"], json!(true));
    let tool_callback_trace_node_id = tool_child_items[0]["trace_node_id"].as_str().unwrap();

    let route_children = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!(
                    "/api/console/applications/{application_id}/logs/runs/{flow_run_id}/trace-tree/nodes?parent_trace_node_id={tool_callback_trace_node_id}"
                ))
                .header("cookie", &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(route_children.status(), StatusCode::OK);
    let route_children_body = to_bytes(route_children.into_body(), usize::MAX)
        .await
        .unwrap();
    let route_children_payload: Value = serde_json::from_slice(&route_children_body).unwrap();
    let route_child_items = route_children_payload["data"]["items"].as_array().unwrap();
    assert_eq!(route_child_items.len(), 1);
    assert_eq!(route_child_items[0]["node_kind"], json!("fusion"));
    assert_eq!(
        route_child_items[0]["node_alias"],
        json!("refund_policy_lookup")
    );
    assert_eq!(route_child_items[0]["has_children"], json!(true));
    assert_eq!(route_child_items[0]["child_count"], json!(1));
    assert_eq!(route_child_items[0]["has_content"], json!(true));
    let fusion_trace_node_id = route_child_items[0]["trace_node_id"].as_str().unwrap();

    let fusion_content = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!(
                    "/api/console/applications/{application_id}/logs/runs/{flow_run_id}/trace-tree/nodes/{fusion_trace_node_id}/content"
                ))
                .header("cookie", &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(fusion_content.status(), StatusCode::OK);
    let fusion_content_body = to_bytes(fusion_content.into_body(), usize::MAX)
        .await
        .unwrap();
    let fusion_content_payload: Value = serde_json::from_slice(&fusion_content_body).unwrap();
    assert_eq!(fusion_content_payload["data"]["node_kind"], json!("fusion"));
    assert_eq!(
        fusion_content_payload["data"]["payload"]["route_kind"],
        json!("fusion")
    );
    assert_eq!(
        fusion_content_payload["data"]["payload"]["branch_traces"][0]["node_alias"],
        json!("Refund Panel")
    );

    let branch_children = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!(
                    "/api/console/applications/{application_id}/logs/runs/{flow_run_id}/trace-tree/nodes?parent_trace_node_id={fusion_trace_node_id}"
                ))
                .header("cookie", &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(branch_children.status(), StatusCode::OK);
    let branch_children_body = to_bytes(branch_children.into_body(), usize::MAX)
        .await
        .unwrap();
    let branch_children_payload: Value = serde_json::from_slice(&branch_children_body).unwrap();
    let branch_child_items = branch_children_payload["data"]["items"].as_array().unwrap();
    assert_eq!(branch_child_items.len(), 1);
    assert_eq!(branch_child_items[0]["node_kind"], json!("branch"));
    assert_eq!(branch_child_items[0]["node_alias"], json!("Refund Panel"));
    assert_eq!(branch_child_items[0]["has_children"], json!(false));
    assert_eq!(branch_child_items[0]["child_count"], json!(0));
    assert_eq!(branch_child_items[0]["has_content"], json!(true));
    let branch_trace_node_id = branch_child_items[0]["trace_node_id"].as_str().unwrap();

    let branch_content = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!(
                    "/api/console/applications/{application_id}/logs/runs/{flow_run_id}/trace-tree/nodes/{branch_trace_node_id}/content"
                ))
                .header("cookie", &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(branch_content.status(), StatusCode::OK);
    let branch_content_body = to_bytes(branch_content.into_body(), usize::MAX)
        .await
        .unwrap();
    let branch_content_payload: Value = serde_json::from_slice(&branch_content_body).unwrap();
    assert_eq!(branch_content_payload["data"]["node_kind"], json!("branch"));
    assert_eq!(
        branch_content_payload["data"]["payload"]["output_payload"]["text"],
        json!("refund panel says 30 days")
    );

    let content = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!(
                    "/api/console/applications/{application_id}/logs/runs/{flow_run_id}/trace-tree/nodes/{llm_trace_node_id}/content"
                ))
                .header("cookie", &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(content.status(), StatusCode::OK);
    let content_body = to_bytes(content.into_body(), usize::MAX).await.unwrap();
    let content_payload: Value = serde_json::from_slice(&content_body).unwrap();
    let node_run = load_trace_node_node_run_detail_payload(
        &app,
        &cookie,
        &application_id,
        flow_run_id,
        llm_trace_node_id,
    )
    .await;
    let debug_payload = &node_run["debug_payload"];
    assert!(
        debug_payload.get("tool_callbacks").is_none(),
        "tool callback summaries should be loaded through projection children"
    );
    assert!(
        debug_payload.get("llm_rounds").is_none(),
        "LLM rounds can regenerate the tool list and must stay out of trace node content"
    );
    assert!(
        content_payload["data"]["payload"].get("node_run").is_none(),
        "trace node content must not duplicate the full node_run in the raw payload"
    );
    assert!(
        content_payload["data"]["payload"]
            .get("checkpoints")
            .is_none(),
        "trace node content must not duplicate typed checkpoint records in the raw payload"
    );
    assert!(
        content_payload["data"]["payload"].get("events").is_none(),
        "trace node content must not duplicate typed event records in the raw payload"
    );

    let detail = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!(
                    "/api/console/applications/{application_id}/logs/runs/{flow_run_id}/trace-tree/nodes/{llm_trace_node_id}/tool-callbacks/call-refund-policy/content"
                ))
                .header("cookie", &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(detail.status(), StatusCode::OK);
    let detail_body = to_bytes(detail.into_body(), usize::MAX).await.unwrap();
    let detail_payload: Value = serde_json::from_slice(&detail_body).unwrap();
    let payload = &detail_payload["data"]["payload"];
    assert_eq!(payload["id"], json!("call-refund-policy"));
    assert_eq!(
        payload["request_payload"]["arguments"]["topic"],
        json!("refund")
    );
    assert_eq!(
        payload["callback_payload"]["content"],
        json!("30 days refund window")
    );
    assert_eq!(
        payload["parsed_result"]["content"],
        json!("30 days refund window")
    );
    assert_eq!(payload["duration_ms"], json!(1234));
    assert_eq!(payload["route_trace"]["route_kind"], json!("fusion"));
    assert_eq!(
        payload["route_trace"]["branch_traces"][0]["node_alias"],
        json!("Refund Panel")
    );
    assert_eq!(
        payload["route_trace"]["branch_traces"][0]["output_payload"]["text"],
        json!("refund panel says 30 days")
    );
}
