use super::*;

// #2018 AC-108: an explicit unavailable run-level MCP selection must not be ignored.
#[tokio::test]
async fn preview_mcp_settings_reject_unavailable_instance() {
    let app = test_app().await;
    let (cookie, csrf) = login_and_capture_cookie(&app, "root", "change-me").await;
    let provider = create_ready_provider_instance(&app, &cookie, &csrf).await;
    let application_id = seed_agent_flow_application(&app, &cookie, &csrf, &provider).await;
    let response = app.clone().oneshot(Request::builder().method("POST")
        .uri("/api/console/mcp/instances").header("cookie", &cookie).header("x-csrf-token", &csrf)
        .header("content-type", "application/json").body(Body::from(json!({
            "instance_id":"disabled-preview-instance", "name":"Disabled", "status":"disabled", "default_entry_path":"/"
        }).to_string())).unwrap()).await.unwrap();
    assert_eq!(response.status(), StatusCode::CREATED);
    for instance_id in ["unavailable-preview-instance", "disabled-preview-instance"] {
        for suffix in ["", "/stream"] {
            let response = app.clone().oneshot(Request::builder()
            .method("POST")
            .uri(format!("/api/console/applications/{application_id}/orchestration/debug-runs{suffix}"))
            .header("cookie", &cookie).header("x-csrf-token", &csrf)
            .header("content-type", "application/json")
            .body(Body::from(json!({
                "input_payload": {"node-start": {"query": "MCP selection validation"}},
                "debug_session_id": DEBUG_SESSION_ID,
                "mcp_instance_ids": [instance_id]
            }).to_string())).unwrap()).await.unwrap();
            assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        }
    }
}

// AC-107: selected tools reach the provider and the run keeps its own selection.
#[tokio::test]
async fn preview_mcp_settings_freeze_selection_and_register_tools() {
    let (state, _) = test_api_state_with_database_url().await;
    let app = crate::app_with_state_and_config(state.clone(), &test_config());
    let (cookie, csrf) = login_and_capture_cookie(&app, "root", "change-me").await;
    let mut headers = axum::http::HeaderMap::new();
    headers.insert("cookie", cookie.parse().unwrap());
    let actor = crate::middleware::require_session::require_session(&state, &headers)
        .await
        .unwrap()
        .actor;
    let grant = app.clone().oneshot(Request::builder().method("POST")
        .uri(format!("/api/console/settings/billing/credits/{}/grant", actor.user_id))
        .header("cookie", &cookie).header("x-csrf-token", &csrf).header("content-type", "application/json")
        .body(Body::from(json!({"amount":"100", "reason":"preview MCP fixture", "source_type":"test", "source_id":"preview-mcp", "idempotency_key":"preview-mcp-credit"}).to_string())).unwrap()).await.unwrap();
    assert_eq!(grant.status(), StatusCode::OK);
    let provider =
        create_provider_instance(&app, &cookie, &csrf, None, Some("__echo_tools__")).await;
    let application_id = seed_agent_flow_application(&app, &cookie, &csrf, &provider).await;
    let created = app.clone().oneshot(Request::builder().method("POST")
        .uri("/api/console/mcp/instances").header("cookie", &cookie).header("x-csrf-token", &csrf)
        .header("content-type", "application/json").body(Body::from(json!({
            "instance_id": "preview_mcp", "name": "Preview MCP", "status": "enabled", "default_entry_path": "/"
        }).to_string())).unwrap()).await.unwrap();
    assert_eq!(created.status(), StatusCode::CREATED);
    let response = app.clone().oneshot(Request::builder().method("POST")
        .uri(format!("/api/console/applications/{application_id}/orchestration/debug-runs"))
        .header("cookie", &cookie).header("x-csrf-token", &csrf).header("content-type", "application/json")
        .body(Body::from(json!({
            "input_payload": {"node-start": {"query": "List test tools"}, "sys": {"mcp_instance_ids": ["spoofed"]}},
            "mcp_instance_ids": ["preview_mcp"], "debug_session_id": DEBUG_SESSION_ID
        }).to_string())).unwrap()).await.unwrap();
    let status = response.status();
    let payload: Value =
        serde_json::from_slice(&to_bytes(response.into_body(), usize::MAX).await.unwrap()).unwrap();
    assert_eq!(status, StatusCode::CREATED, "{payload}");
    assert_eq!(
        payload["data"]["flow_run"]["input_payload"]["sys"]["mcp_instance_ids"],
        json!(["preview_mcp"])
    );
    let run_id = payload["data"]["flow_run"]["id"].as_str().unwrap();
    wait_for_run_detail(&app, &cookie, &application_id, run_id, &["succeeded"]).await;
    let snapshot = app.clone().oneshot(Request::builder()
        .uri(format!("/api/console/applications/{application_id}/orchestration/runs/{run_id}/debug-snapshot"))
        .header("cookie", &cookie).body(Body::empty()).unwrap()).await.unwrap();
    let payload: Value =
        serde_json::from_slice(&to_bytes(snapshot.into_body(), usize::MAX).await.unwrap()).unwrap();
    assert!(
        payload["data"]["flow_run"]["output_payload"]
            .to_string()
            .contains("preview_mcp"),
        "{payload}"
    );
    // The streaming start and later human resume use the frozen run selection.
    let application_id = seed_human_input_application(&app, &cookie, &csrf, &provider).await;
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!(
                    "/api/console/applications/{application_id}/orchestration"
                ))
                .header("cookie", &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let state: Value =
        serde_json::from_slice(&to_bytes(response.into_body(), usize::MAX).await.unwrap()).unwrap();
    let mut document = state["data"]["draft"]["document"].clone();
    // Put the provider after the human pause so tool registration is tested on resume.
    for (edge, (source, target)) in document["graph"]["edges"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .zip([
            ("node-start", "node-human"),
            ("node-human", "node-llm"),
            ("node-llm", "node-answer"),
        ])
    {
        edge["source"] = json!(source);
        edge["target"] = json!(target);
    }
    let human = document["graph"]["nodes"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .find(|node| node["id"] == "node-human")
        .unwrap();
    human["bindings"]["prompt"] =
        json!({"kind":"templated_text","value":"Review: {{node-start.query}}"});
    let answer = document["graph"]["nodes"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .find(|node| node["id"] == "node-answer")
        .unwrap();
    answer["bindings"]["answer_template"] = json!({"kind":"selector","value":["node-llm","text"]});

    let response = app.clone().oneshot(Request::builder().method("POST")
        .uri(format!("/api/console/applications/{application_id}/orchestration/debug-runs/stream"))
        .header("cookie", &cookie).header("x-csrf-token", &csrf).header("content-type", "application/json")
        .body(Body::from(json!({"input_payload":{"node-start":{"query":"Resume MCP test"}}, "document":document, "mcp_instance_ids":["preview_mcp"]}).to_string())).unwrap()).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let frame = crate::_tests::support::read_first_sse_frame(response).await;
    let event: Value = serde_json::from_str(
        frame
            .lines()
            .find_map(|line| line.strip_prefix("data:"))
            .unwrap()
            .trim(),
    )
    .unwrap();
    let run_id = event["run_id"].as_str().unwrap();
    wait_for_run_detail(&app, &cookie, &application_id, run_id, &["waiting_human"]).await;
    let response = app.clone().oneshot(Request::builder()
        .uri(format!("/api/console/applications/{application_id}/orchestration/runs/{run_id}/debug-snapshot"))
        .header("cookie", &cookie).body(Body::empty()).unwrap()).await.unwrap();
    let payload: Value =
        serde_json::from_slice(&to_bytes(response.into_body(), usize::MAX).await.unwrap()).unwrap();
    assert_eq!(
        payload["data"]["flow_run"]["input_payload"]["sys"]["mcp_instance_ids"],
        json!(["preview_mcp"])
    );
    let checkpoint_id = payload["data"]["checkpoints"][0]["id"].as_str().unwrap();
    let response = app.clone().oneshot(Request::builder().method("POST")
        .uri(format!("/api/console/applications/{application_id}/orchestration/runs/{run_id}/resume"))
        .header("cookie", &cookie).header("x-csrf-token", &csrf).header("content-type", "application/json")
        .body(Body::from(json!({"checkpoint_id":checkpoint_id,"input_payload":{"node-human":{"input":"approved"}},"mcp_instance_ids":["unavailable-preview-instance"]}).to_string())).unwrap()).await.unwrap();
    let status = response.status();
    let payload: Value =
        serde_json::from_slice(&to_bytes(response.into_body(), usize::MAX).await.unwrap()).unwrap();
    assert_eq!(status, StatusCode::OK, "{payload}");
    assert!(
        payload["data"]["detail"]["flow_run"]["output_payload"]
            .to_string()
            .contains("preview_mcp"),
        "{payload}"
    );
}
