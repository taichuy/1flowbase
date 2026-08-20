use super::*;

#[tokio::test]
async fn application_runtime_routes_trace_tree_content_exposes_visible_internal_llm_route_trace() {
    let (state, database_url) = test_api_state_with_database_url().await;
    let app = crate::app_with_state_and_config(state.clone(), &test_config());
    let (cookie, csrf) = login_and_capture_cookie(&app, "root", "change-me").await;
    let provider_instance_id = create_ready_provider_instance(&app, &cookie, &csrf).await;
    let application_id =
        seed_agent_flow_application(&app, &cookie, &csrf, &provider_instance_id).await;

    let preview = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!(
                    "/api/console/applications/{application_id}/orchestration/nodes/node-llm/debug-runs"
                ))
                .header("cookie", &cookie)
                .header("x-csrf-token", &csrf)
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "input_payload": {
                            "node-start": { "query": "uploads/image-1.png 帮我看看导航栏代码" }
                        }
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    let preview_status = preview.status();
    let preview_body = to_bytes(preview.into_body(), usize::MAX).await.unwrap();
    assert_eq!(
        preview_status,
        StatusCode::CREATED,
        "{}",
        String::from_utf8_lossy(&preview_body)
    );
    let preview_payload: Value = serde_json::from_slice(&preview_body).unwrap();
    let flow_run_id = preview_payload["data"]["flow_run"]["id"]
        .as_str()
        .unwrap()
        .to_string();
    let flow_run_uuid = Uuid::parse_str(&flow_run_id).unwrap();
    let node_run_id =
        Uuid::parse_str(preview_payload["data"]["node_run"]["id"].as_str().unwrap()).unwrap();
    let pool = sqlx::PgPool::connect(&database_url).await.unwrap();
    let debug_payload = json!({});
    let output_payload = json!({
        "text": "很好，图片分析出来了！这是一个 1flowbase 的顶部导航栏。",
        "usage": {
            "total_tokens": 128
        }
    });
    sqlx::query("update node_runs set debug_payload = $2, output_payload = $3 where id = $1")
        .bind(node_run_id)
        .bind(&debug_payload)
        .bind(&output_payload)
        .execute(&pool)
        .await
        .unwrap();
    seed_flow_run_history_events(
        &database_url,
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
                    "event_type": "visible_internal_llm_tool_started",
                    "tool_call_id": "call-image",
                    "tool_name": "image_llm",
                    "main_node_id": "node-llm",
                    "target_node_id": "node-llm-1",
                    "arguments": {
                        "task": "描述图片里的导航栏",
                        "media": [
                            {
                                "kind": "image",
                                "path": "uploads/image-1.png",
                                "source": "workspace_path"
                            }
                        ]
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
                    "event_type": "visible_internal_llm_tool_completed",
                    "tool_call_id": "call-image",
                    "tool_name": "image_llm",
                    "main_node_id": "node-llm",
                    "target_node_id": "node-llm-1",
                    "node_id": "node-llm-1",
                    "provider_route": {
                        "model": "mimo-v2.5",
                        "protocol": "anthropic_messages",
                        "provider_code": "anthropic",
                        "provider_instance_id": "provider-mimo"
                    }
                }),
                visibility: domain::RuntimeEventVisibility::Workspace,
                durability: domain::RuntimeEventDurability::Durable,
            },
        ],
    )
    .await
    .unwrap();
    let before_artifact_count = sqlx::query_scalar::<_, i64>(
        "select count(*) from runtime_debug_artifacts where flow_run_id = $1",
    )
    .bind(flow_run_uuid)
    .fetch_one(&pool)
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
    let root_trace_node_id = trace_tree_payload["data"]["nodes"]
        .as_array()
        .unwrap()
        .iter()
        .find(|node| node["node_id"] == json!("node-llm"))
        .expect("trace tree should include the llm node")["trace_node_id"]
        .as_str()
        .unwrap();
    assert!(
        trace_tree_payload["data"]["nodes"][0]
            .get("debug_payload")
            .is_none(),
        "trace node summary must not include heavy debug payload"
    );

    let content_payload = get_console_json(
        &app,
        &cookie,
        format!(
            "/api/console/applications/{application_id}/logs/runs/{flow_run_id}/trace-tree/nodes/{root_trace_node_id}/content"
        ),
    )
    .await;
    assert!(
        content_payload["data"]["payload"].get("node_run").is_none(),
        "trace node content should advertise detail refs instead of materializing node_run"
    );
    let node_run_detail_ref_id = content_payload["data"]["detail_refs"]
        .as_array()
        .unwrap()
        .iter()
        .find(|detail_ref| detail_ref["detail_kind"] == json!("node_run"))
        .expect("node_run detail ref should be present")["detail_ref_id"]
        .as_str()
        .unwrap();
    let node_run_detail_payload = get_console_json(
        &app,
        &cookie,
        format!(
            "/api/console/applications/{application_id}/logs/runs/{flow_run_id}/trace-tree/nodes/{root_trace_node_id}/details/{node_run_detail_ref_id}"
        ),
    )
    .await;
    assert!(
        node_run_detail_payload["data"]["payload"]["node_run"]["debug_payload"]
            .get("visible_internal_llm_tool_trace")
            .is_none(),
        "visible internal tool trace should be represented by lazy tool children"
    );

    let root_children_payload = get_console_json(
        &app,
        &cookie,
        format!(
            "/api/console/applications/{application_id}/logs/runs/{flow_run_id}/trace-tree/nodes?parent_trace_node_id={root_trace_node_id}"
        ),
    )
    .await;
    let tool_group_id = root_children_payload["data"]["items"]
        .as_array()
        .unwrap()
        .iter()
        .find(|node| node["node_kind"] == json!("tool_group"))
        .expect("visible internal route trace should synthesize a tools group")["trace_node_id"]
        .as_str()
        .unwrap();
    let tool_children_payload = get_console_json(
        &app,
        &cookie,
        format!(
            "/api/console/applications/{application_id}/logs/runs/{flow_run_id}/trace-tree/nodes?parent_trace_node_id={tool_group_id}"
        ),
    )
    .await;
    let tool_trace_node_id = tool_children_payload["data"]["items"]
        .as_array()
        .unwrap()
        .iter()
        .find(|node| node["node_alias"] == json!("image_llm"))
        .expect("visible internal route trace should expose the tool callback node")
        ["trace_node_id"]
        .as_str()
        .unwrap();
    let tool_content_payload = get_console_json(
        &app,
        &cookie,
        format!(
            "/api/console/applications/{application_id}/logs/runs/{flow_run_id}/trace-tree/nodes/{tool_trace_node_id}/content"
        ),
    )
    .await;
    let trace = &tool_content_payload["data"]["payload"]["route_trace"];
    assert_eq!(trace["kind"], json!("visible_internal_llm_tool_trace"));
    assert_eq!(trace["tool_name"], json!("image_llm"));
    assert_eq!(trace["status"], json!("succeeded"));
    assert_eq!(trace["route_model"], json!("mimo-v2.5"));
    assert_eq!(trace["returned_to_main"], json!(true));
    assert_eq!(trace["main_resume"], json!(true));
    assert_eq!(trace["route_output_summary"]["preview"], json!("null"));
    assert_eq!(
        trace["final_output_summary"]["preview"],
        json!("很好，图片分析出来了！这是一个 1flowbase 的顶部导航栏。")
    );
    assert!(trace.get("artifact_ref").is_none());

    let scoped_node = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!(
                    "/api/console/applications/{application_id}/logs/runs/{flow_run_id}/nodes/node-llm"
                ))
                .header("cookie", &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(scoped_node.status(), StatusCode::OK);
    let scoped_node_body = to_bytes(scoped_node.into_body(), usize::MAX).await.unwrap();
    let scoped_node_payload: Value = serde_json::from_slice(&scoped_node_body).unwrap();
    assert_eq!(
        scoped_node_payload["data"]["node_run"]["debug_payload"]["visible_internal_llm_tool_trace"]
            [0]["route_model"],
        json!("mimo-v2.5")
    );

    let after_artifact_count = sqlx::query_scalar::<_, i64>(
        "select count(*) from runtime_debug_artifacts where flow_run_id = $1",
    )
    .bind(flow_run_uuid)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(after_artifact_count, before_artifact_count);
}
