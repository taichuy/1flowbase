use super::*;
use serde_json::json;

#[test]
fn codex_metadata_resolves_thread_turn_and_distinct_session() {
    let context = McpClientContext::from_protocol(
        Some(&json!({
            "threadId":"thread-a", "sessionId":"session-a",
            "x-codex-turn-metadata": {"thread_id":"thread-a", "session_id":"session-a", "turn_id":"turn-a"},
            "unrelated": {"token":"ignored"}
        })),
        &HeaderMap::new(),
    );
    assert_eq!(
        context,
        McpClientContext {
            thread_id: Some("thread-a".into()),
            turn_id: Some("turn-a".into()),
            session_id: Some("session-a".into()),
            error: None,
        }
    );
}

#[test]
fn identities_are_not_invented_and_conflicts_fail_closed() {
    for meta in [json!({}), json!({"sessionId":"session-only"})] {
        let context = McpClientContext::from_protocol(Some(&meta), &HeaderMap::new());
        assert!(context.thread_id.is_none() && context.turn_id.is_none());
    }
    for (meta, error) in [
        (
            json!({"threadId":"a","x-codex-turn-metadata":{"thread_id":"b"}}),
            "conflicting_identity",
        ),
        (json!({"threadId":"contains space"}), "invalid_identity"),
        (json!({"turnId":42}), "invalid_identity"),
        (
            json!({"x-codex-turn-metadata":"broken"}),
            "invalid_identity",
        ),
        (json!([]), "invalid_identity"),
    ] {
        let context = McpClientContext::from_protocol(Some(&meta), &HeaderMap::new());
        assert_eq!(context.error, Some(error));
        assert!(context.thread_id.is_none());
    }
    let mut headers = HeaderMap::new();
    headers.insert(
        "x-codex-turn-metadata",
        r#"{"thread_id":"b","turn_id":"turn-b"}"#.parse().unwrap(),
    );
    assert_eq!(
        McpClientContext::from_protocol(Some(&json!({"threadId":"a"})), &headers).error,
        Some("conflicting_identity")
    );
    assert_eq!(
        McpClientContext::from_protocol(None, &headers)
            .turn_id
            .as_deref(),
        Some("turn-b")
    );
}

#[tokio::test]
async fn parallel_and_nested_calls_keep_their_own_context_and_clean_up() {
    async fn read(thread: &str) -> String {
        let context = McpClientContext {
            thread_id: Some(thread.into()),
            ..Default::default()
        };
        with_client_context(context, async {
            tokio::task::yield_now().await;
            current_client_context().thread_id.unwrap()
        })
        .await
    }
    let (a, b) = tokio::join!(read("a"), read("b"));
    assert_eq!((a.as_str(), b.as_str()), ("a", "b"));
    assert_eq!(current_client_context(), McpClientContext::default());
    with_client_context(
        McpClientContext {
            thread_id: Some("outer".into()),
            ..Default::default()
        },
        async {
            assert_eq!(read("inner").await, "inner");
            assert_eq!(current_client_context().thread_id.as_deref(), Some("outer"));
        },
    )
    .await;
    assert_eq!(current_client_context(), McpClientContext::default());
}
