use super::*;

#[tokio::test]
async fn c1_anthropic_count_tokens_uses_the_selected_workflow_llm_provider() {
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

    let status = response.status();
    let payload = response_json(response).await;
    assert_eq!(status, StatusCode::OK, "{payload}");
    assert_eq!(payload["input_tokens"], json!(29));
    assert_eq!(payload.as_object().map(|body| body.len()), Some(1));
    assert_eq!(flow_run_count(state.as_ref()).await, before + 1);
}

#[tokio::test]
async fn anthropic_count_tokens_maps_context_management_through_the_workflow() {
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

    let status = response.status();
    let payload = response_json(response).await;
    assert_eq!(status, StatusCode::OK, "{payload}");
    assert_eq!(payload["input_tokens"], json!(29));
    assert_eq!(flow_run_count(state.as_ref()).await, before + 1);
}

#[tokio::test]
async fn anthropic_count_tokens_maps_tools_through_the_workflow() {
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

    let status = response.status();
    let payload = response_json(response).await;
    assert_eq!(status, StatusCode::OK, "{payload}");
    assert_eq!(payload["input_tokens"], json!(29));
    assert_eq!(flow_run_count(state.as_ref()).await, before + 1);
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
