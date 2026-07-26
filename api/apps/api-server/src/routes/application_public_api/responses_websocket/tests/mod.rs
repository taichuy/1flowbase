use axum::{
    extract::ws::Message,
    http::{HeaderMap, HeaderValue},
};
use serde_json::json;

use super::{
    actor::{ConnectionAction, ConnectionState, ResponsesConnectionActor, TurnCompletion},
    auth::{require_responses_websocket_beta, RESPONSES_WEBSOCKET_BETA},
    schema::{
        decode_client_message, ResponsesWebSocketClientMessageError,
        ResponsesWebSocketClientRequest,
    },
};
use crate::routes::application_public_api::{
    callback_adapter::correlate_openai_responses_callback, openai::openai_credential,
    tool_callback_ids::encode_openai_callback_tool_call_id,
};

mod projector;

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
fn responses_websocket_second_turn_uses_the_shared_callback_correlation_adapter() {
    let callback_task_id = uuid::Uuid::from_u128(0xaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa);
    let call_id = encode_openai_callback_tool_call_id(callback_task_id, "call_weather");
    let previous_response_id = "resp_first";
    let message = Message::Text(
        json!({
            "type": "response.create",
            "response": {
                "model": "published-model",
                "previous_response_id": previous_response_id,
                "input": [{
                    "type": "function_call_output",
                    "call_id": call_id,
                    "output": "sunny"
                }]
            }
        })
        .to_string()
        .into(),
    );

    let ResponsesWebSocketClientRequest::Create { response } = decode_client_message(message)
        .expect("WebSocket callback envelope should be valid")
        .expect("WebSocket callback should be a data request");
    let correlated = correlate_openai_responses_callback(
        &response,
        response
            .get("previous_response_id")
            .and_then(|value| value.as_str()),
    )
    .expect("WebSocket callback markers should correlate")
    .expect("WebSocket second turn should resume the callback");
    assert_eq!(correlated.callback_task_id, callback_task_id);
    assert_eq!(
        correlated.tool_results,
        json!([{"tool_call_id": "call_weather", "content": "sunny"}])
    );
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

#[test]
fn prewarm_then_generate_reuses_one_connection_without_starting_two_turns() {
    let mut actor = ResponsesConnectionActor::new();

    let prewarm = actor
        .accept_response(json!({
            "model": "published-model",
            "instructions": "shared instructions",
            "generate": false
        }))
        .expect("a valid prewarm request must be accepted");
    assert_eq!(prewarm, ConnectionAction::Prewarmed);
    assert_eq!(actor.state(), ConnectionState::Prewarming);

    let generated = actor
        .accept_response(json!({
            "input": "first turn",
            "generate": true
        }))
        .expect("the generated request must start the prewarmed turn");
    let ConnectionAction::StartTurn { turn, response } = generated else {
        panic!("generated response.create must start one turn");
    };
    assert_eq!(response["model"], json!("published-model"));
    assert_eq!(response["instructions"], json!("shared instructions"));
    assert_eq!(response["input"], json!("first turn"));
    assert!(response.get("generate").is_none());
    assert_eq!(actor.state(), ConnectionState::Active);

    assert_eq!(actor.complete_turn(turn), TurnCompletion::ReturnedToIdle);
    assert_eq!(actor.state(), ConnectionState::Idle);

    let second = actor
        .accept_response(json!({"model": "published-model", "input": "second turn"}))
        .expect("the same connection must accept a later sequential turn");
    assert!(matches!(second, ConnectionAction::StartTurn { .. }));
    assert_eq!(actor.state(), ConnectionState::Active);
}

#[test]
fn active_turn_is_singleton_and_close_cancels_before_closed() {
    let mut actor = ResponsesConnectionActor::new();
    let first = actor
        .accept_response(json!({"input": "first"}))
        .expect("first generated request must start");
    let ConnectionAction::StartTurn { turn, .. } = first else {
        panic!("first request must start a turn");
    };

    let busy = actor
        .accept_response(json!({"input": "overlap"}))
        .unwrap_err();
    assert_eq!(busy.close_code(), 1008);
    assert_eq!(actor.state(), ConnectionState::Active);

    assert_eq!(actor.begin_close(), ConnectionAction::CancelTurn { turn });
    assert_eq!(actor.state(), ConnectionState::Cancelling);
    assert_eq!(actor.complete_turn(turn), TurnCompletion::Closed);
    assert_eq!(actor.state(), ConnectionState::Closed);
}

#[test]
fn invalid_generate_is_a_protocol_policy_error_and_idle_close_is_terminal() {
    let mut actor = ResponsesConnectionActor::new();
    let invalid = actor
        .accept_response(json!({"input": "hello", "generate": "later"}))
        .unwrap_err();
    assert_eq!(invalid.close_code(), 1008);
    assert_eq!(actor.state(), ConnectionState::Idle);

    assert_eq!(actor.begin_close(), ConnectionAction::Close);
    assert_eq!(actor.state(), ConnectionState::Closed);
}
