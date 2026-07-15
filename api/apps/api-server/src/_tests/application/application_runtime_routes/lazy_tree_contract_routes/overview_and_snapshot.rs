use super::*;

#[tokio::test]
async fn application_runtime_routes_run_overview_loads_detail_without_trace_nodes() {
    let (state, _) = test_api_state_with_database_url().await;
    let app = crate::app_with_state_and_config(state, &test_config());
    let (cookie, csrf) = login_and_capture_cookie(&app, "root", "change-me").await;
    let provider_instance_id = create_ready_provider_instance(&app, &cookie, &csrf).await;
    let application_id =
        seed_agent_flow_application(&app, &cookie, &csrf, &provider_instance_id).await;

    let preview_payload =
        start_llm_preview(&app, &cookie, &csrf, &application_id, "总结退款政策").await;
    let flow_run_id = preview_payload["data"]["flow_run"]["id"].as_str().unwrap();

    let overview = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!(
                    "/api/console/applications/{application_id}/logs/runs/{flow_run_id}/overview"
                ))
                .header("cookie", &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let status = overview.status();
    let body = to_bytes(overview.into_body(), usize::MAX).await.unwrap();
    assert_eq!(status, StatusCode::OK, "{}", String::from_utf8_lossy(&body));
    let payload: Value = serde_json::from_slice(&body).unwrap();
    let data = &payload["data"];

    assert_eq!(data["flow_run"]["id"], json!(flow_run_id));
    assert_eq!(
        data["flow_run"]["input_payload"]["node-start"]["query"],
        json!("总结退款政策")
    );
    assert_eq!(data["run"]["id"], json!(flow_run_id));
    assert!(data["statistics"].is_object());
    assert!(
        data.get("nodes").is_none(),
        "overview must not expose trace root nodes"
    );
    assert!(
        data.get("node_runs").is_none(),
        "overview must not materialize node run content"
    );
}

#[tokio::test]
async fn application_runtime_routes_debug_snapshot_uses_orchestration_plane() {
    let (state, _) = test_api_state_with_database_url().await;
    let app = crate::app_with_state_and_config(state, &test_config());
    let (cookie, csrf) = login_and_capture_cookie(&app, "root", "change-me").await;
    let provider_instance_id = create_ready_provider_instance(&app, &cookie, &csrf).await;
    let application_id =
        seed_agent_flow_application(&app, &cookie, &csrf, &provider_instance_id).await;

    let preview_payload =
        start_llm_preview(&app, &cookie, &csrf, &application_id, "总结退款政策").await;
    let flow_run_id = preview_payload["data"]["flow_run"]["id"].as_str().unwrap();

    let snapshot = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!(
                    "/api/console/applications/{application_id}/orchestration/runs/{flow_run_id}/debug-snapshot"
                ))
                .header("cookie", &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let status = snapshot.status();
    let body = to_bytes(snapshot.into_body(), usize::MAX).await.unwrap();
    assert_eq!(status, StatusCode::OK, "{}", String::from_utf8_lossy(&body));
    let payload: Value = serde_json::from_slice(&body).unwrap();
    let data = &payload["data"];
    assert_eq!(data["flow_run"]["id"], json!(flow_run_id));
    assert!(
        data["node_runs"]
            .as_array()
            .is_some_and(|items| !items.is_empty()),
        "debug snapshot should remain a debug-session detail contract"
    );
}

#[tokio::test]
async fn application_runtime_routes_trace_tree_excludes_llm_tool_calls_as_trace_children() {
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

    <MainDurableStore as OrchestrationRuntimeRepository>::create_callback_task(
        &state.store,
        &CreateCallbackTaskInput {
            flow_run_id: Uuid::parse_str(flow_run_id).unwrap(),
            node_run_id,
            callback_kind: "llm_tool_calls".to_string(),
            request_payload: json!({
                "tool_calls": [
                    {
                        "id": "call-refund-policy",
                        "name": "refund_policy_lookup"
                    }
                ]
            }),
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
    let root_nodes = trace_tree_payload["data"]["nodes"].as_array().unwrap();
    let llm_node = root_nodes
        .iter()
        .find(|node| node["node_id"] == json!("node-llm"))
        .expect("LLM node should be present");
    assert_eq!(llm_node["has_children"].as_bool(), Some(true));
    let llm_trace_node_id = llm_node["trace_node_id"].as_str().unwrap();

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
    let child_items = children_payload["data"]["items"].as_array().unwrap();

    assert_eq!(
        child_items.len(),
        1,
        "llm_tool_calls should appear through the projection tools group"
    );
    assert_eq!(child_items[0]["node_kind"], json!("tool_group"));
}
