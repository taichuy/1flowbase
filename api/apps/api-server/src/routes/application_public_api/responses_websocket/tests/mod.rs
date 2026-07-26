use axum::{
    extract::ws::Message,
    http::{HeaderMap, HeaderValue},
};
use serde_json::json;

use super::{
    auth::{require_responses_websocket_beta, RESPONSES_WEBSOCKET_BETA},
    schema::{
        decode_client_message, ResponsesWebSocketClientMessageError,
        ResponsesWebSocketClientRequest,
    },
};
use crate::routes::application_public_api::openai::openai_credential;

#[test]
fn reuses_openai_bearer_and_x_api_key_credential_semantics() {
    let mut bearer_headers = HeaderMap::new();
    bearer_headers.insert(
        axum::http::header::AUTHORIZATION,
        HeaderValue::from_static("Bearer application-bearer"),
    );
    let bearer = openai_credential(&bearer_headers).unwrap();
    assert_eq!(bearer.token, "application-bearer");
    assert_eq!(bearer.source, "authorization_bearer");

    let mut api_key_headers = HeaderMap::new();
    api_key_headers.insert("x-api-key", HeaderValue::from_static("application-key"));
    let api_key = openai_credential(&api_key_headers).unwrap();
    assert_eq!(api_key.token, "application-key");
    assert_eq!(api_key.source, "x_api_key");

    let Err(missing) = openai_credential(&HeaderMap::new()) else {
        panic!("missing OpenAI credential must be rejected");
    };
    assert_eq!(missing.status, axum::http::StatusCode::UNAUTHORIZED);
    assert_eq!(missing.code, "not_authenticated");
}

#[test]
fn accepts_codex_responses_websocket_beta_token() {
    let mut headers = HeaderMap::new();
    headers.insert(
        "openai-beta",
        HeaderValue::from_static(RESPONSES_WEBSOCKET_BETA),
    );

    assert!(require_responses_websocket_beta(&headers).is_ok());
}

#[test]
fn accepts_beta_token_among_comma_separated_extensions() {
    let mut headers = HeaderMap::new();
    headers.insert(
        "openai-beta",
        HeaderValue::from_static("assistants=v2, responses_websockets=2026-02-06"),
    );

    assert!(require_responses_websocket_beta(&headers).is_ok());
}

#[test]
fn rejects_missing_or_unrecognized_beta_token() {
    let missing = require_responses_websocket_beta(&HeaderMap::new()).unwrap_err();
    assert_eq!(missing.status, axum::http::StatusCode::BAD_REQUEST);
    assert_eq!(missing.code, "responses_websocket_beta_required");

    let mut headers = HeaderMap::new();
    headers.insert(
        "openai-beta",
        HeaderValue::from_static("responses_websockets=2025-01-01"),
    );
    let unsupported = require_responses_websocket_beta(&headers).unwrap_err();
    assert_eq!(unsupported.status, axum::http::StatusCode::BAD_REQUEST);
    assert_eq!(unsupported.code, "responses_websocket_beta_required");
}

#[test]
fn decodes_response_create_envelope_without_renaming_response_fields() {
    let message = Message::Text(
        json!({
            "type": "response.create",
            "response": {
                "model": "published-model",
                "input": "hello",
                "previous_response_id": "resp_previous"
            }
        })
        .to_string()
        .into(),
    );

    let request = decode_client_message(message).unwrap().unwrap();
    let ResponsesWebSocketClientRequest::Create { response } = request;
    assert_eq!(response["model"], json!("published-model"));
    assert_eq!(response["previous_response_id"], json!("resp_previous"));
}

#[test]
fn binary_unknown_and_invalid_envelopes_have_explicit_close_contracts() {
    let binary = decode_client_message(Message::Binary(vec![1, 2, 3].into())).unwrap_err();
    assert_eq!(
        binary,
        ResponsesWebSocketClientMessageError::BinaryNotSupported
    );
    assert_eq!(binary.close_code(), 1003);
    assert_eq!(binary.close_reason(), "binary messages are not supported");

    let unknown =
        decode_client_message(Message::Text(r#"{"type":"response.cancel"}"#.into())).unwrap_err();
    assert_eq!(
        unknown,
        ResponsesWebSocketClientMessageError::UnknownRequestType
    );
    assert_eq!(unknown.close_code(), 1008);
    assert_eq!(unknown.close_reason(), "unknown request type");

    for invalid in [
        "not-json",
        r#"{"response":{}}"#,
        r#"{"type":"response.create"}"#,
        r#"{"type":"response.create","response":[]}"#,
    ] {
        let error = decode_client_message(Message::Text(invalid.into())).unwrap_err();
        assert_eq!(error, ResponsesWebSocketClientMessageError::InvalidEnvelope);
        assert_eq!(error.close_code(), 1007);
        assert_eq!(error.close_reason(), "invalid response.create envelope");
    }
}

#[test]
fn control_frames_are_not_request_envelopes() {
    assert!(decode_client_message(Message::Ping(Vec::new().into()))
        .unwrap()
        .is_none());
    assert!(decode_client_message(Message::Pong(Vec::new().into()))
        .unwrap()
        .is_none());
    assert!(decode_client_message(Message::Close(None))
        .unwrap()
        .is_none());
}
