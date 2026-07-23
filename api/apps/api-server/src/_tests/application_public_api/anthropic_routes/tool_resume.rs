use super::*;
use crate::routes::application_public_api::tool_callback_ids::encode_anthropic_callback_tool_use_id;

#[tokio::test]
async fn ac_008_anthropic_orphan_tool_result_starts_a_new_run() {
    let (app, state) = test_app_with_state().await;
    let token = setup_published_app(&app, "Anthropic Unsupported Tool Result Route App").await;
    let before = flow_run_count(state.as_ref()).await;
    let tool_use_id = encode_anthropic_callback_tool_use_id(
        uuid::Uuid::from_u128(0x11111111111111111111111111111111),
        "toolu_read",
    );

    let response = post_json(
        &app,
        "/v1/messages",
        ("x-api-key", token),
        json!({
            "model": ANTHROPIC_FIXTURE_MODEL,
            "max_tokens": 64,
            "messages": [
                {
                    "role": "assistant",
                    "content": [{
                        "type": "tool_use",
                        "id": tool_use_id.clone(),
                        "name": "read_files",
                        "input": {"path": "."}
                    }]
                },
                {
                    "role": "user",
                    "content": [{
                        "type": "tool_result",
                        "tool_use_id": tool_use_id,
                        "content": "Found 3 files"
                    }]
                }
            ]
        }),
    )
    .await;

    assert_eq!(response.status(), StatusCode::OK);
    let payload = response_json(response).await;
    assert_ne!(payload["id"], Value::Null);
    assert_eq!(flow_run_count(state.as_ref()).await, before + 1);
}

#[tokio::test]
async fn d2_ac_007_anthropic_prompt_marker_is_unsupported_before_run_creation() {
    let (app, state) = test_app_with_state().await;
    let token = setup_published_app(&app, "Anthropic Unsupported Prompt Marker Route App").await;
    let before = flow_run_count(state.as_ref()).await;

    let response = post_json(
        &app,
        "/v1/messages",
        ("x-api-key", token),
        json!({
            "model": ANTHROPIC_FIXTURE_MODEL,
            "max_tokens": 64,
            "system": "Generate a concise, sentence-case title. Return JSON with a single \"title\" field",
            "messages": [{"role": "user", "content": "continue"}]
        }),
    )
    .await;

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    let payload = response_json(response).await;
    assert_eq!(payload["error"]["type"], json!("unsupported_feature"));
    assert_eq!(flow_run_count(state.as_ref()).await, before);
}
