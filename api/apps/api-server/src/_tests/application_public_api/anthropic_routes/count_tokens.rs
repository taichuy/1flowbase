use super::*;

#[tokio::test]
async fn anthropic_count_tokens_returns_usage_without_creating_run() {
    let (app, state) = test_app_with_state().await;
    let token = setup_published_app(&app, "Anthropic Count Tokens Compatible Route App").await;
    let before = flow_run_count(state.as_ref()).await;

    let response = post_json(
        &app,
        "/v1/messages/count_tokens",
        ("x-api-key", token),
        json!({
            "model": "qwen3.6-35b-a3b",
            "messages": [
                {"role": "user", "content": "Count this prompt"}
            ]
        }),
    )
    .await;

    assert_eq!(response.status(), StatusCode::OK);
    let payload = response_json(response).await;
    assert!(
        payload["input_tokens"].as_u64().unwrap_or_default() > 0,
        "{payload}"
    );
    assert_eq!(flow_run_count(state.as_ref()).await, before);
}

#[tokio::test]
async fn d2_ac_007_anthropic_count_tokens_rejects_context_management_without_creating_a_run() {
    let (app, state) = test_app_with_state().await;
    let token = setup_published_app(&app, "Anthropic Unsupported Context Management App").await;
    let before = flow_run_count(state.as_ref()).await;

    let response = post_json(
        &app,
        "/v1/messages/count_tokens",
        ("x-api-key", token),
        json!({
            "model": "qwen3.6-35b-a3b",
            "messages": [{"role": "user", "content": "Count this prompt"}],
            "context_management": {"edits": []}
        }),
    )
    .await;

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    let payload = response_json(response).await;
    assert_eq!(payload["error"]["type"], json!("unsupported_feature"));
    assert_eq!(flow_run_count(state.as_ref()).await, before);
}

#[tokio::test]
async fn d2_ac_007_anthropic_count_tokens_rejects_tools_before_any_public_invocation() {
    let (app, state) = test_app_with_state().await;
    let token = setup_published_app(&app, "Anthropic Count Tokens Unsupported Tools App").await;
    let before = flow_run_count(state.as_ref()).await;

    let response = post_json(
        &app,
        "/v1/messages/count_tokens",
        ("x-api-key", token),
        json!({
            "model": "qwen3.6-35b-a3b",
            "messages": [{"role": "user", "content": "Count this prompt"}],
            "tools": [{
                "name": "lookup_order",
                "input_schema": {"type": "object"}
            }]
        }),
    )
    .await;

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    let payload = response_json(response).await;
    assert_eq!(payload["error"]["type"], json!("unsupported_feature"));
    assert_eq!(flow_run_count(state.as_ref()).await, before);
}
