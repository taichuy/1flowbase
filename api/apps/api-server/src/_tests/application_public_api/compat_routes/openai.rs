use super::*;

#[tokio::test]
async fn openai_chat_completions_accepts_bearer_and_preserves_model() {
    let (app, state) = test_app_with_state().await;
    let token = setup_published_app(&app, "OpenAI Compatible Route App").await;
    assert_published_compat_plan_has_provider_route(state.as_ref()).await;

    let response = post_json(
        &app,
        "/v1/chat/completions",
        ("authorization", format!("Bearer {token}")),
        openai_body(false),
    )
    .await;

    let status = response.status();
    let payload = response_json(response).await;
    assert_eq!(status, StatusCode::OK, "{payload}");
    assert_eq!(payload["object"], json!("chat.completion"));
    assert_eq!(payload["model"], json!("provider/custom-model:latest"));
    assert_eq!(payload["choices"][0]["message"]["role"], json!("assistant"));
}

#[tokio::test]
async fn opencode_chat_stream_options_cross_the_request_boundary() {
    let app = test_app().await;
    let token = setup_published_app(&app, "OpenCode Stream Options App").await;
    let mut body = openai_body(true);
    body["stream_options"] = json!({ "include_usage": true });

    let response = post_json(
        &app,
        "/v1/chat/completions",
        ("authorization", format!("Bearer {token}")),
        body,
    )
    .await;

    assert_eq!(response.status(), StatusCode::OK);
}

#[tokio::test]
async fn codex_native_reasoning_include_reaches_the_selected_provider_capability_boundary() {
    let (app, state) = test_app_with_state().await;
    let token = setup_published_app(&app, "Codex Reasoning Include App").await;
    let before = flow_run_count(state.as_ref()).await;
    let mut body = responses_body(false);
    body["store"] = json!(false);
    body["parallel_tool_calls"] = json!(false);
    body["include"] = json!(["reasoning.encrypted_content"]);
    body["prompt_cache_key"] = json!("thread-1");
    body["client_metadata"] = json!({
        "session_id": "session-1",
        "thread_id": "thread-1"
    });
    body["reasoning"] = Value::Null;

    let response = post_json(
        &app,
        "/v1/responses",
        ("authorization", format!("Bearer {token}")),
        body,
    )
    .await;

    assert_eq!(response.status(), StatusCode::BAD_GATEWAY);
    let payload = response_json(response).await;
    assert_eq!(payload["error"]["code"], json!("provider_invalid_response"));
    assert_eq!(flow_run_count(state.as_ref()).await, before + 1);
}

#[tokio::test]
async fn openai_chat_completions_accepts_root_endpoint_for_plain_base_url_clients() {
    let app = test_app().await;
    let token = setup_published_app(&app, "OpenAI Plain Base URL Compatible Route App").await;

    let response = post_json(
        &app,
        "/chat/completions",
        ("authorization", format!("Bearer {token}")),
        openai_body(false),
    )
    .await;

    assert_eq!(response.status(), StatusCode::OK);
    let payload = response_json(response).await;
    assert_eq!(payload["object"], json!("chat.completion"));
    assert_eq!(payload["model"], json!("provider/custom-model:latest"));
}

#[tokio::test]
async fn openai_chat_completions_rejects_removed_prefixed_openai_alias() {
    let app = test_app().await;
    let token = setup_published_app(&app, "OpenAI Prefixed Alias Compatible Route App").await;

    let response = post_json(
        &app,
        "/openai/v1/chat/completions",
        ("authorization", format!("Bearer {token}")),
        openai_body(false),
    )
    .await;

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn openai_compatible_routes_reject_nested_v1_aliases() {
    let app = test_app().await;
    let token = setup_published_app(&app, "OpenAI Nested Alias Compatible Route App").await;

    let models = get_models(&app, "/v1/chat/completions/v1/models", &token).await;
    assert_eq!(models.status(), StatusCode::NOT_FOUND);

    let chat_completion = post_json(
        &app,
        "/v1/chat/completions/v1/chat/completions",
        ("authorization", format!("Bearer {token}")),
        openai_body(false),
    )
    .await;
    assert_eq!(chat_completion.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn openai_responses_accepts_blocking_text_input() {
    let app = test_app().await;
    let token = setup_published_app(&app, "OpenAI Responses Blocking App").await;

    let response = post_json(
        &app,
        "/v1/responses",
        ("authorization", format!("Bearer {token}")),
        responses_body(false),
    )
    .await;

    assert_eq!(response.status(), StatusCode::OK);
    let payload = response_json(response).await;
    assert_eq!(payload["object"], json!("response"));
    assert_eq!(payload["status"], json!("completed"));
    assert_eq!(payload["model"], json!("provider/custom-model:latest"));
    assert!(payload["id"].as_str().unwrap().starts_with("resp_"));
    assert_eq!(payload["output"][0]["type"], json!("message"));
    assert_eq!(
        payload["output"][0]["content"][0]["type"],
        json!("output_text")
    );
    assert!(payload["output_text"].is_string());
}

#[tokio::test]
async fn codex_responses_store_false_crosses_the_request_boundary() {
    let app = test_app().await;
    let token = setup_published_app(&app, "Codex Store False App").await;
    let mut body = responses_body(false);
    body["store"] = json!(false);

    let response = post_json(
        &app,
        "/v1/responses",
        ("authorization", format!("Bearer {token}")),
        body,
    )
    .await;

    assert_eq!(response.status(), StatusCode::OK);
}

#[tokio::test]
async fn codex_parallel_tool_calls_false_crosses_the_request_boundary() {
    let app = test_app().await;
    let token = setup_published_app(&app, "Codex Parallel Tool Calls False App").await;
    let mut body = responses_body(false);
    body["parallel_tool_calls"] = json!(false);

    let response = post_json(
        &app,
        "/v1/responses",
        ("authorization", format!("Bearer {token}")),
        body,
    )
    .await;

    assert_eq!(response.status(), StatusCode::OK);
}

#[tokio::test]
async fn root_1477_openai_public_runs_persist_trusted_compatibility_mode() {
    let (app, state) = test_app_with_state().await;
    let token = setup_published_app(&app, "OpenAI Canonical Contract App").await;

    let chat = post_json(
        &app,
        "/v1/chat/completions",
        ("authorization", format!("Bearer {token}")),
        openai_body(true),
    )
    .await;
    assert_eq!(chat.status(), StatusCode::OK);
    let responses = post_json(
        &app,
        "/v1/responses",
        ("authorization", format!("Bearer {token}")),
        responses_body(true),
    )
    .await;
    assert_eq!(responses.status(), StatusCode::OK);

    let modes = sqlx::query_scalar::<_, Option<String>>(
        "select compatibility_mode from flow_runs order by created_at asc, id asc",
    )
    .fetch_all(state.store.pool())
    .await
    .unwrap();
    assert_eq!(
        modes,
        vec![
            Some("openai-chat-completions-v1".to_string()),
            Some("openai-responses-v1".to_string()),
        ]
    );
}

#[tokio::test]
async fn openai_responses_accepts_root_endpoint_for_plain_base_url_clients() {
    let app = test_app().await;
    let token = setup_published_app(&app, "OpenAI Responses Root Base URL App").await;

    let response = post_json(
        &app,
        "/responses",
        ("authorization", format!("Bearer {token}")),
        responses_body(false),
    )
    .await;

    assert_eq!(response.status(), StatusCode::OK);
    let payload = response_json(response).await;
    assert_eq!(payload["object"], json!("response"));
    assert_eq!(payload["status"], json!("completed"));
    assert_eq!(payload["model"], json!("provider/custom-model:latest"));
}

#[tokio::test]
async fn openai_responses_resolves_previous_response_id_before_creating_a_run() {
    let (app, state) = test_app_with_state().await;
    let token = setup_published_app(&app, "OpenAI Responses Unsupported Continuation App").await;
    let before = flow_run_count(state.as_ref()).await;
    let mut body = responses_body(false);
    body["previous_response_id"] = json!("resp_11111111-1111-1111-1111-111111111111");

    let response = post_json(
        &app,
        "/v1/responses",
        ("authorization", format!("Bearer {token}")),
        body,
    )
    .await;

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
    assert_eq!(flow_run_count(state.as_ref()).await, before);
}

#[tokio::test]
async fn openai_responses_treats_opaque_previous_response_id_as_provider_lookup() {
    let (app, state) = test_app_with_state().await;
    let token = setup_published_app(&app, "OpenAI Responses Invalid Previous App").await;
    let before = flow_run_count(state.as_ref()).await;
    let mut body = responses_body(false);
    body["previous_response_id"] = json!("resp_not-a-native-run-id");

    let response = post_json(
        &app,
        "/v1/responses",
        ("authorization", format!("Bearer {token}")),
        body,
    )
    .await;

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
    let payload = response_json(response).await;
    assert_eq!(payload["error"]["code"], json!("application_run_not_found"));
    assert_eq!(flow_run_count(state.as_ref()).await, before);
}

#[tokio::test]
async fn openai_responses_rejects_missing_previous_response_before_creating_a_run() {
    let (app, state) = test_app_with_state().await;
    let token = setup_published_app(&app, "OpenAI Responses Previous Consumer App").await;
    let before = flow_run_count(state.as_ref()).await;
    let mut body = responses_body(false);
    body["previous_response_id"] = json!("resp_11111111-1111-1111-1111-111111111111");
    let response = post_json(
        &app,
        "/v1/responses",
        ("authorization", format!("Bearer {token}")),
        body,
    )
    .await;

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
    assert_eq!(flow_run_count(state.as_ref()).await, before);
}

#[tokio::test]
async fn openai_responses_function_call_output_resolves_callback_before_run_creation() {
    let (app, state) = test_app_with_state().await;
    let token = setup_published_app(&app, "OpenAI Responses Callback Binding App").await;
    let before = flow_run_count(state.as_ref()).await;

    let body = json!({
        "model": "provider/custom-model:latest",
        "input": [
            {
                "type": "function_call_output",
                "call_id": encode_openai_callback_tool_call_id(
                    uuid::Uuid::from_u128(0x55555555555555555555555555555555),
                    "call_inventory"
                ),
                "output": { "stock": 7 }
            }
        ]
    });
    let response = post_json(
        &app,
        "/v1/responses",
        ("authorization", format!("Bearer {token}")),
        body,
    )
    .await;

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    let payload = response_json(response).await;
    assert_eq!(payload["error"]["code"], json!("invalid_request"));
    assert_eq!(payload["error"]["param"], json!("input"));
    assert_eq!(flow_run_count(state.as_ref()).await, before);
}

#[tokio::test]
async fn d4_ac_016_openai_responses_input_file_reaches_the_selected_provider_capability_boundary() {
    let (app, state) = test_app_with_state().await;
    let token = setup_published_app(&app, "OpenAI Responses Nested Input File App").await;
    let before = flow_run_count(state.as_ref()).await;
    let body = json!({
        "model": "provider/custom-model:latest",
        "input": [{
            "type": "message",
            "role": "user",
            "content": [{"type": "input_file", "file_id": "file_123"}]
        }]
    });

    let response = post_json(
        &app,
        "/v1/responses",
        ("authorization", format!("Bearer {token}")),
        body,
    )
    .await;

    assert_eq!(response.status(), StatusCode::BAD_GATEWAY);
    let payload = response_json(response).await;
    assert_eq!(payload["error"]["code"], json!("provider_invalid_response"));
    assert_eq!(flow_run_count(state.as_ref()).await, before + 1);
}

#[tokio::test]
async fn d2_ac_001_openai_nested_unknown_content_fields_reject_before_run_or_provider() {
    let (app, state) = test_app_with_state().await;
    let token = setup_published_app(&app, "OpenAI Nested Unknown Content Field App").await;
    let before = flow_run_count(state.as_ref()).await;

    let chat = post_json(
        &app,
        "/v1/chat/completions",
        ("authorization", format!("Bearer {token}")),
        json!({
            "model": "provider/custom-model:latest",
            "messages": [{
                "role": "user",
                "content": [{
                    "type": "text",
                    "text": "hello",
                    "unexpected": "no canonical owner"
                }]
            }]
        }),
    )
    .await;
    assert_eq!(chat.status(), StatusCode::BAD_REQUEST);
    let chat_payload = response_json(chat).await;
    assert_eq!(chat_payload["error"]["code"], json!("invalid_request"));

    let responses = post_json(
        &app,
        "/v1/responses",
        ("authorization", format!("Bearer {token}")),
        json!({
            "model": "provider/custom-model:latest",
            "input": [{
                "type": "message",
                "role": "user",
                "content": [{
                    "type": "input_image",
                    "image_url": {
                        "url": "https://example.com/cat.png",
                        "unexpected": "no canonical owner"
                    }
                }]
            }]
        }),
    )
    .await;
    assert_eq!(responses.status(), StatusCode::BAD_REQUEST);
    let responses_payload = response_json(responses).await;
    assert_eq!(responses_payload["error"]["code"], json!("invalid_request"));
    assert_eq!(flow_run_count(state.as_ref()).await, before);
}

#[tokio::test]
async fn openai_models_lists_start_node_configured_models() {
    let app = test_app().await;
    let token = setup_published_app(&app, "OpenAI Compatible Models App").await;

    let response = get_models(&app, "/v1/models", &token).await;

    assert_eq!(response.status(), StatusCode::OK);
    let payload = response_json(response).await;
    assert_eq!(payload["object"], json!("list"));
    assert_eq!(payload["data"][0]["id"], json!("qwen3.6-35b-a3b"));
    assert_eq!(payload["data"][0]["name"], json!("Qwen 3.6 35B"));
    assert_eq!(payload["data"][0]["object"], json!("model"));
    assert_eq!(payload["data"][0]["context_window"], json!(128000));
    assert_eq!(payload["data"][0]["max_output_tokens"], json!(32000));
    assert_eq!(
        payload["data"][0]["auto_compact_token_limit"],
        json!(110000)
    );
    assert_eq!(
        payload["data"][0]["limit"],
        json!({
            "context": 128000,
            "input": 128000,
            "output": 32000
        })
    );
    assert_eq!(payload["data"][1]["id"], json!("deepseek-v4-flash"));
}

#[tokio::test]
async fn native_models_returns_canonical_start_node_model_capabilities() {
    let app = test_app().await;
    let token = setup_published_app(&app, "Native Canonical Models App").await;

    let response = get_models(&app, "/api/agent/v1/models", &token).await;

    assert_eq!(response.status(), StatusCode::OK);
    let payload = response_json(response).await;
    assert_eq!(payload["object"], json!("list"));
    assert_eq!(payload["data"][0]["id"], json!("qwen3.6-35b-a3b"));
    assert_eq!(payload["data"][0]["context_window"], json!(128000));
    assert_eq!(payload["data"][0]["max_output_tokens"], json!(32000));
    assert_eq!(payload["data"][0]["capabilities"]["reasoning"], json!(true));
    assert_eq!(
        payload["data"][0]["reasoning"]["supported_efforts"],
        json!(["low", "medium", "high"])
    );
}

#[tokio::test]
async fn openai_models_with_client_version_returns_codex_model_metadata() {
    let app = test_app().await;
    let token = setup_published_app(&app, "Codex Compatible Models App").await;

    let response = get_models(&app, "/v1/models?client_version=0.62.0", &token).await;

    assert_eq!(response.status(), StatusCode::OK);
    let payload = response_json(response).await;
    assert!(payload.get("data").is_none(), "{payload}");
    assert_eq!(payload["models"][0]["slug"], json!("qwen3.6-35b-a3b"));
    assert_eq!(payload["models"][0]["display_name"], json!("Qwen 3.6 35B"));
    assert_eq!(payload["models"][0]["context_window"], json!(128000));
    assert_eq!(payload["models"][0]["max_context_window"], json!(128000));
    assert_eq!(payload["models"][0]["max_output_tokens"], json!(32000));
    assert_eq!(
        payload["models"][0]["auto_compact_token_limit"],
        json!(110000)
    );
    assert_eq!(
        payload["models"][0]["limit"],
        json!({
            "context": 128000,
            "input": 128000,
            "output": 32000
        })
    );
    assert_eq!(payload["models"][1]["slug"], json!("deepseek-v4-flash"));
}

#[tokio::test]
async fn openai_models_accepts_full_chat_completions_base_url_alias() {
    let app = test_app().await;
    let token = setup_published_app(&app, "OpenAI Full Endpoint Base URL App").await;

    let response = get_models(&app, "/v1/chat/completions/models", &token).await;

    assert_eq!(response.status(), StatusCode::OK);
    let payload = response_json(response).await;
    assert_eq!(payload["data"][0]["id"], json!("qwen3.6-35b-a3b"));
}

#[tokio::test]
async fn openai_chat_accepts_tools_and_creates_a_run() {
    let (app, state) = test_app_with_state().await;
    let token = setup_published_app(&app, "OpenAI Unsupported Tool Route App").await;
    let before = flow_run_count(state.as_ref()).await;
    let mut body = openai_body(false);
    body["tools"] = json!([{"type": "function", "function": {"name": "lookup"}}]);
    body["tool_choice"] = json!("auto");

    let response = post_json(
        &app,
        "/v1/chat/completions",
        ("authorization", format!("Bearer {token}")),
        body,
    )
    .await;

    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(flow_run_count(state.as_ref()).await, before + 1);
}
