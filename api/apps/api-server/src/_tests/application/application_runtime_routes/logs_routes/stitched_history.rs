use super::*;

#[tokio::test]
async fn application_runtime_routes_trace_tree_stitches_prior_claude_code_tool_runs_lazily() {
    // Mirrors #882 samples:
    // Run A 019ebed1-9f5b-77d0-810c-7227d1b5f7e3,
    // Run B 019ebed3-47ad-7921-8b74-a7def920d9db,
    // Run C 019ebed3-d60a-71d0-9b81-0234a0b89147.
    let (state, database_url) = test_api_state_with_database_url().await;
    let app = crate::app_with_state_and_config(state.clone(), &test_config());
    let (cookie, csrf) = login_and_capture_cookie(&app, "root", "change-me").await;
    let provider_instance_id = create_ready_provider_instance(&app, &cookie, &csrf).await;
    let application_id =
        seed_agent_flow_application(&app, &cookie, &csrf, &provider_instance_id).await;

    let run_a_id = start_full_debug_run(
        &app,
        &cookie,
        &csrf,
        &application_id,
        "来 uploads\\test-01.png 找一下这幅图相关代码",
    )
    .await;
    let run_b_id = start_full_debug_run(
        &app,
        &cookie,
        &csrf,
        &application_id,
        "No matches found\n-rw-r--r-- 1 Lw 197121 uploads/test-01.png\nTook a screenshot of the current page's viewport.",
    )
    .await;
    let run_c_id = start_full_debug_run(
        &app,
        &cookie,
        &csrf,
        &application_id,
        "File does not exist. Note: your current working directory is E:\\code\\taichuCode\\1flowbase.\nFound 4 files\nweb\\app\\src\\routes\\route-config.ts\nweb\\app\\src\\app-shell\\Navigation.tsx",
    )
    .await;

    let run_a_uuid = Uuid::parse_str(&run_a_id).unwrap();
    let run_b_uuid = Uuid::parse_str(&run_b_id).unwrap();
    let run_c_uuid = Uuid::parse_str(&run_c_id).unwrap();
    wait_for_run_detail(
        &app,
        &cookie,
        &application_id,
        &run_a_id,
        &["succeeded", "failed", "cancelled"],
    )
    .await;
    wait_for_run_detail(
        &app,
        &cookie,
        &application_id,
        &run_b_id,
        &["succeeded", "failed", "cancelled"],
    )
    .await;
    wait_for_run_detail(
        &app,
        &cookie,
        &application_id,
        &run_c_id,
        &["succeeded", "failed", "cancelled"],
    )
    .await;

    let pool = sqlx::PgPool::connect(&database_url).await.unwrap();
    let external_user = "claude-code-user-fixture";
    let external_conversation_id = "claude-code-session-fixture";
    for run_id in [run_a_uuid, run_b_uuid, run_c_uuid] {
        sqlx::query(
            r#"
            update flow_runs
            set external_user = $2,
                external_conversation_id = $3,
                compatibility_mode = 'anthropic-messages-v1'
            where id = $1
            "#,
        )
        .bind(run_id)
        .bind(external_user)
        .bind(external_conversation_id)
        .execute(&pool)
        .await
        .unwrap();
    }
    for run_id in [run_a_uuid, run_b_uuid] {
        <MainDurableStore as OrchestrationRuntimeRepository>::update_flow_run(
            &state.store,
            &UpdateFlowRunInput {
                flow_run_id: run_id,
                status: domain::FlowRunStatus::Cancelled,
                output_payload: json!({}),
                error_payload: None,
                finished_at: Some(time::OffsetDateTime::now_utc()),
            },
        )
        .await
        .unwrap();
    }

    let run_a_llm_node_run_id = latest_llm_node_run_id(&pool, run_a_uuid).await;
    let run_b_llm_node_run_id = latest_llm_node_run_id(&pool, run_b_uuid).await;
    let _run_a_callback_task =
        <MainDurableStore as OrchestrationRuntimeRepository>::create_callback_task(
            &state.store,
            &CreateCallbackTaskInput {
                flow_run_id: run_a_uuid,
                node_run_id: run_a_llm_node_run_id,
                callback_kind: "llm_tool_calls".to_string(),
                request_payload: json!({
                    "tool_calls": [
                        { "id": "call_image", "name": "image_llm" },
                        { "id": "call_glob", "name": "Glob" }
                    ]
                }),
                external_ref_payload: None,
            },
        )
        .await
        .unwrap();
    let _run_b_callback_task =
        <MainDurableStore as OrchestrationRuntimeRepository>::create_callback_task(
            &state.store,
            &CreateCallbackTaskInput {
                flow_run_id: run_b_uuid,
                node_run_id: run_b_llm_node_run_id,
                callback_kind: "llm_tool_calls".to_string(),
                request_payload: json!({
                    "tool_calls": [
                        { "id": "call_grep", "name": "Grep" },
                        { "id": "call_read", "name": "Read" }
                    ]
                }),
                external_ref_payload: None,
            },
        )
        .await
        .unwrap();
    <MainDurableStore as OrchestrationRuntimeRepository>::append_runtime_events(
        &state.store,
        &[
            AppendRuntimeEventInput {
                flow_run_id: run_a_uuid,
                node_run_id: Some(run_a_llm_node_run_id),
                span_id: None,
                parent_span_id: None,
                event_type: "visible_internal_llm_tool_started".to_string(),
                layer: domain::RuntimeEventLayer::AgentTransition,
                source: domain::RuntimeEventSource::Host,
                trust_level: domain::RuntimeTrustLevel::HostFact,
                item_id: None,
                ledger_ref: None,
                payload: json!({
                    "tool_call_id": "call_image",
                    "tool_name": "image_llm",
                    "main_node_id": "node-llm",
                    "target_node_id": "node-llm-image",
                    "arguments": {
                        "media": [
                            {
                                "kind": "image",
                                "path": "uploads/test-01.png",
                                "source": "workspace_path"
                            }
                        ]
                    }
                }),
                visibility: domain::RuntimeEventVisibility::Workspace,
                durability: domain::RuntimeEventDurability::Durable,
            },
            AppendRuntimeEventInput {
                flow_run_id: run_a_uuid,
                node_run_id: Some(run_a_llm_node_run_id),
                span_id: None,
                parent_span_id: None,
                event_type: "visible_internal_llm_tool_completed".to_string(),
                layer: domain::RuntimeEventLayer::AgentTransition,
                source: domain::RuntimeEventSource::Host,
                trust_level: domain::RuntimeTrustLevel::HostFact,
                item_id: None,
                ledger_ref: None,
                payload: json!({
                    "tool_call_id": "call_image",
                    "tool_name": "image_llm",
                    "main_node_id": "node-llm",
                    "target_node_id": "node-llm-image",
                    "provider_route": {
                        "model": "mimo-v2.5",
                        "protocol": "anthropic_messages"
                    },
                    "content": "图片是 1flowbase 顶部导航栏"
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
                    "/api/console/applications/{application_id}/logs/runs/{run_c_id}/trace-tree"
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
    let data = &trace_tree_payload["data"];

    assert_eq!(data["flow_run"]["id"], json!(run_c_id));
    assert!(
        data.get("stitched_trace").is_none(),
        "lazy trace tree should expose stitched nodes as expandable summaries, not full stitched detail"
    );
    let root_nodes = data["nodes"].as_array().unwrap();
    assert!(
        root_nodes
            .iter()
            .any(|node| node["flow_run_id"] == json!(run_c_id))
    );
    assert!(
        root_nodes
            .iter()
            .all(|node| node["flow_run_id"] != json!(run_a_id))
    );
    assert!(
        root_nodes
            .iter()
            .all(|node| node["flow_run_id"] != json!(run_b_id))
    );
    assert!(
        root_nodes
            .iter()
            .all(|node| node.get("input_payload").is_none())
    );
    assert!(
        root_nodes
            .iter()
            .all(|node| node.get("debug_payload").is_none())
    );

    let llm_root = root_nodes
        .iter()
        .find(|node| {
            node["node_kind"] == json!("node_run")
                && node["node_type"] == json!("llm")
                && node["flow_run_id"] == json!(run_c_id)
        })
        .expect("current llm root should be present");
    let llm_root_id = llm_root["trace_node_id"].as_str().unwrap();
    assert!(
        root_nodes
            .iter()
            .all(|node| node["node_kind"] != json!("stitched_context"))
    );

    let llm_children = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!(
                    "/api/console/applications/{application_id}/logs/runs/{run_c_id}/trace-tree/nodes?parent_trace_node_id={llm_root_id}"
                ))
                .header("cookie", &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(llm_children.status(), StatusCode::OK);
    let llm_children_body = to_bytes(llm_children.into_body(), usize::MAX)
        .await
        .unwrap();
    let llm_children_payload: Value = serde_json::from_slice(&llm_children_body).unwrap();
    let llm_child_nodes = llm_children_payload["data"]["items"].as_array().unwrap();
    let stitched_group = llm_child_nodes
        .iter()
        .find(|node| node["node_kind"] == json!("stitched_context"))
        .expect("stitched context group should be nested under current llm root");
    let stitched_group_id = stitched_group["trace_node_id"].as_str().unwrap();
    assert_eq!(stitched_group["has_children"].as_bool(), Some(true));

    let stitched_children = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!(
                    "/api/console/applications/{application_id}/logs/runs/{run_c_id}/trace-tree/nodes?parent_trace_node_id={stitched_group_id}"
                ))
                .header("cookie", &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(stitched_children.status(), StatusCode::OK);
    let stitched_children_body = to_bytes(stitched_children.into_body(), usize::MAX)
        .await
        .unwrap();
    let stitched_children_payload: Value = serde_json::from_slice(&stitched_children_body).unwrap();
    let stitched_runs = stitched_children_payload["data"]["items"]
        .as_array()
        .unwrap();
    assert_eq!(stitched_runs.len(), 2);
    assert!(
        stitched_runs
            .iter()
            .all(|node| node["node_kind"] == json!("stitched_run"))
    );
    assert!(
        stitched_runs
            .iter()
            .all(|node| node["has_content"] == json!(false))
    );
    let run_a_trace_node_id = stitched_runs[0]["trace_node_id"].as_str().unwrap();

    let run_a_node_content = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!(
                    "/api/console/applications/{application_id}/logs/runs/{run_c_id}/trace-tree/nodes/{run_a_trace_node_id}/content"
                ))
                .header("cookie", &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(run_a_node_content.status(), StatusCode::NOT_FOUND);

    let run_a_children = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!(
                    "/api/console/applications/{application_id}/logs/runs/{run_c_id}/trace-tree/nodes?parent_trace_node_id={run_a_trace_node_id}"
                ))
                .header("cookie", &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(run_a_children.status(), StatusCode::OK);
    let run_a_children_body = to_bytes(run_a_children.into_body(), usize::MAX)
        .await
        .unwrap();
    let run_a_children_payload: Value = serde_json::from_slice(&run_a_children_body).unwrap();
    let run_a_nodes = run_a_children_payload["data"]["items"].as_array().unwrap();
    assert!(
        run_a_nodes
            .iter()
            .any(|node| node["node_kind"] == json!("node_run")),
        "stitched run should expose historical node children"
    );
    let run_a_llm_node = run_a_nodes
        .iter()
        .find(|node| {
            node["node_kind"] == json!("node_run")
                && node["node_type"] == json!("llm")
                && node["source_flow_run_id"] == json!(run_a_id)
        })
        .expect("historical llm node should remain linked to source run");
    let run_a_llm_trace_node_id = run_a_llm_node["trace_node_id"].as_str().unwrap();

    let run_a_llm_content = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!(
                    "/api/console/applications/{application_id}/logs/runs/{run_c_id}/trace-tree/nodes/{run_a_llm_trace_node_id}/content"
                ))
                .header("cookie", &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(run_a_llm_content.status(), StatusCode::OK);
    let run_a_llm_content_body = to_bytes(run_a_llm_content.into_body(), usize::MAX)
        .await
        .unwrap();
    let run_a_llm_content_payload: Value = serde_json::from_slice(&run_a_llm_content_body).unwrap();
    assert_eq!(
        run_a_llm_content_payload["data"]["payload"]["payload_index"]["source_flow_run_id"],
        json!(run_a_id)
    );

    let run_a_llm_detail = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!(
                    "/api/console/applications/{application_id}/logs/runs/{run_c_id}/trace-tree/nodes/{run_a_llm_trace_node_id}/details/node_run"
                ))
                .header("cookie", &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(run_a_llm_detail.status(), StatusCode::OK);
    let run_a_llm_detail_body = to_bytes(run_a_llm_detail.into_body(), usize::MAX)
        .await
        .unwrap();
    let run_a_llm_detail_payload: Value = serde_json::from_slice(&run_a_llm_detail_body).unwrap();
    assert_eq!(
        run_a_llm_detail_payload["data"]["payload"]["node_run"]["flow_run_id"],
        json!(run_a_id)
    );
}
