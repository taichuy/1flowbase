use super::*;

#[tokio::test]
async fn openai_chat_completions_accepts_bearer_and_preserves_model() {
    let app = test_app().await;
    let token = setup_published_app(&app, "OpenAI Compatible Route App").await;

    let response = post_json(
        &app,
        "/v1/chat/completions",
        ("authorization", format!("Bearer {token}")),
        openai_body(false),
    )
    .await;

    assert_eq!(response.status(), StatusCode::OK);
    let payload = response_json(response).await;
    assert_eq!(payload["object"], json!("chat.completion"));
    assert_eq!(payload["model"], json!("provider/custom-model:latest"));
    assert_eq!(payload["choices"][0]["message"]["role"], json!("assistant"));
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
async fn openai_responses_continues_from_previous_response_id() {
    let app = test_app().await;
    let token = setup_published_app(&app, "OpenAI Responses Continuation App").await;

    let first = post_json(
        &app,
        "/v1/responses",
        ("authorization", format!("Bearer {token}")),
        responses_body(false),
    )
    .await;
    assert_eq!(first.status(), StatusCode::OK);
    let first_payload = response_json(first).await;
    let previous_response_id = first_payload["id"].as_str().unwrap().to_string();

    let mut next_body = responses_body(false);
    next_body["input"] = json!("Follow up");
    next_body["previous_response_id"] = json!(previous_response_id);
    let next = post_json(
        &app,
        "/v1/responses",
        ("authorization", format!("Bearer {token}")),
        next_body,
    )
    .await;

    assert_eq!(next.status(), StatusCode::OK);
    let next_payload = response_json(next).await;
    assert_eq!(next_payload["previous_response_id"], first_payload["id"]);
    assert_ne!(next_payload["id"], first_payload["id"]);
}

#[tokio::test]
async fn openai_responses_rejects_invalid_previous_response_id() {
    let app = test_app().await;
    let token = setup_published_app(&app, "OpenAI Responses Invalid Previous App").await;
    let mut body = responses_body(false);
    body["previous_response_id"] = json!("resp_not-a-native-run-id");

    let response = post_json(
        &app,
        "/v1/responses",
        ("authorization", format!("Bearer {token}")),
        body,
    )
    .await;

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    let payload = response_json(response).await;
    assert_eq!(payload["error"]["param"], json!("previous_response_id"));
    assert_eq!(payload["error"]["code"], json!("invalid_request"));
}

#[tokio::test]
async fn openai_responses_rejects_previous_response_from_another_api_key() {
    let app = test_app().await;
    let first_token = setup_published_app(&app, "OpenAI Responses Previous Owner App").await;
    let second_token = setup_published_app(&app, "OpenAI Responses Previous Consumer App").await;

    let first = post_json(
        &app,
        "/v1/responses",
        ("authorization", format!("Bearer {first_token}")),
        responses_body(false),
    )
    .await;
    assert_eq!(first.status(), StatusCode::OK);
    let first_payload = response_json(first).await;

    let mut body = responses_body(false);
    body["previous_response_id"] = first_payload["id"].clone();
    let response = post_json(
        &app,
        "/v1/responses",
        ("authorization", format!("Bearer {second_token}")),
        body,
    )
    .await;

    assert_eq!(response.status(), StatusCode::FORBIDDEN);
    let payload = response_json(response).await;
    assert_eq!(payload["error"]["code"], json!("application_run_forbidden"));
}

#[tokio::test]
async fn openai_responses_rejects_function_call_output_when_previous_response_mismatches_callback_run(
) {
    let (app, state) = test_app_with_state().await;
    let token = setup_published_app(&app, "OpenAI Responses Callback Binding App").await;

    let first = post_json(
        &app,
        "/v1/responses",
        ("authorization", format!("Bearer {token}")),
        responses_body(false),
    )
    .await;
    assert_eq!(first.status(), StatusCode::OK);
    let first_payload = response_json(first).await;
    let first_run_id = run_id_from_response_id(first_payload["id"].as_str().unwrap()).unwrap();
    let callback_task = seed_llm_callback_for_response_run(state.as_ref(), first_run_id).await;

    let mut second_body = responses_body(false);
    second_body["input"] = json!("Different response");
    let second = post_json(
        &app,
        "/v1/responses",
        ("authorization", format!("Bearer {token}")),
        second_body,
    )
    .await;
    assert_eq!(second.status(), StatusCode::OK);
    let second_payload = response_json(second).await;

    let body = json!({
        "model": "provider/custom-model:latest",
        "previous_response_id": second_payload["id"],
        "input": [
            {
                "type": "function_call_output",
                "call_id": encode_openai_callback_tool_call_id(callback_task.id, "call_inventory"),
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
    assert_eq!(payload["error"]["param"], json!("previous_response_id"));
    assert_eq!(payload["error"]["code"], json!("invalid_request"));
    let stored_task = state
        .store
        .get_callback_task(callback_task.id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(stored_task.status, domain::CallbackTaskStatus::Pending);
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
async fn openai_chat_completions_accepts_tools_for_agent_framework_compatibility() {
    let app = test_app().await;
    let token = setup_published_app(&app, "OpenAI Tool Compatible Route App").await;
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
    let payload = response_json(response).await;
    assert_eq!(payload["object"], json!("chat.completion"));
    assert_eq!(payload["model"], json!("provider/custom-model:latest"));
}
