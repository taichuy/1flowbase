use super::*;

#[tokio::test]
async fn c1_anthropic_count_tokens_never_falls_back_to_a_local_estimate() {
    let (app, state) = test_app_with_state().await;
    let token = setup_published_app(&app, "Anthropic Count Tokens Compatible Route App").await;
    let before = flow_run_count(state.as_ref()).await;

    let response = post_json(
        &app,
        "/v1/messages/count_tokens",
        ("x-api-key", token),
        json!({
            "model": ANTHROPIC_FIXTURE_MODEL,
            "messages": [
                {"role": "user", "content": "Count this prompt"}
            ]
        }),
    )
    .await;

    assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);
    let payload = response_json(response).await;
    assert_eq!(payload["error"]["type"], json!("operation_unbound"));
    assert!(payload.get("input_tokens").is_none(), "{payload}");
    assert_eq!(flow_run_count(state.as_ref()).await, before);
}

#[tokio::test]
async fn anthropic_count_tokens_accepts_context_management_before_operation_resolution() {
    let (app, state) = test_app_with_state().await;
    let token = setup_published_app(&app, "Anthropic Unsupported Context Management App").await;
    let before = flow_run_count(state.as_ref()).await;

    let response = post_json(
        &app,
        "/v1/messages/count_tokens",
        ("x-api-key", token),
        json!({
            "model": ANTHROPIC_FIXTURE_MODEL,
            "messages": [{"role": "user", "content": "Count this prompt"}],
            "context_management": {"edits": []}
        }),
    )
    .await;

    assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);
    let payload = response_json(response).await;
    assert_eq!(payload["error"]["type"], json!("operation_unbound"));
    assert_eq!(flow_run_count(state.as_ref()).await, before);
}

#[tokio::test]
async fn anthropic_count_tokens_accepts_tools_before_operation_resolution() {
    let (app, state) = test_app_with_state().await;
    let token = setup_published_app(&app, "Anthropic Count Tokens Unsupported Tools App").await;
    let before = flow_run_count(state.as_ref()).await;

    let response = post_json(
        &app,
        "/v1/messages/count_tokens",
        ("x-api-key", token),
        json!({
            "model": ANTHROPIC_FIXTURE_MODEL,
            "messages": [{"role": "user", "content": "Count this prompt"}],
            "tools": [{
                "name": "lookup_order",
                "input_schema": {"type": "object"}
            }]
        }),
    )
    .await;

    assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);
    let payload = response_json(response).await;
    assert_eq!(payload["error"]["type"], json!("operation_unbound"));
    assert_eq!(flow_run_count(state.as_ref()).await, before);
}

#[tokio::test]
async fn d2_f1_anthropic_count_tokens_rejects_unknown_metadata_before_any_public_invocation() {
    let (app, state) = test_app_with_state().await;
    let token = setup_published_app(&app, "Anthropic Count Tokens Typed Metadata App").await;
    let before = flow_run_count(state.as_ref()).await;
    let sentinel = "D2-F1-ANTHROPIC-COUNT-TOKENS-METADATA-SECRET";

    let response = post_json(
        &app,
        "/v1/messages/count_tokens",
        ("x-api-key", token),
        json!({
            "model": ANTHROPIC_FIXTURE_MODEL,
            "messages": [{"role": "user", "content": "Count this prompt"}],
            "metadata": {
                "trace_id": "count-trace-1",
                sentinel: "must-not-reach-response"
            }
        }),
    )
    .await;

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    let payload = response_json(response).await;
    assert_eq!(payload["error"]["type"], json!("invalid_request"));
    assert!(
        !serde_json::to_string(&payload)
            .expect("error response serializes")
            .contains(sentinel),
        "unknown metadata must not be echoed from the shared translator"
    );
    assert_eq!(flow_run_count(state.as_ref()).await, before);
}
