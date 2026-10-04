use super::*;
use axum::response::{IntoResponse, Sse};
use control_plane::application_public_api::native::{NativeError, NativeRunStatus};
use time::OffsetDateTime;

#[tokio::test]
async fn stream_failure_preserves_upstream_error_and_does_not_emit_success_terminal() {
    for (status, kind) in [
        (400, "invalid_request_error"),
        (429, "rate_limit_error"),
        (529, "overloaded_error"),
    ] {
        let run = NativeRunResult {
            id: Uuid::nil(),
            application_id: Uuid::nil(),
            api_key_id: Uuid::nil(),
            publication_version_id: Uuid::nil(),
            status: NativeRunStatus::Failed,
            node_input_payload: json!({}),
            metadata: json!({}),
            answer: None,
            answer_segments: None,
            required_action: None,
            tool_calls: None,
            usage: None,
            error: Some(NativeError {
                code: "provider_upstream_error".to_string(),
                message: "fallback".to_string(),
                details: json!({"status_code":status,"upstream_error":{"type":kind,"message":"Your client version is too old.","request_id":"req_stream"}}),
            }),
            operation_terminal: None,
            created_at: OffsetDateTime::UNIX_EPOCH,
        };
        let mut mapper = AnthropicStreamMapper::new("model".to_string());
        let events = mapper.anthropic_failed_events(&run, &json!({}));
        assert!(mapper.anthropic_stop_events(None).is_empty());
        let response = Sse::new(tokio_stream::iter(events)).into_response();
        let bytes = axum::body::to_bytes(response.into_body(), 4096)
            .await
            .unwrap();
        let wire = String::from_utf8(bytes.to_vec()).unwrap();
        assert!(wire.contains("event: error"));
        assert!(!wire.contains("message_stop"));
        let data = wire
            .lines()
            .find_map(|line| line.strip_prefix("data: "))
            .unwrap();
        let body: Value = serde_json::from_str(data).unwrap();
        assert_eq!(body["error"]["type"], kind);
        assert_eq!(body["error"]["message"], "Your client version is too old.");
        assert_eq!(body["request_id"], "req_stream");
    }
}
