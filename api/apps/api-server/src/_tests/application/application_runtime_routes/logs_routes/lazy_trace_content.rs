use super::*;

#[tokio::test]
async fn application_runtime_routes_log_trace_tree_loads_summary_children_and_content_lazily() {
    let (state, _) = test_api_state_with_database_url().await;
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
                            "node-start": { "query": "总结退款政策" }
                        }
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    let preview_body = to_bytes(preview.into_body(), usize::MAX).await.unwrap();
    let preview_payload: Value = serde_json::from_slice(&preview_body).unwrap();
    let flow_run_id = preview_payload["data"]["flow_run"]["id"]
        .as_str()
        .unwrap()
        .to_string();
    let node_run_id =
        Uuid::parse_str(preview_payload["data"]["node_run"]["id"].as_str().unwrap()).unwrap();

    let _callback = <MainDurableStore as OrchestrationRuntimeRepository>::create_callback_task(
        &state.store,
        &CreateCallbackTaskInput {
            flow_run_id: Uuid::parse_str(&flow_run_id).unwrap(),
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
    assert_eq!(
        trace_tree_payload["data"]["flow_run"]["id"].as_str(),
        Some(flow_run_id.as_str())
    );
    let root_nodes = trace_tree_payload["data"]["nodes"].as_array().unwrap();
    assert_eq!(root_nodes.len(), 1);
    assert_eq!(root_nodes[0]["node_id"].as_str(), Some("node-llm"));
    assert_eq!(root_nodes[0]["node_kind"].as_str(), Some("node_run"));
    assert_eq!(root_nodes[0]["has_children"].as_bool(), Some(true));
    assert_eq!(root_nodes[0]["has_content"].as_bool(), Some(true));
    assert!(
        root_nodes[0].get("input_payload").is_none(),
        "trace node summary must not include heavy node input payload"
    );
    assert!(
        root_nodes[0].get("debug_payload").is_none(),
        "trace node summary must not include heavy node debug payload"
    );
    let root_trace_node_id = root_nodes[0]["trace_node_id"].as_str().unwrap();

    let children = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!(
                    "/api/console/applications/{application_id}/logs/runs/{flow_run_id}/trace-tree/nodes?parent_trace_node_id={root_trace_node_id}"
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
    let child_nodes = children_payload["data"]["items"].as_array().unwrap();
    assert_eq!(
        child_nodes.len(),
        1,
        "llm_tool_calls are exposed through the projection tools group"
    );
    assert_eq!(child_nodes[0]["node_kind"], json!("tool_group"));
    assert_eq!(child_nodes[0]["has_children"], json!(true));
    assert_eq!(child_nodes[0]["has_content"], json!(false));

    let content = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!(
                    "/api/console/applications/{application_id}/logs/runs/{flow_run_id}/trace-tree/nodes/{root_trace_node_id}/content"
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
    assert!(
        content_payload["data"]["payload"].get("node_run").is_none(),
        "trace node content should advertise detail refs instead of materializing node_run"
    );
    let node_run_detail_payload = load_trace_node_detail_payload_for_kind(
        &app,
        &cookie,
        &application_id,
        &flow_run_id,
        root_trace_node_id,
        "node_run",
    )
    .await
    .expect("root trace node should advertise a node_run detail ref");
    assert_eq!(
        node_run_detail_payload["data"]["payload"]["node_run"]["output_payload"]["text"],
        json!("reply:总结退款政策")
    );
    let events_detail_payload = load_trace_node_detail_payload_for_kind(
        &app,
        &cookie,
        &application_id,
        &flow_run_id,
        root_trace_node_id,
        "events",
    )
    .await
    .expect("root trace node should advertise an events detail ref");
    assert!(events_detail_payload["data"]["payload"]["events"]
        .as_array()
        .unwrap()
        .iter()
        .all(|event| event["node_run_id"] == json!(node_run_id.to_string())));
}

#[tokio::test]
async fn application_runtime_routes_logs_include_public_run_identity_fields() {
    let app = test_app().await;
    let (cookie, csrf) = login_and_capture_cookie(&app, "root", "change-me").await;
    let provider_instance_id = create_ready_provider_instance(&app, &cookie, &csrf).await;
    let application_id =
        seed_agent_flow_application(&app, &cookie, &csrf, &provider_instance_id).await;
    publish_application_public_api(&app, &cookie, &csrf, &application_id).await;
    let token = create_application_public_api_key(&app, &cookie, &csrf, &application_id).await;

    let create = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/agent/v1/runs")
                .header("authorization", format!("Bearer {token}"))
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "query": "请总结退款政策",
                        "title": "公开 API 退款总结",
                        "expand_id": "customer-42",
                        "compatibility_mode": "native-v1",
                        "response_mode": "queued"
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(create.status(), StatusCode::CREATED);
    let create_body = to_bytes(create.into_body(), usize::MAX).await.unwrap();
    let create_payload: Value = serde_json::from_slice(&create_body).unwrap();
    let flow_run_id = create_payload["data"]["id"].as_str().unwrap().to_string();

    let list = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!(
                    "/api/console/applications/{application_id}/logs/runs"
                ))
                .header("cookie", &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(list.status(), StatusCode::OK);
    let list_body = to_bytes(list.into_body(), usize::MAX).await.unwrap();
    let list_payload: Value = serde_json::from_slice(&list_body).unwrap();
    assert_eq!(list_payload["data"]["page"].as_i64(), Some(1));
    assert_eq!(list_payload["data"]["page_size"].as_i64(), Some(20));
    assert_eq!(list_payload["data"]["total"].as_i64(), Some(1));
    assert_eq!(list_payload["data"]["items"].as_array().unwrap().len(), 1);
    assert_eq!(
        list_payload["data"]["items"][0]["id"].as_str(),
        Some(flow_run_id.as_str())
    );
    assert_eq!(
        list_payload["data"]["items"][0]["run_mode"].as_str(),
        Some("published_api_run")
    );
    assert_eq!(
        list_payload["data"]["items"][0]["title"].as_str(),
        Some("公开 API 退款总结")
    );
    assert_eq!(
        list_payload["data"]["items"][0]["expand_id"].as_str(),
        Some("customer-42")
    );
    assert_eq!(
        list_payload["data"]["items"][0]["authorized_account"].as_str(),
        Some("root")
    );
    assert_eq!(
        list_payload["data"]["items"][0]["source"].as_str(),
        Some("public_api")
    );
    assert_eq!(
        list_payload["data"]["items"][0]["compatibility_mode"].as_str(),
        Some("native-v1")
    );
    assert!(!list_payload["data"]["items"][0]
        .as_object()
        .unwrap()
        .contains_key("protocol"));
    assert_eq!(
        list_payload["data"]["items"][0]["correlation"]["external_user"].as_str(),
        Some("customer-42")
    );
    assert!(list_payload["data"]["items"][0]["correlation"]["api_key_id"].is_string());
    assert!(list_payload["data"]["items"][0]["correlation"]["publication_version_id"].is_string());

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
    assert_eq!(
        trace_tree_payload["data"]["flow_run"]["title"].as_str(),
        Some("公开 API 退款总结")
    );
    assert_eq!(
        trace_tree_payload["data"]["flow_run"]["expand_id"].as_str(),
        Some("customer-42")
    );
    assert_eq!(
        trace_tree_payload["data"]["flow_run"]["authorized_account"].as_str(),
        Some("root")
    );
    assert_eq!(
        trace_tree_payload["data"]["run"]["source"].as_str(),
        Some("public_api")
    );
    assert_eq!(
        trace_tree_payload["data"]["run"]["compatibility_mode"].as_str(),
        Some("native-v1")
    );
    assert!(!trace_tree_payload["data"]["run"]
        .as_object()
        .unwrap()
        .contains_key("protocol"));
    assert_eq!(
        trace_tree_payload["data"]["run"]["correlation"]["external_user"].as_str(),
        Some("customer-42")
    );
    assert!(trace_tree_payload["data"]["run"]["correlation"]["api_key_id"].is_string());
    assert!(trace_tree_payload["data"]["run"]["correlation"]["publication_version_id"].is_string());
}
