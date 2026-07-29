use super::*;

#[tokio::test]
async fn anthropic_messages_accepts_x_api_key_and_preserves_model() {
    let (app, state) = test_app_with_state().await;
    let token = setup_published_app(&app, "Anthropic Compatible Route App").await;
    assert_published_anthropic_plan_has_provider_route(state.as_ref()).await;

    let response = post_json(&app, "/v1/messages", ("x-api-key", token), anthropic_body()).await;

    assert_eq!(response.status(), StatusCode::OK);
    let payload = response_json(response).await;
    assert_eq!(payload["type"], json!("message"));
    assert_eq!(payload["model"], json!(ANTHROPIC_FIXTURE_MODEL));
    assert_eq!(payload["content"][0]["type"], json!("text"));
}

#[tokio::test]
async fn anthropic_messages_accepts_last_user_multimodal_content() {
    let (app, state) = test_app_with_state().await;
    let token = setup_published_app(&app, "Anthropic Multimodal Compatible Route App").await;
    assert_published_anthropic_plan_has_provider_route(state.as_ref()).await;

    let response = post_json(
        &app,
        "/v1/messages",
        ("x-api-key", token),
        anthropic_multimodal_body(),
    )
    .await;

    assert_eq!(response.status(), StatusCode::OK);
    let payload = response_json(response).await;
    assert_eq!(payload["type"], json!("message"));
    assert_ne!(
        payload["error"]["message"],
        json!("messages is not supported by this endpoint")
    );
}

#[tokio::test]
async fn ac_001_anthropic_messages_accept_tools_and_create_a_run() {
    let (app, state) = test_app_with_state().await;
    let token = setup_published_app(&app, "Anthropic Unsupported Tool Route App").await;
    let before = flow_run_count(state.as_ref()).await;
    let mut body = anthropic_body();
    body["tools"] = json!([
        {
            "name": "lookup_order",
            "description": "Find an order",
            "input_schema": {
                "type": "object",
                "properties": {
                    "order_id": {"type": "string"}
                }
            }
        }
    ]);
    body["tool_choice"] = json!({"type": "auto"});

    let response = post_json(&app, "/v1/messages", ("x-api-key", token), body).await;

    assert_eq!(response.status(), StatusCode::OK);
    let payload = response_json(response).await;
    assert_eq!(payload["type"], json!("message"));
    assert_eq!(flow_run_count(state.as_ref()).await, before + 1);
    let input_payload: Value = sqlx::query_scalar(
        "select input_payload from flow_runs order by created_at desc, id desc limit 1",
    )
    .fetch_one(state.store.pool())
    .await
    .expect("Anthropic tool run should persist its Native input");
    assert_eq!(
        input_payload["node-start"]["tools"][0]["name"],
        "lookup_order"
    );
    assert_eq!(input_payload["node-start"]["tool_choice"], "auto");
}

#[tokio::test]
async fn d2_ac_001_anthropic_nested_unknown_fields_reject_before_run_or_provider() {
    let (app, state) = test_app_with_state().await;
    let token = setup_published_app(&app, "Anthropic Nested Unknown Field App").await;
    let before = flow_run_count(state.as_ref()).await;
    let requests = [
        json!({
            "model": ANTHROPIC_FIXTURE_MODEL,
            "messages": [{
                "role": "user",
                "content": [{
                    "type": "text",
                    "text": "hello",
                    "unexpected": "no canonical owner"
                }]
            }]
        }),
        json!({
            "model": ANTHROPIC_FIXTURE_MODEL,
            "messages": [{
                "role": "user",
                "content": [{
                    "type": "image",
                    "source": {
                        "type": "base64",
                        "media_type": "image/png",
                        "data": "aW1hZ2U=",
                        "unexpected": "no canonical owner"
                    }
                }]
            }]
        }),
        json!({
            "model": ANTHROPIC_FIXTURE_MODEL,
            "system": [{
                "type": "text",
                "text": "Use the support playbook.",
                "cache_control": {
                    "type": "ephemeral",
                    "unexpected": "no canonical owner"
                }
            }],
            "messages": [{"role": "user", "content": "hello"}]
        }),
    ];

    for body in requests {
        let response = post_json(&app, "/v1/messages", ("x-api-key", token.clone()), body).await;
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        let payload = response_json(response).await;
        assert_eq!(payload["error"]["type"], json!("invalid_request"));
    }
    assert_eq!(flow_run_count(state.as_ref()).await, before);
}

#[tokio::test]
async fn root_1477_anthropic_messages_persist_trusted_compatibility_mode() {
    let (app, state) = test_app_with_state().await;
    let token = setup_published_app(&app, "Anthropic Session History Route App").await;
    let session_id = "claude-code-session-1".to_string();

    let first = post_json_with_headers(
        &app,
        "/v1/messages",
        ("x-api-key", token.clone()),
        vec![("x-claude-code-session-id", session_id.clone())],
        json!({
            "model": ANTHROPIC_FIXTURE_MODEL,
            "max_tokens": 64,
            "stream": true,
            "messages": [
                {"role": "user", "content": "Describe uploads/agent-flow-preview-debug.png"}
            ]
        }),
    )
    .await;
    assert_eq!(first.status(), StatusCode::OK);

    let second = post_json_with_headers(
        &app,
        "/v1/messages",
        ("x-api-key", token.clone()),
        vec![("x-claude-code-session-id", session_id.clone())],
        json!({
            "model": ANTHROPIC_FIXTURE_MODEL,
            "max_tokens": 64,
            "stream": true,
            "messages": [
                {"role": "user", "content": "Find the corresponding code"}
            ]
        }),
    )
    .await;
    assert_eq!(second.status(), StatusCode::OK);

    let compatibility_modes = sqlx::query_scalar::<_, Option<String>>(
        "select compatibility_mode from flow_runs order by created_at asc, id asc",
    )
    .fetch_all(state.store.pool())
    .await
    .unwrap();
    assert_eq!(
        compatibility_modes,
        vec![
            Some("anthropic-messages-v1".to_string()),
            Some("anthropic-messages-v1".to_string()),
        ]
    );

    let third = post_json_with_headers(
        &app,
        "/v1/messages",
        ("x-api-key", token.clone()),
        vec![("x-claude-code-session-id", session_id)],
        json!({
            "model": ANTHROPIC_FIXTURE_MODEL,
            "max_tokens": 64,
            "stream": true,
            "messages": [
                {"role": "user", "content": "Keep going"}
            ]
        }),
    )
    .await;
    assert_eq!(third.status(), StatusCode::OK);

    let compatibility_modes = sqlx::query_scalar::<_, Option<String>>(
        "select compatibility_mode from flow_runs order by created_at asc, id asc",
    )
    .fetch_all(state.store.pool())
    .await
    .unwrap();
    assert_eq!(
        compatibility_modes,
        vec![
            Some("anthropic-messages-v1".to_string()),
            Some("anthropic-messages-v1".to_string()),
            Some("anthropic-messages-v1".to_string()),
        ]
    );
}
