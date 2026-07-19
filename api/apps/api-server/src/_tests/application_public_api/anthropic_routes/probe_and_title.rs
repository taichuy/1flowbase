use super::*;

#[tokio::test]
async fn anthropic_probe_message_uses_published_native_run() {
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

    if response.status() != StatusCode::OK {
        let response_status = response.status();
        let payload = response_json(response).await;
        let latest_flow_run = if flow_run_count(state.as_ref()).await > before {
            sqlx::query_as::<_, (String, Option<Value>)>(
                "select status::text, error_payload from flow_runs order by created_at desc, id desc limit 1",
            )
            .fetch_optional(state.store.pool())
            .await
            .expect("failure diagnostic should query the latest flow run")
        } else {
            None
        };
        let (latest_flow_run_status, latest_error_payload) = latest_flow_run
            .map(|(status, error_payload)| (Some(status), error_payload))
            .unwrap_or((None, None));
        let error_payload = latest_error_payload
            .as_ref()
            .and_then(Value::as_object)
            .map(|payload| {
                Value::Object(
                    ["code", "stage", "source", "status_code", "message"]
                        .into_iter()
                        .filter_map(|field| {
                            payload
                                .get(field)
                                .filter(|value| {
                                    value.is_string()
                                        || value.is_number()
                                        || value.is_boolean()
                                        || value.is_null()
                                })
                                .cloned()
                                .map(|value| (field.to_string(), value))
                        })
                        .collect(),
                )
            })
            .unwrap_or_else(|| json!({}));

        panic!(
            "expected Anthropic probe status 200; actual_status={response_status}; \
             anthropic_error.type={:?}; anthropic_error.message={:?}; \
             latest_flow_run.status={:?}; latest_flow_run.error_payload={error_payload}",
            payload["error"]["type"].as_str(),
            payload["error"]["message"].as_str(),
            latest_flow_run_status,
        );
    }

    assert_eq!(response.status(), StatusCode::OK);
    let payload = response_json(response).await;
    assert_eq!(payload["type"], json!("message"));
    assert_eq!(flow_run_count(state.as_ref()).await, before + 1);
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
