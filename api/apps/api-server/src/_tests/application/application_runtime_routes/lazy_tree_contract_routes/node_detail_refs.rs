use super::*;

#[tokio::test]
async fn application_runtime_routes_trace_node_detail_ref_loads_node_run_payload_lazily() {
    let (state, database_url) = test_api_state_with_database_url().await;
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

    seed_node_run_history(
        &database_url,
        &UpdateNodeRunInput {
            node_run_id,
            status: domain::NodeRunStatus::Succeeded,
            output_payload: json!({
                "answer": "退款政策摘要"
            }),
            error_payload: None,
            metrics_payload: json!({
                "usage": {
                    "total_tokens": 42
                }
            }),
            debug_payload: json!({
                "provider": "deepseek",
                "prompt_messages": [
                    {
                        "role": "user",
                        "content": "总结退款政策"
                    }
                ]
            }),
            finished_at: Some(time::OffsetDateTime::now_utc()),
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
    assert!(
        content_payload["data"]["payload"].get("node_run").is_none(),
        "default content must not return the full node run"
    );
    let detail_refs = content_payload["data"]["detail_refs"].as_array().unwrap();
    let node_run_ref = detail_refs
        .iter()
        .find(|detail_ref| detail_ref["detail_kind"] == json!("node_run"))
        .expect("node_run detail ref should be advertised");
    assert_eq!(node_run_ref["detail_ref_id"], json!("node_run"));

    let detail = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!(
                    "/api/console/applications/{application_id}/logs/runs/{flow_run_id}/trace-tree/nodes/{llm_trace_node_id}/details/node_run"
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
    assert_eq!(
        detail_payload["data"]["trace_node_id"],
        json!(llm_trace_node_id)
    );
    assert_eq!(detail_payload["data"]["detail_ref_id"], json!("node_run"));
    assert_eq!(detail_payload["data"]["detail_kind"], json!("node_run"));
    let node_run = &detail_payload["data"]["payload"]["node_run"];
    assert_eq!(node_run["id"], json!(node_run_id.to_string()));
    assert!(
        node_run["input_payload"]
            .to_string()
            .contains("总结退款政策"),
        "node_run detail should preserve the current node input payload"
    );
    assert_eq!(node_run["debug_payload"]["provider"], json!("deepseek"));
    assert_eq!(node_run["output_payload"]["answer"], json!("退款政策摘要"));
    assert_eq!(
        node_run["metrics_payload"]["usage"]["total_tokens"],
        json!(42)
    );
    assert!(
        detail_payload["data"]["payload"]
            .get("checkpoints")
            .is_none(),
        "node_run detail ref must not smuggle checkpoint details"
    );
    assert!(
        detail_payload["data"]["payload"].get("events").is_none(),
        "node_run detail ref must not smuggle event details"
    );
}

#[tokio::test]
async fn application_runtime_routes_trace_node_detail_offloads_provider_events() {
    let (state, database_url) = test_api_state_with_database_url().await;
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
    let provider_events: Vec<_> = (0..240)
        .map(|index| {
            json!({
                "type": "text_delta",
                "delta": format!("chunk-{index}-{}", "x".repeat(40))
            })
        })
        .collect();
    let provider_raw_response = "raw-provider-response-".repeat(180);

    seed_node_run_history(
        &database_url,
        &UpdateNodeRunInput {
            node_run_id,
            status: domain::NodeRunStatus::Succeeded,
            output_payload: json!({
                "answer": "退款政策摘要"
            }),
            error_payload: None,
            metrics_payload: json!({}),
            debug_payload: json!({
                "provider": "deepseek",
                "provider_events": provider_events.clone(),
                "provider_raw_response": provider_raw_response.clone()
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
    let content_payload = load_trace_node_content_payload(
        &app,
        &cookie,
        &application_id,
        flow_run_id,
        llm_trace_node_id,
    )
    .await;
    let detail_ref_id = content_payload["data"]["detail_refs"]
        .as_array()
        .unwrap()
        .iter()
        .find(|detail_ref| detail_ref["detail_kind"] == json!("node_run"))
        .expect("node_run detail ref should be advertised")["detail_ref_id"]
        .as_str()
        .expect("node_run detail ref id should be a string");
    let raw_detail_payload = load_trace_node_detail_payload(
        &app,
        &cookie,
        &application_id,
        flow_run_id,
        llm_trace_node_id,
        detail_ref_id,
    )
    .await;
    let raw_node_run = &raw_detail_payload["data"]["payload"]["node_run"];
    assert_eq!(
        raw_node_run["debug_payload"]["provider_events"],
        json!(provider_events),
        "trace node detail should return raw debug fields unless artifact_preview is requested"
    );
    assert_eq!(
        raw_node_run["debug_payload"]["provider_raw_response"],
        json!(provider_raw_response)
    );

    let auto_detail_payload = load_trace_node_detail_payload_with_query(
        &app,
        &cookie,
        &application_id,
        flow_run_id,
        llm_trace_node_id,
        detail_ref_id,
        "?artifact_preview=auto",
    )
    .await;
    let node_run = &auto_detail_payload["data"]["payload"]["node_run"];
    let provider_events_preview = &node_run["debug_payload"]["provider_events"];

    assert_eq!(
        provider_events_preview["__runtime_debug_artifact"],
        json!(true)
    );
    assert_eq!(provider_events_preview["artifact_scope"], json!("field"));
    assert_eq!(
        provider_events_preview["field_path"],
        json!(["provider_events"])
    );
    assert!(provider_events_preview["artifact_ref"].is_string());
    assert!(provider_events_preview["preview"].is_string());
    assert!(
        provider_events_preview["original_size_bytes"]
            .as_i64()
            .unwrap()
            > provider_events_preview["preview_size_bytes"]
                .as_i64()
                .unwrap()
    );

    let targeted_detail_payload = load_trace_node_detail_payload_with_query(
        &app,
        &cookie,
        &application_id,
        flow_run_id,
        llm_trace_node_id,
        detail_ref_id,
        "?artifact_preview_field=node_run.debug_payload.provider_events",
    )
    .await;
    let targeted_node_run = &targeted_detail_payload["data"]["payload"]["node_run"];
    assert_eq!(
        targeted_node_run["debug_payload"]["provider_events"]["__runtime_debug_artifact"],
        json!(true)
    );
    assert_eq!(
        targeted_node_run["debug_payload"]["provider_events"]["field_path"],
        json!(["node_run", "debug_payload", "provider_events"])
    );
    assert_eq!(
        targeted_node_run["debug_payload"]["provider_raw_response"],
        json!(provider_raw_response),
        "artifact_preview_field must not materialize sibling fields"
    );

    let artifact_ref = provider_events_preview["artifact_ref"]
        .as_str()
        .expect("provider_events artifact ref should exist");
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
                    json!({
                        "artifact_refs": [artifact_ref, artifact_ref]
                    })
                    .to_string(),
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
    let artifacts = batch_payload["data"]["artifacts"]
        .as_array()
        .expect("batch resolve should return artifacts");
    assert_eq!(artifacts.len(), 1);
    assert_eq!(artifacts[0]["artifact_ref"], json!(artifact_ref));
    assert_eq!(artifacts[0]["value"], json!(provider_events));

    let stored_node_runs =
        <MainDurableStore as OrchestrationRuntimeRepository>::list_application_run_trace_node_run_details(
            &state.store,
            Uuid::parse_str(flow_run_id).unwrap(),
            vec![node_run_id],
        )
        .await
        .unwrap();
    let stored_debug_payload = &stored_node_runs[0].debug_payload;
    assert!(
        stored_debug_payload["provider_events"].is_array(),
        "trace detail response compression must not rewrite the stored node_run debug payload"
    );
}
