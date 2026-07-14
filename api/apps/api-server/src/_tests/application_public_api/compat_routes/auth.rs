use super::*;

#[tokio::test]
async fn compatible_routes_require_application_api_key() {
    let app = test_app().await;

    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/chat/completions")
                .header("content-type", "application/json")
                .body(Body::from(openai_body(false).to_string()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    let payload = response_json(response).await;
    assert_eq!(payload["error"]["code"], json!("not_authenticated"));

    let responses = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/responses")
                .header("content-type", "application/json")
                .body(Body::from(responses_body(false).to_string()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(responses.status(), StatusCode::UNAUTHORIZED);
    let payload = response_json(responses).await;
    assert_eq!(payload["error"]["code"], json!("not_authenticated"));
}

#[tokio::test]
async fn compatible_routes_return_not_published_for_unpublished_key_application() {
    let app = test_app().await;
    let token = setup_unpublished_app_key(&app, "Compatible Unpublished Route App").await;

    let response = post_json(
        &app,
        "/v1/messages",
        ("x-api-key", token),
        anthropic_body(false),
    )
    .await;

    assert_eq!(response.status(), StatusCode::CONFLICT);
    let payload = response_json(response).await;
    assert_eq!(payload["error"]["type"], json!("application_not_published"));
}

async fn assert_run_creation_route_uses_last_used_cache(
    app: &Router,
    state: &ApiState,
    api_key_id: uuid::Uuid,
    uri: &str,
    header_name: &str,
    header_value: String,
    body: Value,
) {
    assert!(application_api_key_last_used_at(state, api_key_id)
        .await
        .is_none());

    let first = post_json(app, uri, (header_name, header_value.clone()), body.clone()).await;
    assert_eq!(first.status(), StatusCode::OK);
    let first_used_at = application_api_key_last_used_at(state, api_key_id)
        .await
        .expect("first route call should mark the application API key used");

    tokio::time::sleep(Duration::from_millis(10)).await;

    let second = post_json(app, uri, (header_name, header_value), body).await;
    assert_eq!(second.status(), StatusCode::OK);
    let second_used_at = application_api_key_last_used_at(state, api_key_id)
        .await
        .expect("second route call should preserve the first use timestamp under cache TTL");

    assert_eq!(second_used_at, first_used_at);
}

#[tokio::test]
async fn compatible_run_creation_routes_use_application_api_key_last_used_cache() {
    let (app, state) = test_app_with_state().await;

    let (chat_token, chat_api_key_id) =
        setup_published_app_with_key_id(&app, "OpenAI Chat Last Used Cache App").await;
    assert_run_creation_route_uses_last_used_cache(
        &app,
        state.as_ref(),
        chat_api_key_id,
        "/v1/chat/completions",
        "authorization",
        format!("Bearer {chat_token}"),
        openai_body(false),
    )
    .await;

    let (responses_token, responses_api_key_id) =
        setup_published_app_with_key_id(&app, "OpenAI Responses Last Used Cache App").await;
    assert_run_creation_route_uses_last_used_cache(
        &app,
        state.as_ref(),
        responses_api_key_id,
        "/v1/responses",
        "authorization",
        format!("Bearer {responses_token}"),
        responses_body(false),
    )
    .await;

    let (anthropic_token, anthropic_api_key_id) =
        setup_published_app_with_key_id(&app, "Anthropic Last Used Cache App").await;
    assert_run_creation_route_uses_last_used_cache(
        &app,
        state.as_ref(),
        anthropic_api_key_id,
        "/v1/messages",
        "x-api-key",
        anthropic_token,
        anthropic_body(false),
    )
    .await;
}
