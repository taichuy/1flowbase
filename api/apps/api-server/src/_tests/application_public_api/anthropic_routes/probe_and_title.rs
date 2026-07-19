use super::*;

/// Root #1366 AC-003 / AC-005: Anthropic metadata user IDs require provider support before runs exist.
#[tokio::test]
async fn d2_ac_003_anthropic_metadata_user_id_requires_end_user_reference_capability_before_run_creation(
) {
    let (app, state) = test_app_with_state().await;
    let token = setup_published_app(&app, "Anthropic Probe Compatible Route App").await;
    assert_published_anthropic_plan_has_provider_route(state.as_ref()).await;
    let before = flow_run_count(state.as_ref()).await;

    let response = post_json(
        &app,
        "/v1/messages",
        ("x-api-key", token),
        json!({
            "model": ANTHROPIC_FIXTURE_MODEL,
            "max_tokens": 1,
            "messages": [
                {"role": "user", "content": "test"}
            ],
            "metadata": {
                "user_id": "{\"device_id\":\"probe-device\",\"account_uuid\":\"\",\"session_id\":\"probe-session\"}"
            }
        }),
    )
    .await;

    assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);
    let payload = response_json(response).await;
    assert_eq!(
        payload["error"]["type"],
        json!("provider_capability_mismatch")
    );
    assert_eq!(flow_run_count(state.as_ref()).await, before);
}

#[tokio::test]
async fn anthropic_probe_message_requires_active_publication() {
    let (app, state) = test_app_with_state().await;
    let token =
        setup_unpublished_app_key(&app, "Anthropic Unpublished Probe Compatible Route App").await;

    let response = post_json(
        &app,
        "/v1/messages",
        ("x-api-key", token),
        json!({
            "model": ANTHROPIC_FIXTURE_MODEL,
            "max_tokens": 1,
            "messages": [
                {"role": "user", "content": "test"}
            ]
        }),
    )
    .await;

    assert_eq!(response.status(), StatusCode::CONFLICT);
    let payload = response_json(response).await;
    assert_eq!(payload["error"]["type"], json!("application_not_published"));
    assert_eq!(flow_run_count(state.as_ref()).await, 0);
}

#[tokio::test]
async fn d2_ac_007_anthropic_output_config_is_unsupported_before_publication_lookup() {
    let (app, state) = test_app_with_state().await;
    let token = setup_unpublished_app_key(
        &app,
        "Anthropic Unpublished Structured Compatible Route App",
    )
    .await;

    let response = post_json(
        &app,
        "/v1/messages",
        ("x-api-key", token),
        json!({
            "model": ANTHROPIC_FIXTURE_MODEL,
            "max_tokens": 64,
            "stream": true,
            "messages": [
                {"role": "user", "content": "帮我找找这个代码位置"}
            ],
            "output_config": {
                "format": {
                    "type": "json_schema",
                    "schema": {
                        "type": "object",
                        "properties": {
                            "title": { "type": "string" }
                        },
                        "required": ["title"],
                        "additionalProperties": false
                    }
                }
            }
        }),
    )
    .await;

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    let payload = response_json(response).await;
    assert_eq!(payload["error"]["type"], json!("unsupported_feature"));
    assert_eq!(flow_run_count(state.as_ref()).await, 0);
}
