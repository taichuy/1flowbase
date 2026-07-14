use super::*;

#[tokio::test]
async fn application_runtime_routes_trace_node_content_offloads_route_provider_events() {
    let (state, _) = test_api_state_with_database_url().await;
    let app = crate::app_with_state_and_config(state.clone(), &test_config());
    let (cookie, csrf) = login_and_capture_cookie(&app, "root", "change-me").await;
    let provider_instance_id = create_ready_provider_instance(&app, &cookie, &csrf).await;
    let application_id =
        seed_agent_flow_application(&app, &cookie, &csrf, &provider_instance_id).await;

    let preview_payload =
        start_llm_preview(&app, &cookie, &csrf, &application_id, "评审最近提交").await;
    let flow_run_id = preview_payload["data"]["flow_run"]["id"].as_str().unwrap();
    let node_run_id =
        Uuid::parse_str(preview_payload["data"]["node_run"]["id"].as_str().unwrap()).unwrap();
    let provider_events: Vec<_> = (0..240)
        .map(|index| {
            json!({
                "type": "text_delta",
                "delta": format!("route-chunk-{index}-{}", "x".repeat(40))
            })
        })
        .collect();
    let route_raw_response = "route-raw-response-".repeat(180);
    let route_output_text = "route-output-text-".repeat(180);

    <MainDurableStore as OrchestrationRuntimeRepository>::update_node_run(
        &state.store,
        &UpdateNodeRunInput {
            node_run_id,
            status: domain::NodeRunStatus::Succeeded,
            output_payload: json!({
                "answer": "评审完成"
            }),
            error_payload: None,
            metrics_payload: json!({}),
            debug_payload: json!({
                "visible_internal_llm_tool_trace": [
                    {
                        "kind": "visible_internal_llm_tool_trace",
                        "route_kind": "fusion",
                        "tool_call_id": "call-problem-review",
                        "tool_name": "problem_review",
                        "status": "succeeded",
                        "route_model": "gemini-3-flash",
                        "branch_traces": [
                            {
                                "node_id": "node-llm-2",
                                "node_type": "llm",
                                "node_alias": "LLM2",
                                "status": "succeeded",
                                "output_payload": {
                                    "text": route_output_text.clone()
                                },
                                "debug_payload": {
                                    "provider_events": provider_events.clone(),
                                    "provider_raw_response": route_raw_response.clone()
                                }
                            }
                        ]
                    }
                ]
            }),
            finished_at: Some(time::OffsetDateTime::now_utc()),
        },
    )
    .await
    .unwrap();

    let trace_tree = load_trace_tree_payload(&app, &cookie, &application_id, flow_run_id).await;
    let llm_trace_node_id = trace_tree["data"]["nodes"]
        .as_array()
        .unwrap()
        .iter()
        .find(|node| node["node_id"] == json!("node-llm"))
        .expect("LLM node should be present")["trace_node_id"]
        .as_str()
        .unwrap();
    let tools_children = load_trace_node_children_payload(
        &app,
        &cookie,
        &application_id,
        flow_run_id,
        llm_trace_node_id,
    )
    .await;
    let tools_trace_node_id = tools_children["data"]["items"][0]["trace_node_id"]
        .as_str()
        .expect("tools group trace node id should exist");
    let tool_children = load_trace_node_children_payload(
        &app,
        &cookie,
        &application_id,
        flow_run_id,
        tools_trace_node_id,
    )
    .await;
    let problem_review_trace_node_id = tool_children["data"]["items"][0]["trace_node_id"]
        .as_str()
        .expect("problem review trace node id should exist");
    let route_children = load_trace_node_children_payload(
        &app,
        &cookie,
        &application_id,
        flow_run_id,
        problem_review_trace_node_id,
    )
    .await;
    let fusion_trace_node_id = route_children["data"]["items"][0]["trace_node_id"]
        .as_str()
        .expect("fusion trace node id should exist");
    let branch_children = load_trace_node_children_payload(
        &app,
        &cookie,
        &application_id,
        flow_run_id,
        fusion_trace_node_id,
    )
    .await;
    let branch_trace_node_id = branch_children["data"]["items"][0]["trace_node_id"]
        .as_str()
        .expect("branch trace node id should exist");

    let raw_branch_content = load_trace_node_content_payload(
        &app,
        &cookie,
        &application_id,
        flow_run_id,
        branch_trace_node_id,
    )
    .await;
    assert_eq!(
        raw_branch_content["data"]["payload"]["debug_payload"]["provider_events"],
        json!(provider_events),
        "trace node content should return raw payload fields unless artifact_preview is requested"
    );
    assert_eq!(
        raw_branch_content["data"]["payload"]["debug_payload"]["provider_raw_response"],
        json!(route_raw_response)
    );
    assert_eq!(
        raw_branch_content["data"]["payload"]["output_payload"]["text"],
        json!(route_output_text)
    );

    let branch_content = load_trace_node_content_payload_with_query(
        &app,
        &cookie,
        &application_id,
        flow_run_id,
        branch_trace_node_id,
        "?artifact_preview=auto",
    )
    .await;
    let provider_events_preview =
        &branch_content["data"]["payload"]["debug_payload"]["provider_events"];
    assert_eq!(
        provider_events_preview["__runtime_debug_artifact"],
        json!(true)
    );
    assert_eq!(provider_events_preview["artifact_scope"], json!("field"));
    assert_eq!(
        provider_events_preview["field_path"],
        json!(["debug_payload", "provider_events"])
    );

    let targeted_branch_content = load_trace_node_content_payload_with_query(
        &app,
        &cookie,
        &application_id,
        flow_run_id,
        branch_trace_node_id,
        "?artifact_preview_field=debug_payload.provider_events&artifact_preview_field=output_payload",
    )
    .await;
    assert_eq!(
        targeted_branch_content["data"]["payload"]["debug_payload"]["provider_events"]
            ["__runtime_debug_artifact"],
        json!(true)
    );
    assert_eq!(
        targeted_branch_content["data"]["payload"]["debug_payload"]["provider_events"]
            ["field_path"],
        json!(["debug_payload", "provider_events"])
    );
    assert_eq!(
        targeted_branch_content["data"]["payload"]["debug_payload"]["provider_raw_response"],
        json!(route_raw_response),
        "artifact_preview_field must leave non-selected siblings raw"
    );
    assert_eq!(
        targeted_branch_content["data"]["payload"]["output_payload"]["__runtime_debug_artifact"],
        json!(true)
    );
    assert_eq!(
        targeted_branch_content["data"]["payload"]["output_payload"]["field_path"],
        json!(["output_payload"])
    );

    let artifact_ref = provider_events_preview["artifact_ref"]
        .as_str()
        .expect("route provider_events artifact ref should exist");
    let batch_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!(
                    "/api/console/applications/{application_id}/orchestration/debug-artifacts/resolve"
                ))
                .header("cookie", &cookie)
                .header("x-csrf-token", &csrf)
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({ "artifact_refs": [artifact_ref] }).to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    let batch_status = batch_response.status();
    let batch_body = to_bytes(batch_response.into_body(), usize::MAX)
        .await
        .unwrap();
    assert_eq!(
        batch_status,
        StatusCode::OK,
        "{}",
        String::from_utf8_lossy(&batch_body)
    );
    let batch_payload: Value = serde_json::from_slice(&batch_body).unwrap();
    assert_eq!(
        batch_payload["data"]["artifacts"][0]["value"],
        json!(provider_events)
    );
}

#[tokio::test]
async fn application_runtime_routes_trace_tree_paginates_high_fan_out_children() {
    let (state, _) = test_api_state_with_database_url().await;
    let app = crate::app_with_state_and_config(state.clone(), &test_config());
    let (cookie, csrf) = login_and_capture_cookie(&app, "root", "change-me").await;
    let provider_instance_id = create_ready_provider_instance(&app, &cookie, &csrf).await;
    let application_id =
        seed_agent_flow_application(&app, &cookie, &csrf, &provider_instance_id).await;

    let preview_payload =
        start_llm_preview(&app, &cookie, &csrf, &application_id, "总结退款政策").await;
    let flow_run_id = preview_payload["data"]["flow_run"]["id"].as_str().unwrap();
    let node_run_id =
        Uuid::parse_str(preview_payload["data"]["node_run"]["id"].as_str().unwrap()).unwrap();
    let tool_calls = (0..25)
        .map(|index| {
            json!({
                "id": format!("call-tool-{index:02}"),
                "name": format!("tool_{index:02}")
            })
        })
        .collect::<Vec<_>>();

    <MainDurableStore as OrchestrationRuntimeRepository>::create_callback_task(
        &state.store,
        &CreateCallbackTaskInput {
            flow_run_id: Uuid::parse_str(flow_run_id).unwrap(),
            node_run_id,
            callback_kind: "llm_tool_calls".to_string(),
            request_payload: json!({ "tool_calls": tool_calls }),
            external_ref_payload: None,
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
    let llm_trace_node_id = trace_tree_payload["data"]["nodes"]
        .as_array()
        .unwrap()
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
    let tools_trace_node_id = children_payload["data"]["items"][0]["trace_node_id"]
        .as_str()
        .unwrap();

    let first_page = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!(
                    "/api/console/applications/{application_id}/logs/runs/{flow_run_id}/trace-tree/nodes?parent_trace_node_id={tools_trace_node_id}&page_size=20"
                ))
                .header("cookie", &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(first_page.status(), StatusCode::OK);
    let first_page_body = to_bytes(first_page.into_body(), usize::MAX).await.unwrap();
    let first_page_payload: Value = serde_json::from_slice(&first_page_body).unwrap();
    assert_eq!(
        first_page_payload["data"]["items"]
            .as_array()
            .unwrap()
            .len(),
        20
    );
    assert_eq!(
        first_page_payload["data"]["page_info"]["page_size"],
        json!(20)
    );
    assert_eq!(
        first_page_payload["data"]["page_info"]["has_more"],
        json!(true)
    );
    let next_cursor = first_page_payload["data"]["page_info"]["next_cursor"]
        .as_str()
        .expect("first page should expose next cursor");

    let second_page = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!(
                    "/api/console/applications/{application_id}/logs/runs/{flow_run_id}/trace-tree/nodes?parent_trace_node_id={tools_trace_node_id}&page_size=20&cursor={next_cursor}"
                ))
                .header("cookie", &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(second_page.status(), StatusCode::OK);
    let second_page_body = to_bytes(second_page.into_body(), usize::MAX).await.unwrap();
    let second_page_payload: Value = serde_json::from_slice(&second_page_body).unwrap();
    assert_eq!(
        second_page_payload["data"]["items"]
            .as_array()
            .unwrap()
            .len(),
        5
    );
    assert_eq!(
        second_page_payload["data"]["page_info"]["has_more"],
        json!(false)
    );
    assert_eq!(
        second_page_payload["data"]["page_info"]["next_cursor"],
        Value::Null
    );

    let invalid_cursor_page = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!(
                    "/api/console/applications/{application_id}/logs/runs/{flow_run_id}/trace-tree/nodes?parent_trace_node_id={tools_trace_node_id}&page_size=20&cursor=not-a-valid-cursor"
                ))
                .header("cookie", &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(invalid_cursor_page.status(), StatusCode::BAD_REQUEST);
    let invalid_cursor_body = to_bytes(invalid_cursor_page.into_body(), usize::MAX)
        .await
        .unwrap();
    let invalid_cursor_payload: Value = serde_json::from_slice(&invalid_cursor_body).unwrap();
    assert_eq!(invalid_cursor_payload["code"], json!("cursor"));

    let oversized_page = app
        .oneshot(
            Request::builder()
                .uri(format!(
                    "/api/console/applications/{application_id}/logs/runs/{flow_run_id}/trace-tree/nodes?parent_trace_node_id={tools_trace_node_id}&page_size=500"
                ))
                .header("cookie", &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(oversized_page.status(), StatusCode::OK);
    let oversized_page_body = to_bytes(oversized_page.into_body(), usize::MAX)
        .await
        .unwrap();
    let oversized_page_payload: Value = serde_json::from_slice(&oversized_page_body).unwrap();
    assert_eq!(
        oversized_page_payload["data"]["items"]
            .as_array()
            .unwrap()
            .len(),
        25
    );
    assert_eq!(
        oversized_page_payload["data"]["page_info"]["page_size"],
        json!(100)
    );
}
