use super::*;
use control_plane::ports::{AppendRuntimeEventInput, OrchestrationRuntimeRepository};
use storage_durable::MainDurableStore;

#[tokio::test]
async fn get_runtime_debug_stream_returns_trusted_parts() {
    let app = test_app().await;
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
                            "node-start": { "query": "总结退款政策" },
                            "node-llm": { "prompt_messages": ["resolved prompt must stay audit-only"] }
                        },
                        "debug_session_id": DEBUG_SESSION_ID
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(preview.status(), StatusCode::CREATED);
    let preview_body = to_bytes(preview.into_body(), usize::MAX).await.unwrap();
    let preview_payload: Value = serde_json::from_slice(&preview_body).unwrap();
    let run_id = preview_payload["data"]["flow_run"]["id"].as_str().unwrap();

    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri(format!(
                    "/api/console/applications/{application_id}/logs/runs/{run_id}/debug-stream"
                ))
                .header("cookie", &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let payload: Value =
        serde_json::from_slice(&to_bytes(response.into_body(), usize::MAX).await.unwrap()).unwrap();
    assert!(payload["data"]["parts"]
        .as_array()
        .unwrap()
        .iter()
        .any(|part| part["trust_level"] == "host_fact"));
}

#[tokio::test]
async fn get_runtime_debug_stream_uses_sequence_cursor_and_limit() {
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
                            "node-start": { "query": "总结退款政策" }
                        },
                        "debug_session_id": DEBUG_SESSION_ID
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(preview.status(), StatusCode::CREATED);
    let preview_body = to_bytes(preview.into_body(), usize::MAX).await.unwrap();
    let preview_payload: Value = serde_json::from_slice(&preview_body).unwrap();
    let run_id =
        Uuid::parse_str(preview_payload["data"]["flow_run"]["id"].as_str().unwrap()).unwrap();
    wait_for_run_detail(
        &app,
        &cookie,
        &application_id,
        &run_id.to_string(),
        &["succeeded", "failed", "cancelled"],
    )
    .await;

    let store = storage_durable::build_main_durable_postgres(&database_url)
        .await
        .unwrap()
        .store;
    let appended = <MainDurableStore as OrchestrationRuntimeRepository>::append_runtime_events(
        &store,
        &[
            AppendRuntimeEventInput {
                flow_run_id: run_id,
                node_run_id: None,
                span_id: None,
                parent_span_id: None,
                event_type: "handoff".into(),
                layer: domain::RuntimeEventLayer::AgentTransition,
                source: domain::RuntimeEventSource::Host,
                trust_level: domain::RuntimeTrustLevel::HostFact,
                item_id: None,
                ledger_ref: None,
                payload: json!({ "label": "first bounded event" }),
                visibility: domain::RuntimeEventVisibility::Workspace,
                durability: domain::RuntimeEventDurability::Durable,
            },
            AppendRuntimeEventInput {
                flow_run_id: run_id,
                node_run_id: None,
                span_id: None,
                parent_span_id: None,
                event_type: "handoff".into(),
                layer: domain::RuntimeEventLayer::AgentTransition,
                source: domain::RuntimeEventSource::Host,
                trust_level: domain::RuntimeTrustLevel::HostFact,
                item_id: None,
                ledger_ref: None,
                payload: json!({ "label": "second bounded event" }),
                visibility: domain::RuntimeEventVisibility::Workspace,
                durability: domain::RuntimeEventDurability::Durable,
            },
        ],
    )
    .await
    .unwrap();
    let from_sequence = appended[0].sequence - 1;

    let first_page = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri(format!(
                    "/api/console/applications/{application_id}/logs/runs/{run_id}/debug-stream?from_sequence={from_sequence}&limit=1"
                ))
                .header("cookie", &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(first_page.status(), StatusCode::OK);
    let first_payload: Value =
        serde_json::from_slice(&to_bytes(first_page.into_body(), usize::MAX).await.unwrap())
            .unwrap();
    assert_eq!(first_payload["data"]["page_size"].as_i64(), Some(1));
    assert_eq!(first_payload["data"]["has_more"].as_bool(), Some(true));
    assert_eq!(
        first_payload["data"]["next_sequence"].as_i64(),
        Some(appended[0].sequence)
    );
    assert_eq!(first_payload["data"]["parts"].as_array().unwrap().len(), 1);
    assert_eq!(
        first_payload["data"]["parts"][0]["payload"]["payload"]["label"].as_str(),
        Some("first bounded event")
    );

    let second_page = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri(format!(
                    "/api/console/applications/{application_id}/logs/runs/{run_id}/debug-stream?from_sequence={}&limit=1",
                    appended[0].sequence
                ))
                .header("cookie", &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(second_page.status(), StatusCode::OK);
    let second_payload: Value =
        serde_json::from_slice(&to_bytes(second_page.into_body(), usize::MAX).await.unwrap())
            .unwrap();
    assert_eq!(second_payload["data"]["has_more"].as_bool(), Some(false));
    assert_eq!(
        second_payload["data"]["next_sequence"].as_i64(),
        Some(appended[1].sequence)
    );
    assert_eq!(
        second_payload["data"]["parts"][0]["payload"]["payload"]["label"].as_str(),
        Some("second bounded event")
    );
}

#[tokio::test]
async fn get_debug_variable_snapshot_restores_latest_preview_inputs_and_outputs() {
    let app = test_app().await;
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
                            "node-start": { "query": "总结退款政策" },
                            "node-llm": { "prompt_messages": ["resolved prompt must stay audit-only"] }
                        },
                        "debug_session_id": DEBUG_SESSION_ID
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(preview.status(), StatusCode::CREATED);
    let preview_body = to_bytes(preview.into_body(), usize::MAX).await.unwrap();
    let preview_payload: Value = serde_json::from_slice(&preview_body).unwrap();
    let flow_run_id = preview_payload["data"]["flow_run"]["id"].as_str().unwrap();
    let draft_id = preview_payload["data"]["flow_run"]["draft_id"]
        .as_str()
        .unwrap();

    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri(format!(
                    "/api/console/applications/{application_id}/orchestration/debug-variable-snapshot"
                ))
                .header("cookie", &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let payload: Value =
        serde_json::from_slice(&to_bytes(response.into_body(), usize::MAX).await.unwrap()).unwrap();
    assert_eq!(
        payload["data"]["snapshot_schema_version"],
        "1flowbase.debug-variable-snapshot/v1"
    );
    assert!(payload["data"]["workspace_id"].is_string());
    assert!(payload["data"]["actor_user_id"].is_string());
    assert_eq!(payload["data"]["draft_id"], draft_id);
    assert_eq!(payload["data"]["flow_schema_version"], "1flowbase.flow/v2");
    let document_hash = payload["data"]["document_hash"].as_str().unwrap();
    assert!(document_hash.starts_with("sha256:"));
    let debug_session_id = payload["data"]["debug_session_id"].as_str().unwrap();
    assert_eq!(debug_session_id, "");
    assert!(payload["data"]["document_hash"]
        .as_str()
        .unwrap()
        .starts_with("sha256:"));
    assert_eq!(payload["data"]["snapshot_completeness"], "complete");
    assert_eq!(
        payload["data"]["latest_run_scope"],
        json!({
            "flow_run_id": flow_run_id,
            "run_mode": "debug_node_preview",
            "status": "succeeded",
            "target_node_id": "node-llm"
        })
    );
    assert_eq!(
        payload["data"]["variable_cache"]["node-start"]["query"],
        "总结退款政策"
    );
    assert_eq!(payload["data"]["source_flow_run_ids"], json!({}));
    assert!(payload["data"]["variable_cache"]["node-llm"]["prompt_messages"].is_null());
    assert_eq!(
        payload["data"]["variable_cache"]["node-llm"]["text"],
        "reply:总结退款政策"
    );
    assert!(payload["data"]["source_node_run_ids"]["node-llm"]["text"].is_null());
}

#[tokio::test]
async fn external_agent_opaque_boundary_keeps_external_trust_level() {
    let (app, database_url) = test_app_with_database_url().await;
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
    assert_eq!(preview.status(), StatusCode::CREATED);
    let preview_body = to_bytes(preview.into_body(), usize::MAX).await.unwrap();
    let preview_payload: Value = serde_json::from_slice(&preview_body).unwrap();
    let run_id =
        Uuid::parse_str(preview_payload["data"]["flow_run"]["id"].as_str().unwrap()).unwrap();
    let store = storage_durable::build_main_durable_postgres(&database_url)
        .await
        .unwrap()
        .store;
    control_plane::runtime_observability::mark_external_opaque_boundary(
        &store,
        run_id,
        json!({ "reason": "external local tool execution not observed" }),
    )
    .await
    .unwrap();

    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri(format!(
                    "/api/console/applications/{application_id}/logs/runs/{run_id}/debug-stream"
                ))
                .header("cookie", &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let payload: Value =
        serde_json::from_slice(&to_bytes(response.into_body(), usize::MAX).await.unwrap()).unwrap();
    assert!(payload["data"]["parts"]
        .as_array()
        .unwrap()
        .iter()
        .any(|part| {
            part["trust_level"] == "external_opaque"
                && part["payload"]["event_type"] == "external_agent_opaque_boundary_marked"
        }));
}

#[tokio::test]
async fn stream_debug_run_returns_flow_accepted_before_background_compile_finishes() {
    let (state, _database_url) = crate::_tests::support::test_api_state_with_database_url().await;
    let app = crate::app_with_state(state.clone());
    let (cookie, csrf) = login_and_capture_cookie(&app, "root", "change-me").await;
    let create = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/console/applications")
                .header("cookie", &cookie)
                .header("x-csrf-token", &csrf)
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "application_type": "agent_flow",
                        "name": "Fast Start SSE",
                        "description": "runtime stream",
                        "icon": "RobotOutlined",
                        "icon_type": "iconfont",
                        "icon_background": "#E6F7F2"
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(create.status(), StatusCode::CREATED);
    let body = to_bytes(create.into_body(), usize::MAX).await.unwrap();
    let payload: Value = serde_json::from_slice(&body).unwrap();
    let application_id = payload["data"]["id"].as_str().unwrap();

    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!(
                    "/api/console/applications/{application_id}/orchestration/debug-runs/stream"
                ))
                .header("cookie", &cookie)
                .header("x-csrf-token", &csrf)
                .header("accept", "text/event-stream")
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "input_payload": { "node-start": { "query": "hello" } }
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), axum::http::StatusCode::OK);
    let body = crate::_tests::support::read_first_sse_frame(response).await;
    assert!(body.contains("\"type\":\"flow_accepted\""), "{body}");
    assert!(!body.contains("\"type\":\"flow_started\""), "{body}");
}

#[tokio::test]
async fn application_runtime_routes_stream_debug_run_returns_flow_accepted() {
    let app = test_app().await;
    let (cookie, csrf) = login_and_capture_cookie(&app, "root", "change-me").await;
    let provider_instance_id = create_ready_provider_instance(&app, &cookie, &csrf).await;
    let application_id =
        seed_agent_flow_application(&app, &cookie, &csrf, &provider_instance_id).await;

    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!(
                    "/api/console/applications/{application_id}/orchestration/debug-runs/stream"
                ))
                .header("cookie", &cookie)
                .header("x-csrf-token", &csrf)
                .header("accept", "text/event-stream")
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "input_payload": {
                            "node-start": { "query": "请总结退款政策" }
                        }
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(
        response.headers()["content-type"].to_str().unwrap(),
        "text/event-stream"
    );
    let stream_text = crate::_tests::support::read_first_sse_frame(response).await;

    assert!(
        stream_text.contains("event: flow_accepted"),
        "{stream_text}"
    );
    assert!(
        stream_text.contains("\"type\":\"flow_accepted\""),
        "{stream_text}"
    );
    let run_id = sse_data_payload(&stream_text)["run_id"]
        .as_str()
        .unwrap()
        .to_string();
    let text_delta_events =
        wait_for_persisted_text_delta_events(&app, &cookie, &application_id, &run_id).await;
    assert!(
        !text_delta_events.is_empty(),
        "streamed debug run should persist durable text_delta evidence"
    );
    let text_delta = &text_delta_events[0];
    let text_delta_payload = resolve_runtime_debug_artifact_value(
        &app,
        &cookie,
        &application_id,
        &text_delta["payload"]["payload"],
    )
    .await;
    assert!(!text_delta_payload["text"].as_str().unwrap().is_empty());
    assert!(
        text_delta_payload["delta"].is_null(),
        "streamed debug run should not persist legacy provider delta payload: {text_delta_payload:?}"
    );
}
