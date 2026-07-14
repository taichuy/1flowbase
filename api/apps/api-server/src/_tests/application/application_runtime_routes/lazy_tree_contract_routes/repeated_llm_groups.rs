use super::*;

#[tokio::test]
async fn application_runtime_routes_trace_tree_groups_repeated_llm_node_runs_at_root() {
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
    let first_node_run_id =
        Uuid::parse_str(preview_payload["data"]["node_run"]["id"].as_str().unwrap()).unwrap();

    <MainDurableStore as OrchestrationRuntimeRepository>::update_node_run(
        &state.store,
        &UpdateNodeRunInput {
            node_run_id: first_node_run_id,
            status: domain::NodeRunStatus::Succeeded,
            output_payload: json!({
                "tool_calls": [
                    {
                        "id": "call_weather",
                        "name": "lookup_weather"
                    }
                ]
            }),
            error_payload: None,
            metrics_payload: json!({
                "usage": {
                    "total_tokens": 14
                }
            }),
            debug_payload: json!({
                "llm_rounds": [
                    {
                        "round_index": 0,
                        "assistant": {
                            "role": "assistant",
                            "tool_calls": [
                                {
                                    "id": "call_weather",
                                    "name": "lookup_weather"
                                }
                            ]
                        }
                    }
                ]
            }),
            finished_at: Some(time::OffsetDateTime::now_utc()),
        },
    )
    .await
    .unwrap();

    let second_node_run = <MainDurableStore as OrchestrationRuntimeRepository>::create_node_run(
        &state.store,
        &CreateNodeRunInput {
            flow_run_id: flow_run_uuid,
            node_id: "node-llm".to_string(),
            node_type: "llm".to_string(),
            node_alias: "LLM".to_string(),
            status: domain::NodeRunStatus::WaitingCallback,
            input_payload: json!({
                "prompt": "continue refund policy"
            }),
            debug_payload: json!({
                "llm_rounds": [
                    {
                        "round_index": 1,
                        "assistant": {
                            "role": "assistant",
                            "tool_calls": [
                                {
                                    "id": "call_policy",
                                    "name": "read_policy"
                                }
                            ]
                        }
                    }
                ]
            }),
            started_at: time::OffsetDateTime::now_utc(),
        },
    )
    .await
    .unwrap();
    <MainDurableStore as OrchestrationRuntimeRepository>::update_node_run(
        &state.store,
        &UpdateNodeRunInput {
            node_run_id: second_node_run.id,
            status: domain::NodeRunStatus::WaitingCallback,
            output_payload: json!({
                "tool_calls": [
                    {
                        "id": "call_policy",
                        "name": "read_policy"
                    }
                ]
            }),
            error_payload: None,
            metrics_payload: json!({
                "usage": {
                    "total_tokens": 24
                }
            }),
            debug_payload: json!({
                "llm_rounds": [
                    {
                        "round_index": 1,
                        "assistant": {
                            "role": "assistant",
                            "tool_calls": [
                                {
                                    "id": "call_policy",
                                    "name": "read_policy"
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

    assert_eq!(
        root_nodes.len(),
        1,
        "trace root should expose one display node for repeated LLM node runs"
    );
    assert_eq!(root_nodes[0]["node_id"], json!("node-llm"));
    assert_eq!(root_nodes[0]["status"], json!("waiting_callback"));
    let trace_node_id = root_nodes[0]["trace_node_id"].as_str().unwrap();
    Uuid::parse_str(trace_node_id).expect("trace_node_id is deterministic UUID");
    assert!(root_nodes[0]["stable_locator"]
        .as_str()
        .unwrap()
        .contains("/node_group:"));

    let content = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!(
                    "/api/console/applications/{application_id}/logs/runs/{flow_run_id}/trace-tree/nodes/{trace_node_id}/content"
                ))
                .header("cookie", &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(content.status(), StatusCode::OK);
    let content_body = to_bytes(content.into_body(), usize::MAX).await.unwrap();
    let _content_payload: Value = serde_json::from_slice(&content_body).unwrap();
    let node_run = load_trace_node_node_run_detail_payload(
        &app,
        &cookie,
        &application_id,
        flow_run_id,
        trace_node_id,
    )
    .await;
    assert!(
        node_run["debug_payload"].get("llm_rounds").is_none(),
        "grouped LLM rounds can regenerate tool lists and stay behind children"
    );
    assert!(
        node_run["debug_payload"].get("tool_callbacks").is_none(),
        "grouped LLM tool callbacks are loaded through projection children"
    );
    assert_eq!(
        node_run["output_payload"]["tool_calls"][0]["id"],
        json!("call_policy")
    );

    let tools = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!(
                    "/api/console/applications/{application_id}/logs/runs/{flow_run_id}/trace-tree/nodes?parent_trace_node_id={trace_node_id}"
                ))
                .header("cookie", &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(tools.status(), StatusCode::OK);
    let tools_body = to_bytes(tools.into_body(), usize::MAX).await.unwrap();
    let tools_payload: Value = serde_json::from_slice(&tools_body).unwrap();
    let tool_group_id = tools_payload["data"]["items"][0]["trace_node_id"]
        .as_str()
        .unwrap();
    let tool_callbacks = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!(
                    "/api/console/applications/{application_id}/logs/runs/{flow_run_id}/trace-tree/nodes?parent_trace_node_id={tool_group_id}"
                ))
                .header("cookie", &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(tool_callbacks.status(), StatusCode::OK);
    let tool_callbacks_body = to_bytes(tool_callbacks.into_body(), usize::MAX)
        .await
        .unwrap();
    let tool_callbacks_payload: Value = serde_json::from_slice(&tool_callbacks_body).unwrap();
    let tool_callback_aliases = tool_callbacks_payload["data"]["items"]
        .as_array()
        .unwrap()
        .iter()
        .map(|callback| callback["node_alias"].as_str().unwrap())
        .collect::<Vec<_>>();
    assert_eq!(tool_callback_aliases, vec!["lookup_weather", "read_policy"]);
}
