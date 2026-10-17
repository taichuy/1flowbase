use super::*;
use control_plane::application_public_api::{
    compat::{anthropic::translate_messages_request, openai::translate_chat_completion_request},
    mapping::ApplicationApiMappingConfig,
    native::NativeInputMapper,
};
use serde_json::json;
use uuid::Uuid;

fn principal(app: Uuid, key: Uuid, workspace: Uuid) -> ApplicationPrincipal {
    ApplicationPrincipal::new(
        app,
        key,
        workspace,
        domain::ActorContext::scoped(Uuid::now_v7(), workspace, "member", []),
    )
    .unwrap()
}

fn chat() -> NativeRunRequest {
    translate_chat_completion_request(json!({
        "model": "test", "messages": [{"role": "user", "content": "hello"}]
    }))
    .unwrap()
    .request
}

fn mapped_identity(request: &NativeRunRequest) -> Option<String> {
    NativeInputMapper::map(request, &ApplicationApiMappingConfig::default_native())
        .unwrap()
        .node_input_payload
        .pointer("/sys/provider_session_identity")
        .and_then(serde_json::Value::as_str)
        .map(ToOwned::to_owned)
}

#[test]
fn chat_session_identity_reaches_native_input_without_protocol_context() {
    let principal = principal(Uuid::now_v7(), Uuid::now_v7(), Uuid::now_v7());
    for name in ["session-id", "thread-id"] {
        let mut headers = HeaderMap::new();
        headers.insert(name, "same-session".parse().unwrap());
        let mut first = chat();
        let conversation = first.conversation.clone();
        bind_chat(&mut first, &principal, &headers).unwrap();
        let mut second = chat();
        bind_chat(&mut second, &principal, &headers).unwrap();
        assert!(first.client_protocol_envelope.is_none());
        assert_eq!(first.conversation, conversation);
        assert_eq!(mapped_identity(&first), mapped_identity(&second));
        assert_eq!(mapped_identity(&first).unwrap().len(), 64);
    }
}

#[test]
fn messages_session_identity_uses_existing_metadata_translation_and_protocol_scope() {
    let principal = principal(Uuid::now_v7(), Uuid::now_v7(), Uuid::now_v7());
    let mut messages = translate_messages_request(json!({
        "model": "test", "max_tokens": 32, "metadata": {"session_id": "session-A"},
        "messages": [{"role": "user", "content": "hello"}]
    }))
    .unwrap()
    .request;
    assert_eq!(messages.conversation["id"], json!("session-A"));
    bind_messages(&mut messages, &principal);
    messages.client_protocol_envelope = None;
    let mut chat = chat();
    let mut headers = HeaderMap::new();
    headers.insert("session-id", "session-A".parse().unwrap());
    bind_chat(&mut chat, &principal, &headers).unwrap();
    assert!(mapped_identity(&messages).is_some());
    assert_ne!(mapped_identity(&messages), mapped_identity(&chat));
}

#[test]
fn missing_session_never_creates_a_shared_provider_identity() {
    let principal = principal(Uuid::now_v7(), Uuid::now_v7(), Uuid::now_v7());
    let mut first = chat();
    let mut second = chat();
    bind_chat(&mut first, &principal, &HeaderMap::new()).unwrap();
    bind_messages(&mut second, &principal);
    assert_eq!(mapped_identity(&first), None);
    assert_eq!(mapped_identity(&second), None);
}

#[test]
fn same_client_session_is_separated_by_application_key_and_workspace() {
    let app = Uuid::now_v7();
    let key = Uuid::now_v7();
    let workspace = Uuid::now_v7();
    let mut identities = std::collections::HashSet::new();
    let mut headers = HeaderMap::new();
    headers.insert("session-id", "session-A".parse().unwrap());
    for principal in [
        principal(app, key, workspace),
        principal(Uuid::now_v7(), key, workspace),
        principal(app, Uuid::now_v7(), workspace),
        principal(app, key, Uuid::now_v7()),
    ] {
        let mut request = chat();
        bind_chat(&mut request, &principal, &headers).unwrap();
        identities.insert(mapped_identity(&request).unwrap());
    }
    assert_eq!(identities.len(), 4);
}

#[test]
fn session_headers_reject_duplicate_empty_and_invalid_text() {
    for name in ["session-id", "thread-id", "x-claude-code-session-id"] {
        let mut headers = HeaderMap::new();
        headers.insert(name, "".parse().unwrap());
        assert!(session_header(&headers, name).is_err());
        headers.insert(name, "one".parse().unwrap());
        headers.append(name, "two".parse().unwrap());
        assert!(session_header(&headers, name).is_err());
        headers.clear();
        headers.insert(name, axum::http::HeaderValue::from_bytes(&[0xff]).unwrap());
        assert!(session_header(&headers, name).is_err());
    }
}

#[test]
fn long_chat_and_messages_session_headers_remain_valid() {
    let principal = principal(Uuid::now_v7(), Uuid::now_v7(), Uuid::now_v7());
    let session = "s".repeat(1024);
    for name in ["session-id", "thread-id", "x-claude-code-session-id"] {
        let mut headers = HeaderMap::new();
        headers.insert(name, session.parse().unwrap());
        assert_eq!(
            session_header(&headers, name).unwrap().as_deref(),
            Some(session.as_str())
        );
        let mut request = if name == "x-claude-code-session-id" {
            translate_messages_request(json!({
                "model": "test", "max_tokens": 32,
                "metadata": {"session_id": session_header(&headers, name).unwrap()},
                "messages": [{"role": "user", "content": "hello"}]
            }))
            .unwrap()
            .request
        } else {
            chat()
        };
        if name == "x-claude-code-session-id" {
            bind_messages(&mut request, &principal);
        } else {
            bind_chat(&mut request, &principal, &headers).unwrap();
        }
        assert_eq!(mapped_identity(&request).unwrap().len(), 64);
    }
}
